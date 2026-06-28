//! Signed reproducible run manifest (".ducklock"). After a successful run the
//! runner can emit a small JSON manifest pinning the pipeline and compiled-plan
//! hashes plus the engine versions, signed with a per-workspace Ed25519 key, so
//! an auditor can verify offline that a given run came from a given pipeline.
//!
//! The signing key is generated on first use under `<workspace>/.duckle/keys/`
//! (the same place the secret-encryption key lives). The manifest embeds its own
//! public key so verification needs nothing but the file; compare the embedded
//! key with `<workspace>/.duckle/keys/manifest.pub` to also establish trust.

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use duckle_duckdb_engine::{compile_pipeline_sql, PipelineDoc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const SCHEMA_VERSION: u32 = 1;
const DUCKDB_VERSION: &str = "1.5.4";

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Load the workspace Ed25519 signing key, generating one on first use.
fn signing_key(workspace: &Path) -> Result<SigningKey, String> {
    let dir = workspace.join(".duckle").join("keys");
    let key_path = dir.join("manifest.key");
    if key_path.exists() {
        let raw = std::fs::read(&key_path).map_err(|e| format!("read signing key: {e}"))?;
        let seed: [u8; 32] = raw
            .try_into()
            .map_err(|_| "signing key file is not 32 bytes".to_string())?;
        return Ok(SigningKey::from_bytes(&seed));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("create keys dir: {e}"))?;
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|e| format!("generate key: {e}"))?;
    std::fs::write(&key_path, seed).map_err(|e| format!("write signing key: {e}"))?;
    let sk = SigningKey::from_bytes(&seed);
    let _ = std::fs::write(dir.join("manifest.pub"), B64.encode(sk.verifying_key().to_bytes()));
    Ok(sk)
}

/// The exact bytes we sign and verify: the manifest body as JSON. The same
/// in-memory body is embedded in the file, so a parse + re-serialize round-trip
/// reproduces these bytes regardless of serde_json key-ordering.
fn body_bytes(body: &Value) -> Vec<u8> {
    serde_json::to_vec(body).unwrap_or_default()
}

/// Build, sign and write a manifest for a completed run. Returns the file path.
pub fn write_manifest(
    workspace: &Path,
    name: &str,
    doc: &PipelineDoc,
    status: &str,
    duration_ms: u64,
    stamp_ms: u128,
) -> Result<PathBuf, String> {
    let pipeline_hash = sha256_hex(&serde_json::to_vec(doc).unwrap_or_default());
    let compiled_hash = match compile_pipeline_sql(doc) {
        Ok(stages) => sha256_hex(&serde_json::to_vec(&stages).unwrap_or_default()),
        Err(e) => return Err(format!("compile for manifest: {e}")),
    };
    let body = json!({
        "schemaVersion": SCHEMA_VERSION,
        "pipeline": name,
        "atEpochMs": stamp_ms.to_string(),
        "status": status,
        "durationMs": duration_ms,
        "nodeCount": doc.nodes.len(),
        "pipelineHash": pipeline_hash,
        "compiledPlanHash": compiled_hash,
        "duckleVersion": env!("CARGO_PKG_VERSION"),
        "duckdbVersion": DUCKDB_VERSION,
    });

    let sk = signing_key(workspace)?;
    let signature = sk.sign(&body_bytes(&body));
    let manifest = json!({
        "alg": "ed25519",
        "publicKey": B64.encode(sk.verifying_key().to_bytes()),
        "signature": B64.encode(signature.to_bytes()),
        "body": body,
    });

    let dir = workspace.join("manifests");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create manifests dir: {e}"))?;
    let safe: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let path = dir.join(format!("{safe}-{stamp_ms}.ducklock"));
    std::fs::write(&path, serde_json::to_vec_pretty(&manifest).unwrap_or_default())
        .map_err(|e| format!("write manifest: {e}"))?;
    Ok(path)
}

/// Verify a manifest's signature over its embedded body. Ok(true) if intact.
pub fn verify_manifest(path: &Path) -> Result<bool, String> {
    let raw = std::fs::read(path).map_err(|e| format!("read manifest: {e}"))?;
    let m: Value = serde_json::from_slice(&raw).map_err(|e| format!("parse manifest: {e}"))?;
    let body = m.get("body").ok_or("manifest has no body")?;
    let pk_b64 = m
        .get("publicKey")
        .and_then(|v| v.as_str())
        .ok_or("manifest has no publicKey")?;
    let sig_b64 = m
        .get("signature")
        .and_then(|v| v.as_str())
        .ok_or("manifest has no signature")?;
    let pk_bytes: [u8; 32] = B64
        .decode(pk_b64)
        .map_err(|e| format!("publicKey base64: {e}"))?
        .try_into()
        .map_err(|_| "publicKey is not 32 bytes".to_string())?;
    let sig_bytes: [u8; 64] = B64
        .decode(sig_b64)
        .map_err(|e| format!("signature base64: {e}"))?
        .try_into()
        .map_err(|_| "signature is not 64 bytes".to_string())?;
    let vk = VerifyingKey::from_bytes(&pk_bytes).map_err(|e| format!("publicKey invalid: {e}"))?;
    let signature = Signature::from_bytes(&sig_bytes);
    Ok(vk.verify(&body_bytes(body), &signature).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_then_verify_roundtrips_and_detects_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let doc: PipelineDoc = serde_json::from_str(
            r#"{"nodes":[{"id":"s","position":{"x":0,"y":0},"data":{"label":"A","componentId":"src.csv","properties":{"path":"a.csv"}}}],"edges":[]}"#,
        )
        .unwrap();
        let path = write_manifest(ws, "demo", &doc, "ok", 12, 1_700_000_000_000).unwrap();
        assert!(verify_manifest(&path).unwrap(), "fresh manifest should verify");

        // Tamper with the body: verification must fail.
        let mut m: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        m["body"]["durationMs"] = json!(99999);
        std::fs::write(&path, serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(!verify_manifest(&path).unwrap(), "tampered manifest must fail");
    }
}
