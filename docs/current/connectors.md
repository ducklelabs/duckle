# Connectors & Sources / Sinks

Duckle separates connector metadata and schema inspection from actual execution. Connectors declare their properties and schemas to the visual planner, while execution engines compile those properties into physical read/write operations (e.g., SQL commands in DuckDB).

---

## 1. The Connector Contracts (`plugin-sdk`)

All connectors implement the core contracts defined in the `duckle-plugin-sdk` crate. The desktop shell queries these traits to inspect schemas and construct previews without loading entire datasets.

```rust
#[async_trait]
pub trait SchemaInspector: Send + Sync {
    /// Stable identifier matching the palette `componentId` (e.g. `"src.csv"`).
    fn component_id(&self) -> &str;

    /// Read config JSON, infer schema columns, and pull preview sample rows.
    async fn inspect(&self, config: JsonValue) -> Result<Inspection, InspectError>;
}

#[async_trait]
pub trait Connector: SchemaInspector {
    fn kind(&self) -> ConnectorKind;
}

pub enum ConnectorKind {
    Source,
    Sink,
}
```

---

## 2. Deep Dive: CSV Source Connector (`src.csv`)

The CSV/TSV source connector is implemented natively in Rust (`crates/connectors/src/csv.rs`). 

### Core Features
* **Stateless Inspection**: One singleton `CsvConnector` handles all CSV/TSV node schema checks asynchronously.
* **Non-blocking Execution**: Sync parsing runs inside a `tokio::task::spawn_blocking` pool to prevent locking the main async runtime loop when handling large files.
* **Cap on Memory**: Inspection limits reads to `8 MB` (`MAX_INSPECT_BYTES`). Even on a 50 GB CSV, Duckle only reads enough to extract headers and preview rows, completing in milliseconds.
* **Encoding Support**: Decodes bytes using `encoding_rs`, providing built-in support for `UTF-8`, `UTF-16`, `Latin-1`, `Windows-1252`, and more.

### Configuration Properties
When dragging a CSV source, the properties panel maps to the following Rust options:

| Property | JSON Key | Default | Description |
| :--- | :--- | :--- | :--- |
| **Path** | `path` | *Required* | Absolute path to the local CSV/TSV file. |
| **Has Header** | `hasHeader` | `true` | If false, columns are automatically named `col_1`, `col_2`, etc. |
| **Delimiter** | `delimiter` | `,` | Separator byte. Supports literal escapes like `\t` for tabs. |
| **Quote Char** | `quoteChar` | `"` | Enclosure character. Leaving blank disables quoting. |
| **Encoding** | `encoding` | `utf-8` | Target text decoder format. |
| **Skip Lines** | `skipLines` | `0` | Skips header preambles (e.g., generated comment headers). |
| **Sample Rows** | `sampleRows` | `200` | Number of rows scanned to infer schema and types. |
| **Null Value** | `nullValue` | `None` | Case-insensitive token recognized as `NULL` (e.g., `NA`, `N/A`, `NaN`). |

### Type Inference Rules
The CSV inspector infers column data types by running sample rows against validators in order of specificity:
1. **Timestamp**: Matches `YYYY-MM-DD HH:MM[:SS][.fff][Z|+-HH:MM]`.
2. **Date**: Matches standard ISO `YYYY-MM-DD` structures.
3. **Int64**: Successfully parses to a 64-bit integer.
4. **Float64**: Parses to a finite real number (NaN and infinities are rejected to maintain JSON-safety).
5. **Bool**: Matches named boolean tokens like `true`/`false` or `yes`/`no` (ambiguous `0`/`1` values stay numeric to avoid mis-typing flags).
6. **String**: Fallback type if any row contains a non-null incompatible value.

---

## 3. Other Available Connectors

Duckle supports over 290+ connectors. For relational databases, warehouses, and object stores, the planner collects metadata properties and translates them into appropriate SQL statements executed via DuckDB extensions.

### Files
* **Supported Formats**: CSV/TSV, Parquet, JSON/JSONL, Excel (.xlsx), YAML, TOML, XML (path-based parsing), Fixed-Width, and Apache Avro.
* **Geospatial Files**: GeoJSON, Shapefile, GeoPackage, KML, GPX, and GML via DuckDB's `spatial` extension.

### Databases & Warehouses
* **Relational DBs**: PostgreSQL, MySQL, MariaDB, CockroachDB, SQL Server, Oracle, and ClickHouse.
* **Lakehouses**: Apache Iceberg, Delta Lake, and DuckLake.
* **Cloud Warehouses**: MotherDuck, Snowflake (SQL API), BigQuery, Redshift, and Databricks.

### Object & Cloud Storage
* Amazon S3, Google Cloud Storage, Azure Blob Storage, Cloudflare R2, Backblaze B2, MinIO, and local/HTTP directories.

### Streaming & NoSQL
* **Event Streams**: Kafka, Redpanda, NATS JetStream, GCP Pub/Sub, RabbitMQ, and AWS Kinesis.
* **Document/Key-Value**: MongoDB, Redis, Cassandra/ScyllaDB, Elasticsearch, and DynamoDB.

### Vector Databases & Sinks
* pgvector, Pinecone, Qdrant, Weaviate, Milvus, Chroma, and LanceDB.
