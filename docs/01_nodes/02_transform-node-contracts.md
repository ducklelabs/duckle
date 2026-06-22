# Transform Node Contracts

This note documents `xf.*` transform behavior at the contract level. It is intended for agents that need to build workflows or add/modify transform nodes.

Authoritative files:

- Palette entries: `frontend/src/workflow-ui/palette-data.ts`
- Form contracts: `frontend/src/workflow-ui/fields/manifest-synth.ts`
- SQL builders: `crates/duckdb-engine/src/plan/builders.rs`
- Runtime-backed transform specs: `crates/duckdb-engine/src/plan/mod.rs`
- Schema propagation and column validation: `crates/duckdb-engine/src/plan/graph.rs`
- Runtime execution: `crates/duckdb-engine/src/lib.rs`

## Common Transform Contract

Transform nodes usually consume one upstream relation from the `main` port and produce one downstream relation.

| Aspect | Contract |
|---|---|
| Main input | Required for almost all transforms. |
| Secondary inputs | Joins, lookups, reconciliation, SCD, mapping, and some quality nodes use a lookup/reference input. |
| Main output | A DuckDB table or view named from the transform node id. |
| Reject output | Some validators and join options can route rejected/unmatched rows to `<node>__reject`. Most transforms do not produce rejects. |
| Schema | Pass-through transforms preserve columns. Shape-changing transforms may return unknown schema to avoid invalid column validation. |
| Runtime | Most `xf.*` nodes compile to DuckDB SQL. AI, dbt, and some custom execution paths use runtime specs. |

## Runtime Classes

| Runtime class | Nodes | Processing model |
|---|---|---|
| Pure SQL transforms | Most fields, rows, aggregate, join, set, window, string, datetime, numeric, pivot, JSON, array, CDC, geo, and debug transforms | Planner emits a DuckDB `SELECT` or `COPY`-adjacent SQL fragment. Fastest and easiest to inspect. |
| Extension-backed SQL | Vector search, full-text search, geospatial, IP parsing, spatial join | Uses DuckDB extensions or special PRAGMAs. Requires the extension/preamble path to be correct. |
| Runtime-backed local transforms | `xf.ai.chunk`, `xf.ai.pii`, `xf.ai.dedupe` | Runtime reads upstream rows, processes locally, then materializes output. |
| Runtime-backed API transforms | `xf.ai.embed`, `xf.ai.llm`, `xf.ai.classify` | Runtime calls an OpenAI-compatible API per batch or row, then materializes output. |
| dbt transform | `xf.dbt` | Runtime invokes dbt against the run DuckDB database and optionally reads a built model back as output. |

## Schema Propagation

The planner has lightweight schema propagation in `plan/graph.rs`. This is used for early column-reference errors.

| Schema behavior | Nodes |
|---|---|
| Exact pass-through | `xf.filter`, `xf.distinct`, `xf.sort`, `xf.limit`, `xf.topn`, `xf.sample`, `xf.skip`, `xf.log`, `xf.fill_forward`, `xf.fill_backward`, `xf.fill_constant`, `xf.cast`, `xf.rank.filter` |
| Schema can be derived | `xf.drop`, `xf.rename`, `xf.project` |
| Schema intentionally unknown | Column-adding transforms such as `xf.uuid`, `xf.audit`, `xf.row_hash`, window functions, aggregates, joins, custom SQL, pivots, and many reshaping nodes |

Agent rule: if a downstream node references a column added by an upstream transform, expect planner validation to be disabled or weaker. Do not assume a column is invalid just because the schema panel cannot infer it.

## Fields

| Nodes | Input | Output shape | Notes |
|---|---|---|---|
| `xf.map` | Main plus optional lookup/reference | User-mapped output columns | Visual mapper. Treat as schema-changing unless declared schema exists. |
| `xf.project` | Main | Selected columns only | Good for narrowing before expensive joins/API calls. |
| `xf.cast` | Main | Same columns, selected column retyped | `onError` can null, reject, or fail depending builder support. |
| `xf.rename` | Main | Same data with renamed columns | Supports inline mapping and mapping-file forms. |
| `xf.addcol`, `xf.coalesce` | Main | Upstream plus one expression column | Expression is DuckDB SQL. |
| `xf.dropcol`, `xf.reorder` | Main | Same or fewer columns | Drop/reorder are schema-derived where possible. |
| `xf.uuid` | Main | Upstream plus UUID column | Random per row, not stable across runs. |
| `xf.surrogatekey` | Main | Upstream plus deterministic hash or sequence key | Use hash mode for stable dimensional keys. |
| `xf.compare` | Main | Upstream plus boolean comparison column | Compares two row columns with basic operators. |

## Rows

| Nodes | Input | Output shape | Notes |
|---|---|---|---|
| `xf.filter` | Main | Same columns, fewer rows | Predicate is DuckDB SQL. |
| `xf.distinct` | Main | Same columns, deduped rows | Can dedupe whole row or selected columns. |
| `xf.sample` | Main | Same columns, sampled rows | Useful before LLM/API transforms. |
| `xf.topn`, `xf.skip` | Main | Same columns, subset rows | Prefer explicit sort upstream if row order matters. |
| `xf.sort` | Main | Same columns, ordered rows | Ordering can be lost after later set/aggregate transforms. |
| `xf.rank.filter` | Main | Same columns, top N per partition | Uses row_number-style window and filter. |
| `xf.fill_forward`, `xf.fill_backward`, `xf.fill_constant` | Main | Same columns, nulls filled | Forward/backward fill need an `orderBy` column. |

## Aggregate

| Nodes | Input | Output shape | Notes |
|---|---|---|---|
| `xf.groupby` | Main | Group keys plus aggregate columns | Standard aggregate. |
| `xf.rollup`, `xf.cube` | Main | Grouping-set aggregate output | Useful for subtotal/grand-total reporting. |
| `xf.aggwin` | Main | Upstream plus window aggregate column | Keeps every row. |
| `xf.cumulative` | Main | Upstream plus running aggregate column | Needs `orderBy`; can partition. |
| `xf.count` | Main | Single count row | Useful for diagnostics or quality reporting. |
| `xf.approx.quantile` | Main | Quantile per full input or group | Uses approximate quantile. Good for p50/p95/p99. |

## Joins and Set Operations

| Nodes | Input | Output shape | Notes |
|---|---|---|---|
| `xf.join.inner`, `xf.join.left`, `xf.join.right`, `xf.join.full` | Main plus lookup/reference | Joined columns | Key props are left/right keys. Multi-key forms exist. |
| `xf.join.cross` | Main plus lookup/reference | Cartesian product | Use cautiously. Can explode row counts. |
| `xf.join.spatial` | Two inputs with geometry columns | Spatially joined rows | Uses spatial predicates such as intersects/contains/within. |
| `xf.lookup` | Main plus lookup/reference | Left-join shape | Alias for lookup-style enrichment. |
| `xf.semi` | Main plus lookup/reference | Main rows with a match | Existence filter. |
| `xf.anti` | Main plus lookup/reference | Main rows without a match | Useful for orphan/missing-record checks. |
| `xf.union`, `xf.unionall` | Multiple inputs | Combined rows | `union` dedupes, `unionall` preserves duplicates. |
| `xf.intersect`, `xf.except` | Multiple inputs | Set comparison output | Inputs must be shape-compatible. |

Agent rule: joins are the common point where workflow shape matters. Make sure ports are correct before debugging SQL.

## Window

| Nodes | Input | Output shape | Notes |
|---|---|---|---|
| `xf.rownum` | Main | Upstream plus row number | Requires/order benefits from `orderBy`. |
| `xf.rank`, `xf.denserank` | Main | Upstream plus rank column | Partition/order driven. |
| `xf.lead`, `xf.lag`, `xf.first`, `xf.last` | Main | Upstream plus derived value column | Operates over ordered windows. |
| `xf.ntile` | Main | Upstream plus bucket number | Splits ordered partitions into N buckets. |
| `xf.sessionize` | Main | Upstream plus session fields | Requires event order column and inactivity gap. |

## Strings, Dates, and Numbers

| Family | Nodes | Output shape | Notes |
|---|---|---|---|
| Regex/string basics | `xf.regex`, `xf.regex.extract`, `xf.regex.match`, `xf.trim`, `xf.case`, `xf.length`, `xf.substring`, `xf.concat`, `xf.split`, `xf.format` | Usually upstream plus/replaced output column | DuckDB SQL string expressions. |
| Text utilities | `xf.text.similarity`, `xf.text.base64`, `xf.text.padding`, `xf.text.match`, `xf.text.reverse`, `xf.text.repeat`, `xf.text.replace`, `xf.text.slug`, `xf.text.strip_html` | Usually upstream plus/replaced output column | Local SQL/string processing. |
| URL/IP/hash | `xf.url.parse`, `xf.ip.parse`, `xf.hash` | Upstream plus parsed/hash columns | IP parsing may require extension prelude. |
| Date/time | `xf.dt.parse`, `xf.dt.format`, `xf.dt.extract`, `xf.dt.diff`, `xf.dt.add`, `xf.dt.trunc`, `xf.dt.tz`, `xf.dt.bin`, `xf.dt.now`, `xf.dt.epoch` | Upstream plus/replaced date column | Uses DuckDB date/time functions. |
| Numeric | `xf.num.round`, `xf.num.mod`, `xf.num.abs`, `xf.num.log`, `xf.num.power`, `xf.num.sqrt`, `xf.num.bucketize`, `xf.num.zscore`, `xf.num.clamp`, `xf.num.sign` | Upstream plus/replaced numeric output | Good for standard feature prep and outlier handling. |

## Pivot, JSON, and Array

| Nodes | Input | Output shape | Notes |
|---|---|---|---|
| `xf.pivot` | Main | Wide table | Values from `pivotColumn` become columns. |
| `xf.unpivot` | Main | Long table | Selected columns become name/value rows. |
| `xf.denorm` | Main | One row per group with aggregated delimited cells | Useful for report-shaped outputs. |
| `xf.norm` | Main | More rows after splitting/exploding one column | Opposite of denormalize. |
| `xf.transpose` | Main | Rows/columns swapped | Requires compatible column types. |
| `xf.json.parse`, `xf.json.stringify`, `xf.json.path` | Main | Upstream plus/replaced JSON-derived column | JSONPath/JSON conversion. |
| `xf.json.flatten` | Main | Struct fields become top-level columns | Schema-changing. |
| `xf.json.merge`, `xf.json.array_agg` | Main | Merged object or collected JSON array | `array_agg` can collapse groups. |
| `xf.arr.explode`, `xf.arr.collect`, `xf.arr.element`, `xf.arr.contains`, `xf.arr.distinct`, `xf.arr.length`, `xf.zip` | Main | Array scalar, exploded rows, collected rows, or zipped table | `explode` and `zip` are major shape changes. |

## CDC and SCD

| Nodes | Input | Output shape | Notes |
|---|---|---|---|
| `xf.incremental` | Main | Rows past persisted high-water mark | Persists watermark after successful run. Use monotonic timestamp/id. |
| `xf.cdc.diff` | Current plus previous/reference | Rows tagged inserted/updated/deleted | Compare by natural key and compare columns. |
| `xf.diffsummary` | Change feed | One summary row | Expects change-type column, default `change_type`. |
| `xf.cdc.scd1` | Current plus previous/reference | Current state rows | Resolves current snapshot. |
| `xf.cdc.scd2` | Current plus previous/reference | Versioned rows with validity/current flags | For history-preserving dimensions. |
| `xf.cdc.scd3` | Current plus previous/reference | Current fields plus previous value columns | For limited history columns. |
| `xf.cdc.upsert` | Current plus previous/reference | New/changed upsert payload | Feed into relational/vector/noSQL sinks with upsert modes. |
| `xf.row_hash` | Main | Upstream plus stable fingerprint | Useful before diff/upsert. |
| `xf.audit` | Main | Upstream plus audit columns | Adds loaded/source/batch provenance. |

## AI

| Nodes | Runtime | Input | Output shape | Notes |
|---|---|---|---|---|
| `xf.ai.chunk` | Local runtime | Text column | Exploded chunk rows or array column | No API call. Good before embeddings. |
| `xf.ai.pii` | Local regex runtime | Text column | Redacted text column | No API call. Regex-based. |
| `xf.ai.dedupe` | Local runtime | Embedding column | Fewer rows after similarity dedupe | O(N^2). Use after sampling/filtering for large inputs. |
| `xf.ai.embed` | OpenAI-compatible API runtime | Text column | Embedding column | Requires `apiKey`; supports `baseUrl`, `model`, `batchSize`. |
| `xf.ai.llm` | OpenAI-compatible API runtime | Rows/text columns | Completion/output column | One call per row. Sample first. |
| `xf.ai.classify` | OpenAI-compatible API runtime | Text column | Category column | Requires non-empty category list and `apiKey`. |
| `xf.ai.vector_search` | DuckDB vss SQL | Embedding/vector rows | Ranked similarity output | Requires vector-search prelude/extension. |
| `xf.ai.text_search` | DuckDB fts runtime path | Text columns | Ranked keyword-search output | Uses two-step executor path because FTS PRAGMA cannot always see same-invocation tables. |

## Geospatial and Debug

| Nodes | Input | Output shape | Notes |
|---|---|---|---|
| `xf.geo.distance` | Main | Upstream plus distance column | Uses spatial functions. |
| `xf.geo.buffer` | Main | Upstream plus buffered geometry | Uses spatial functions. |
| `xf.geo.intersects` | Main | Upstream plus boolean column | Uses spatial functions. |
| `xf.log` | Main | Same rows | Pass-through for preview/output inspection. |
| `xf.assert` | Main | Same rows or failure | Fails pipeline if predicate violation is found. |

## dbt Transform

`xf.dbt` lives in the Custom Code palette group but uses an `xf.*` id.

| Aspect | Contract |
|---|---|
| Input | Optional upstream views. Inline models can reference the upstream via generated dbt context. |
| Props | `projectDir` or inline `model` required. Optional `command`, `outputModel`, `dbtBin`, `database`, `schema`, `timeoutMs`. |
| Processing | Runtime generates/uses dbt project/profile against the run DuckDB database and invokes dbt. |
| Output | If `outputModel` is set, downstream reads the built dbt model. Otherwise this is mainly a side-effect/build node. |
| Restrictions | Requires dbt setup to be available through the app-managed or configured dbt binary. |

## Agent Rules

- Prefer SQL transforms for deterministic local workflows.
- Use `xf.sample`, `xf.topn`, or `xf.filter` before API-backed AI transforms.
- Treat joins, pivots, JSON flattening, array exploding, aggregates, dbt, and custom SQL as schema-changing.
- For raw workflow JSON, verify exact prop names in builders/runtime specs. The form manifest is helpful but not always the final runtime contract.
- Use `xf.row_hash`, `xf.audit`, and `xf.incremental` as building blocks for migration, CDC, and repeatable local-studio workflows.
- Use `xf.log`, `xf.count`, `xf.diffsummary`, and `xf.assert` to make debugging and CI checks explicit.

## Adding a New Transform Node

Minimum implementation checklist:

1. Add the palette entry in `palette-data.ts`.
2. Add or route the form manifest in `manifest-synth.ts`.
3. Decide whether it is pure SQL, extension-backed SQL, or runtime-backed.
4. For SQL transforms, add a `build_view_sql` branch in `builders.rs`.
5. For runtime transforms, add a spec type in `plan/specs.rs`, planner extraction in `plan/mod.rs`, and executor handling in `lib.rs`.
6. Add schema propagation in `plan/graph.rs` only when exact and safe. Return unknown rather than wrong.
7. Add column-reference validation for props that name upstream columns.
8. Add planner/runtime tests for missing input, missing required props, generated SQL/spec, and representative output shape.
9. Update this doc and `00_node-inventory.md`.
