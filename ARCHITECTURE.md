# Duckle Architecture

This document describes Duckle's design and how the pieces fit together. It is a living document; expect it to evolve with the codebase.

## Goals

1. **Embedded** — no JVM, no daemon, no cluster, no cloud account.
2. **Fast** — Arrow-native, vectorized, parallel, lazy.
3. **Visual** — pipelines are designed in a drag-and-drop DAG editor and stored as declarative documents.
4. **Modular** — connectors, transforms, and engines are plugins that can be developed and shipped independently.

## Layers

```
┌──────────────────────────────────────────────────────────────┐
│  UI layer (React + Vite + Tauri 2 webview)                   │
│    canvas / components / workflow-ui                          │
└───────────────┬──────────────────────────────────────────────┘
                │  Tauri commands (JSON-RPC over IPC)
┌───────────────▼──────────────────────────────────────────────┐
│  Runtime (crates/runtime)                                    │
│    app state, command handlers, IPC plumbing, log streaming  │
└───────────────┬──────────────────────────────────────────────┘
                │
┌───────────────▼──────────────────────────────────────────────┐
│  Workflow layer                                              │
│    workflow-engine   DAG model, validation, scheduling        │
│    metadata          pipeline docs, schemas, lineage          │
│    scheduler         time- and event-driven triggers          │
└───────────────┬──────────────────────────────────────────────┘
                │
┌───────────────▼──────────────────────────────────────────────┐
│  Execution layer                                             │
│    execution-core    engine-agnostic execution abstractions   │
│    transform-engine  native vectorized transforms             │
│    stream-engine     streaming and incremental execution      │
│    duckdb-engine     DuckDB-backed execution                  │
│    slothdb-engine    SlothDB-backed execution                 │
└───────────────┬──────────────────────────────────────────────┘
                │
┌───────────────▼──────────────────────────────────────────────┐
│  I/O layer                                                   │
│    connectors        sources and sinks                        │
│    plugin-sdk        contract for third-party extensions      │
└──────────────────────────────────────────────────────────────┘
```

## Pipeline model

A pipeline is a directed acyclic graph of **nodes** connected by **edges**. Nodes belong to one of four kinds:

- **Source** — reads from a connector.
- **Transform** — applies a vectorized operation to one or more inputs.
- **Sink** — writes to a connector.
- **Control** — branches, routes, or merges flow.

Each node declares its input and output schemas. Schemas are first-class and propagate through the DAG; the editor validates connections statically before execution.

Pipelines are serialized as canonical JSON documents (`metadata` crate). The document is the source of truth; the visual layout is metadata stored alongside it.

## Execution model

Execution is a two-phase process:

1. **Planning** — `workflow-engine` validates the DAG, performs schema propagation, applies optimizer passes (predicate pushdown, projection pruning, operator fusion), and produces an engine-agnostic logical plan.
2. **Running** — `execution-core` translates the logical plan to one of the registered engines. The engine produces Arrow record batches that flow through the pipeline.

Engines implement a small trait:

```rust
trait Engine {
    fn name(&self) -> &str;
    fn execute(&self, plan: LogicalPlan) -> Result<ExecutionHandle>;
}
```

The default engine is DuckDB. SlothDB and the native engine are alternatives selectable per pipeline or per run.

## Data movement

All in-process data is **Arrow record batches**. Connectors produce and consume Arrow; transforms operate on Arrow; engines speak Arrow over their FFI boundary (DuckDB Arrow extension, SlothDB Arrow API). Where Arrow is not native to a sink format, the connector converts at the boundary.

Streaming pipelines use the same Arrow batches with bounded backpressure-aware channels between operators.

## Plugin contract

The `plugin-sdk` crate defines stable Rust traits for:

- `Connector` — source or sink with a typed configuration schema.
- `Transform` — typed input and output schemas plus a `process(batch) -> batch` function.
- `Engine` — execute a `LogicalPlan` and return an `ExecutionHandle`.

Plugins compile as `cdylib` crates and are loaded dynamically. A versioned ABI guards against incompatible loads.

## Frontend ↔ Backend

The frontend speaks to the Rust runtime through Tauri commands. Commands are typed on both sides: Rust handlers use `serde`-derived structs, and the frontend imports matching TypeScript types generated at build time.

Long-running operations (pipeline runs, log streams) use Tauri events for streaming updates.

## Performance principles

- **Arrow-first**: zero-copy where possible, columnar everywhere.
- **Vectorized**: operators process batches, not rows.
- **Lazy**: planning is separate from execution; we materialize as late as possible.
- **Parallel**: independent subgraphs run on a Tokio multi-threaded runtime; CPU-bound operators use `rayon` where applicable.
- **Pushdown**: filters and projections push into sources whenever the source supports them.

## What Duckle is not

- Not a cluster orchestrator. Use Airflow or Dagster on top if you need scheduling at scale.
- Not a data warehouse. Use DuckDB, SlothDB, or your warehouse as the analytical store.
- Not a SaaS. Duckle is local-first; a server mode may follow, but the desktop is the primary experience.
