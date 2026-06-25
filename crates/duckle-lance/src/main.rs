//! duckle-lance: a standalone sidecar that bridges LanceDB <-> the Duckle engine
//! through Parquet temp files. Kept out of the core engine so lancedb's arrow 58
//! / DataFusion / protoc build deps never touch it (the engine stays on arrow
//! 53). The engine shells out:
//!   duckle-lance read  --uri <ds> --table <t> [--api-key K --region R --limit N] --out file.parquet
//!   duckle-lance write --uri <ds> --table <t> --in file.parquet [--mode create|append]
//! `read` writes the table's rows to a Parquet file the engine ingests via
//! DuckDB read_parquet; `write` reads a Parquet file the engine produced and
//! creates/appends the Lance table.

use std::collections::HashMap;
use std::process::ExitCode;

use arrow_array::{RecordBatch, RecordBatchIterator, RecordBatchReader};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};

fn parse_args() -> (String, HashMap<String, String>) {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    let mut map = HashMap::new();
    while let Some(a) = it.next() {
        if let Some(key) = a.strip_prefix("--") {
            map.insert(key.to_string(), it.next().unwrap_or_default());
        }
    }
    (cmd, map)
}

async fn connect(args: &HashMap<String, String>) -> Result<lancedb::Connection, String> {
    let uri = args.get("uri").ok_or("--uri required")?;
    let mut b = lancedb::connect(uri);
    if let Some(k) = args.get("api-key").filter(|s| !s.is_empty()) {
        b = b.api_key(k);
    }
    if let Some(r) = args.get("region").filter(|s| !s.is_empty()) {
        b = b.region(r);
    }
    b.execute().await.map_err(|e| format!("connect: {e}"))
}

async fn run_read(args: HashMap<String, String>) -> Result<(), String> {
    let table = args.get("table").ok_or("--table required")?;
    let out = args.get("out").ok_or("--out required")?;
    let db = connect(&args).await?;
    let tbl = db
        .open_table(table)
        .execute()
        .await
        .map_err(|e| format!("open_table {table}: {e}"))?;
    // Always emit the table schema (even with zero rows) so DuckDB sees the
    // columns; an empty query() with no nearest_to is a full scan.
    let schema = tbl.schema().await.map_err(|e| format!("schema: {e}"))?;
    let mut q = tbl.query();
    if let Some(lim) = args.get("limit").and_then(|s| s.parse::<usize>().ok()) {
        q = q.limit(lim);
    }
    let batches: Vec<RecordBatch> = q
        .execute()
        .await
        .map_err(|e| format!("query: {e}"))?
        .try_collect()
        .await
        .map_err(|e| format!("collect: {e}"))?;
    let file = std::fs::File::create(out).map_err(|e| format!("create {out}: {e}"))?;
    let props = parquet::file::properties::WriterProperties::builder().build();
    let mut writer = parquet::arrow::ArrowWriter::try_new(file, schema, Some(props))
        .map_err(|e| format!("parquet writer: {e}"))?;
    let mut n = 0u64;
    for b in &batches {
        n += b.num_rows() as u64;
        writer.write(b).map_err(|e| format!("write batch: {e}"))?;
    }
    writer.close().map_err(|e| format!("close parquet: {e}"))?;
    eprintln!("duckle-lance: read {n} rows from {table}");
    Ok(())
}

async fn run_write(args: HashMap<String, String>) -> Result<(), String> {
    let table = args.get("table").ok_or("--table required")?;
    let input = args.get("in").ok_or("--in required")?;
    let mode = args.get("mode").map(String::as_str).unwrap_or("create");
    let file = std::fs::File::open(input).map_err(|e| format!("open {input}: {e}"))?;
    let builder = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("parquet reader: {e}"))?;
    let schema = builder.schema().clone();
    let reader = builder.build().map_err(|e| format!("build reader: {e}"))?;
    let batches: Vec<RecordBatch> = reader
        .collect::<Result<_, _>>()
        .map_err(|e| format!("read parquet: {e}"))?;
    let rows: u64 = batches.iter().map(|b| b.num_rows() as u64).sum();
    let db = connect(&args).await?;
    let iter = RecordBatchIterator::new(batches.into_iter().map(Ok), schema);
    let reader: Box<dyn RecordBatchReader + Send> = Box::new(iter);
    if mode == "append" {
        let tbl = db
            .open_table(table)
            .execute()
            .await
            .map_err(|e| format!("open_table {table}: {e}"))?;
        tbl.add(reader)
            .execute()
            .await
            .map_err(|e| format!("add: {e}"))?;
    } else {
        // create = overwrite: drop any existing table first (best-effort).
        let _ = db.drop_table(table, &[]).await;
        db.create_table(table, reader)
            .execute()
            .await
            .map_err(|e| format!("create_table {table}: {e}"))?;
    }
    eprintln!("duckle-lance: wrote {rows} rows to {table} ({mode})");
    Ok(())
}

fn main() -> ExitCode {
    let (cmd, args) = parse_args();
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("duckle-lance error: tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let res = match cmd.as_str() {
        "read" => rt.block_on(run_read(args)),
        "write" => rt.block_on(run_write(args)),
        other => Err(format!("unknown command {other:?} (use read|write)")),
    };
    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("duckle-lance error: {e}");
            ExitCode::FAILURE
        }
    }
}
