# Control and Code Node Contracts

This note documents `ctl.*` and `code.*` node behavior at the contract level. It is intended for agents that need to build orchestration/debug workflows or add/modify control/custom-code nodes.

Authoritative files:

- Palette entries: `frontend/src/workflow-ui/palette-data.ts`
- Form contracts: `frontend/src/workflow-ui/fields/manifest-synth.ts`
- Control/custom SQL builders: `crates/duckdb-engine/src/plan/builders.rs`
- Runtime-backed specs: `crates/duckdb-engine/src/plan/mod.rs`
- Runtime execution: `crates/duckdb-engine/src/lib.rs`

## Common Control Contract

Control nodes mostly pass rows through while adding orchestration side effects.

| Aspect | Contract |
|---|---|
| Input | Most consume `main`; some can run as orchestration placeholders without upstream input. |
| Output | Usually pass-through rows or placeholder rows. |
| Side effects | Child pipeline execution, logging, failure, delay, checkpoint files, dead-letter writes, branch execution. |
| Runtime | Mix of SQL passthrough, planner graph behavior, and executor runtime specs. |
| Composition | Current child pipeline/job nodes are side-effect oriented; child output is not composed into the parent graph. |

## Routing Control

| Nodes | Input | Output / side effect | Notes |
|---|---|---|---|
| `ctl.replicate` | Main | Same rows | Logical tee. Multiple downstream edges can read the same materialized output. |
| `ctl.switch` | Main | One table per branch/default | Routes rows to case outputs by condition. No normal main output table. |
| `ctl.merge` | Multiple inputs | Union-all combined rows | Concatenates streams. Inputs should be shape-compatible. |
| `ctl.iterate` | Optional main | Runs child pipeline N times, passes through/placeholder | Substitutes `${ITER_INDEX}` in child. Side-effect model. |
| `ctl.foreach` | Main | Runs child once per upstream row, passes through | Substitutes `${ITER_INDEX}` and `${ITER_ITEM_<FIELD>}`. Optional concurrency. |

## Timing and Pipeline Control

| Nodes | Input | Output / side effect | Notes |
|---|---|---|---|
| `ctl.wait` | Main | Same rows after delay | Delay based on duration/unit. |
| `ctl.throttle` | Main | Same rows with delay hook | Batch-oriented best-effort delay from rows/sec. |
| `ctl.schedule` | Main | Planned | Palette entry exists, but do not rely on it as runtime-supported. |
| `ctl.runpipeline`, `ctl.trigger` | Optional main | Runs referenced pipeline, then passes through/placeholder | Side-effect model; child output is not composed back. |
| `ctl.runjob` | Optional main | Runs child job/pipeline with context variables | Good for master-job orchestration. |
| `ctl.parallelize` | Main | Snapshots upstream, runs independent branches concurrently | Branches are isolated sub-pipelines and joined after completion. |
| `ctl.checkpoint` | Main | Same rows plus parquet sidecar | Writes durable parquet snapshot to `storage`/`path`. |

## Error and Logging Control

| Nodes | Input | Output / side effect | Notes |
|---|---|---|---|
| `ctl.try` | Optional main | Installs fallback pipeline, passes through/placeholder | If a later stage fails, fallback runs as side effect before original error surfaces. Not full continuation-style try/catch. |
| `ctl.retry` | Main | Same rows | Node exists, but practical retry is also implemented as advanced per-stage `retryAttempts`/`retryBackoffMs`. Use advanced retry for most cases. |
| `ctl.deadletter` | Main/reject branch | Writes rows to JSON/CSV/Parquet | Terminal sink for rejected rows. |
| `ctl.log` | Optional main | Logs info message, passes through/placeholder | `{rows}` expands to upstream row count. Written to run log. |
| `ctl.warn` | Optional main | Logs warning, passes through/placeholder | Same as log with warn level. |
| `ctl.die` | Optional main | Fails run when condition matches | Conditions include always, input has rows, input is empty. Useful after reject/count branches. |

## Custom Code Nodes

| Nodes | Runtime | Input | Output | Notes |
|---|---|---|---|---|
| `code.sql` | DuckDB SQL | Optional main as `input` | SQL query result | Best custom escape hatch. Can start a graph with a literal `SELECT`. |
| `code.sqltemplate` | DuckDB SQL with substitution | Optional main as `input` plus context variables | SQL query result | Use for parameterized routines. Can start a graph with a literal `SELECT`. |
| `code.javascript` | Rust Boa runtime | Main rows | Per-row transformed objects | Script must define `transform(row)` and return an object. No fetch/fs/DOM. |
| `code.shell` | System shell runtime | Optional main | Single row `{stdout, stderr, exit_code, duration_ms}` plus input metadata when connected | Runs arbitrary command. Defaults to platform shell. Optional working dir and timeout. When connected, upstream rows are materialized to JSONL and exposed to the shell. |
| `code.wasm` | Rust wasmi runtime | Main rows | Per-row transformed output column | Module must export `memory` and transform function contract. |
| `code.python`, `code.rust` | Planned | N/A | N/A | Palette entries exist but should not be used as runtime dependencies. |

## dbt Node

`xf.dbt` is documented in transform contracts, but it lives in the Custom Code palette group.

| Aspect | Contract |
|---|---|
| Runtime | Invokes dbt against the run DuckDB database. |
| Props | `projectDir` or inline `model` required. Optional `command`, `outputModel`, `dbtBin`, `database`, `schema`, `timeoutMs`. |
| Output | Reads `outputModel` back when set; otherwise mainly side-effect/build behavior. |
| Good for | Modeling layers, SQL projects, reusable transformations. |

## Useful Agent Patterns

| Pattern | Suggested graph |
|---|---|
| Debug row count | Any branch -> `xf.count` -> `ctl.log` |
| Guard reject branch | Validator reject -> `xf.count` -> `ctl.die` with `has-rows` |
| Dead-letter rejects | Validator reject -> `ctl.deadletter` |
| Run bootstrap/setup with config | `code.sql config -> code.shell -> downstream parse/assert/log` |
| Master job | `ctl.runjob` -> `ctl.runjob` -> `ctl.runjob` |
| Per-item orchestration | Source rows -> `ctl.foreach` child pipeline |
| Durable checkpoint | Any branch -> `ctl.checkpoint` -> continue |
| Parallel independent branches | Shared upstream -> `ctl.parallelize` -> branch outputs |

## Runtime and Safety Notes

- `code.shell` is powerful and should be treated as an escape hatch. It can mutate the local machine or call external tools.
- When `code.shell` has an upstream main input, the runtime sets `DUCKLE_INPUT_PATH`, `DUCKLE_INPUT_FORMAT=jsonl`, `DUCKLE_INPUT_TABLE`, `DUCKLE_INPUT_ROW_COUNT`, and `DUCKLE_DUCKDB_DATABASE`.
- Treat `code.shell` upstream handoff as a control/config interface, not a bulk data transport. For large data movement, use DuckDB SQL, parquet sinks, or connector nodes.
- `ctl.runpipeline`, `ctl.runjob`, `ctl.iterate`, and `ctl.foreach` are side-effect oriented. Do not expect child outputs to appear as parent rows.
- `ctl.try` runs fallback on failure but does not resume the failed branch as a full block-scoped try/catch.
- `ctl.parallelize` snapshots upstream once and runs isolated branch sub-pipelines. Shared external side effects still need careful design.
- `ctl.deadletter` is a sink-like node. It should normally terminate a reject branch.
- Advanced per-stage settings such as retry attempts, retry backoff, and memory limit live on all components, not only control nodes.

## Agent Rules

- Use control nodes to make workflow intent explicit: logging, gating, dead-lettering, orchestration, checkpointing.
- Prefer `code.sql` before `code.shell` when the work is data-local and can run inside DuckDB.
- Prefer built-in source/sink nodes before shelling out to external CLIs.
- Use `code.sql` to create literal config rows before `code.shell` when a CLI step needs workflow-local parameters.
- Use `code.shell` for bootstrap/migration tasks that are inherently CLI-driven, such as Dolt setup or local project scaffolding.
- Keep child-pipeline orchestration separate from data-transform composition until the DAG/block model changes.
- Verify exact prop names in `plan/mod.rs` before generating raw workflow JSON.

## Adding a New Control or Code Node

Minimum implementation checklist:

1. Add the palette entry in `palette-data.ts`.
2. Add or route the form manifest in `manifest-synth.ts`.
3. Decide whether the node is SQL passthrough, graph-compiler behavior, or runtime spec behavior.
4. For SQL passthrough/control, add builder support in `builders.rs` or `plan/mod.rs`.
5. For runtime behavior, add a spec type in `plan/specs.rs`, planner extraction in `plan/mod.rs`, and executor handling in `lib.rs`.
6. Define whether the node preserves rows, emits placeholder rows, creates branch outputs, or terminates a branch.
7. Add tests for missing props, missing input when required, and side-effect spec creation.
8. Update this doc and `00_node-inventory.md`.
