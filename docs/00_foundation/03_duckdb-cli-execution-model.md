# DuckDB CLI Execution Model

This note explains how the Duckle fork uses DuckDB, why that differs from a
compile/link-in-process model, and what efficiencies and tradeoffs matter for
Stitchly v2.

## Core Difference

In Stitchly v1, DuckDB had to be compiled into the application stack. That
usually means linking against a DuckDB library through a Rust crate such as
`duckdb` / `libduckdb-sys`, which can trigger native C/C++ compilation and
platform-specific build friction.

This fork takes a different approach:

```text
Rust app
  ↓
downloads/finds official DuckDB CLI binary
  ↓
spawns DuckDB as a subprocess
  ↓
sends SQL through `duckdb -json -bail -c "<sql>"`
  ↓
parses JSON output and maps it back to run status / previews
```

So DuckDB is used as a managed local executable, not as a library compiled into
the Rust process.

## Where The Binary Comes From

The desktop app pins a DuckDB CLI version in:

```text
apps/desktop/src/engine_manager.rs
```

At the time of this note:

```text
DUCKDB_VERSION = 1.5.4
```

On first launch, the app's setup flow downloads the official DuckDB CLI release
asset for the current OS/architecture and installs it into the app data
directory under the managed engines folder.

The desktop shell then resolves that path and publishes it as:

```text
DUCKLE_DUCKDB_BIN=/path/to/app-data/engines/duckdb/duckdb
```

The engine also stores the same path in its `DuckdbEngine`.

Local repo-root binaries such as `.duckdb-cli-v1.5.3/duckdb.exe` may exist, but
they are not the default desktop engine path. The app expects its managed
first-launch install unless a headless command is explicitly pointed elsewhere.

## Runtime Flow

The normal pipeline execution path is:

```text
React canvas nodes/edges
  ↓
Tauri command: run_pipeline / run_pipeline_partial
  ↓
crates/duckdb-engine planner
  ↓
ordered stages
  ↓
DuckDB CLI subprocesses
  ↓
run events, previews, history, logs
```

The planner converts nodes into SQL or runtime stages. Pure SQL components
become statements such as:

```sql
CREATE OR REPLACE TABLE "node_id" AS
SELECT ...
```

Sinks become statements such as:

```sql
COPY (...) TO 'output.parquet' (FORMAT PARQUET);
```

The executor uses a temporary on-disk `.duckdb` database for a run so separate
CLI invocations can share intermediate tables.

Example shape:

```bash
duckdb /tmp/duckle_run_123.duckdb -json -bail -c "CREATE TABLE ...; SELECT ...;"
```

For source inspection/autodetect, the engine can use an in-memory database:

```bash
duckdb :memory: -json -bail -c "DESCRIBE SELECT * FROM read_csv_auto(...);"
```

## Efficiency Benefits

### Faster Rust Builds

The main app does not need to compile DuckDB's native code during normal
development. This avoids a large source build and reduces the chance of
platform-specific compiler/linker failures.

### Smaller Desktop Binary

DuckDB is not statically embedded into the desktop app binary. The desktop app
can stay relatively small, and DuckDB is installed as a managed engine binary
on first launch.

### Cleaner Engine Upgrades

Upgrading DuckDB is mostly a matter of changing the pinned CLI version and
download asset mapping, then validating behavior. That is simpler than
debugging native library build changes across OSes.

### Less Native Toolchain Risk

Using the official DuckDB CLI binary reduces the amount of C/C++ toolchain work
required from each contributor machine. Developers still need Rust, Node, npm,
and Tauri prerequisites, but not a successful DuckDB source build in the common
path.

### Better Isolation

DuckDB runs out-of-process. If a DuckDB invocation hangs or needs cancellation,
the Rust engine can kill the child process. The app process is not the DuckDB
process.

### Headless Reuse

The same CLI-driven engine can be used from:

- Desktop Tauri commands.
- `duckle-runner`.
- `duckle-runner serve`.
- MCP tools, when configured with `DUCKLE_DUCKDB_BIN`.

This gives us one execution model across studio and server-style workflows.

## Performance Path

Subprocess execution has overhead. The fork compensates with a batched path.

If the whole pipeline is pure SQL and has no special hooks, retries, waits,
memory overrides, or local overwrite checks, the engine can collapse multiple
stages into one DuckDB CLI invocation.

That avoids paying process startup cost for every stage.

When a stage needs Rust-side behavior, the engine falls back to per-stage
execution. Examples include:

- REST/API connectors.
- Kafka, RabbitMQ, NATS, Kinesis.
- MongoDB, Redis, Cassandra, Elasticsearch.
- Snowflake or Databricks API paths.
- Control flow nodes.
- Retry/wait behavior.
- Some special materialization or sink behavior.

## Tradeoffs

### Runtime Dependency

Pipeline execution depends on a DuckDB executable being present. If DuckDB has
not been installed, the app must prompt the user or fail clearly.

### Subprocess Complexity

The engine must manage process spawning, stdout/stderr draining, cancellation,
exit statuses, and JSON parsing.

### Temp Database Discipline

Because separate CLI invocations need shared state, runs use a temp on-disk
`.duckdb` file. The engine must keep intermediate table naming, cleanup, and
concurrent runs disciplined.

### Extension Management

DuckDB extensions need to be installed/available for the managed CLI. The
desktop engine installer pre-installs a set of expected extensions. Built
pipeline artifacts may also bundle DuckDB and extension files.

### Less Direct In-Process API Control

The Rust process does not call DuckDB's in-process API directly. It sends SQL
to a CLI and receives output. This is simpler operationally, but less direct
than library integration for advanced embedded use cases.

## Build Pipeline Artifacts

The headless runner has a build mode that can create a self-contained pipeline
artifact. In that path, the builder may copy the DuckDB CLI into the artifact
payload:

```text
artifact executable
  + embedded pipeline JSON
  + bin/duckdb
  + selected extensions
```

At runtime, the artifact extracts its payload and sets:

```text
DUCKLE_DUCKDB_BIN=<extracted>/bin/duckdb
```

Then it runs the same CLI-driven engine path.

If no DuckDB binary is available during build, the artifact can fall back to
requiring `duckdb` on `PATH`, but that is less self-contained.

## Practical Implication For Stitchly v2

For early Stitchly v2, we should treat DuckDB as a managed local engine binary.

That means:

- Do not reintroduce native DuckDB compilation unless there is a clear need.
- Keep `DUCKLE_DUCKDB_BIN` explicit in docs and headless workflows.
- Make first-launch engine setup reliable and visible.
- Add diagnostics for "which DuckDB binary am I using?".
- Prefer testing planner behavior separately from DuckDB CLI integration when
  possible, then run CLI-backed integration tests where execution matters.

This model is a pragmatic fit for a local studio: faster contributor setup,
less native build pain, and one engine path that works from desktop, runner,
and operations panel.
