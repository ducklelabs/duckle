//! Tool, resource and prompt implementations for the Duckle MCP server.
//!
//! Tools return their structured result as a single pretty-printed JSON text
//! content block (the universally supported MCP content type); a tool failure
//! is reported with `isError: true` rather than a JSON-RPC error, so the model
//! can read and react to it.

use crate::catalog;
use duckle_duckdb_engine::{compile_pipeline_sql, DuckdbEngine, PipelineDoc};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Sent to the client on initialize: a compact operating guide.
pub const INSTRUCTIONS: &str = "\
Duckle MCP: generate, validate, run and build Duckle ETL pipelines.

A pipeline is JSON: { \"name\", \"nodes\": [...], \"edges\": [...] }.
- node: { \"id\": \"n1\", \"type\": \"source|transform|sink\", \"position\": {\"x\":0,\"y\":0}, \
\"data\": { \"label\": \"...\", \"componentId\": \"src.csv\", \"properties\": { ... } } }
- edge: { \"id\": \"e1\", \"source\": \"n1\", \"target\": \"n2\", \"sourceHandle\": \"main\", \
\"targetHandle\": \"main\", \"data\": { \"connectionType\": \"main\" } }

Workflow: call list_components to find component ids, get_component_schema for a \
component's property keys, then create_pipeline (it validates before writing). Use \
validate_pipeline to compile-check without running and run_pipeline to execute headlessly. \
Never hardcode secrets: use ${ENV:KEY} placeholders in properties and supply the value via \
the environment at run time. run_pipeline and build_pipeline need a DuckDB binary \
(DUCKLE_DUCKDB_BIN env or a 'duckdb' arg); build_pipeline also needs the duckle-runner binary.";

// ---------------------------------------------------------------------------
// tools/list
// ---------------------------------------------------------------------------

pub fn list_tools() -> Value {
    json!([
        tool("list_components",
            "List Duckle components (sources, transforms, sinks, control, quality, custom code). Optionally filter by kind or a search query.",
            json!({ "type": "object", "properties": {
                "kind": { "type": "string", "enum": ["source","transform","sink","control","quality","custom"], "description": "Filter to one kind." },
                "query": { "type": "string", "description": "Case-insensitive substring over id/label/summary." }
            }})),
        tool("get_component_schema",
            "Get the full property schema (form fields + input/output ports) for one component id, so you know which properties to set.",
            json!({ "type": "object", "properties": {
                "componentId": { "type": "string", "description": "e.g. src.csv, xf.map, snk.postgres" }
            }, "required": ["componentId"] })),
        tool("create_pipeline",
            "Validate a pipeline and write it as <name>.json into a chosen directory. Fails (without writing) if the pipeline does not compile, unless validate=false.",
            json!({ "type": "object", "properties": {
                "directory": { "type": "string", "description": "Target directory (created if missing)." },
                "name": { "type": "string", "description": "Pipeline name; also the file stem." },
                "pipeline": { "type": "object", "description": "The pipeline object with at least a 'nodes' array (and usually 'edges')." },
                "overwrite": { "type": "boolean", "description": "Replace an existing file. Default false." },
                "validate": { "type": "boolean", "description": "Compile-check before writing. Default true." }
            }, "required": ["directory","name","pipeline"] })),
        tool("validate_pipeline",
            "Compile a pipeline to SQL without running it. Returns the per-stage SQL on success, or a structured error.",
            json!({ "type": "object", "properties": {
                "pipeline": { "type": "object", "description": "Inline pipeline object." },
                "path": { "type": "string", "description": "Path to a pipeline .json (use instead of 'pipeline')." }
            }})),
        tool("run_pipeline",
            "Run a pipeline headlessly through the DuckDB engine. Returns per-node status, row counts, errors and a small result preview. Needs a DuckDB binary.",
            json!({ "type": "object", "properties": {
                "pipeline": { "type": "object" },
                "path": { "type": "string" },
                "duckdb": { "type": "string", "description": "Path to the DuckDB CLI. Defaults to DUCKLE_DUCKDB_BIN or 'duckdb' on PATH." },
                "workspace": { "type": "string", "description": "Workspace root for run logs + child-job resolution." }
            }})),
        tool("list_pipelines",
            "List pipeline .json files in a directory with their node/edge counts.",
            json!({ "type": "object", "properties": {
                "directory": { "type": "string" }
            }, "required": ["directory"] })),
        tool("read_pipeline",
            "Read and return a pipeline .json file.",
            json!({ "type": "object", "properties": {
                "path": { "type": "string" }
            }, "required": ["path"] })),
        tool("read_run_logs",
            "Read the tail of a pipeline's NDJSON run log (component-level events).",
            json!({ "type": "object", "properties": {
                "pipelineName": { "type": "string" },
                "workspace": { "type": "string", "description": "Reads <workspace>/logs/<name>/runtime.log." },
                "logDir": { "type": "string", "description": "Log dir directly (use instead of 'workspace')." },
                "tail": { "type": "integer", "description": "Number of trailing lines. Default 100." }
            }, "required": ["pipelineName"] })),
        tool("build_pipeline",
            "Build a pipeline into ONE self-contained executable for server deployment (the Talend Build Job equivalent). Needs the duckle-runner binary (DUCKLE_RUNNER_BIN or on PATH).",
            json!({ "type": "object", "properties": {
                "pipeline": { "type": "object" },
                "path": { "type": "string" },
                "name": { "type": "string", "description": "Display/file name for the artifact." },
                "out": { "type": "string", "description": "Output artifact file path." },
                "secrets": { "type": "string", "enum": ["env","passphrase"], "description": "Secret delivery mode. Default env. Passphrase needs DUCKLE_BUNDLE_PASSPHRASE." },
                "duckdb": { "type": "string" }
            }, "required": ["out"] })),
        tool("list_connections",
            "List the workspace's saved connections (secret fields masked).",
            json!({ "type": "object", "properties": {
                "workspace": { "type": "string" }
            }, "required": ["workspace"] })),
        tool("create_connection",
            "Create a workspace saved connection JSON so pipelines can reference its fields. Writes connections/<id>.json and registers it in repository.json when present.",
            json!({ "type": "object", "properties": {
                "workspace": { "type": "string" },
                "name": { "type": "string" },
                "connection": { "type": "object", "description": "Fields like { kind, host, port, database, username, password }." }
            }, "required": ["workspace","name","connection"] }))
    ])
}

fn tool(name: &str, description: &str, schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": schema })
}

// ---------------------------------------------------------------------------
// tools/call
// ---------------------------------------------------------------------------

pub fn call_tool(params: Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or((-32602, "missing tool name".to_string()))?;
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

    let result = match name {
        "list_components" => t_list_components(&args),
        "get_component_schema" => t_get_component_schema(&args),
        "create_pipeline" => t_create_pipeline(&args),
        "validate_pipeline" => t_validate_pipeline(&args),
        "run_pipeline" => t_run_pipeline(&args),
        "list_pipelines" => t_list_pipelines(&args),
        "read_pipeline" => t_read_pipeline(&args),
        "read_run_logs" => t_read_run_logs(&args),
        "build_pipeline" => t_build_pipeline(&args),
        "list_connections" => t_list_connections(&args),
        "create_connection" => t_create_connection(&args),
        other => return Err((-32602, format!("unknown tool: {other}"))),
    };

    Ok(match result {
        Ok(v) => content_ok(&v),
        Err(e) => content_err(&e),
    })
}

fn content_ok(v: &Value) -> Value {
    let text = serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string());
    json!({ "content": [ { "type": "text", "text": text } ], "isError": false })
}

fn content_err(msg: &str) -> Value {
    json!({ "content": [ { "type": "text", "text": msg } ], "isError": true })
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

fn t_list_components(args: &Value) -> Result<Value, String> {
    Ok(catalog::list(arg_str(args, "kind"), arg_str(args, "query")))
}

fn t_get_component_schema(args: &Value) -> Result<Value, String> {
    let id = arg_str(args, "componentId").ok_or("missing 'componentId'")?;
    catalog::schema(id).ok_or_else(|| format!("unknown componentId: {id}"))
}

fn t_validate_pipeline(args: &Value) -> Result<Value, String> {
    let (v, _name) = load_pipeline_value(args)?;
    let doc = to_doc(&v)?;
    match compile_pipeline_sql(&doc) {
        Ok(stages) => Ok(json!({
            "ok": true,
            "stageCount": stages.len(),
            "stages": serde_json::to_value(&stages).unwrap_or_else(|_| json!([]))
        })),
        Err(e) => Ok(json!({ "ok": false, "error": e.to_string() })),
    }
}

fn t_create_pipeline(args: &Value) -> Result<Value, String> {
    let dir = arg_str(args, "directory").ok_or("missing 'directory'")?;
    let name = arg_str(args, "name").ok_or("missing 'name'")?;
    let pipeline = args
        .get("pipeline")
        .filter(|v| v.is_object())
        .ok_or("missing 'pipeline' object")?;
    let do_validate = arg_bool(args, "validate", true);
    let overwrite = arg_bool(args, "overwrite", false);

    // Normalize into the full saved-pipeline shape the GUI also writes.
    let mut obj = pipeline.as_object().cloned().unwrap_or_default();
    if !obj.get("nodes").map(|n| n.is_array()).unwrap_or(false) {
        return Err("pipeline must have a 'nodes' array".to_string());
    }
    obj.entry("edges").or_insert_with(|| json!([]));
    obj.entry("version").or_insert_with(|| json!(1));
    obj.entry("name").or_insert_with(|| json!(name));
    obj.entry("id").or_insert_with(|| json!(gen_id("p")));
    let full = Value::Object(obj);

    let mut validation = Value::Null;
    if do_validate {
        let doc = to_doc(&full)?;
        match compile_pipeline_sql(&doc) {
            Ok(stages) => validation = json!({ "ok": true, "stageCount": stages.len() }),
            Err(e) => return Err(format!("pipeline did not validate (not written): {e}")),
        }
    }

    let fname = format!("{}.json", sanitize_filename(name));
    let path = std::path::Path::new(dir).join(&fname);
    if path.exists() && !overwrite {
        return Err(format!(
            "{} already exists (pass overwrite=true to replace)",
            path.display()
        ));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {dir}: {e}"))?;
    let pretty = serde_json::to_string_pretty(&full).map_err(|e| e.to_string())?;
    std::fs::write(&path, pretty).map_err(|e| format!("write {}: {e}", path.display()))?;

    Ok(json!({ "ok": true, "path": path.to_string_lossy(), "validation": validation }))
}

fn t_run_pipeline(args: &Value) -> Result<Value, String> {
    let (v, name) = load_pipeline_value(args)?;
    let doc = to_doc(&v)?;
    let duckdb = resolve_duckdb(arg_str(args, "duckdb"))
        .ok_or("no DuckDB binary found; set DUCKLE_DUCKDB_BIN or pass 'duckdb'")?;
    std::env::set_var("DUCKLE_DUCKDB_BIN", &duckdb);
    if let Some(ws) = arg_str(args, "workspace") {
        std::env::set_var("DUCKLE_WORKSPACE", ws);
        std::env::set_var("DUCKLE_LOG_DIR", std::path::Path::new(ws).join("logs"));
    }

    let engine = DuckdbEngine::new(duckdb);
    let result = engine.execute_pipeline_named(&doc, &name);

    let mut out = serde_json::to_value(&result).map_err(|e| e.to_string())?;
    // Cap preview rows so the response stays small.
    if let Some(prev) = out.get_mut("preview").and_then(|p| p.as_array_mut()) {
        for node in prev.iter_mut() {
            if let Some(rows) = node.get_mut("rows").and_then(|r| r.as_array_mut()) {
                rows.truncate(20);
            }
        }
    }
    Ok(out)
}

fn t_list_pipelines(args: &Value) -> Result<Value, String> {
    let dir = arg_str(args, "directory").ok_or("missing 'directory'")?;
    let rd = std::fs::read_dir(dir).map_err(|e| format!("read_dir {dir}: {e}"))?;
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let v: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(nodes) = v.get("nodes").and_then(|n| n.as_array()) {
            out.push(json!({
                "file": path.to_string_lossy(),
                "name": v.get("name").and_then(|x| x.as_str()).unwrap_or(""),
                "nodeCount": nodes.len(),
                "edgeCount": v.get("edges").and_then(|e| e.as_array()).map(|a| a.len()).unwrap_or(0),
            }));
        }
    }
    Ok(json!({ "count": out.len(), "pipelines": out }))
}

fn t_read_pipeline(args: &Value) -> Result<Value, String> {
    let path = arg_str(args, "path").ok_or("missing 'path'")?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("parse {path}: {e}"))
}

fn t_read_run_logs(args: &Value) -> Result<Value, String> {
    let pipeline_name = arg_str(args, "pipelineName").ok_or("missing 'pipelineName'")?;
    let tail = args.get("tail").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let base: PathBuf = if let Some(ld) = arg_str(args, "logDir") {
        PathBuf::from(ld)
    } else if let Some(ws) = arg_str(args, "workspace") {
        PathBuf::from(ws).join("logs")
    } else {
        return Err("provide 'logDir' or 'workspace'".to_string());
    };
    let file = base.join(sanitize_segment(pipeline_name)).join("runtime.log");
    let text = std::fs::read_to_string(&file).map_err(|e| format!("read {}: {e}", file.display()))?;
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(tail);
    let entries: Vec<Value> = lines[start..]
        .iter()
        .map(|l| serde_json::from_str::<Value>(l).unwrap_or_else(|_| json!({ "raw": l })))
        .collect();
    Ok(json!({ "file": file.to_string_lossy(), "lineCount": entries.len(), "entries": entries }))
}

fn t_build_pipeline(args: &Value) -> Result<Value, String> {
    let (v, default_name) = load_pipeline_value(args)?;
    to_doc(&v)?; // reject an invalid pipeline before staging anything
    let out = arg_str(args, "out").ok_or("missing 'out' (output artifact path)")?;
    let secrets = arg_str(args, "secrets").unwrap_or("env");
    if secrets != "env" && secrets != "passphrase" {
        return Err("secrets must be 'env' or 'passphrase'".to_string());
    }
    let name = arg_str(args, "name").unwrap_or(&default_name).to_string();

    let runner = resolve_runner().ok_or(
        "duckle-runner binary not found; set DUCKLE_RUNNER_BIN or put duckle-runner on PATH / next to duckle-mcp",
    )?;

    // Synthesize the minimal workspace layout `duckle-runner build` understands.
    let ws = std::env::temp_dir().join(format!(
        "duckle-mcp-build-{}-{}",
        sanitize_filename(&name),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(ws.join("pipelines")).map_err(|e| format!("mkdir: {e}"))?;
    let pid = "p1";
    let repo = json!([{ "id": pid, "name": name, "type": "pipeline" }]);
    std::fs::write(
        ws.join("repository.json"),
        serde_json::to_string_pretty(&repo).unwrap_or_default(),
    )
    .map_err(|e| format!("write repository.json: {e}"))?;
    std::fs::write(
        ws.join("pipelines").join(format!("{pid}.json")),
        serde_json::to_string_pretty(&v).unwrap_or_default(),
    )
    .map_err(|e| format!("write pipeline: {e}"))?;

    let mut cmd = std::process::Command::new(&runner);
    cmd.arg("build")
        .arg("--workspace")
        .arg(&ws)
        .arg("--pipeline-id")
        .arg(pid)
        .arg("--out")
        .arg(out)
        .arg("--secrets")
        .arg(secrets);
    if let Some(d) = arg_str(args, "duckdb") {
        cmd.arg("--duckdb").arg(d);
    }
    let output = cmd.output().map_err(|e| format!("spawn duckle-runner: {e}"))?;
    let _ = std::fs::remove_dir_all(&ws);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        Ok(json!({ "ok": true, "out": out, "secrets": secrets, "log": stderr.trim() }))
    } else {
        let detail = if stderr.trim().is_empty() { stdout } else { stderr };
        Err(format!("duckle-runner build failed: {}", detail.trim()))
    }
}

fn t_list_connections(args: &Value) -> Result<Value, String> {
    let ws = arg_str(args, "workspace").ok_or("missing 'workspace'")?;
    let dir = std::path::Path::new(ws).join("connections");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let mut v: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };
            mask_secrets(&mut v);
            out.push(json!({
                "file": path.file_name().map(|s| s.to_string_lossy().into_owned()),
                "connection": v
            }));
        }
    }
    Ok(json!({ "count": out.len(), "connections": out }))
}

fn t_create_connection(args: &Value) -> Result<Value, String> {
    let ws = arg_str(args, "workspace").ok_or("missing 'workspace'")?;
    let name = arg_str(args, "name").ok_or("missing 'name'")?;
    let conn = args
        .get("connection")
        .filter(|v| v.is_object())
        .ok_or("missing 'connection' object")?;
    let dir = std::path::Path::new(ws).join("connections");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    let id = gen_id("c");
    let path = dir.join(format!("{id}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(conn).unwrap_or_default())
        .map_err(|e| format!("write: {e}"))?;

    // Best-effort: register in repository.json so the GUI lists it.
    let repo_path = std::path::Path::new(ws).join("repository.json");
    let mut registered = false;
    if let Ok(text) = std::fs::read_to_string(&repo_path) {
        if let Ok(mut repo) = serde_json::from_str::<Value>(&text) {
            if let Some(arr) = repo.as_array_mut() {
                arr.push(json!({ "id": id, "name": name, "type": "connection" }));
                if std::fs::write(&repo_path, serde_json::to_string_pretty(&repo).unwrap_or_default())
                    .is_ok()
                {
                    registered = true;
                }
            }
        }
    }
    Ok(json!({ "ok": true, "id": id, "path": path.to_string_lossy(), "registeredInRepository": registered }))
}

// ---------------------------------------------------------------------------
// resources/list + resources/read
// ---------------------------------------------------------------------------

pub fn list_resources() -> Value {
    json!([
        { "uri": "duckle://catalog", "name": "Component catalog", "description": "All Duckle components with property schemas + ports.", "mimeType": "application/json" },
        { "uri": "duckle://pipeline-format", "name": "Pipeline JSON format", "description": "The shape of a Duckle pipeline file.", "mimeType": "text/markdown" }
    ])
}

pub fn read_resource(params: Value) -> Result<Value, (i64, String)> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or((-32602, "missing 'uri'".to_string()))?;
    let (text, mime) = match uri {
        "duckle://catalog" => (
            serde_json::to_string_pretty(catalog::full()).unwrap_or_else(|_| "{}".to_string()),
            "application/json",
        ),
        "duckle://pipeline-format" => (PIPELINE_FORMAT_DOC.to_string(), "text/markdown"),
        other => return Err((-32602, format!("unknown resource: {other}"))),
    };
    Ok(json!({ "contents": [ { "uri": uri, "mimeType": mime, "text": text } ] }))
}

const PIPELINE_FORMAT_DOC: &str = "\
# Duckle pipeline format

```json
{
  \"name\": \"my pipeline\",
  \"nodes\": [
    { \"id\": \"src\", \"type\": \"source\", \"position\": {\"x\":0,\"y\":0},
      \"data\": { \"label\": \"orders\", \"componentId\": \"src.csv\",
                  \"properties\": { \"path\": \"orders.csv\", \"hasHeader\": true } } },
    { \"id\": \"snk\", \"type\": \"sink\", \"position\": {\"x\":300,\"y\":0},
      \"data\": { \"label\": \"out\", \"componentId\": \"snk.csv\",
                  \"properties\": { \"path\": \"out.csv\" } } }
  ],
  \"edges\": [
    { \"id\": \"e1\", \"source\": \"src\", \"target\": \"snk\",
      \"sourceHandle\": \"main\", \"targetHandle\": \"main\",
      \"data\": { \"connectionType\": \"main\" } }
  ]
}
```

- Find component ids + property keys with list_components / get_component_schema.
- Handles: most nodes use the `main` port; transforms add ports like `reject`,
  `lookup_1`, `case_1`, `main_1`. Edge `data.connectionType` mirrors the handle.
- Secrets: put `${ENV:KEY}` in a property and set the env var at run time; never
  inline real credentials.";

// ---------------------------------------------------------------------------
// prompts/list + prompts/get
// ---------------------------------------------------------------------------

pub fn list_prompts() -> Value {
    json!([
        { "name": "generate_pipeline", "description": "Generate a Duckle pipeline from a plain-English goal.",
          "arguments": [ { "name": "goal", "description": "What the pipeline should do.", "required": true } ] }
    ])
}

pub fn get_prompt(params: Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or((-32602, "missing prompt name".to_string()))?;
    if name != "generate_pipeline" {
        return Err((-32602, format!("unknown prompt: {name}")));
    }
    let goal = params
        .get("arguments")
        .and_then(|a| a.get("goal"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let text = format!(
        "Build a Duckle pipeline that: {goal}\n\nFirst call list_components and \
get_component_schema to choose components and property keys. Then call create_pipeline \
to write and validate it. Keep credentials as ${{ENV:KEY}} placeholders. {INSTRUCTIONS}"
    );
    Ok(json!({
        "messages": [ { "role": "user", "content": { "type": "text", "text": text } } ]
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty())
}

fn arg_bool(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

/// Load a pipeline from inline `pipeline` or a `path`, returning the raw JSON
/// value and a derived display name.
fn load_pipeline_value(args: &Value) -> Result<(Value, String), String> {
    if let Some(p) = args.get("pipeline").filter(|v| v.is_object()) {
        let name = p
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| "pipeline".to_string());
        Ok((p.clone(), name))
    } else if let Some(path) = arg_str(args, "path") {
        let text = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("parse {path}: {e}"))?;
        let name = v
            .get("name")
            .and_then(|x| x.as_str())
            .map(String::from)
            .unwrap_or_else(|| {
                std::path::Path::new(path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "pipeline".to_string())
            });
        Ok((v, name))
    } else {
        Err("provide either 'pipeline' (object) or 'path' (string)".to_string())
    }
}

fn to_doc(v: &Value) -> Result<PipelineDoc, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("not a valid pipeline: {e}"))
}

fn resolve_duckdb(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(env) = std::env::var("DUCKLE_DUCKDB_BIN") {
        let pb = PathBuf::from(env);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for c in ["duckdb", "duckdb.exe"] {
                let pb = dir.join(c);
                if pb.exists() {
                    return Some(pb);
                }
            }
        }
    }
    Some(PathBuf::from(if cfg!(windows) { "duckdb.exe" } else { "duckdb" }))
}

fn resolve_runner() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("DUCKLE_RUNNER_BIN") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for c in ["duckle-runner", "duckle-runner.exe"] {
                let pb = dir.join(c);
                if pb.exists() {
                    return Some(pb);
                }
            }
        }
    }
    Some(PathBuf::from(if cfg!(windows) {
        "duckle-runner.exe"
    } else {
        "duckle-runner"
    }))
}

fn mask_secrets(v: &mut Value) {
    if let Some(obj) = v.as_object_mut() {
        for key in ["password", "secretKey", "accountKey", "accountName", "token", "apiKey"] {
            if let Some(val) = obj.get_mut(key) {
                if val.is_string() {
                    *val = json!("***");
                }
            }
        }
    }
}

/// A short, unique-enough id (no Date/random deps needed): prefix + pid + counter.
fn gen_id(prefix: &str) -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    let n = N.fetch_add(1, Ordering::Relaxed);
    format!("{}_{}_{}", prefix, std::process::id(), n)
}

fn sanitize_filename(name: &str) -> String {
    let mut out = String::new();
    let mut prev_us = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            out.push(ch);
            prev_us = false;
        } else if !prev_us {
            out.push('_');
            prev_us = true;
        }
    }
    let t = out.trim_matches(|c| c == '_' || c == '.');
    if t.is_empty() {
        "pipeline".to_string()
    } else {
        t.to_string()
    }
}

fn sanitize_segment(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let c = cleaned.trim().trim_matches('.').trim();
    if c.is_empty() {
        "pipeline".to_string()
    } else {
        c.to_string()
    }
}
