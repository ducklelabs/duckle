//! Run history persistence.
//!
//! Every pipeline execution (manual, partial, or scheduled) appends a
//! [`RunRecord`] to `<workspace>/runs/<pipeline_id>.json`. We keep the
//! most recent [`MAX_RECORDS`] entries so the file stays small and
//! git-diffs stay readable.

use crate::RunResult;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;

const MAX_RECORDS: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    /// RFC3339 timestamp of when the run started.
    pub at: String,
    pub status: String,
    pub duration_ms: u64,
    /// Total rows written across all sinks.
    pub rows: u64,
    pub node_count: usize,
    /// What kicked off the run: "manual" / "partial" / "scheduled".
    pub trigger: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Coarse error bucket (see error_category) - present only on failure.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub category: Option<String>,
}

impl RunRecord {
    pub fn from_result(result: &RunResult, trigger: &str) -> Self {
        // Only sinks "write" rows; summing view stages too massively
        // overcounts (every intermediate stage reports its row count).
        let rows: u64 = result
            .nodes
            .values()
            .filter(|n| n.kind.as_deref() == Some("sink"))
            .filter_map(|n| n.rows)
            .sum();
        RunRecord {
            at: Utc::now().to_rfc3339(),
            status: result.status.clone(),
            duration_ms: result.duration_ms,
            rows,
            node_count: result.nodes.len(),
            trigger: trigger.to_string(),
            error: result.error.clone(),
            category: result.category.clone(),
        }
    }
}

fn history_file(workspace: &Path, pipeline_id: &str) -> std::path::PathBuf {
    workspace.join("runs").join(format!("{}.json", pipeline_id))
}

/// Append a record, trimming to the most recent MAX_RECORDS. Best
/// effort - IO failures are logged by the caller, not propagated.
pub fn append_run_record(
    workspace: &Path,
    pipeline_id: &str,
    record: RunRecord,
) -> std::io::Result<()> {
    let path = history_file(workspace, pipeline_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut records = load_run_history(workspace, pipeline_id);
    records.push(record);
    let start = records.len().saturating_sub(MAX_RECORDS);
    let trimmed = &records[start..];
    let json = serde_json::to_string_pretty(trimmed)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&path, json)?;
    // Refresh the OpenMetrics textfile alongside the history. Best-effort:
    // monitoring must never fail a run.
    let _ = write_metrics_textfile(workspace);
    Ok(())
}

/// Export run state for every pipeline as an OpenMetrics/Prometheus textfile
/// at `<workspace>/logs/duckle_metrics.prom`, suitable for node_exporter's
/// textfile collector (or Grafana Alloy) - monitoring integration with no
/// HTTP server and no agent inside Duckle, which keeps headless and
/// air-gapped deployments covered.
///
/// All series are gauges derived from the retained run history (a rolling
/// MAX_RECORDS window per pipeline), so totals are windowed, not lifetime
/// counters - the metric names say so. Written atomically (temp file +
/// rename) so a concurrent scrape never reads a half-written file.
pub fn write_metrics_textfile(workspace: &Path) -> std::io::Result<()> {
    let runs_dir = workspace.join("runs");
    let mut out = String::new();
    out.push_str("# HELP duckle_run_last_status 1 when the pipeline's most recent run succeeded, 0 when it failed or was cancelled.\n# TYPE duckle_run_last_status gauge\n");
    let mut last_status = String::new();
    let mut last_duration = String::new();
    let mut last_rows = String::new();
    let mut last_ts = String::new();
    let mut window_runs = String::new();

    let entries = std::fs::read_dir(&runs_dir)?;
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    for path in files {
        let Some(pipeline_id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let records = load_run_history(workspace, pipeline_id);
        let Some(last) = records.last() else {
            continue;
        };
        let label = escape_label(pipeline_id);
        let ok = if last.status == "ok" { 1 } else { 0 };
        last_status.push_str(&format!(
            "duckle_run_last_status{{pipeline=\"{}\"}} {}\n",
            label, ok
        ));
        last_duration.push_str(&format!(
            "duckle_run_last_duration_seconds{{pipeline=\"{}\"}} {}\n",
            label,
            last.duration_ms as f64 / 1000.0
        ));
        last_rows.push_str(&format!(
            "duckle_run_last_rows{{pipeline=\"{}\"}} {}\n",
            label, last.rows
        ));
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&last.at) {
            last_ts.push_str(&format!(
                "duckle_run_last_timestamp_seconds{{pipeline=\"{}\"}} {}\n",
                label,
                ts.timestamp()
            ));
        }
        for status in ["ok", "error", "cancelled"] {
            let n = records.iter().filter(|r| r.status == status).count();
            window_runs.push_str(&format!(
                "duckle_runs_window{{pipeline=\"{}\",status=\"{}\"}} {}\n",
                label, status, n
            ));
        }
    }

    out.push_str(&last_status);
    out.push_str("# HELP duckle_run_last_duration_seconds Wall-clock duration of the most recent run.\n# TYPE duckle_run_last_duration_seconds gauge\n");
    out.push_str(&last_duration);
    out.push_str("# HELP duckle_run_last_rows Rows written across all sinks in the most recent run.\n# TYPE duckle_run_last_rows gauge\n");
    out.push_str(&last_rows);
    out.push_str("# HELP duckle_run_last_timestamp_seconds Unix time the most recent run started.\n# TYPE duckle_run_last_timestamp_seconds gauge\n");
    out.push_str(&last_ts);
    out.push_str("# HELP duckle_runs_window Runs by status within the retained history window (not a lifetime counter).\n# TYPE duckle_runs_window gauge\n");
    out.push_str(&window_runs);

    let logs_dir = workspace.join("logs");
    std::fs::create_dir_all(&logs_dir)?;
    let final_path = logs_dir.join("duckle_metrics.prom");
    let tmp_path = logs_dir.join("duckle_metrics.prom.tmp");
    std::fs::write(&tmp_path, &out)?;
    std::fs::rename(&tmp_path, &final_path)
}

/// Escape a value for a Prometheus label: backslash, quote, newline.
fn escape_label(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(status: &str, duration_ms: u64, rows: u64) -> RunRecord {
        RunRecord {
            at: Utc::now().to_rfc3339(),
            status: status.into(),
            duration_ms,
            rows,
            node_count: 3,
            trigger: "manual".into(),
            error: (status == "error").then(|| "Binder Error: column gone".into()),
            category: (status == "error").then(|| "schema".into()),
        }
    }

    #[test]
    fn metrics_textfile_written_with_run_history() {
        let ws = tempfile::tempdir().unwrap();
        append_run_record(ws.path(), "orders_etl", record("ok", 1500, 42)).unwrap();
        append_run_record(ws.path(), "orders_etl", record("error", 900, 0)).unwrap();
        append_run_record(ws.path(), "other", record("ok", 100, 7)).unwrap();

        let text =
            std::fs::read_to_string(ws.path().join("logs").join("duckle_metrics.prom")).unwrap();
        assert!(text.contains("duckle_run_last_status{pipeline=\"orders_etl\"} 0"));
        assert!(text.contains("duckle_run_last_status{pipeline=\"other\"} 1"));
        assert!(text.contains("duckle_run_last_duration_seconds{pipeline=\"orders_etl\"} 0.9"));
        assert!(text.contains("duckle_runs_window{pipeline=\"orders_etl\",status=\"ok\"} 1"));
        assert!(text.contains("duckle_runs_window{pipeline=\"orders_etl\",status=\"error\"} 1"));
        assert!(text.contains("# TYPE duckle_run_last_rows gauge"));
        // No half-written temp file left behind.
        assert!(!ws
            .path()
            .join("logs")
            .join("duckle_metrics.prom.tmp")
            .exists());
    }

    #[test]
    fn run_record_carries_error_category() {
        let ws = tempfile::tempdir().unwrap();
        append_run_record(ws.path(), "p", record("error", 10, 0)).unwrap();
        let loaded = load_run_history(ws.path(), "p");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].category.as_deref(), Some("schema"));
    }
}

/// Load the run history for a pipeline (oldest first). Returns an empty
/// vec if there's no history yet or it can't be parsed.
pub fn load_run_history(workspace: &Path, pipeline_id: &str) -> Vec<RunRecord> {
    let path = history_file(workspace, pipeline_id);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}
