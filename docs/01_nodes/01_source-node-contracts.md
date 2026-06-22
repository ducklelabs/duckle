# Source Node Contracts

This note documents how source nodes work at the contract level. It is intended for agents that need to build workflows or add/modify nodes.

Authoritative files:

- Palette entries: `frontend/src/workflow-ui/palette-data.ts`
- Form contracts: `frontend/src/workflow-ui/fields/manifest-synth.ts`
- SQL source builders: `crates/duckdb-engine/src/plan/builders.rs`
- Runtime-backed source specs: `crates/duckdb-engine/src/plan/mod.rs`
- Runtime execution: `crates/duckdb-engine/src/lib.rs`

## Common Source Contract

Source nodes normally have no upstream input. They create one named output relation that downstream nodes read.

| Aspect | Contract |
|---|---|
| Input ports | Usually none. Sources start a graph branch. |
| Main output | A DuckDB table or view named from the node id by the planner. |
| Reject output | Only some readers/validators have meaningful reject paths. CSV/TSV can split parse failures when a reject port is wired and a declared schema exists. |
| Schema | Usually inferred from the external data. CSV/TSV and Excel can use declared schema metadata in specific paths. Runtime/API sources infer from JSON rows after materialization. |
| State | Most sources are stateless. `src.ducklake.changes` persists a consumed snapshot. Incremental behavior can also be modeled with downstream `xf.incremental`. |
| Side effects | Sources should not mutate external systems, except listener-style nodes such as webhook and queue consumers that acknowledge/drain messages. |

## Runtime Classes

For workflow design, classify sources by runtime. This determines speed, dependencies, debuggability, and failure modes.

| Runtime class | Sources | Processing model |
|---|---|---|
| DuckDB file readers | `src.csv`, `src.tsv`, `src.parquet`, `src.json`, `src.jsonl`, `src.excel`, `src.spatial`, local `src.duckdb`, local `src.sqlite` | Planner emits DuckDB SQL such as `read_csv`, `read_parquet`, `read_json`, `ST_Read`, or `ATTACH`. Good for fast local studio workflows. |
| DuckDB extensions / attach | `src.postgres`, `src.mysql`, `src.mariadb`, `src.cockroach`, `src.redshift`, `src.bigquery`, `src.motherduck`, `src.ducklake`, `src.quack`, `src.pgvector`, `src.iceberg`, `src.delta`, object storage | Planner emits install/load/attach/read SQL. Depends on DuckDB extensions and external credentials/network. |
| Rust runtime materializers | `src.sqlserver`, `src.oracle`, `src.adbc`, `src.mongodb`, `src.cassandra`, `src.scylla`, `src.redis`, `src.elastic`, `src.opensearch`, `src.kafka`, `src.nats`, `src.rabbit`, `src.kinesis`, `src.pubsub`, `src.webhook`, `src.email`, `src.git`, `src.clipboard`, `src.ftp`, `src.xml`, `src.avro`, `src.yaml`, `src.toml`, vector stores | Planner creates a runtime spec. Runtime fetches data, writes/materializes rows, then exposes them to DuckDB for downstream stages. |
| REST/GraphQL aliases | `src.rest`, `src.graphql`, SaaS nodes, `src.odata`, `src.soap`, `src.couchdb`, many API/productivity/devtools/comms nodes | All ride the same REST source machinery with different defaults for URL, auth, response path, and pagination. |
| Planned/preview | Listed in `00_node-inventory.md` | Do not use as foundation workflow dependencies until verified. |

## Agent Rules

- Prefer available nodes over planned/preview nodes.
- Prefer DuckDB-native sources for local repeatability and fast previews.
- For SaaS/API work, treat vendor tiles as preconfigured `src.rest` or `src.graphql` unless the planner has a dedicated runtime branch.
- Verify prop names in `plan/mod.rs` before generating workflow JSON. Some UI manifests are generic and may not perfectly match runtime field names.
- For new source nodes, decide early whether it should compile to DuckDB SQL or require a runtime spec.
- Any source that touches credentials should support saved connections eventually; current coverage is uneven.

## File Sources

| Nodes | Runtime | Key props | Output | Restrictions / notes |
|---|---|---|---|---|
| `src.csv`, `src.tsv` | DuckDB `read_csv_auto` path | `path`, `hasHeader`, `delimiter`, `quoteChar`, `skipLines`, optional `dateFormat`, `timestampFormat`, `filename`, `readOptions` | Rows from one file or glob. Optional `filename` column. | With reject wired and declared schema, planner can split invalid parse rows instead of aborting main output. |
| `src.parquet` | DuckDB `read_parquet` | `path`, optional `columns`, `glob` | Typed parquet rows. | Best default for internal artifacts/checkpoints. |
| `src.json`, `src.jsonl` | DuckDB JSON reader | `path`, `format`, `flatten`, `recordsPath` | Rows from top-level array, JSONL, object, or nested records path. | Schema inference depends on sampled JSON shape. |
| `src.excel` | DuckDB Excel extension | `path`, `sheet`, `range`, optional declared schema | Worksheet rows. | Requires DuckDB excel extension. Declared schema path casts/project wraps because `read_xlsx` has limited type mapping. |
| `src.xml` | Rust `quick-xml` materializer | `path`, runtime `rowPath` | One row per matching element. Attributes use `@`, text uses `_text`, nested children become nested values. | UI currently exposes `rootPath`; runtime branch expects `rowPath`. Check before emitting workflow JSON. |
| `src.avro` | Rust `apache-avro` materializer | `path` | Rows using Avro object container schema. | No DuckDB Avro extension dependency. File carries schema. |
| `src.yaml`, `src.toml` | Rust serde materializer | `path` | YAML array becomes rows; non-array doc becomes one row. TOML doc becomes one row. | Useful for config ETL, not large bulk logs. |
| `src.fixedwidth` | DuckDB SQL projection over text | `path`, widths/column definitions | Columns sliced by position. | Good for banking/mainframe exports. Verify exact prop shape in builder before generating. |
| `src.spatial` | DuckDB spatial extension / GDAL | `path` | Geospatial rows/geometry. | Requires spatial extension support and readable format driver. |

## Lakehouse Sources

| Nodes | Runtime | Key props | Output | Restrictions / notes |
|---|---|---|---|---|
| `src.iceberg` | DuckDB iceberg extension | `path` | Table rows from Iceberg table root. | Local or object-store table root. Requires extension support and storage credentials for remote paths. |
| `src.delta` | DuckDB delta extension | `path` | Table rows from Delta table root. | Same remote storage caveats as Iceberg. |
| `src.ducklake` | DuckDB DuckLake attach | `path`, read fields, optional `asOfVersion`, `asOfTimestamp` | Rows from DuckLake table. | Supports time travel via snapshot/version or timestamp. |
| `src.ducklake.changes` | Runtime/DuckLake stateful source | `path`, `schema`, `table`, `initialSnapshot`, `insertsOnly` | Change rows with `change_type`. | Persists consumed snapshot only after successful run. |
| `src.ducklake.diff` | DuckDB DuckLake diff SQL | `path`, `schema`, `table`, `fromVersion`, `toVersion` | Change rows between snapshots with `change_type`. | Good for CI/data-diff workflows. |

## Database and Warehouse Sources

| Nodes | Runtime | Key props | Output | Restrictions / notes |
|---|---|---|---|---|
| `src.postgres`, `src.cockroach`, `src.pgvector`, `src.redshift` | DuckDB postgres attach | host/port/user/password/database plus table or query | SQL result rows. | Redshift uses Postgres wire. Pgvector reads vector columns through Postgres extension path. |
| `src.mysql`, `src.mariadb` | DuckDB mysql attach | host/port/user/password/database plus table or query | SQL result rows. | Dolt SQL Server should likely be tested through this path because Dolt speaks MySQL wire protocol. |
| `src.sqlserver`, `src.synapse` | Rust TDS runtime | `host`, `port`, `user`, `password`, `database`, `query` or `tableName` + `schema`, `trustCert` | SQL result rows materialized to DuckDB. | Pure Rust client. Port range checked. |
| `src.oracle` | Rust `oracle` crate runtime | `connect`, `user`, `password`, `query` or `tableName` + `schema` | SQL result rows. | Requires Oracle Instant Client at runtime. |
| `src.sqlite`, `src.duckdb` | DuckDB attach/read | file path plus table/query | Local database rows. | Good for local studio fixtures and artifacts. |
| `src.clickhouse` | Rust HTTP runtime | `endpoint`, optional `database`, `user`, `password`, `query` or `tableName` | SQL result rows. | Uses ClickHouse HTTP interface. |
| `src.adbc` | Rust ADBC runtime | `driver`/`driverPath`, optional `entrypoint`, `uri`/`options`, `query` | Arrow result rows materialized through Parquet. | Requires external ADBC driver shared library. |
| `src.snowflake` | Rust SQL API runtime | `account`, auth (`pat` or JWT private key), warehouse/role/database/schema, query/table | SQL result rows. | No native driver; uses Snowflake SQL API. |
| `src.databricks` | Rust Statement Execution API runtime | `workspace`, `pat`, `warehouseId`, optional catalog/schema, query/table | SQL result rows. | Waits on Databricks SQL statement execution. |
| `src.bigquery` | DuckDB community extension | credentials by GCP discovery plus table/query fields | BigQuery rows. | Depends on DuckDB extension and local GCP credential setup. |
| `src.motherduck`, `src.quack` | DuckDB attach | token/URL/database/table/query | Remote DuckDB rows. | Good fit for DuckDB-native workflows when remote services are reachable. |

## Object Storage Sources

| Nodes | Runtime | Key props | Output | Restrictions / notes |
|---|---|---|---|---|
| `src.s3` | DuckDB httpfs | `bucket`, `key`, `region`, credentials/connection, `format`, optional glob | Rows from remote CSV/JSON/JSONL/Parquet. | Creates DuckDB secrets/remote read SQL. |
| `src.minio`, `src.r2`, `src.b2` | DuckDB S3-compatible path | S3 props plus `endpoint`, `urlStyle`, `useSsl` | Rows from S3-compatible object store. | URL style and TLS matter for local MinIO/B2/R2. |
| `src.gcs` | DuckDB httpfs/GCS path | bucket/key/credentials/format | Rows from GCS object. | Credential handling should be verified per environment. |
| `src.azureblob` | DuckDB azure extension | bucket/container-ish fields, key, credentials/format | Rows from Azure Blob object. | Requires azure extension support. |
| `src.http` | Generic API source alias in UI, cloud source in planner for readable file URLs | URL/API props or HTTP file props depending path | API rows or file rows depending planner branch. | This node has overlapping meanings. Verify planner path and props before relying on it. |

## Streaming and Queue Sources

These are batch-drain sources, not continuous streaming operators.

| Nodes | Runtime | Key props | Output | Restrictions / notes |
|---|---|---|---|---|
| `src.kafka`, `src.redpanda` | Rust `rskafka` runtime | `brokers`, `topic`, optional `partitionId`, `offset`/`startOffset`, `maxRecords` | `{offset, key, value, timestamp_ms}` rows. | Current runtime reads a single partition. UI has security fields, but runtime branch currently only consumes broker/topic/offset-related fields. |
| `src.nats` | Rust `async-nats` runtime | `urls`/`servers`, `subject`, `maxRecords`, `timeoutMs` | `{subject, payload}` rows. | Drains until max or timeout. |
| `src.rabbit` | Rust `lapin` runtime | `url`, `queue`, `maxMessages`, `timeoutMs` | `{payload, routing_key, exchange, delivery_tag}` rows. | Auto-acks pulled messages. Queue must exist. |
| `src.kinesis` | Direct AWS HTTP + SigV4 runtime | `region`, `accessKeyId`, `secretAccessKey`, optional `sessionToken`, `streamName`, `shardIndex`, `iteratorType`, `maxRecords` | Records unfolded from JSON payloads or fallback record metadata/data. | Single-shard read. No AWS SDK. |
| `src.pubsub` | GCP Pub/Sub REST runtime | `project`, `subscription`, `accessToken`, `maxMessages` | Pulled message rows. | Auto-acks pulled batch. Access token must be provided. |

## API and SaaS Sources

| Nodes | Runtime | Key props | Output | Restrictions / notes |
|---|---|---|---|---|
| `src.rest` | Rust HTTP runtime | `url`, `method`, `headers`, `body`, auth, `responsePath`, pagination props, `maxPages` | JSON rows from root or response path. | Supports cursor, offset, page, Link header, next URL. |
| `src.graphql`, `src.linear`, `src.monday` | REST runtime with GraphQL body | `url`/`endpoint`, `query`, `variables`, auth, `responsePath` | Rows under GraphQL response path, default `/data`. | Single POST request currently unless workflow handles pagination manually. |
| `src.soap` | REST runtime with XML parsing defaults | `url`, `body`, optional `soapAction`, `responsePath` | XML element rows converted to JSON-like rows. | Defaults POST and `Content-Type: text/xml`. |
| `src.odata` | REST runtime with OData defaults | `url`, auth, response/pagination props | Rows from `/value` by default. | Defaults next-url pagination at `/@odata.nextLink`. |
| SaaS REST aliases | REST runtime | Same as `src.rest`, often prefilled/expected for vendor API shape | Vendor API rows. | Vendor nodes share generic REST machinery; auth, pagination, and response paths are still user-configurable. |

SaaS aliases currently include Salesforce, HubSpot, Pipedrive, Zendesk, Intercom, Stripe, QuickBooks, Xero, Shopify, Notion, Airtable, Asana, Trello, ClickUp, GitHub, GitLab, Jira, Mailchimp, SendGrid, Segment, Slack, Discord, Telegram, and Twilio. Linear and Monday use the GraphQL branch.

## NoSQL and Search Sources

| Nodes | Runtime | Key props | Output | Restrictions / notes |
|---|---|---|---|---|
| `src.mongodb` | Rust MongoDB driver | `uri`, `database`, `collection`, optional `filter`, `projection`, `limit` | BSON documents flattened/materialized as rows. | Filter/projection are JSON/extended JSON strings. |
| `src.cassandra`, `src.scylla` | Rust Scylla/Cassandra driver | `contactPoints`, optional `user`, `password`, `keyspace`, `query` or `tableName` | CQL result rows. | Query required unless keyspace + tableName can form `SELECT *`. |
| `src.redis` | Rust Redis client | `url`/`connectionString`, `keyPattern`, `limit` | `{key, value}` rows. | SCAN + GET path. Complex Redis values may stringify poorly. |
| `src.dynamodb` | Direct AWS HTTP + SigV4 runtime | `region`, `accessKeyId`, `secretAccessKey`, optional `sessionToken`, `tableName`, `limitPerPage`, `maxPages` | DynamoDB items unwrapped from typed attribute shape. | Scan source, so use with caps for large tables. |
| `src.elastic`, `src.opensearch` | Rust HTTP runtime | `endpoint`, `index`, optional `apiKey`, `query`, `size`, `maxPages`, pagination mode | Search hit documents as rows. | Supports from/size and search-after paths in runtime. |
| `src.couchdb` | REST alias | URL/auth/response path | CouchDB document rows. | Rides generic REST source. |

## Other Sources

| Nodes | Runtime | Key props | Output | Restrictions / notes |
|---|---|---|---|---|
| `src.ftp` | Rust FTP/FTPS/SFTP runtime | `protocol`, `host`, `port`, `user`, `password`, optional SFTP key/fingerprint, `directory`, `pattern`, `maxFiles` | One row per file: filename, size, modified, content as base64. | SFTP requires user. FTP defaults anonymous. |
| `src.email` | Rust IMAP runtime | Runtime expects `host`, `user`, `password`, optional `port`, `mailbox`, `maxMessages` | Recent message rows: uid/from/to/subject/date/body text. | UI currently labels fields as username/folder; verify prop aliases before generating JSON. |
| `src.git` | System `git` CLI runtime | Runtime expects `repo`, `mode`, `revision`, optional `pathFilter`, `maxRows` | Commit log rows or file tree rows. | UI currently exposes URL/branch/path. Runtime reads a local clone, not a remote clone workflow. |
| `src.clipboard` | Rust `arboard` runtime | none | Clipboard text row, or one row per JSON array object. | Desktop/display-server only. Fails on headless Linux. |

## Vector Sources

| Nodes | Runtime | Key props | Output | Restrictions / notes |
|---|---|---|---|---|
| `src.pgvector` | DuckDB Postgres attach | Postgres connection + schema/table | Embedding table rows. | Server must have pgvector extension and readable vector columns. |
| `src.qdrant` | Rust HTTP runtime | `clusterUrl`, `collection`, optional `apiKey`, `pageSize`, `maxPages`, `withVector` | Point rows: id, payload fields, optional vector. | Scrolls points. Vectors disabled by default because they are large. |
| `src.weaviate` | Rust HTTP runtime | `endpoint`, `class`, optional `apiKey`, `pageSize`, `maxPages`, `withVector` | Object rows: id, properties, optional vector. | Cursor uses last object id. |
| `src.milvus` | Rust HTTP runtime | `endpoint`, `collection`, optional `apiKey`, `filter`, `outputFields`, `pageSize`, `maxPages` | Query result rows. | Default filter is `id > 0`. |

Preview vector sources in the palette are not documented here as reliable runtime contracts.

## Adding a New Source Node

Minimum implementation checklist:

1. Add the palette entry in `palette-data.ts`.
2. Add or route a form manifest in `manifest-synth.ts`.
3. Choose SQL builder or runtime spec.
4. If SQL-backed, add a `build_view_sql` branch in `builders.rs` and any extension/secret prelude required.
5. If runtime-backed, add a spec type in `plan/specs.rs`, planner extraction in `plan/mod.rs`, and executor handling in `lib.rs`.
6. Define output shape clearly enough for preview and downstream contracts.
7. Add tests for missing required props and the generated SQL/runtime spec path.
8. Update this doc and `00_node-inventory.md`.

For workflow-building agents, the most important practical rule is: build the graph from available nodes first, then inspect the planner branch when emitting raw workflow JSON. The UI form is a guide; the planner/runtime is the contract that actually runs.
