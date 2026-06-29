//! Sample-workspace seeding.
//!
//! Duckle ships a small bundled starter workspace (a handful of sample
//! pipelines plus a DuckDB data generator) embedded into the binary. When a
//! user opens a brand-new / empty workspace folder, we lay the samples down and
//! generate their input files locally - so they have something that runs on the
//! first launch, with no external services or downloads. Every pipeline here is
//! DuckDB / file-only; nothing needs a live database.

use include_dir::{include_dir, Dir};
use std::path::Path;

// Embedded at compile time from the repo's examples/starter-workspace dir:
// pipelines/<id>.pipeline.json, gen_samples.sql, repository.json, duckle.json.
static STARTER: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../examples/starter-workspace");

/// True if `ws` has not been initialised as a Duckle workspace yet (no
/// metadata file). Mirrors the frontend's loadWorkspace() "fresh folder" check.
pub fn is_fresh(ws: &Path) -> bool {
    !ws.join("duckle.json").exists() && !ws.join("workspace.json").exists()
}

/// Seed the bundled sample pipelines + generate their data into `ws`, using the
/// provisioned `duckdb_bin` to synthesize the input files. No-op (returns
/// Ok(false)) if the workspace already looks initialised. Returns Ok(true) when
/// it actually seeded, so the caller knows to re-hydrate.
pub fn seed(ws: &Path, duckdb_bin: &Path) -> Result<bool, String> {
    if !is_fresh(ws) {
        return Ok(false);
    }
    for sub in ["data", "output", "pipelines"] {
        std::fs::create_dir_all(ws.join(sub)).map_err(|e| e.to_string())?;
    }

    // Pipeline files: bundled as <id>.pipeline.json, written as <id>.json
    // (the on-disk name a workspace expects, keyed by repository.json id).
    if let Some(dir) = STARTER.get_dir("pipelines") {
        for f in dir.files() {
            let name = f
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("bad sample pipeline file name")?;
            let id = name
                .strip_suffix(".pipeline.json")
                .ok_or("sample pipeline must be named <id>.pipeline.json")?;
            std::fs::write(ws.join("pipelines").join(format!("{id}.json")), f.contents())
                .map_err(|e| e.to_string())?;
        }
    }

    // Project tree + metadata, written verbatim.
    for top in ["repository.json", "duckle.json"] {
        if let Some(f) = STARTER.get_file(top) {
            std::fs::write(ws.join(top), f.contents()).map_err(|e| e.to_string())?;
        }
    }

    // Generate the sample input files via the provisioned DuckDB. The script
    // uses ${workspace} placeholders; substitute the real path (forward slashes
    // so the SQL string literals are valid on Windows too).
    let tmpl = STARTER
        .get_file("gen_samples.sql")
        .and_then(|f| f.contents_utf8())
        .ok_or("missing or non-utf8 gen_samples.sql")?;
    let ws_fwd = ws.to_string_lossy().replace('\\', "/");
    let sql = tmpl.replace("${workspace}", &ws_fwd);
    let gen_path = ws.join(".duckle-gen.sql");
    std::fs::write(&gen_path, &sql).map_err(|e| e.to_string())?;

    // Run the generator. `.read <file>` as a positional command (dot-commands
    // are not honored via -c). CREATE_NO_WINDOW: no console flash on Windows.
    let read_cmd = format!(".read {}", gen_path.to_string_lossy().replace('\\', "/"));
    let mut cmd = std::process::Command::new(duckdb_bin);
    cmd.arg(":memory:").arg(&read_cmd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&gen_path);
    if !out.status.success() {
        return Err(format!(
            "sample data generation failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    // End-to-end check of the real seeding path: the embedded starter workspace
    // is laid down and its data generated through DuckDB. Skips when
    // DUCKLE_DUCKDB_BIN is unset (mirrors the engine crate's engine_or_skip).
    #[test]
    fn seed_lays_down_pipelines_and_generates_data() {
        let bin = match std::env::var("DUCKLE_DUCKDB_BIN") {
            Ok(b) if !b.is_empty() => std::path::PathBuf::from(b),
            _ => {
                eprintln!("skipping seed test: DUCKLE_DUCKDB_BIN not set");
                return;
            }
        };
        let ws = std::env::temp_dir().join(format!("duckle-seed-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ws);
        std::fs::create_dir_all(&ws).unwrap();

        let seeded = seed(&ws, &bin).expect("seed should succeed");
        assert!(seeded, "a fresh workspace should report it seeded");

        // Workspace metadata + a representative pipeline from each format.
        for f in [
            "duckle.json",
            "repository.json",
            "pipelines/orders_filter.json",
            "pipelines/ducklake_cdc.json",
            "pipelines/enrich_parallel.json",
            "pipelines/_csv_split_child.json",
        ] {
            assert!(ws.join(f).exists(), "expected seeded file {f}");
        }
        // Generated inputs across the formats the samples read.
        for f in [
            "data/orders.csv",
            "data/orders.parquet",
            "data/customers.csv",
            "data/products.duckdb",
            "data/regions.sqlite",
            "data/cdc.ducklake",
        ] {
            assert!(ws.join(f).exists(), "expected generated data file {f}");
        }
        // The .pipeline.json suffix must be stripped on the way to disk.
        assert!(
            !ws.join("pipelines/orders_filter.pipeline.json").exists(),
            "pipeline files should be written as <id>.json, not <id>.pipeline.json"
        );
        // Idempotent: a second call no-ops because duckle.json now exists.
        assert!(
            !seed(&ws, &bin).expect("second seed should succeed"),
            "seeding an already-initialised workspace should be a no-op"
        );

        let _ = std::fs::remove_dir_all(&ws);
    }
}
