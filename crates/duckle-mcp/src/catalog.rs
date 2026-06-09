//! Component catalog, embedded from a committed catalog.json that is exported
//! from the frontend manifest (`npm --prefix frontend run export-catalog`).
//!
//! The catalog is read loosely as a `serde_json::Value` so the Rust side does
//! not have to track every field the frontend manifest emits: the TS manifest
//! stays the single source of truth and this module just indexes + filters it.

use serde_json::{json, Value};
use std::sync::OnceLock;

const CATALOG_JSON: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/catalog.json"));

fn catalog() -> &'static Value {
    static C: OnceLock<Value> = OnceLock::new();
    C.get_or_init(|| serde_json::from_str(CATALOG_JSON).unwrap_or_else(|_| json!({ "components": [] })))
}

fn components() -> &'static [Value] {
    static EMPTY: Vec<Value> = Vec::new();
    catalog()
        .get("components")
        .and_then(|c| c.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&EMPTY)
}

/// List components, optionally filtered by `kind` (source/transform/sink/
/// control/quality/custom) and/or a case-insensitive substring `query` over
/// id/label/summary. Returns only the summary fields (not the full schema).
pub fn list(kind: Option<&str>, query: Option<&str>) -> Value {
    let q = query.map(|s| s.to_lowercase());
    let items: Vec<Value> = components()
        .iter()
        .filter(|c| {
            let ok_kind = kind.map_or(true, |want| {
                c.get("kind").and_then(|v| v.as_str()) == Some(want)
            });
            let ok_q = match &q {
                Some(q) => {
                    let hay = |k: &str| {
                        c.get(k)
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_lowercase()
                    };
                    hay("id").contains(q.as_str())
                        || hay("label").contains(q.as_str())
                        || hay("summary").contains(q.as_str())
                }
                None => true,
            };
            ok_kind && ok_q
        })
        .map(|c| {
            json!({
                "id": c.get("id"),
                "label": c.get("label"),
                "kind": c.get("kind"),
                "availability": c.get("availability"),
                "summary": c.get("summary"),
            })
        })
        .collect();
    json!({ "count": items.len(), "components": items })
}

/// Full schema (property fields + ports) for one component id, or None.
pub fn schema(id: &str) -> Option<Value> {
    components()
        .iter()
        .find(|c| c.get("id").and_then(|v| v.as_str()) == Some(id))
        .cloned()
}

/// The whole embedded catalog (for the duckle://catalog resource).
pub fn full() -> &'static Value {
    catalog()
}
