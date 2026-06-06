# Execution Engines

Duckle features a multi-engine architecture. Visual pipeline graphs compile into an engine-agnostic logical representation, which can then be dispatched to different execution engines.

---

## 1. DuckDB Engine (`duckle-duckdb-engine`)

The default, production-ready execution engine compiles visual pipeline graphs into complex, nested SQL scripts and runs them natively through the embedded **DuckDB CLI**.

### Core Attributes
* **DuckDB Version**: Tracks stable release **v1.5.3**.
* **SQL Compilation**: Rather than running slow row-by-row lookups, the compiler maps visual nodes to nested subqueries and joins. Relational operations (aggregates, joins, window queries) achieve native execution speeds.
* **Concurrency**: Duckle automatically parses separate parallel branches on your canvas (such as a single source fanning out to multiple files) and runs them concurrently, scaling database workers to match local CPU core availability.
* **Mid-Run Cancellations**: A cancellation command cleanly kills the underlying DuckDB subprocess, immediately releasing resources without leaving orphaned database files.

### Extension Pre-Fetching
To ensure offline compatibility, the first-launch setup pre-downloads all extension libraries used by Duckle's connectors:
* `httpfs` (S3 / GCS reads)
* `azure` (Azure Blob Storage connector)
* `sqlite` & `postgres` & `mysql` (Relational attachment secrets)
* `excel` (Spreadsheet importer)
* `iceberg` & `delta` & `ducklake` (Lakehouse structures)
* `vss` (Vector Similarity Search)
* `fts` (Full-Text BM25 Search)
* *Note: The `spatial` extension (~50 MB GDAL bundle) is lazy-loaded on the first drag-and-drop of a geospatial node to keep the initial installer package compact.*

---

## 2. SlothDB Adapter (`duckle-slothdb-engine`)

Duckle includes an adapter for **SlothDB**, an alternative embedded analytical database engine.

* **Upstream**: [SouravRoy-ETL/slothdb](https://github.com/SouravRoy-ETL/slothdb).
* **Role**: Configured per pipeline when a lightweight alternative to DuckDB is needed.
* **Setup**: Downloaded via the engine manager panel inside the desktop application.

---

## 3. Future Engine Architectures

Duckle's workspace code files contain placeholders and early FFI integrations for two upcoming execution environments:

### Stream Engine (`duckle-stream-engine`)
* **Role**: Designed to process infinite, event-driven message feeds.
* **Model**: Implements bounded, backpressure-aware operator pipelines.
* **Integrations**: Designed to consume directly from streaming brokers like Apache Kafka, Apache Pulsar, and NATS JetStream, transferring data in unified Arrow schemas.

### Transform Engine (`duckle-transform-engine`)
* **Role**: In-process Rust execution runner.
* **Model**: Composes Arrow-native vectorized operators.
* **Benefit**: Bypasses compilation to SQL dialects, running pure in-process transformations on tabular `RecordBatch` streams.
