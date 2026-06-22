# Stitchly v2 Foundation Notes

These notes capture the first walk through the Duckle fork as the starting
point for Stitchly v2. Keep this file practical: enough context to help us make
changes confidently, without turning it into public-facing documentation too
early.

## What This Repo Is

Duckle is a local-first desktop ETL / ELT studio. In this fork, it gives us a
working base for Stitchly v2:

- A React + Vite visual pipeline designer.
- A Tauri desktop shell for local filesystem/process access.
- A Rust engine that compiles visual graphs into DuckDB SQL and runtime stages.
- Workspace persistence as plain files on disk.
- Local engine installation, scheduler, Git integration, MCP server, and a
  headless runner for server-style execution.

The important framing: this is not just a UI prototype. There is already a
substantial execution path from canvas to DuckDB-backed pipeline runs.

## Repo Map

- `frontend/`
  React 19 + Vite + TypeScript app. This is the visual canvas, palette,
  properties panel, workspace browser, chat panel, run views, and modals.

- `apps/desktop/`
  Tauri 2 desktop shell. It exposes commands to the frontend for running
  pipelines, compiling SQL, scheduling, Git actions, secrets, settings, MCP,
  update checks, engine installation, and local AI chat.

- `crates/duckdb-engine/`
  The main execution brain. It compiles the node graph into ordered stages,
  generates SQL, handles runtime connector hooks, shells out to the downloaded
  DuckDB CLI, streams run events, records history, manages watermarks, and
  supports pipeline lineage/context resolution.

- `crates/duckle-runner/`
  Headless pipeline runner. It can run a pipeline JSON directly and also acts
  as the embedded stub for "Build Pipeline" single-file artifacts.

- `crates/duckle-mcp/`
  MCP server over stdio. It exposes tools/resources so an LLM client can browse
  the catalog, validate/run/build pipelines, and work with saved connections.

- `crates/scheduler/`
  Cron, interval, and file-watch schedules persisted in the workspace.

- `crates/metadata/`
  Shared Rust model for schemas, nodes, edges, and pipeline documents.

- `crates/plugin-sdk/` and `crates/connectors/`
  Early connector abstraction. At the moment, most real execution still lives in
  `duckdb-engine`; these crates are not yet the whole plugin system.

- `crates/runtime`, `workflow-engine`, `transform-engine`, `stream-engine`,
  `execution-core`, `slothdb-engine`
  Mostly placeholder or early abstraction crates. Do not assume the crate names
  reflect fully separated production responsibilities yet.

## Runtime Mental Model

The frontend stores React Flow nodes and edges. Each node has a `componentId`
such as `src.csv`, `xf.filter`, or `snk.parquet`.

The normal run path is:

1. The React app calls a Tauri command through `frontend/src/tauri-bridge.ts`.
2. `apps/desktop/src/lib.rs` receives the command, resolves the DuckDB binary,
   and calls into `duckle-duckdb-engine`.
3. `crates/duckdb-engine/src/plan/mod.rs` topologically sorts the graph and
   compiles nodes into ordered `Stage`s.
4. Pure SQL stages run through DuckDB CLI. Non-SQL work is represented by
   `RuntimeSpec` variants and handled by Rust code around the SQL execution.
5. Run events stream back to the frontend so the canvas and bottom panel can
   update live.

Pure SQL pipelines can be batched into a single DuckDB CLI invocation for
speed. Stages with driver connectors, retries, waits, control flow, special
runtime hooks, or local file overwrite checks fall back to per-stage execution.

## Workspace Model

The app is local-first. A workspace is a folder chosen by the user.

Current v2 workspace layout:

- `duckle.json`: workspace metadata.
- `repository.json`: project tree metadata.
- `pipelines/`: individual pipeline JSON files.
- `connections/`: saved connection payloads.
- `contexts/`: context/environment JSON.
- `routines/`: reusable SQL/custom routines.
- `docs/`: workspace notes.
- `schedules.json`: scheduler config.
- `run-history/`: per-pipeline run history.
- `.duckle/keys/`: local encryption key material for secrets.

Frontend workspace code lives in `frontend/src/workspace.ts`.

## Build And Run

Local prerequisites:

- Rust toolchain. The repo pins one in `rust-toolchain.toml`.
- Node.js and npm.
- Tauri 2 CLI: `cargo install tauri-cli --version "^2"`.
- OS-specific Tauri webview dependencies.

The local checkout currently has no `frontend/node_modules`, so first install:

```bash
npm --prefix frontend install
```

Build the embedded headless runner before building/running the desktop app:

```bash
cargo build --profile release-runner -p duckle-runner
```

Run desktop development from the Tauri app directory:

```bash
cd apps/desktop
cargo tauri dev
```

Package the app:

```bash
cd apps/desktop
cargo tauri build
```

Useful checks:

```bash
cargo test -p duckle-metadata
npm --prefix frontend run lint
```

Full engine integration tests generally need a DuckDB CLI:

```bash
DUCKLE_DUCKDB_BIN=/path/to/duckdb cargo test -p duckle-duckdb-engine
```

## Build Gotchas

- `apps/desktop/build.rs` embeds `duckle-runner` at compile time. If the runner
  has not been built or staged, the desktop build can fail with a missing
  `duckle-runner` message.
- The MCP binary is optional for desktop builds. If it is not staged, the app
  embeds an empty MCP payload and reports that bundled MCP is unavailable.
- Cross-building a Linux pipeline artifact from a non-Linux host needs the
  static Linux runner staged at `apps/desktop/bin/duckle-runner-linux-x64`.
- `CONTRIBUTING.md` currently says to use `cargo run -p duckle-desktop`, but
  the scripts and Tauri config point to `cargo tauri dev` as the real dev path
  because it starts Vite and the Tauri shell together.

## Where Features Attach

Adding a new visual component usually means touching both frontend and engine:

1. Add or adjust the palette item in
   `frontend/src/workflow-ui/palette-data.ts`.
2. Add or adjust the properties manifest in
   `frontend/src/workflow-ui/fields/component-manifests.ts`.
3. If it needs ports or derived form behavior, check
   `frontend/src/workflow-ui/fields/manifest-synth.ts`.
4. If it is pure SQL, add planner/builder support in
   `crates/duckdb-engine/src/plan/builders.rs` and route the `componentId` in
   `build_view_sql` / `build_stage`.
5. If it needs non-SQL execution, add a spec in
   `crates/duckdb-engine/src/plan/specs.rs`, a `RuntimeSpec` variant in
   `crates/duckdb-engine/src/plan/mod.rs`, and executor handling in
   `crates/duckdb-engine/src/lib.rs`.
6. Add tests. Planner tests are usually the fastest first safety net; execution
   tests are needed once behavior depends on DuckDB or external systems.

Important: a palette entry alone only makes a component visible. The engine
must understand the `componentId` before the node is truly executable.

## First Verification

Smoke test run during the initial repo walk:

```bash
cargo test -p duckle-metadata
```

Result: passed, 3 tests ok.

No frontend lint/build was run yet because dependencies are not installed. No
full engine suite was run yet because that depends on `DUCKLE_DUCKDB_BIN` and
some tests touch external connector paths.

## Initial Stitchly v2 Read

This fork looks like a strong base for Stitchly v2 because it already has:

- Canvas-to-engine execution.
- Local-first workspace persistence.
- Many existing component definitions.
- Scheduler and headless runner paths.
- A realistic desktop shell instead of a web-only prototype.

The main risk is concentration of complexity in `crates/duckdb-engine`. Many
other crates look aspirational or partial. For early Stitchly v2 development,
we should probably move with the current architecture rather than refactor it
prematurely, then extract boundaries once we know which features we are keeping
and which product workflows matter most.

## Open Dev Process Threads

- Decide how much rebranding to do up front versus after the first Stitchly v2
  feature lands.
- Create a repeatable first-run setup checklist for contributors.
- Decide a standard test ladder: cheap crate tests, frontend typecheck, planner
  tests, DuckDB integration tests, desktop smoke.
- Document the feature-addition recipe with one real example.
- Decide how we want to track upstream Duckle changes while this fork diverges.
