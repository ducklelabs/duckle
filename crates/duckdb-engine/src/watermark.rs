//! Backfill support: inspect, set, and clear the persisted state that
//! `xf.incremental` (a high-water mark) and `src.ducklake.changes` (a CDC
//! snapshot id) advance only on a fully successful run.
//!
//! State lives at `<workspace>/state/<pipeline>/<node_id>.json`, either
//! `{ "value": "...", "type": "TIMESTAMP" }` (incremental) or
//! `{ "snapshot_id": 42 }` (DuckLake CDC). Editing it lets an operator replay
//! from an earlier point ("backfill from date X" / "re-read from snapshot N")
//! or clear it to force a full reload, without touching the pipeline. The path
//! rule mirrors the executor's own resolution in connectors.rs.

use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// One node's saved watermark/snapshot, for display in the backfill UI.
#[derive(Debug, Clone, Serialize)]
pub struct WatermarkEntry {
    pub node_id: String,
    /// "incremental" (value + type) or "snapshot" (DuckLake CDC).
    pub kind: String,
    /// The watermark value, or the snapshot id rendered as a string.
    pub value: String,
    /// SQL type for incremental marks (e.g. TIMESTAMP, BIGINT); None for snapshots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
}

fn state_dir(workspace: &Path, pipeline: &str) -> PathBuf {
    workspace.join("state").join(sanitize_segment(pipeline))
}

/// Path to one node's state file under a workspace + pipeline name.
pub fn state_path(workspace: &Path, pipeline: &str, node_id: &str) -> PathBuf {
    state_dir(workspace, pipeline).join(format!("{}.json", sanitize_segment(node_id)))
}

/// List the saved watermarks/snapshots for a pipeline (empty if none).
/// node_id is recovered from the file stem, so it round-trips only when the
/// id had no characters the sanitizer rewrote - good enough for display and
/// for matching against the live graph's node ids.
pub fn list(workspace: &Path, pipeline: &str) -> Vec<WatermarkEntry> {
    let dir = state_dir(workspace, pipeline);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(node_id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if let Some(snap) = v.get("snapshot_id") {
            let value = snap
                .as_u64()
                .map(|n| n.to_string())
                .or_else(|| snap.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            out.push(WatermarkEntry {
                node_id: node_id.to_string(),
                kind: "snapshot".into(),
                value,
                value_type: None,
            });
        } else if let Some(val) = v.get("value").and_then(|x| x.as_str()) {
            out.push(WatermarkEntry {
                node_id: node_id.to_string(),
                kind: "incremental".into(),
                value: val.to_string(),
                value_type: v.get("type").and_then(|x| x.as_str()).map(String::from),
            });
        }
    }
    out.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    out
}

/// Set an incremental high-water mark. `value_type` defaults to VARCHAR.
pub fn set_incremental(
    workspace: &Path,
    pipeline: &str,
    node_id: &str,
    value: &str,
    value_type: Option<&str>,
) -> std::io::Result<()> {
    write_state(
        workspace,
        pipeline,
        node_id,
        &json!({ "value": value, "type": value_type.unwrap_or("VARCHAR") }),
    )
}

/// Set a DuckLake CDC snapshot id.
pub fn set_snapshot(
    workspace: &Path,
    pipeline: &str,
    node_id: &str,
    snapshot_id: u64,
) -> std::io::Result<()> {
    write_state(
        workspace,
        pipeline,
        node_id,
        &json!({ "snapshot_id": snapshot_id }),
    )
}

/// Remove a node's state file so the next run starts from its initial value
/// (incremental) / earliest snapshot (CDC) - i.e. a full reload. A missing
/// file is treated as success.
pub fn clear(workspace: &Path, pipeline: &str, node_id: &str) -> std::io::Result<()> {
    let path = state_path(workspace, pipeline, node_id);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn write_state(
    workspace: &Path,
    pipeline: &str,
    node_id: &str,
    value: &Value,
) -> std::io::Result<()> {
    let path = state_path(workspace, pipeline, node_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&path, text)
}

/// Filesystem-safe single path segment - keep alphanumerics, space, dash,
/// underscore, dot; replace anything else with '_'. Mirrors the executor's
/// sanitize_path_segment so paths line up with what a run actually writes.
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
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        "pipeline".to_string()
    } else {
        cleaned.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_list_clear_roundtrip() {
        let ws = tempfile::tempdir().unwrap();
        set_incremental(ws.path(), "orders", "inc1", "2024-01-01", Some("TIMESTAMP")).unwrap();
        set_snapshot(ws.path(), "orders", "cdc1", 42).unwrap();

        let mut got = list(ws.path(), "orders");
        got.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].node_id, "cdc1");
        assert_eq!(got[0].kind, "snapshot");
        assert_eq!(got[0].value, "42");
        assert_eq!(got[1].node_id, "inc1");
        assert_eq!(got[1].kind, "incremental");
        assert_eq!(got[1].value, "2024-01-01");
        assert_eq!(got[1].value_type.as_deref(), Some("TIMESTAMP"));

        clear(ws.path(), "orders", "inc1").unwrap();
        assert_eq!(list(ws.path(), "orders").len(), 1);
        // Clearing a missing file is a no-op, not an error.
        clear(ws.path(), "orders", "inc1").unwrap();
    }

    #[test]
    fn matches_executor_path_layout() {
        let ws = tempfile::tempdir().unwrap();
        let p = state_path(ws.path(), "My Pipe", "node/1");
        // pipeline + node sanitized; under <ws>/state/.
        assert!(
            p.ends_with("state/My Pipe/node_1.json") || p.ends_with("state\\My Pipe\\node_1.json")
        );
    }
}
