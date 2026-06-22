# Node Inventory

This note captures the node surface currently exposed by the Duckle fork as a baseline for Stitchly v2 planning.

Source of truth checked:

- `frontend/src/workflow-ui/palette-data.ts` for the visible palette.
- `crates/duckdb-engine/src/plan/builders.rs` and `crates/duckdb-engine/src/plan/mod.rs` for planner/runtime handling.

Current palette count:

| Status | Count |
|---|---:|
| Total | 348 |
| Available | 327 |
| Planned | 16 |
| Preview | 5 |

## Prefix Model

| Prefix | Type | Meaning |
|---|---|---|
| `src.*` | Source | Pull/read data into the graph. |
| `xf.*` | Transform | Shape, enrich, join, validate, or model data. |
| `snk.*` | Sink | Write/push data out. |
| `qa.*` | Quality | Validation, profiling, cleansing, and governance nodes. |
| `ctl.*` | Control | Branching, orchestration, retries, checkpoints, and logging. |
| `code.*` | Custom | SQL, JavaScript, shell, WASM, and planned UDF surfaces. |

## Sources

| Group | Available nodes |
|---|---|
| Files | `src.csv`, `src.tsv`, `src.json`, `src.jsonl`, `src.xml`, `src.excel`, `src.avro`, `src.parquet`, `src.fixedwidth`, `src.yaml`, `src.toml`, `src.spatial` |
| Lakehouse table formats | `src.iceberg`, `src.delta`, `src.ducklake`, `src.ducklake.changes`, `src.ducklake.diff` |
| Databases | `src.postgres`, `src.mysql`, `src.mariadb`, `src.sqlserver`, `src.oracle`, `src.sqlite`, `src.duckdb`, `src.clickhouse`, `src.cockroach`, `src.adbc` |
| Cloud Warehouses | `src.snowflake`, `src.bigquery`, `src.redshift`, `src.databricks`, `src.synapse`, `src.motherduck`, `src.quack` |
| Object Storage | `src.s3`, `src.gcs`, `src.azureblob`, `src.minio`, `src.r2`, `src.b2` |
| Streaming | `src.kafka`, `src.redpanda`, `src.nats`, `src.rabbit`, `src.kinesis`, `src.pubsub` |
| APIs | `src.rest`, `src.graphql`, `src.webhook`, `src.soap`, `src.odata` |
| NoSQL and Search | `src.mongodb`, `src.cassandra`, `src.scylla`, `src.redis`, `src.dynamodb`, `src.elastic`, `src.opensearch`, `src.couchdb` |
| Other | `src.ftp`, `src.http`, `src.email`, `src.git`, `src.clipboard` |
| Vector / AI Databases | `src.pgvector`, `src.qdrant`, `src.weaviate`, `src.milvus` |

### SaaS Sources

Most SaaS sources are wrappers around the generic REST or GraphQL source infrastructure.

| Group | Available nodes |
|---|---|
| CRM | `src.salesforce`, `src.hubspot`, `src.pipedrive`, `src.zendesk`, `src.intercom` |
| Finance | `src.stripe`, `src.quickbooks`, `src.xero`, `src.shopify` |
| Productivity | `src.notion`, `src.airtable`, `src.asana`, `src.trello`, `src.clickup`, `src.monday` |
| Dev Tools | `src.github`, `src.gitlab`, `src.linear`, `src.jira` |
| Marketing | `src.mailchimp`, `src.sendgrid`, `src.segment` |
| Communication | `src.slack`, `src.discord`, `src.telegram`, `src.twilio` |

## Transforms

| Group | Available nodes |
|---|---|
| Fields | `xf.map`, `xf.project`, `xf.cast`, `xf.rename`, `xf.addcol`, `xf.dropcol`, `xf.reorder`, `xf.coalesce`, `xf.uuid`, `xf.surrogatekey`, `xf.compare` |
| Rows | `xf.filter`, `xf.distinct`, `xf.sample`, `xf.topn`, `xf.sort`, `xf.skip`, `xf.rank.filter`, `xf.fill_forward`, `xf.fill_backward`, `xf.fill_constant` |
| Aggregate | `xf.groupby`, `xf.rollup`, `xf.cube`, `xf.aggwin`, `xf.cumulative`, `xf.count`, `xf.approx.quantile` |
| Join | `xf.join.inner`, `xf.join.left`, `xf.join.right`, `xf.join.full`, `xf.join.cross`, `xf.join.spatial`, `xf.lookup`, `xf.semi`, `xf.anti` |
| Set Operations | `xf.union`, `xf.unionall`, `xf.intersect`, `xf.except` |
| Window | `xf.rownum`, `xf.rank`, `xf.denserank`, `xf.lead`, `xf.lag`, `xf.first`, `xf.last`, `xf.ntile`, `xf.sessionize` |
| Strings | `xf.regex`, `xf.regex.extract`, `xf.regex.match`, `xf.url.parse`, `xf.text.similarity`, `xf.text.base64`, `xf.text.padding`, `xf.text.match`, `xf.text.reverse`, `xf.text.repeat`, `xf.text.replace`, `xf.text.slug`, `xf.text.strip_html`, `xf.split`, `xf.concat`, `xf.trim`, `xf.case`, `xf.length`, `xf.substring`, `xf.format`, `xf.hash`, `xf.ip.parse` |
| Date / Time | `xf.dt.parse`, `xf.dt.format`, `xf.dt.extract`, `xf.dt.diff`, `xf.dt.add`, `xf.dt.trunc`, `xf.dt.tz`, `xf.dt.bin`, `xf.dt.now`, `xf.dt.epoch` |
| Numeric | `xf.num.round`, `xf.num.mod`, `xf.num.abs`, `xf.num.log`, `xf.num.power`, `xf.num.sqrt`, `xf.num.bucketize`, `xf.num.zscore`, `xf.num.clamp`, `xf.num.sign` |
| Pivot / Shape | `xf.pivot`, `xf.unpivot`, `xf.denorm`, `xf.norm`, `xf.transpose` |
| JSON / Nested | `xf.json.parse`, `xf.json.stringify`, `xf.json.flatten`, `xf.json.path`, `xf.json.merge`, `xf.json.array_agg` |
| Array | `xf.arr.explode`, `xf.arr.collect`, `xf.arr.element`, `xf.arr.contains`, `xf.arr.distinct`, `xf.arr.length`, `xf.zip` |
| CDC / SCD | `xf.incremental`, `xf.cdc.diff`, `xf.diffsummary`, `xf.cdc.scd1`, `xf.cdc.scd2`, `xf.cdc.scd3`, `xf.cdc.upsert`, `xf.row_hash`, `xf.audit` |
| AI | `xf.ai.embed`, `xf.ai.llm`, `xf.ai.chunk`, `xf.ai.pii`, `xf.ai.classify`, `xf.ai.dedupe`, `xf.ai.vector_search`, `xf.ai.text_search` |
| Geospatial | `xf.geo.distance`, `xf.geo.buffer`, `xf.geo.intersects` |
| Debug | `xf.log`, `xf.assert` |

## Sinks

| Group | Available nodes |
|---|---|
| Files | `snk.csv`, `snk.tsv`, `snk.json`, `snk.jsonl`, `snk.xml`, `snk.excel`, `snk.parquet`, `snk.avro`, `snk.yaml`, `snk.toml`, `snk.spatial`, `snk.ftp` |
| Lakehouse table formats | `snk.iceberg`, `snk.ducklake` |
| Databases | `snk.postgres`, `snk.cockroach`, `snk.mysql`, `snk.mariadb`, `snk.sqlserver`, `snk.oracle`, `snk.sqlite`, `snk.duckdb`, `snk.clickhouse` |
| Cloud Warehouses | `snk.motherduck`, `snk.quack`, `snk.snowflake`, `snk.bigquery`, `snk.redshift`, `snk.databricks`, `snk.synapse` |
| Object Storage | `snk.s3`, `snk.gcs`, `snk.azureblob` |
| Streaming | `snk.kafka`, `snk.redpanda`, `snk.nats`, `snk.rabbit`, `snk.pubsub` |
| APIs | `snk.rest`, `snk.webhook`, `snk.graphql`, `snk.email` |
| NoSQL and Search | `snk.mongodb`, `snk.cassandra`, `snk.scylla`, `snk.redis`, `snk.elastic`, `snk.opensearch` |
| Vector / AI Databases | `snk.pgvector`, `snk.pinecone`, `snk.qdrant`, `snk.weaviate`, `snk.milvus` |

## Control Flow

| Group | Available nodes |
|---|---|
| Routing | `ctl.replicate`, `ctl.switch`, `ctl.merge`, `ctl.iterate`, `ctl.foreach` |
| Timing | `ctl.wait`, `ctl.throttle` |
| Pipelines | `ctl.runpipeline`, `ctl.trigger`, `ctl.runjob`, `ctl.parallelize`, `ctl.checkpoint` |
| Error Handling | `ctl.try`, `ctl.retry`, `ctl.deadletter` |
| Logging and Alerts | `ctl.log`, `ctl.warn`, `ctl.die` |

## Data Quality

| Group | Available nodes |
|---|---|
| Validation | `qa.schemavalidate`, `qa.regex`, `qa.range`, `qa.notnull`, `qa.unique`, `qa.outlier` |
| Profiling | `qa.profile`, `qa.profile.adv`, `qa.describe`, `qa.histogram` |
| Cleansing | `qa.standardize`, `qa.mask`, `qa.dedupe`, `qa.match`, `qa.survivor`, `qa.matchgroup`, `qa.expect`, `qa.contract`, `qa.freshness`, `qa.sample.adv`, `qa.refintegrity`, `qa.link`, `qa.reconcile`, `qa.classify` |

## Custom Code

| Group | Available nodes |
|---|---|
| SQL | `code.sql`, `code.sqltemplate` |
| Scripting | `code.javascript`, `code.shell`, `code.wasm` |
| dbt | `xf.dbt` |

## Planned and Preview Nodes

Treat these as not ready for core Stitchly v2 workflows until verified in the runtime.

| Status | Nodes |
|---|---|
| Planned | `src.orc`, `src.db2`, `src.jdbc`, `src.pulsar`, `src.eventhubs`, `src.grpc`, `src.gsheets`, `src.excel-online`, `snk.orc`, `snk.jdbc`, `snk.pulsar`, `snk.kinesis`, `ctl.schedule`, `qa.addressclean`, `code.python`, `code.rust` |
| Preview | `src.pinecone`, `src.chroma`, `src.lancedb`, `snk.chroma`, `snk.lancedb` |

## Notes for Stitchly v2 Planning

- Many SaaS nodes are thin wrappers over `src.rest` or `src.graphql`, so improving generic API auth, pagination, schema preview, and secrets handling will improve many connectors at once.
- `code.shell` is immediately useful for migration/bootstrap work such as Dolt setup, local CLI orchestration, dbt commands, or project scaffolding.
- DuckDB-native transforms and sinks should be preferred for repeatable local-studio workflows because they fit the current CLI execution model.
- Planned/preview nodes should not be used as foundation dependencies until we test their planner/runtime path.
