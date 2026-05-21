//! Duckle connectors: typed sources and sinks for files, databases,
//! streaming systems, and object storage.
//!
//! Each connector implements the [`duckle_plugin_sdk::SchemaInspector`]
//! trait and accepts a JSON-described configuration so the UI can render
//! property panels generically. The runtime calls into each connector
//! through that trait; nothing here knows about Tauri or the canvas.

pub mod csv;

pub use crate::csv::{CsvConnector, CsvOptions};
