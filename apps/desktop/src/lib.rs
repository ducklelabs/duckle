//! Duckle desktop shell.
//!
//! Boots the Tauri runtime, wires it to `duckle-runtime`, and exposes
//! invoke commands to the frontend.

use duckle_connectors::CsvConnector;
use duckle_metadata::Schema;
use duckle_plugin_sdk::{InspectError, SchemaInspector};
use serde::Serialize;
use serde_json::Value as JsonValue;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    tracing::info!("duckle starting");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![ping, autodetect_schema])
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

/// Inspect a source's schema. The frontend hands us a format string
/// (`"csv"`, `"parquet"`, ...) and the connector-specific options, and
/// we return inferred columns plus a small sample for the Preview tab.
#[tauri::command]
async fn autodetect_schema(
    format: String,
    options: JsonValue,
) -> Result<InspectionPayload, String> {
    let inspection = match format.as_str() {
        "csv" | "tsv" => CsvConnector
            .inspect(options)
            .await
            .map_err(format_inspect_error)?,
        other => {
            return Err(format!(
                "Autodetect for format '{}' is not implemented yet",
                other
            ));
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
