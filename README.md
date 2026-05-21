# Duckle

> Embedded, ultra-fast, local-first data engineering platform — ETL, ELT, and streaming with a drag-and-drop UI.

Duckle is an open-source data integration platform built around three principles:

- **Embedded** — single small binary, no JVM, no cluster, no cloud account required.
- **Fast** — vectorized, Arrow-native execution with pluggable engines.
- **Visual** — a node-based DAG editor for designing pipelines, plus a SQL editor and live execution view.

Duckle is in early development. Expect rapid change.

## Architecture overview

Duckle is a Rust workspace with a Tauri 2 desktop shell and a React + TypeScript frontend.

```
duckle/
├── apps/
│   └── desktop/              Tauri 2 desktop shell
├── crates/
│   ├── runtime/              Process lifecycle, IPC plumbing, app state
│   ├── connectors/           Source and sink connectors (CSV, Parquet, SQLite, ...)
│   ├── workflow-engine/      DAG model, validation, topological scheduling
│   ├── transform-engine/     Native vectorized transforms (filter, project, join, ...)
│   ├── stream-engine/        Streaming and incremental pipelines
│   ├── execution-core/       Cross-engine execution abstractions
│   ├── duckdb-engine/        DuckDB-backed execution
│   ├── slothdb-engine/       SlothDB-backed execution
│   ├── plugin-sdk/           Plugin contract: connectors, transforms, engines
│   ├── metadata/             Pipeline definitions, schemas, lineage
│   └── scheduler/            Time- and event-driven pipeline scheduling
└── frontend/                 React 19 + Vite 6 + TypeScript + @xyflow/react
    ├── canvas/               DAG editor
    ├── components/           Shared UI primitives
    └── workflow-ui/          Sidebar, properties, run view, lineage
```

See [ARCHITECTURE.md](./ARCHITECTURE.md) for the design in depth.

## Execution engines

A pipeline is independent of the engine that runs it. Duckle ships three:

| Engine        | When to use                                              |
|---------------|----------------------------------------------------------|
| **DuckDB**    | Default. Local analytics, file formats, ad-hoc queries.  |
| **SlothDB**   | Optional. See https://github.com/SouravRoy-ETL/slothdb.  |
| **Native**    | Streaming and incremental pipelines, no DB dependency.   |

The DuckDB engine binary is downloaded on first run if not present.

## Connectors (planned for Phase 1)

Files: CSV, TSV, JSON, XML, Excel, Avro, Parquet, ORC.
Embedded DBs: SQLite, DuckDB, SlothDB.
Databases: PostgreSQL, MySQL, MariaDB, Oracle, SQL Server, DB2, Snowflake, BigQuery, Redshift.
Streaming: Kafka, Redpanda, Pulsar.
Object storage: S3, Azure Blob, GCS.
Other: REST, GraphQL, Webhooks.

Phase 1 ships CSV, Parquet, and SQLite. Everything else lands incrementally.

## Status

Phase 1, day zero. The workspace, crates, and UI shell are scaffolded; runtime behavior is being implemented.

## Building from source

Prerequisites:

- Rust stable (install via https://rustup.rs)
- Node.js 20+ and npm
- Platform build tools for Tauri 2 (see https://tauri.app/start/prerequisites)

```sh
# install frontend deps
npm --prefix frontend install

# run the desktop app in dev mode
cargo run -p duckle-desktop
```

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE))
- MIT license ([LICENSE-MIT](./LICENSE-MIT))

at your option.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md).
