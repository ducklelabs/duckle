# Architecture & Internals

Duckle is a multi-tier visual ETL platform designed around a local-first, privacy-respecting runtime. 

---

## 1. High-Level Architecture

The system consists of a TypeScript visual editor front-end, a Tauri-based desktop shell, and modular Rust crates that handle workspace directories, orchestration, compilation, and execution.

```text
  ┌──────────────────────────────────────────────────────────┐
  │                 React 19 / TS Front-End                  │
  │     (Canvas editor, Palette, properties forms, Chat)     │
  └───────────────┬──────────────────────────────▲───────────┘
                  │ Tauri IPC Invoke             │ Event Stream
  ┌───────────────▼──────────────────────────────┴───────────┐
  │                  Tauri Desktop Shell                     │
  │   (Workspace manager, Git integrations, process manager)  │
  └───────────────┬──────────────────────────────────────────┘
                  │ Inter-Crate FFI / Logic
  ┌───────────────▼──────────────────────────────────────────┐
  │                   Workflow Engine                        │
  │     (DAG Validation, Schema Propagation, Optimizer)      │
  └───────────────┬──────────────────────────────────────────┘
                  │ Logical Plan AST
  ┌───────────────▼──────────────────────────────────────────┐
  │                  Execution Engines                       │
  │  (DuckDB compiler, SlothDB adapter, future executors)    │
  └──────────────────────────────────────────────────────────┘
```

---

## 2. Crate Architecture

The backend code is divided into highly focused, decoupled Rust crates in a workspace:

### `apps/desktop` (Desktop Shell)
* **Role**: The main application runner. Binds the Tauri interface and acts as the bridge between frontend user actions and background threads.
* **Key Modules**:
  * `workspace_git.rs`: Standardizes command executions against local CLI `git` directories (stage, commit, push, pull, branch creation).
  * `engine_manager.rs`: Manages binary installations and updates for DuckDB and SlothDB.
  * `llama_chat.rs`: Manages the lifecycle of the local Qwen-Coder chat assistant, spawning a local `llama-server` subprocess.
  * `ci_status.rs`: Polls branch pipeline statuses using GitHub and GitLab APIs.

### `crates/workflow-engine` (Validation & Optimization)
* **Role**: Processes the visually declared node network.
* **Responsibilities**:
  * Runs cycle-detection validation on the directed graph (DAG).
  * Propagates schema configurations across connecting edges (e.g. mapping source formats to matching sink schemas).
  * Applies query optimization passes (such as filter pushdown).
  * Compiles the graph into the AST representation defined by `duckle-execution-core`.

### `crates/execution-core` (Logical Contracts)
* **Role**: Defines the engine-agnostic logical structure of pipelines.
* **Responsibilities**:
  * Defines intermediate execution plan models (tables, filters, projections, joins).
  * Establishes the engine traits that target runtimes (like DuckDB and SlothDB) implement.

### `crates/metadata` (Serializations & Schemes)
* **Role**: Holds the authoritative struct definitions for workspaces.
* **Structures**:
  * `PipelineNode`, `PipelineEdge`, `Position`, `EdgeData`, `NodeData`, and `Pipeline`.
  * `DataType`: Enumerates the primitive data types (String, Int32, Int64, Float32, Float64, Bool, Date, Timestamp, Time, Decimal, Json, Binary).
  * *Note: All models round-trip cleanly with the TypeScript interfaces of the frontend visual workspace.*

### `crates/plugin-sdk` (Connector Interface)
* **Role**: Establishes standard traits to build extensions.
* **Structures**:
  * Declares `SchemaInspector` and `Connector` traits.
  * Declares `Inspection` results (columns and preview row lists).

---

## 3. Local AI Subprocess: Duckie

The Duckie assistant runs completely locally. When a user activates the AI assistant:
1. The `llama_chat.rs` controller checks the `%APPDATA%/io.duckle.app/engines/` directory for the `llama-server` and model weight binaries.
2. It spawns a sandboxed `llama-server` child process bound to `127.0.0.1`.
3. The process runs Qwen 2.5 Coder 1.5B GGUF.
4. Chat prompts are streamed from the server to the frontend via server-sent events.
5. If the model outputs a pipeline JSON object, the frontend intercepts the markdown formatting, parses it, and places the corresponding visual graph onto the canvas.
