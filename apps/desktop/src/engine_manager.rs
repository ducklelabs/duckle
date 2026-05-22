//! Engine installation manager.
//!
//! Duckle ships a tiny shell and downloads its execution engines on
//! first launch into the app-data directory, rather than statically
//! bundling them. This module knows where each engine lives, whether
//! it's installed, and how to fetch + verify it from its official
//! release.
//!
//! DuckDB is downloaded as the official single-file CLI from GitHub
//! releases. SlothDB is an optional, user-supplied engine; its
//! distribution URL isn't wired yet, so it shows as optional and
//! skippable.

use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};

/// DuckDB CLI version we install. Keep in step with the SQL dialect the
/// engine generates.
pub const DUCKDB_VERSION: &str = "1.1.3";

#[derive(Debug, Serialize)]
pub struct EngineStatus {
    pub id: String,
    pub name: String,
    pub description: String,
    pub required: bool,
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    /// Whether Duckle has a download for this platform.
    pub available: bool,
}

/// Progress event streamed to the frontend during a download.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum InstallProgress {
    Downloading { received: u64, total: Option<u64> },
    Extracting,
    Verifying,
    Done { path: String },
}

pub fn engines_root(app_data: &Path) -> PathBuf {
    app_data.join("engines")
}

fn duckdb_binary_name() -> &'static str {
    if cfg!(windows) {
        "duckdb.exe"
    } else {
        "duckdb"
    }
}

pub fn duckdb_path(app_data: &Path) -> PathBuf {
    engines_root(app_data)
        .join("duckdb")
        .join(duckdb_binary_name())
}

/// The release asset name for this OS/arch, or None if unsupported.
fn duckdb_asset() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("duckdb_cli-windows-amd64.zip"),
        ("windows", "aarch64") => Some("duckdb_cli-windows-arm64.zip"),
        ("linux", "x86_64") => Some("duckdb_cli-linux-amd64.zip"),
        ("linux", "aarch64") => Some("duckdb_cli-linux-aarch64.zip"),
        ("macos", _) => Some("duckdb_cli-osx-universal.zip"),
        _ => None,
    }
}

pub fn status(app_data: &Path) -> Vec<EngineStatus> {
    let duck = duckdb_path(app_data);
    let duck_installed = duck.exists();
    vec![
        EngineStatus {
            id: "duckdb".into(),
            name: "DuckDB".into(),
            description: "Default engine — local analytics, file formats, SQL.".into(),
            required: true,
            installed: duck_installed,
            version: duck_installed.then(|| DUCKDB_VERSION.to_string()),
            path: duck_installed.then(|| duck.to_string_lossy().to_string()),
            available: duckdb_asset().is_some(),
        },
        EngineStatus {
            id: "slothdb".into(),
            name: "SlothDB".into(),
            description: "Optional embedded engine. Configure its source to install.".into(),
            required: false,
            installed: false,
            version: None,
            path: None,
            // Distribution URL not wired yet — shown as optional/skippable.
            available: false,
        },
    ]
}

/// Download + extract the DuckDB CLI into the app-data engines dir.
/// Reports progress through `on_progress`. Returns the installed
/// binary path on success.
pub fn install_duckdb<F: FnMut(InstallProgress)>(
    app_data: &Path,
    mut on_progress: F,
) -> Result<String, String> {
    let asset = duckdb_asset().ok_or_else(|| {
        format!(
            "No DuckDB build for {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let url = format!(
        "https://github.com/duckdb/duckdb/releases/download/v{}/{}",
        DUCKDB_VERSION, asset
    );

    let dir = engines_root(app_data).join("duckdb");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // Stream the download so we can report progress.
    let client = reqwest::blocking::Client::builder()
        .user_agent("duckle")
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client.get(&url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Download failed: HTTP {}", resp.status()));
    }
    let total = resp.content_length();
    let mut buf: Vec<u8> = Vec::with_capacity(total.unwrap_or(0) as usize);
    let mut chunk = [0u8; 64 * 1024];
    let mut received: u64 = 0;
    on_progress(InstallProgress::Downloading {
        received: 0,
        total,
    });
    loop {
        let n = resp.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        received += n as u64;
        on_progress(InstallProgress::Downloading { received, total });
    }

    on_progress(InstallProgress::Extracting);
    let reader = std::io::Cursor::new(buf);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
    let target = duckdb_path(app_data);
    let mut extracted = false;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();
        let leaf = name.rsplit('/').next().unwrap_or(&name);
        if leaf.eq_ignore_ascii_case("duckdb") || leaf.eq_ignore_ascii_case("duckdb.exe") {
            let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
            extracted = true;
            break;
        }
    }
    if !extracted {
        return Err("DuckDB binary not found inside the downloaded archive".into());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755));
    }

    // Verify it actually runs.
    on_progress(InstallProgress::Verifying);
    let ok = std::process::Command::new(&target)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        return Err("Installed DuckDB binary failed to run (--version)".into());
    }

    let path = target.to_string_lossy().to_string();
    on_progress(InstallProgress::Done { path: path.clone() });
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_duckdb_missing_in_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let st = status(tmp.path());
        let duck = st.iter().find(|e| e.id == "duckdb").unwrap();
        assert!(!duck.installed);
        assert!(duck.required);
        // DuckDB should be available to download on any tier-1 platform.
        assert!(duck.available, "expected a DuckDB build for this platform");
    }

    #[test]
    #[ignore = "downloads the DuckDB CLI from GitHub releases (network)"]
    fn downloads_extracts_and_verifies_duckdb() {
        let tmp = tempfile::tempdir().unwrap();
        let path = install_duckdb(tmp.path(), |_p| {}).expect("install should succeed");
        assert!(std::path::Path::new(&path).exists(), "binary should exist");

        let st = status(tmp.path());
        let duck = st.iter().find(|e| e.id == "duckdb").unwrap();
        assert!(duck.installed, "status should now report installed");

        // The installed binary actually runs.
        let out = std::process::Command::new(&path)
            .arg("--version")
            .output()
            .expect("run --version");
        assert!(out.status.success());
    }
}
