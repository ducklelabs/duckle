//! Duckle desktop shell.
//!
//! Boots the Tauri runtime, wires it to `duckle-runtime`, and exposes
//! invoke commands to the frontend.

use duckle_connectors::CsvConnector;
use duckle_duckdb_engine::{
    append_run_record, compile_pipeline_sql, load_run_history, DuckdbEngine, PipelineDoc,
    PipelineEvent, RunRecord, RunResult, StageSql,
};
use duckle_metadata::Schema;
use duckle_plugin_sdk::{InspectError, SchemaInspector};
use duckle_scheduler::{Schedule, Scheduler};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::ipc::Channel;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

mod ci_status;
mod engine_manager;
mod llama_chat;
mod update_check;
mod workspace_git;
use engine_manager::{EngineStatus, InstallProgress};
use llama_chat::{ChatEvent, ChatMessage};

/// The headless duckle-runner, embedded at compile time (apps/desktop/build.rs
/// stages a freshly built runner and points DUCKLE_EMBEDDED_RUNNER at it).
/// "Build Pipeline" writes these bytes to a temp stub and uses it both as the
/// builder and as the artifact stub, so no separate runner download or
/// compile-on-click is needed.
const EMBEDDED_RUNNER: &[u8] = include_bytes!(env!("DUCKLE_EMBEDDED_RUNNER"));

/// The duckle-mcp server, embedded at compile time when staged. Empty when this
/// build did not bundle it (see build.rs embed_mcp). Written to a stable
/// app-data path on demand so an MCP client config can point at it.
const EMBEDDED_MCP: &[u8] = include_bytes!(env!("DUCKLE_EMBEDDED_MCP"));

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    tracing::info!("duckle starting");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Resolve where the downloaded DuckDB CLI lives, so the
            // engine can shell out to it. The binary may not exist yet
            // (first run installs it via the setup screen); the engine
            // just errors clearly until then.
            //
            // ALSO publish the path as DUCKLE_DUCKDB_BIN. The engine's
            // primary execution path takes the binary as a constructor
            // arg, but rest_source_apply (used by REST-shaped sources:
            // Oracle, SQL Server, Snowflake, Databricks, Synapse,
            // BigQuery, and the various SaaS aliases that materialize
            // their inline result set) is a free helper that reads the
            // env var directly. Without this set, those sources fail
            // with "DUCKLE_DUCKDB_BIN not set" while plain file flows
            // work fine. See issue #2.
            if let Ok(dir) = app.path().app_data_dir() {
                let bin = engine_manager::duckdb_path(&dir);
                std::env::set_var("DUCKLE_DUCKDB_BIN", &bin);
                let _ = DUCKDB_BIN.set(bin);
            }
            // Boot the scheduler. The `.setup` hook runs on the main
            // thread, OUTSIDE any tokio runtime, so calling spawn_ticker
            // (which uses tokio::spawn) directly here panics with
            // "there is no reactor running". Hop onto Tauri's async
            // runtime first.
            if let Ok(eng) = engine() {
                let s = Scheduler::new(eng);
                let _ = SCHEDULER.set(s.clone());
                tauri::async_runtime::spawn(async move {
                    s.spawn_ticker();
                });
            }
            // The window launches hidden (visible:false) so there's no
            // white flash - the frontend calls show() once it has
            // painted. Safety net: reveal it after a few seconds no
            // matter what, so a frontend hiccup can't leave the window
            // stuck invisible.
            if let Some(win) = app.get_webview_window("main") {
                // Open maximized (fill the work area) on every OS. The
                // config `maximized: true` is unreliable when the window
                // starts hidden (visible:false), so maximize explicitly
                // while it is still hidden - it then reveals already
                // maximized with no resize flicker.
                let _ = win.maximize();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                    let _ = win.show();
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            autodetect_schema,
            run_pipeline,
            run_pipeline_partial,
            run_history,
            cancel_pipeline,
            compile_pipeline,
            schedule_set_workspace,
            schedule_list,
            schedule_upsert,
            schedule_delete,
            schedule_run_now,
            engine_status,
            engine_install,
            chat_send,
            chat_extract_pipeline,
            workspace_git_status,
            workspace_git_init,
            workspace_git_commit,
            workspace_git_push,
            workspace_git_pull,
            workspace_git_branches,
            workspace_git_branch_create,
            workspace_git_branch_checkout,
            workspace_git_remote_set,
            workspace_git_save_pat,
            workspace_git_clear_pat,
            workspace_ci_status,
            check_for_update,
            build_pipeline_bundle,
            mcp_connection_info,
            connect_claude_code,
            mcp_inject_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running duckle");
}

/// Liveness probe. Returns the string `"pong"`.
#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

#[derive(Debug, Serialize)]
pub struct InspectionPayload {
    pub columns: Schema,
    #[serde(rename = "sampleRows")]
    pub sample_rows: Vec<JsonValue>,
}

static DUCKDB_BIN: OnceLock<PathBuf> = OnceLock::new();
static DUCKDB_ENGINE: OnceLock<DuckdbEngine> = OnceLock::new();

/// The shared engine, pointed at the downloaded DuckDB CLI. Cheap to
/// build (just holds a path); cached so the cancel flag is shared
/// between a run and a cancel.
fn engine() -> Result<DuckdbEngine, String> {
    let bin = DUCKDB_BIN
        .get()
        .cloned()
        .ok_or_else(|| "Engine path not resolved yet".to_string())?;
    Ok(DUCKDB_ENGINE
        .get_or_init(|| DuckdbEngine::new(bin))
        .clone())
}

/// Inspect a source's schema. The frontend hands us a format string
/// (`"csv"`, `"parquet"`, `"json"`, `"sqlite"`, `"duckdb"`, ...) and the
/// connector-specific options, and we return inferred columns plus a
/// small sample for the Preview tab.
///
/// Most formats go through DuckDB's native readers - `read_csv_auto`,
/// `read_parquet`, `read_json_auto`, `sqlite_scan`. The hand-rolled
/// `CsvConnector` stays as a backup for environments where the DuckDB
/// engine fails to come up.
#[tauri::command]
async fn autodetect_schema(
    format: String,
    options: JsonValue,
) -> Result<InspectionPayload, String> {
    let inspection = match engine() {
        Ok(eng) => match eng.inspect(&format, options.clone()) {
            Ok(insp) => insp,
            Err(e) => {
                tracing::warn!(
                    "DuckDB autodetect failed for {} ({}); falling back",
                    format,
                    e
                );
                if matches!(format.as_str(), "csv" | "tsv") {
                    CsvConnector
                        .inspect(options)
                        .await
                        .map_err(format_inspect_error)?
                } else {
                    return Err(e.to_string());
                }
            }
        },
        Err(boot_err) => {
            tracing::error!("DuckDB engine failed to start: {}", boot_err);
            if matches!(format.as_str(), "csv" | "tsv") {
                CsvConnector
                    .inspect(options)
                    .await
                    .map_err(format_inspect_error)?
            } else {
                return Err(format!("DuckDB engine unavailable: {}", boot_err));
            }
        }
    };
    Ok(InspectionPayload {
        columns: inspection.schema,
        sample_rows: inspection.sample_rows,
    })
}

fn format_inspect_error(err: InspectError) -> String {
    err.to_string()
}

/// Run a pipeline through the DuckDB engine. Receives the React Flow
/// nodes + edges as JSON; compiles to SQL; executes via DuckDB; returns
/// per-node status + preview rows for any leaf node that didn't feed a
/// sink.
///
/// `on_event` is a Tauri Channel - every stage start / stage finish /
/// cancellation is pushed through it so the frontend can light up
/// status badges in real time.
#[tauri::command]
async fn run_pipeline(
    pipeline: PipelineDoc,
    on_event: Channel<PipelineEvent>,
    pipeline_id: Option<String>,
    pipeline_name: Option<String>,
    workspace_path: Option<String>,
) -> Result<RunResult, String> {
    let engine = engine()?;
    let name = pipeline_name.clone();
    let result = tokio::task::spawn_blocking(move || {
        engine.execute_pipeline_with_events(&pipeline, None, name.as_deref(), |evt| {
            let _ = on_event.send(evt);
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    record_history(&pipeline_id, &workspace_path, &result, "manual");
    Ok(result)
}

fn record_history(
    pipeline_id: &Option<String>,
    workspace_path: &Option<String>,
    result: &RunResult,
    trigger: &str,
) {
    if let (Some(id), Some(ws)) = (pipeline_id, workspace_path) {
        let record = RunRecord::from_result(result, trigger);
        if let Err(e) = append_run_record(std::path::Path::new(ws), id, record) {
            tracing::warn!("Failed to record run history: {}", e);
        }
    }
}

/// Same as `run_pipeline` but only executes the subgraph upstream of
/// (and including) `target_node_id`. The target becomes the leaf and
/// returns a preview.
#[tauri::command]
async fn run_pipeline_partial(
    pipeline: PipelineDoc,
    target_node_id: String,
    on_event: Channel<PipelineEvent>,
    pipeline_id: Option<String>,
    pipeline_name: Option<String>,
    workspace_path: Option<String>,
) -> Result<RunResult, String> {
    let engine = engine()?;
    let target = target_node_id;
    let name = pipeline_name.clone();
    let result = tokio::task::spawn_blocking(move || {
        engine.execute_pipeline_with_events(
            &pipeline,
            Some(target.as_str()),
            name.as_deref(),
            |evt| {
                let _ = on_event.send(evt);
            },
        )
    })
    .await
    .map_err(|e| e.to_string())?;
    record_history(&pipeline_id, &workspace_path, &result, "partial");
    Ok(result)
}

/// Read the run history for a pipeline (newest first).
#[tauri::command]
fn run_history(workspace_path: String, pipeline_id: String) -> Result<Vec<RunRecord>, String> {
    let mut records = load_run_history(std::path::Path::new(&workspace_path), &pipeline_id);
    records.reverse();
    Ok(records)
}

/// Signal the engine to stop at the next stage boundary. The current
/// stage (if mid-flight) still finishes; subsequent stages are
/// skipped.
#[tauri::command]
fn cancel_pipeline() -> Result<(), String> {
    let engine = engine()?;
    engine.request_cancel();
    Ok(())
}

/// Compile a pipeline to DuckDB SQL without executing. Used by the
/// "Copy SQL" / "Export SQL" features so users can copy the generated
/// statements out of the app.
#[tauri::command]
fn compile_pipeline(pipeline: PipelineDoc) -> Result<Vec<StageSql>, String> {
    compile_pipeline_sql(&pipeline).map_err(|e| e.to_string())
}

// ---- Scheduler commands ------------------------------------------------

static SCHEDULER: OnceLock<Scheduler> = OnceLock::new();

fn scheduler() -> Result<&'static Scheduler, String> {
    SCHEDULER
        .get()
        .ok_or_else(|| "Scheduler not initialized".to_string())
}

/// Point the scheduler at a workspace folder; loads schedules from
/// `<workspace>/schedules.json`. Called by the frontend whenever the
/// workspace path changes.
#[tauri::command]
fn schedule_set_workspace(path: String) -> Result<(), String> {
    let sched = scheduler()?;
    // Publish the workspace to the engine so child-pipeline references
    // (Run Job / Iterate / Foreach) stored as bare pipeline ids resolve to
    // their `<workspace>/pipelines/<id>.json` file, including for headless
    // scheduled runs that never pass through the frontend. Called whenever
    // the workspace changes, so this stays in sync.
    if path.is_empty() {
        std::env::remove_var("DUCKLE_WORKSPACE");
        std::env::remove_var("DUCKLE_LOG_DIR");
    } else {
        std::env::set_var("DUCKLE_WORKSPACE", &path);
        // Universal, component-level run logging lands in the user's chosen
        // workspace under logs/ (NDJSON) for Splunk / Dynatrace ingestion.
        std::env::set_var("DUCKLE_LOG_DIR", PathBuf::from(&path).join("logs"));
    }
    let p = if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    };
    sched.set_workspace(p);
    Ok(())
}

#[tauri::command]
fn schedule_list() -> Result<Vec<Schedule>, String> {
    Ok(scheduler()?.list())
}

#[tauri::command]
fn schedule_upsert(schedule: Schedule) -> Result<Schedule, String> {
    scheduler()?.upsert(schedule)
}

#[tauri::command]
fn schedule_delete(id: String) -> Result<(), String> {
    scheduler()?.delete(&id)
}

#[tauri::command]
async fn schedule_run_now(id: String) -> Result<RunResult, String> {
    scheduler()?.run_now(&id).await
}

// ---- Engine install (first-run guided setup) ---------------------------

/// Which engines are present in the app-data dir, and whether each can
/// be installed on this platform.
#[tauri::command]
fn engine_status(app: tauri::AppHandle) -> Result<Vec<EngineStatus>, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(engine_manager::status(&dir))
}

/// Download + install an engine (duckdb / slothdb / llamacpp) into
/// app-data, streaming progress.
#[tauri::command]
async fn engine_install(
    app: tauri::AppHandle,
    engine: String,
    on_progress: Channel<InstallProgress>,
) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let result = tokio::task::spawn_blocking(move || {
        engine_manager::install(&dir, &engine, |p| {
            let _ = on_progress.send(p);
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Err(ref e) = result {
        tracing::warn!("Engine install failed: {}", e);
    }
    result
}

// ---- AI chat assistant -------------------------------------------------

/// Send a message to the local Qwen model and stream tokens back over
/// the `on_event` channel. Lazy-boots `llama-server` on the first call
/// of an app run; reuses the same subprocess for subsequent calls.
#[tauri::command]
async fn chat_send(
    app: tauri::AppHandle,
    history: Vec<ChatMessage>,
    on_event: Channel<ChatEvent>,
) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let bin = engine_manager::llamacpp_path(&dir);
    let model = engine_manager::llama_model_path(&dir);
    tokio::task::spawn_blocking(move || {
        // Lazy boot: hold the mutex only long enough to check + spawn.
        let port = {
            let mut guard = llama_chat::LLAMA_SERVER.lock().unwrap();
            if guard.is_none() {
                match llama_chat::LlamaServer::spawn(&bin, &model) {
                    Ok(srv) => {
                        let p = srv.port();
                        *guard = Some(srv);
                        p
                    }
                    Err(e) => {
                        let _ = on_event.send(ChatEvent::Error { message: e.clone() });
                        return Err(e);
                    }
                }
            } else {
                guard.as_ref().unwrap().port()
            }
        };
        if let Err(e) = llama_chat::chat_stream(port, &history, |evt| {
            let _ = on_event.send(evt);
        }) {
            let _ = on_event.send(ChatEvent::Error { message: e.clone() });
            return Err(e);
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Pull a Duckle pipeline JSON out of an assistant message - the
/// model is asked to wrap pipelines in ```json fenced code blocks.
/// Returns the parsed JSON for the frontend to merge into the canvas.
#[tauri::command]
fn chat_extract_pipeline(text: String) -> Result<JsonValue, String> {
    llama_chat::extract_pipeline(&text)
}

// ---- In-app Git integration -------------------------------------------
// Wraps the system git CLI on the user's workspace folder so they can
// commit / push / pull / branch from inside Duckle. Auth: try without
// explicit creds first (system credential helper), fall back to a PAT
// prompt from the frontend on 401.

fn ws_path(workspace_path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(workspace_path)
}

#[tauri::command]
fn workspace_git_status(workspace_path: String) -> Result<workspace_git::GitStatus, String> {
    workspace_git::status(&ws_path(&workspace_path))
}

#[tauri::command]
fn workspace_git_init(workspace_path: String) -> Result<(), String> {
    workspace_git::init(&ws_path(&workspace_path))
}

#[tauri::command]
fn workspace_git_commit(workspace_path: String, message: String) -> Result<String, String> {
    let p = ws_path(&workspace_path);
    workspace_git::add_all(&p)?;
    workspace_git::commit(&p, &message)
}

#[tauri::command]
fn workspace_git_push(workspace_path: String) -> Result<String, String> {
    workspace_git::push(&ws_path(&workspace_path))
}

#[tauri::command]
fn workspace_git_pull(workspace_path: String) -> Result<String, String> {
    workspace_git::pull(&ws_path(&workspace_path))
}

#[tauri::command]
fn workspace_git_branches(workspace_path: String) -> Result<Vec<String>, String> {
    workspace_git::branches(&ws_path(&workspace_path))
}

#[tauri::command]
fn workspace_git_branch_create(workspace_path: String, name: String) -> Result<(), String> {
    workspace_git::branch_create(&ws_path(&workspace_path), &name)
}

#[tauri::command]
fn workspace_git_branch_checkout(workspace_path: String, name: String) -> Result<(), String> {
    workspace_git::branch_checkout(&ws_path(&workspace_path), &name)
}

#[tauri::command]
fn workspace_git_remote_set(workspace_path: String, url: String) -> Result<(), String> {
    workspace_git::remote_set(&ws_path(&workspace_path), &url)
}

#[tauri::command]
fn workspace_git_save_pat(workspace_path: String, token: String) -> Result<(), String> {
    workspace_git::save_pat(&ws_path(&workspace_path), &token)
}

#[tauri::command]
fn workspace_git_clear_pat(workspace_path: String) -> Result<(), String> {
    workspace_git::clear_pat(&ws_path(&workspace_path))
}

#[tauri::command]
async fn workspace_ci_status(workspace_path: String) -> Result<ci_status::CiStatus, String> {
    // HTTP call - keep off the main runtime thread.
    let p = ws_path(&workspace_path);
    tokio::task::spawn_blocking(move || ci_status::poll(&p))
        .await
        .map_err(|e| e.to_string())?
}

/// Check Duckle's GitHub releases for a build newer than this one. Returns a
/// quiet, non-fatal result (offline -> error field set, update_available
/// false) so the frontend can show an upgrade banner without ever blocking.
#[tauri::command]
async fn check_for_update() -> Result<update_check::UpdateInfo, String> {
    tokio::task::spawn_blocking(update_check::check)
        .await
        .map_err(|e| e.to_string())
}

/// Write the embedded duckle-runner bytes to a temp stub file and return the
/// handle. The runner serves as BOTH the builder (run with `build ...`) and
/// the artifact stub (passed via --stub). The NamedTempFile auto-deletes on
/// drop, so it must be kept alive until after the build Command has run.
fn staged_stub() -> Result<PathBuf, String> {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let dir = std::env::temp_dir();
    // Cache path keyed by the embedded runner size, so repeated builds reuse
    // the same already-on-disk (already AV-scanned) file instead of writing a
    // fresh exe and immediately executing it every time. The executed file
    // must have NO open handle, or Windows CreateProcess fails with
    // ERROR_SHARING_VIOLATION (os error 32) - which is exactly why the old
    // NamedTempFile (kept open during the run) broke.
    let path = dir.join(format!("duckle-stub-{}{}", EMBEDDED_RUNNER.len(), suffix));
    let correct = |p: &std::path::Path| {
        std::fs::metadata(p)
            .map(|m| m.len() as usize == EMBEDDED_RUNNER.len())
            .unwrap_or(false)
    };
    if correct(&path) {
        return Ok(path);
    }
    // Write to a unique sibling and rename into place so a concurrent build
    // never executes a half-written stub. std::fs::write closes the handle.
    let tmp = dir.join(format!(
        "duckle-stub-{}-{}{}",
        EMBEDDED_RUNNER.len(),
        std::process::id(),
        suffix
    ));
    std::fs::write(&tmp, EMBEDDED_RUNNER).map_err(|e| format!("write stub: {}", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("chmod stub: {}", e))?;
    }
    match std::fs::rename(&tmp, &path) {
        Ok(()) => Ok(path),
        // Windows rename fails if the destination exists; if another build
        // already staged a correct copy, use it.
        Err(_) if correct(&path) => {
            let _ = std::fs::remove_file(&tmp);
            Ok(path)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("stage stub: {}", e))
        }
    }
}

/// Build a self-contained, server-runnable single file for a workspace
/// pipeline using the embedded `duckle-runner build` subcommand. The same
/// embedded runner is used as the builder and as the artifact stub. Returns
/// the path to the produced single file on success.
#[tauri::command]
async fn build_pipeline_bundle(
    workspace_path: String,
    pipeline_id: String,
    out_file: String,
    context: Option<String>,
    secrets_mode: String,
    passphrase: Option<String>,
) -> Result<String, String> {
    if secrets_mode != "env" && secrets_mode != "passphrase" {
        return Err(format!("secrets mode must be env|passphrase, got {}", secrets_mode));
    }
    if secrets_mode == "passphrase" && passphrase.as_deref().unwrap_or("").is_empty() {
        return Err("Passphrase is required for passphrase mode".to_string());
    }

    let duckdb = DUCKDB_BIN
        .get()
        .cloned()
        .ok_or_else(|| "Engine path not resolved yet (open the app fully first)".to_string())?;
    // The runner treats a non-existent --duckdb as "no duckdb" and still
    // produces a (best-effort) artifact that needs duckdb on PATH. That's by
    // design, but warn so the missing-self-contained case is visible in the
    // logs rather than silent. See issue #2 (DUCKDB_BIN is set unconditionally
    // during setup even before the CLI is installed).
    if !duckdb.exists() {
        tracing::warn!(
            "build_pipeline_bundle: duckdb not found at {} - the file will not embed duckdb and will rely on it being on PATH at run time",
            duckdb.display()
        );
    }

    let stub_path = staged_stub()?;
    let out_fallback = out_file.clone();

    let output = tokio::task::spawn_blocking(move || {
        let spawn_once = || {
            let mut cmd = std::process::Command::new(&stub_path);
            cmd.arg("build")
                .arg("--workspace").arg(&workspace_path)
                .arg("--pipeline-id").arg(&pipeline_id)
                .arg("--out").arg(&out_file)
                .arg("--secrets").arg(&secrets_mode)
                .arg("--stub").arg(&stub_path)
                .arg("--duckdb").arg(&duckdb);
            if let Some(ctx) = context.as_deref() {
                if !ctx.is_empty() {
                    cmd.arg("--context").arg(ctx);
                }
            }
            if secrets_mode == "passphrase" {
                cmd.env("DUCKLE_BUNDLE_PASSPHRASE", passphrase.clone().unwrap_or_default());
            }
            cmd.output()
        };
        // Windows antivirus can briefly lock a just-written exe, so the first
        // execute returns ERROR_SHARING_VIOLATION (os error 32). Retry a few
        // times before giving up; the cached stub means later builds skip this.
        let mut attempt = 0;
        loop {
            match spawn_once() {
                Ok(o) => return Ok(o),
                Err(e) if e.raw_os_error() == Some(32) && attempt < 10 => {
                    attempt += 1;
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(e) => return Err(format!("failed to start duckle-runner: {}", e)),
            }
        }
    })
    .await
    .map_err(|e| e.to_string())??;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if err.is_empty() { "duckle-runner build failed".to_string() } else { err });
    }

    // The build subcommand prints `duckle-runner build: wrote <path>` to STDERR.
    let stderr = String::from_utf8_lossy(&output.stderr);
    const PREFIX: &str = "duckle-runner build: wrote ";
    let file_path = stderr
        .lines()
        .filter_map(|l| l.trim().strip_prefix(PREFIX))
        .last()
        .map(|s| s.trim().to_string());
    match file_path {
        Some(p) => Ok(p),
        None => {
            // The runner reliably emits the prefix (build.rs); if it ever
            // stops, fall back to the chosen out file but warn loudly.
            tracing::warn!(
                "build_pipeline_bundle: runner did not print the '{}' line; returning the out file as a fallback path",
                PREFIX.trim()
            );
            Ok(out_fallback)
        }
    }
}

// ---- MCP server connection -------------------------------------------------

/// What the MCP popup needs to show the user: the staged binary paths, a
/// ready-to-paste `claude mcp add` command, a generic mcpServers JSON config,
/// and flags for whether the server is bundled / DuckDB is installed / the
/// Claude Code CLI is present.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct McpConnInfo {
    bundled: bool,
    duckdb_found: bool,
    claude_cli: bool,
    mcp_path: String,
    duckdb_path: String,
    runner_path: String,
    claude_command: String,
    config_json: String,
}

/// Write `bytes` to `path` only when the on-disk size differs, via a unique
/// sibling + rename so a concurrent reader never sees a half-written file.
fn write_if_changed(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let same = std::fs::metadata(path)
        .map(|m| m.len() as usize == bytes.len())
        .unwrap_or(false);
    if same {
        return Ok(());
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("write {}: {}", tmp.display(), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
    }
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_)
            if std::fs::metadata(path)
                .map(|m| m.len() as usize == bytes.len())
                .unwrap_or(false) =>
        {
            let _ = std::fs::remove_file(&tmp);
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("stage {}: {}", path.display(), e))
        }
    }
}

/// Stage the embedded MCP server into a stable app-data dir, with the embedded
/// runner written alongside it (so duckle-mcp's sibling lookup finds the runner
/// for build_pipeline). Returns (mcp_path, runner_path).
fn stage_mcp(app_data: &std::path::Path) -> Result<(PathBuf, PathBuf), String> {
    if EMBEDDED_MCP.is_empty() {
        return Err("This build does not bundle the duckle-mcp server".to_string());
    }
    let dir = app_data.join("engines").join("mcp");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {}", dir.display(), e))?;
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let mcp = dir.join(format!("duckle-mcp{suffix}"));
    write_if_changed(&mcp, EMBEDDED_MCP)?;
    let runner = dir.join(format!("duckle-runner{suffix}"));
    write_if_changed(&runner, EMBEDDED_RUNNER)?;
    Ok((mcp, runner))
}

/// Double-quote a token for a copyable shell command line (paths have spaces).
fn shell_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

/// Best-effort probe for the Claude Code CLI (`claude --version`).
fn claude_available() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .raw_arg("/C claude --version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("claude")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// Stage the bundled MCP server and return everything the popup renders:
/// resolved paths, a `claude mcp add` one-liner, and an mcpServers JSON config.
#[tauri::command]
fn mcp_connection_info(app: tauri::AppHandle) -> Result<McpConnInfo, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {}", e))?;
    let bundled = !EMBEDDED_MCP.is_empty();
    let (mcp_path, runner_path) = if bundled {
        stage_mcp(&app_data)?
    } else {
        (PathBuf::new(), PathBuf::new())
    };
    let duckdb = DUCKDB_BIN.get().cloned().unwrap_or_default();
    let duckdb_found = duckdb.exists();

    let mcp_s = mcp_path.to_string_lossy().to_string();
    let runner_s = runner_path.to_string_lossy().to_string();
    let duckdb_s = duckdb.to_string_lossy().to_string();

    let claude_command = format!(
        "claude mcp add duckle --env {} --env {} -- {}",
        shell_quote(&format!("DUCKLE_DUCKDB_BIN={}", duckdb_s)),
        shell_quote(&format!("DUCKLE_RUNNER_BIN={}", runner_s)),
        shell_quote(&mcp_s),
    );

    let config = serde_json::json!({
        "mcpServers": {
            "duckle": {
                "command": mcp_s,
                "env": { "DUCKLE_DUCKDB_BIN": duckdb_s, "DUCKLE_RUNNER_BIN": runner_s }
            }
        }
    });
    let config_json = serde_json::to_string_pretty(&config).unwrap_or_default();

    Ok(McpConnInfo {
        bundled,
        duckdb_found,
        claude_cli: claude_available(),
        mcp_path: mcp_s,
        duckdb_path: duckdb_s,
        runner_path: runner_s,
        claude_command,
        config_json,
    })
}

/// Run `claude mcp add duckle ...` so the user is connected to Claude Code in
/// one click. Returns the CLI output on success; errors (with a hint to copy
/// the command) when the CLI is missing or the add fails.
#[tauri::command]
async fn connect_claude_code(app: tauri::AppHandle) -> Result<String, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {}", e))?;
    let (mcp_path, runner_path) = stage_mcp(&app_data)?;
    let duckdb = DUCKDB_BIN.get().cloned().unwrap_or_default();
    let mcp_s = mcp_path.to_string_lossy().to_string();
    let runner_s = runner_path.to_string_lossy().to_string();
    let duckdb_s = duckdb.to_string_lossy().to_string();

    let output = tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // raw_arg so cmd resolves the claude.cmd npm shim and our quoting
            // survives; each path is wrapped so spaces do not split args.
            let line = format!(
                "/C claude mcp add duckle --env \"DUCKLE_DUCKDB_BIN={}\" --env \"DUCKLE_RUNNER_BIN={}\" -- \"{}\"",
                duckdb_s, runner_s, mcp_s
            );
            std::process::Command::new("cmd").raw_arg(line).output()
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("claude")
                .arg("mcp")
                .arg("add")
                .arg("duckle")
                .arg("--env")
                .arg(format!("DUCKLE_DUCKDB_BIN={}", duckdb_s))
                .arg("--env")
                .arg(format!("DUCKLE_RUNNER_BIN={}", runner_s))
                .arg("--")
                .arg(&mcp_s)
                .output()
        }
    })
    .await
    .map_err(|e| format!("join: {}", e))?;

    match output {
        Ok(o) if o.status.success() => {
            let out = String::from_utf8_lossy(&o.stdout);
            let err = String::from_utf8_lossy(&o.stderr);
            let msg = format!("{} {}", out.trim(), err.trim());
            Ok(if msg.trim().is_empty() {
                "Added the duckle MCP server to Claude Code.".to_string()
            } else {
                msg.trim().to_string()
            })
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let out = String::from_utf8_lossy(&o.stdout);
            let detail = if err.trim().is_empty() { out.trim() } else { err.trim() };
            Err(format!("claude mcp add failed: {}", detail))
        }
        Err(e) => Err(format!(
            "Claude Code CLI not found (is `claude` installed and on PATH?). Copy the command instead. ({})",
            e
        )),
    }
}

/// The MCP servers config file for a desktop client.
fn mcp_client_config_path(app: &tauri::AppHandle, client: &str) -> Result<PathBuf, String> {
    match client {
        // %APPDATA%/Claude/... (Windows standalone), ~/Library/Application
        // Support/Claude/... (macOS), ~/.config/Claude/... (Linux).
        // The Windows STORE (MSIX) Claude Desktop sandboxes its config under
        // %LOCALAPPDATA%/Packages/Claude_*/LocalCache/Roaming/Claude/ and
        // ignores the standalone path entirely - prefer the MSIX path when the
        // packaged install is present.
        "claude_desktop" => {
            #[cfg(windows)]
            {
                if let Ok(local) = app.path().local_data_dir() {
                    if let Ok(entries) = std::fs::read_dir(local.join("Packages")) {
                        for e in entries.flatten() {
                            if e.file_name().to_string_lossy().starts_with("Claude_") {
                                let dir = e
                                    .path()
                                    .join("LocalCache")
                                    .join("Roaming")
                                    .join("Claude");
                                if dir.is_dir() {
                                    return Ok(dir.join("claude_desktop_config.json"));
                                }
                            }
                        }
                    }
                }
            }
            let cfg = app.path().config_dir().map_err(|e| format!("config dir: {}", e))?;
            Ok(cfg.join("Claude").join("claude_desktop_config.json"))
        }
        // Cursor reads a global ~/.cursor/mcp.json.
        "cursor" => {
            let home = app.path().home_dir().map_err(|e| format!("home dir: {}", e))?;
            Ok(home.join(".cursor").join("mcp.json"))
        }
        other => Err(format!("unknown MCP client: {}", other)),
    }
}

/// Inject (merge) a "duckle" entry into a desktop MCP client's config file,
/// preserving any existing servers. Returns the written config path. These are
/// per-user config files (no elevation needed); on a permission/parse failure
/// the error tells the user to retry elevated or copy the config manually.
#[tauri::command]
fn mcp_inject_config(app: tauri::AppHandle, client: String) -> Result<String, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {}", e))?;
    let (mcp_path, runner_path) = stage_mcp(&app_data)?;
    let duckdb = DUCKDB_BIN.get().cloned().unwrap_or_default();
    let target = mcp_client_config_path(&app, &client)?;

    // Read the existing config (preserve other servers) or start fresh.
    let mut root: JsonValue = if target.exists() {
        let text = std::fs::read_to_string(&target)
            .map_err(|e| format!("read {}: {}", target.display(), e))?;
        if text.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&text).map_err(|e| {
                format!(
                    "{} is not valid JSON ({}); add the duckle entry manually instead",
                    target.display(),
                    e
                )
            })?
        }
    } else {
        serde_json::json!({})
    };

    {
        let obj = root
            .as_object_mut()
            .ok_or_else(|| format!("{} root is not a JSON object", target.display()))?;
        let servers = obj
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}));
        let servers = servers
            .as_object_mut()
            .ok_or_else(|| "mcpServers is not a JSON object".to_string())?;
        servers.insert(
            "duckle".to_string(),
            serde_json::json!({
                "command": mcp_path.to_string_lossy(),
                "args": [],
                "env": {
                    "DUCKLE_DUCKDB_BIN": duckdb.to_string_lossy(),
                    "DUCKLE_RUNNER_BIN": runner_path.to_string_lossy()
                }
            }),
        );
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {}", parent.display(), e))?;
    }
    let pretty = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(&target, pretty).map_err(|e| {
        format!(
            "could not write {} ({}). If this needs elevated permissions, run Duckle as administrator and retry, or copy the config manually.",
            target.display(),
            e
        )
    })?;
    Ok(target.to_string_lossy().to_string())
}
