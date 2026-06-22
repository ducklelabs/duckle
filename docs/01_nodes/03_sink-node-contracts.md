# Sink Node Contracts

This note documents `snk.*` sink behavior at the contract level. It is intended for agents that need to build workflows or add/modify sink nodes.

Authoritative files:

- Palette entries: `frontend/src/workflow-ui/palette-data.ts`
- Form contracts: `frontend/src/workflow-ui/fields/manifest-synth.ts`
- Sink SQL builders: `crates/duckdb-engine/src/plan/builders.rs`
- Runtime-backed sink specs: `crates/duckdb-engine/src/plan/mod.rs`
- Runtime execution: `crates/duckdb-engine/src/lib.rs`

## Common Sink Contract

Sink nodes consume an upstream relation and usually terminate a graph branch.

| Aspect | Contract |
|---|---|
| Input | `main` upstream input is required. |
| Output | Sinks usually do not create a meaningful downstream dataset. Some remain previewable through upstream passthrough/count behavior. |
| Side effects | Sinks write files, remote tables, object-store objects, messages, emails, HTTP requests, or vector-store documents. |
| Write modes | File and relational sinks commonly support overwrite/append/truncate/upsert/merge shapes, but support differs per sink. |
| Schema | Sinks rely on upstream column names and types. Upsert/merge modes need key/conflict columns. |
| Runtime | Some sinks compile to DuckDB `COPY`/`ATTACH` SQL. Others use Rust runtime clients or HTTP request specs. |

## Runtime Classes

| Runtime class | Nodes | Processing model |
|---|---|---|
| DuckDB file/object writers | `snk.csv`, `snk.tsv`, `snk.parquet`, `snk.json`, `snk.jsonl`, `snk.excel`, `snk.spatial`, `snk.s3`, `snk.gcs`, `snk.azureblob`, `snk.iceberg` | Planner emits DuckDB `COPY` or table-format write SQL. |
| DuckDB attach relational writers | `snk.postgres`, `snk.cockroach`, `snk.mysql`, `snk.mariadb`, `snk.redshift`, `snk.bigquery`, `snk.motherduck`, `snk.ducklake`, `snk.quack`, `snk.pgvector`, default bulk `snk.sqlserver`/`snk.synapse` | Planner emits attach/load plus relational sink SQL. |
| Runtime database writers | `snk.oracle`, `snk.clickhouse`, `snk.mongodb`, `snk.cassandra`, `snk.scylla`, `snk.redis`, non-bulk `snk.sqlserver`/`snk.synapse`, `snk.snowflake`, `snk.databricks` | Runtime materializes upstream rows and sends batches through a driver/API. |
| HTTP/request writers | `snk.rest`, `snk.webhook`, `snk.graphql`, `snk.elastic`, `snk.opensearch`, vector HTTP sinks | Runtime builds HTTP request payloads from upstream rows. |
| Messaging writers | `snk.kafka`, `snk.redpanda`, `snk.nats`, `snk.rabbit`, `snk.pubsub` | Runtime publishes one message per row or batches. |
| Format runtime writers | `snk.xml`, `snk.avro`, `snk.yaml`, `snk.toml`, `snk.ftp`, `snk.email` | Runtime serializes rows or dispatches side effects. |

## File and Lakehouse Sinks

| Nodes | Runtime | Key props | Side effect | Notes |
|---|---|---|---|---|
| `snk.csv`, `snk.tsv` | DuckDB `COPY` | `path`, `mode`, format options | Writes delimited file. | Good for local exports and debugging. |
| `snk.parquet` | DuckDB `COPY` | `path`, `mode`, compression, optional partitioning | Writes parquet dataset/file. | Preferred durable checkpoint/export format. |
| `snk.json`, `snk.jsonl` | DuckDB `COPY` | `path`, `mode` | Writes JSON/NDJSON. | JSONL is better for streaming/log-like outputs. |
| `snk.excel` | DuckDB excel extension | `path`, header options | Writes XLSX. | Requires DuckDB excel extension. |
| `snk.spatial` | DuckDB spatial/GDAL | `path`, `driver` | Writes GeoJSON/GPKG/Shapefile/etc. | Requires spatial extension and driver support. |
| `snk.xml` | Rust XML writer | `path`, `rootElement`, `rowElement` | Writes XML file. | Complex cell values are serialized in XML-friendly form. |
| `snk.avro` | Rust Avro writer | `path`, optional `schemaJson`, `recordName` | Writes Avro object container file. | Schema can be inferred or supplied. |
| `snk.yaml`, `snk.toml` | Rust serializer | `path` | Writes YAML or TOML document. | YAML emits top-level array; TOML wraps under `rows`. |
| `snk.iceberg` | DuckDB iceberg extension | table `path` | Writes Iceberg table root. | Remote storage needs credentials/extension support. |
| `snk.ducklake` | DuckDB DuckLake attach | `path`, `schemaName`, `tableName`, `mode`, conflict columns | Writes DuckLake table. | Supports overwrite/append/truncate/upsert/merge-style modes. |
| `snk.ftp` | Rust FTP/FTPS/SFTP runtime | protocol, host, auth, `remotePath`, `format` | Writes temp local file then uploads. | SFTP requires user; can use password or private key. |

## Database and Warehouse Sinks

| Nodes | Runtime | Key props | Side effect | Notes |
|---|---|---|---|---|
| `snk.postgres`, `snk.cockroach` | DuckDB postgres attach | connection, `schemaName`, `tableName`, `mode`, conflict columns | Writes relational table. | Upsert uses Postgres-style conflict handling. |
| `snk.mysql`, `snk.mariadb` | DuckDB mysql attach | connection, `tableName`, `mode`, conflict columns | Writes MySQL/MariaDB table. | Upsert uses MySQL-family handling. |
| `snk.redshift` | DuckDB postgres attach | Redshift connection, table/mode | Writes Redshift table. | Redshift rides Postgres wire. |
| `snk.bigquery` | DuckDB BigQuery extension | GCP/BigQuery props | Writes BigQuery table. | Depends on DuckDB community extension and credential discovery. |
| `snk.motherduck`, `snk.quack` | DuckDB attach | remote DuckDB connection/table/mode | Writes remote DuckDB table. | Strong fit for DuckDB-native workflows. |
| `snk.pgvector` | DuckDB postgres attach | Postgres connection/table/vector columns | Writes embedding rows. | Server must support pgvector. |
| `snk.sqlserver`, `snk.synapse` | Default bulk path via DuckDB mssql extension; optional row runtime when `bulk=false` | host/user/password/database/table/mode | Writes SQL Server/Synapse table. | Bulk path is default; row-by-row runtime path is available for compatibility. |
| `snk.oracle` | Rust Oracle runtime | `connect`, `user`, `password`, `schema`, `tableName`, `batchSize` | Inserts/merges Oracle rows. | Requires Oracle Instant Client at runtime. |
| `snk.clickhouse` | Rust HTTP runtime | `endpoint`, database/table/auth/batch | Inserts JSONEachRow-style batches. | HTTP interface, not native driver. |
| `snk.snowflake` | Snowflake SQL API runtime | `account`, PAT/JWT auth, database/schema/table/batch/upsert keys | Inserts/upserts Snowflake rows. | Batches multi-row INSERTs through SQL API. |
| `snk.databricks` | Databricks Statement Execution API runtime | workspace, PAT, warehouse id, catalog/schema/table | Inserts/upserts Databricks SQL table. | Wait timeout capped by API behavior. |
| `snk.sqlite`, `snk.duckdb` | DuckDB attach | file path, table, mode | Writes local database file. | Good for durable local studio outputs. |

## Object Storage Sinks

| Nodes | Runtime | Key props | Side effect | Notes |
|---|---|---|---|---|
| `snk.s3` | DuckDB httpfs | bucket, key, region, credentials, format, mode | Writes object/dataset to S3. | Supports standard file formats exposed in form. |
| `snk.gcs` | DuckDB GCS/httpfs path | bucket, key, credentials, format | Writes object/dataset to GCS. | Verify credential path per environment. |
| `snk.azureblob` | DuckDB azure extension | container/bucket-ish fields, key, credentials, format | Writes Azure Blob object/dataset. | Requires azure extension support. |

## Streaming, API, and Messaging Sinks

| Nodes | Runtime | Key props | Side effect | Notes |
|---|---|---|---|---|
| `snk.kafka`, `snk.redpanda` | Rust `rskafka` runtime | `brokers`, `topic`, optional `partitionId`, `keyColumn`, `batchSize` | Publishes one JSON row per message. | Redpanda is Kafka-wire alias. |
| `snk.nats` | Rust `async-nats` runtime | `urls`/`servers`, `subject`, optional `subjectSuffixColumn`, `batchSize` | Publishes NATS messages. | Suffix column can route rows to per-row subjects. |
| `snk.rabbit` | Rust `lapin` runtime | `url`, `exchange`, `routingKey`, `batchSize` | Publishes persistent AMQP messages. | Empty exchange means default direct exchange. |
| `snk.pubsub` | GCP Pub/Sub REST runtime | `project`, `topic`, `accessToken`, `batchSize` | Publishes Pub/Sub messages. | Access token is pre-minted OAuth2 bearer token. |
| `snk.rest` | Rust HTTP runtime | `url`, `method`, headers/auth, body shape/batch mode | Sends one batch request by default. | `batchMode=array` maps to batched payload. |
| `snk.webhook` | Rust HTTP runtime | `url`, `method`, headers/auth | Sends one request per row by default. | Good for notifications and row-level calls. |
| `snk.graphql` | Rust HTTP runtime | endpoint, mutation/query, headers/auth | Sends GraphQL mutation with row data as variables. | Rides webhook/request machinery. |
| `snk.email` | Rust SMTP runtime | SMTP host/port/auth/from plus row columns for to/subject/body | Sends one email per row. | Plain text path currently. |

## NoSQL, Search, and Vector Sinks

| Nodes | Runtime | Key props | Side effect | Notes |
|---|---|---|---|---|
| `snk.mongodb` | Rust MongoDB runtime | `uri`, `database`, `collection`, mode, conflict columns | Inserts/replaces/upserts documents. | Replace mode drops collection first. |
| `snk.cassandra`, `snk.scylla` | Rust Scylla/Cassandra runtime | contact points, keyspace, table | Inserts CQL rows. | CQL uses single-row inserts; batch size is mostly descriptive/chunking. |
| `snk.redis` | Rust Redis runtime | `url`, `keyColumn`, optional `valueColumn`, `ttlSeconds`, `batchSize` | SETs Redis keys. | Empty value column serializes whole row as JSON. |
| `snk.elastic`, `snk.opensearch` | Rust HTTP bulk runtime | endpoint, index, optional API key | Bulk-indexes NDJSON docs. | Uses `_bulk` endpoint with action/doc line pairs. |
| `snk.pinecone` | Rust HTTP runtime | `indexHost`, `apiKey`, vector/id fields | Upserts vectors. | Body wrapped as `vectors`. |
| `snk.qdrant` | Rust HTTP runtime | `clusterUrl`, `collection`, `apiKey` | Upserts points. | PUTs to collection points endpoint. |
| `snk.weaviate` | Rust HTTP runtime | `endpoint`, `class`, `apiKey` | Batch upserts objects. | Body wrapped as `objects`. |
| `snk.milvus` | Rust HTTP runtime | `endpoint`, `collection`, `apiKey` | Inserts vector rows. | Body wrapped as `data` with collectionName extra. |

## Agent Rules

- Treat every sink as a side-effect boundary. Add `xf.count`, `xf.log`, `qa.contract`, or `xf.assert` before important sinks when correctness matters.
- For local studio workflows, prefer `snk.parquet`, `snk.duckdb`, or `snk.sqlite` for durable artifacts.
- For relational upsert/merge, ensure key/conflict columns exist and are stable.
- For HTTP/vector/message sinks, sample before sending if the upstream can be large.
- Verify exact prop names in the planner branch before generating raw workflow JSON. Some UI labels are generic; runtime branches are authoritative.
- Never put planned sinks (`snk.orc`, `snk.jdbc`, `snk.pulsar`, `snk.kinesis`) on critical paths until runtime support is verified.

## Adding a New Sink Node

Minimum implementation checklist:

1. Add the palette entry in `palette-data.ts`.
2. Add or route a form manifest in `manifest-synth.ts`.
3. Decide whether the sink is DuckDB SQL-backed, attach-backed, or runtime-backed.
4. For SQL sinks, add `build_sink_sql` and any attach/extension prelude support in `builders.rs`.
5. For runtime sinks, add a spec type in `plan/specs.rs`, extraction in `plan/mod.rs`, and executor handling in `lib.rs`.
6. Define write modes and key/upsert semantics explicitly.
7. Validate required props and missing input in planner tests.
8. Add at least one test for generated SQL/spec shape and one for failure behavior.
9. Update this doc and `00_node-inventory.md`.
