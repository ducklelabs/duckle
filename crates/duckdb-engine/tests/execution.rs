//! End-to-end execution tests for the DuckDB engine.
//!
//! Unlike the unit tests in `src/`, which check SQL *generation*, these
//! exercise the real read → transform → write path against temp files
//! and then read the output back to prove the data actually landed.

use duckle_duckdb_engine::{DuckdbEngine, PipelineDoc};
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;

/// These tests drive the real DuckDB CLI. Point DUCKLE_DUCKDB_BIN at a
/// `duckdb` binary to run them; otherwise they soft-skip so `cargo test`
/// stays green in environments without it.
fn engine() -> Option<DuckdbEngine> {
    let bin = std::env::var("DUCKLE_DUCKDB_BIN").ok()?;
    let p = std::path::PathBuf::from(bin);
    p.exists().then(|| DuckdbEngine::new(p))
}

macro_rules! engine_or_skip {
    () => {
        match engine() {
            Some(e) => e,
            None => {
                eprintln!("skipping: set DUCKLE_DUCKDB_BIN to a duckdb CLI to run");
                return;
            }
        }
    };
}

fn write_file(dir: &Path, name: &str, content: &str) -> String {
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    norm(&path.to_string_lossy())
}

fn out_path(dir: &Path, name: &str) -> String {
    norm(&dir.join(name).to_string_lossy())
}

/// DuckDB is happiest with forward slashes even on Windows.
fn norm(p: &str) -> String {
    p.replace('\\', "/")
}

fn doc(nodes: Value, edges: Value) -> PipelineDoc {
    serde_json::from_value(json!({ "nodes": nodes, "edges": edges })).unwrap()
}

fn node(id: &str, component: &str, props: Value) -> Value {
    json!({
        "id": id,
        "position": { "x": 0, "y": 0 },
        "data": { "label": id, "componentId": component, "properties": props }
    })
}

fn main_edge(id: &str, source: &str, target: &str) -> Value {
    json!({ "id": id, "source": source, "target": target, "data": { "connectionType": "main" } })
}

/// Edge that leaves a specific output handle of the source (e.g. the
/// "reject" port of a validator).
fn port_edge(id: &str, source: &str, source_handle: &str, target: &str) -> Value {
    json!({
        "id": id,
        "source": source,
        "sourceHandle": source_handle,
        "target": target,
        "data": { "connectionType": if source_handle == "reject" { "reject" } else { "main" } }
    })
}

/// Read back output files independently of the engine, by shelling out
/// to the same DuckDB CLI (only called after engine_or_skip!, so the
/// binary is present).
fn duckdb_json(sql: &str) -> Vec<Value> {
    let bin = std::env::var("DUCKLE_DUCKDB_BIN").expect("DUCKLE_DUCKDB_BIN set");
    let out = std::process::Command::new(bin)
        .arg(":memory:")
        .arg("-json")
        .arg("-c")
        .arg(sql)
        .output()
        .expect("run duckdb");
    let s = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(s.trim()).unwrap_or_default()
}

fn count(from: &str) -> i64 {
    let rows = duckdb_json(&format!("SELECT COUNT(*) AS n FROM {}", from));
    rows.first()
        .and_then(|r| r.get("n"))
        .and_then(|v| v.as_i64())
        .unwrap_or(-1)
}

fn scalar_string(sql: &str) -> String {
    let rows = duckdb_json(sql);
    rows.first()
        .and_then(|r| r.as_object())
        .and_then(|o| o.values().next())
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}

#[test]
fn csv_filter_parquet_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let csv = write_file(
        tmp.path(),
        "orders.csv",
        "order_id,status,amount\n1,paid,10\n2,pending,20\n3,paid,30\n4,refunded,5\n",
    );
    let out = out_path(tmp.path(), "paid.parquet");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node("s1", "src.csv", json!({ "path": csv, "hasHeader": true })),
            node("f1", "xf.filter", json!({ "predicate": "status = 'paid'" })),
            node("k1", "snk.parquet", json!({ "path": out })),
        ]),
        json!([main_edge("e1", "s1", "f1"), main_edge("e2", "f1", "k1")]),
    );

    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "ok", "run failed: {:?}", result.error);

    // Sink reports the 2 paid rows written.
    let sink = result.nodes.get("k1").expect("sink status present");
    assert_eq!(sink.rows, Some(2), "sink should report 2 rows");

    // The Parquet file exists and, read back independently, has exactly
    // the 2 paid rows.
    assert!(Path::new(&out).exists(), "parquet file should exist");
    assert_eq!(count(&format!("read_parquet('{}')", out)), 2);

    // And both rows really are 'paid'.
    let bad = count(&format!(
        "read_parquet('{}') WHERE status != 'paid'",
        out
    ));
    assert_eq!(bad, 0, "every output row must be paid");
}

#[test]
fn csv_to_csv_roundtrip_preserves_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let csv = write_file(
        tmp.path(),
        "in.csv",
        "id,name\n1,alice\n2,bob\n3,carol\n",
    );
    let out = out_path(tmp.path(), "out.csv");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node("s1", "src.csv", json!({ "path": csv, "hasHeader": true })),
            node("k1", "snk.csv", json!({ "path": out, "hasHeader": true })),
        ]),
        json!([main_edge("e1", "s1", "k1")]),
    );
    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "ok", "run failed: {:?}", result.error);
    assert!(Path::new(&out).exists());
    assert_eq!(count(&format!("read_csv_auto('{}')", out)), 3);
}

#[test]
fn aggregate_groups_and_sums() {
    let tmp = tempfile::tempdir().unwrap();
    let csv = write_file(
        tmp.path(),
        "sales.csv",
        "region,amount\nwest,10\nwest,20\neast,5\neast,15\neast,5\n",
    );
    let out = out_path(tmp.path(), "agg.csv");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node("s1", "src.csv", json!({ "path": csv, "hasHeader": true })),
            node(
                "a1",
                "xf.agg",
                json!({
                    "groupBy": ["region"],
                    "aggregations": [
                        { "column": "amount", "function": "sum", "alias": "total" }
                    ]
                }),
            ),
            node("k1", "snk.csv", json!({ "path": out, "hasHeader": true })),
        ]),
        json!([main_edge("e1", "s1", "a1"), main_edge("e2", "a1", "k1")]),
    );
    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "ok", "run failed: {:?}", result.error);

    // Two groups out.
    assert_eq!(count(&format!("read_csv_auto('{}')", out)), 2);
    // west total = 30.
    let west = scalar_string(&format!(
        "SELECT CAST(total AS VARCHAR) FROM read_csv_auto('{}') WHERE region = 'west'",
        out
    ));
    assert_eq!(west, "30");
}

#[test]
fn preview_returned_for_leaf_without_sink() {
    let tmp = tempfile::tempdir().unwrap();
    let csv = write_file(tmp.path(), "p.csv", "a,b\n1,x\n2,y\n");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node("s1", "src.csv", json!({ "path": csv, "hasHeader": true })),
            node("f1", "xf.filter", json!({ "predicate": "a >= 1" })),
        ]),
        json!([main_edge("e1", "s1", "f1")]),
    );
    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "ok", "run failed: {:?}", result.error);

    // The leaf (filter) has no downstream sink, so it returns a preview.
    let preview = result
        .preview
        .iter()
        .find(|p| p.node_id == "f1")
        .expect("filter leaf preview present");
    assert_eq!(preview.rows.len(), 2);
    assert_eq!(preview.columns.len(), 2);

    // The filter's view row-count is reported on the node status.
    let f = result.nodes.get("f1").unwrap();
    assert_eq!(f.rows, Some(2));
}

#[test]
fn structured_filter_predicate_actually_filters() {
    // The visual filter builder stores a structured object carrying its
    // compiled SQL — the executor must honor it, not fall back to TRUE.
    let tmp = tempfile::tempdir().unwrap();
    let csv = write_file(
        tmp.path(),
        "orders.csv",
        "id,status\n1,paid\n2,pending\n3,paid\n",
    );
    let out = out_path(tmp.path(), "filtered.csv");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node("s1", "src.csv", json!({ "path": csv, "hasHeader": true })),
            node(
                "f1",
                "xf.filter",
                json!({
                    "predicate": {
                        "mode": "builder",
                        "match": "all",
                        "conditions": [
                            { "id": "c1", "column": "status", "op": "eq", "value": "paid" }
                        ],
                        "sql": "status = 'paid'"
                    }
                }),
            ),
            node("k1", "snk.csv", json!({ "path": out, "hasHeader": true })),
        ]),
        json!([main_edge("e1", "s1", "f1"), main_edge("e2", "f1", "k1")]),
    );
    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "ok", "run failed: {:?}", result.error);
    // Header + 2 paid rows — NOT all 3 (which is what the WHERE TRUE bug did).
    assert_eq!(count(&format!("read_csv_auto('{}')", out)), 2);
}

#[test]
fn aggregate_accepts_func_output_keys() {
    // The UI stores aggregations as { column, func, output }; the
    // executor must accept those spellings (not only function/alias).
    let tmp = tempfile::tempdir().unwrap();
    let csv = write_file(
        tmp.path(),
        "sales.csv",
        "region,amount\nwest,10\nwest,20\neast,5\n",
    );
    let out = out_path(tmp.path(), "agg.csv");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node("s1", "src.csv", json!({ "path": csv, "hasHeader": true })),
            node(
                "a1",
                "xf.agg",
                json!({
                    "groupBy": ["region"],
                    "aggregations": [
                        { "column": "amount", "func": "sum", "output": "total" }
                    ]
                }),
            ),
            node("k1", "snk.csv", json!({ "path": out, "hasHeader": true })),
        ]),
        json!([main_edge("e1", "s1", "a1"), main_edge("e2", "a1", "k1")]),
    );
    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "ok", "run failed: {:?}", result.error);
    assert_eq!(count(&format!("read_csv_auto('{}')", out)), 2);
    let west = scalar_string(&format!(
        "SELECT CAST(total AS VARCHAR) FROM read_csv_auto('{}') WHERE region = 'west'",
        out
    ));
    assert_eq!(west, "30");
}

#[test]
fn custom_sql_runs_with_input_alias() {
    // A Custom-SQL node runs its SELECT as a real stage, with the
    // upstream exposed as `input`.
    let tmp = tempfile::tempdir().unwrap();
    let csv = write_file(tmp.path(), "in.csv", "id,amount\n1,10\n2,20\n3,5\n");
    let out = out_path(tmp.path(), "out.csv");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node("s1", "src.csv", json!({ "path": csv, "hasHeader": true })),
            node(
                "q1",
                "code.sql",
                json!({ "sql": "SELECT id, amount * 2 AS dbl FROM input WHERE amount >= 10" }),
            ),
            node("k1", "snk.csv", json!({ "path": out, "hasHeader": true })),
        ]),
        json!([main_edge("e1", "s1", "q1"), main_edge("e2", "q1", "k1")]),
    );
    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "ok", "run failed: {:?}", result.error);
    // Rows with amount >= 10 → ids 1 and 2.
    assert_eq!(count(&format!("read_csv_auto('{}')", out)), 2);
    let dbl = scalar_string(&format!(
        "SELECT CAST(dbl AS VARCHAR) FROM read_csv_auto('{}') WHERE id = 1",
        out
    ));
    assert_eq!(dbl, "20");
}

#[test]
fn quality_range_splits_pass_and_reject() {
    // A Range validator must route in-range rows to its main output and
    // out-of-range rows to its reject port (two materialized tables).
    let tmp = tempfile::tempdir().unwrap();
    let csv = write_file(tmp.path(), "in.csv", "id,amount\n1,5\n2,50\n3,500\n");
    let pass = out_path(tmp.path(), "pass.csv");
    let rej = out_path(tmp.path(), "reject.csv");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node("s1", "src.csv", json!({ "path": csv, "hasHeader": true })),
            node(
                "v1",
                "qa.range",
                json!({ "column": "amount", "min": 10, "max": 100, "inclusive": true }),
            ),
            node("kp", "snk.csv", json!({ "path": pass, "hasHeader": true })),
            node("kr", "snk.csv", json!({ "path": rej, "hasHeader": true })),
        ]),
        json!([
            main_edge("e1", "s1", "v1"),
            port_edge("e2", "v1", "main", "kp"),
            port_edge("e3", "v1", "reject", "kr"),
        ]),
    );
    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "ok", "run failed: {:?}", result.error);
    // 50 is in [10,100] -> pass; 5 and 500 -> reject.
    assert_eq!(count(&format!("read_csv_auto('{}')", pass)), 1);
    assert_eq!(count(&format!("read_csv_auto('{}')", rej)), 2);
}

#[test]
fn window_row_number_partitions() {
    let tmp = tempfile::tempdir().unwrap();
    let csv = write_file(tmp.path(), "in.csv", "g,v\na,1\na,2\nb,9\n");
    let out = out_path(tmp.path(), "win.csv");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node("s1", "src.csv", json!({ "path": csv, "hasHeader": true })),
            node(
                "w1",
                "xf.rownum",
                json!({ "partitionBy": ["g"], "orderBy": ["v"], "outputName": "rn" }),
            ),
            node("k1", "snk.csv", json!({ "path": out, "hasHeader": true })),
        ]),
        json!([main_edge("e1", "s1", "w1"), main_edge("e2", "w1", "k1")]),
    );
    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "ok", "run failed: {:?}", result.error);
    // Partition 'a' has two rows ranked 1 and 2 by v.
    let max_rn = scalar_string(&format!(
        "SELECT CAST(MAX(rn) AS VARCHAR) FROM read_csv_auto('{}') WHERE g = 'a'",
        out
    ));
    assert_eq!(max_rn, "2");
    let b_rn = scalar_string(&format!(
        "SELECT CAST(rn AS VARCHAR) FROM read_csv_auto('{}') WHERE g = 'b'",
        out
    ));
    assert_eq!(b_rn, "1");
}

#[test]
fn missing_source_file_errors_cleanly() {
    let tmp = tempfile::tempdir().unwrap();
    let out = out_path(tmp.path(), "never.parquet");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node(
                "s1",
                "src.csv",
                json!({ "path": "/no/such/file/orders.csv", "hasHeader": true }),
            ),
            node("k1", "snk.parquet", json!({ "path": out })),
        ]),
        json!([main_edge("e1", "s1", "k1")]),
    );
    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "error");
    assert!(result.error.is_some(), "an error message should be present");
    // No output file should have been created.
    assert!(!Path::new(&out).exists());
}

#[test]
fn project_and_rename_reshape_columns() {
    let tmp = tempfile::tempdir().unwrap();
    let csv = write_file(
        tmp.path(),
        "wide.csv",
        "id,first,last,age\n1,ada,lovelace,36\n2,alan,turing,41\n",
    );
    let out = out_path(tmp.path(), "narrow.parquet");

    let engine = engine_or_skip!();
    let d = doc(
        json!([
            node("s1", "src.csv", json!({ "path": csv, "hasHeader": true })),
            node("p1", "xf.project", json!({ "columns": ["id", "first"] })),
            node("k1", "snk.parquet", json!({ "path": out })),
        ]),
        json!([main_edge("e1", "s1", "p1"), main_edge("e2", "p1", "k1")]),
    );
    let result = engine.execute_pipeline(&d);
    assert_eq!(result.status, "ok", "run failed: {:?}", result.error);

    // Output has 2 rows and exactly 2 columns (id, first).
    assert_eq!(count(&format!("read_parquet('{}')", out)), 2);
    // DESCRIBE returns one row per column.
    let cols = count(&format!(
        "(DESCRIBE SELECT * FROM read_parquet('{}'))",
        out
    ));
    assert_eq!(cols, 2, "should have projected to 2 columns");
}
