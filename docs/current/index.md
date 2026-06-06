# Duckle Documentation (Current)

Welcome to the official documentation for **Duckle**, the local-first desktop ETL / ELT studio with a built-in local AI assistant.

Duckle compiles visual data pipelines into SQL queries and executes them with native speed using embedded, columnar analytical engines. Featuring over 290+ connectors, 120+ transforms, a built-in scheduler, and a sandboxed AI assistant running entirely on your CPU, Duckle packs a complete ETL suite into a small desktop binary.

---

## Navigation & Structure

This documentation is structured into the following sections, designed to take you from initial setup to a complete understanding of Duckle's internals:

### 1. [Installation & Setup](installation.md)
* Learn how to download and install Duckle on **Windows, macOS, and Linux**.
* Understand the guided first-launch setup of the DuckDB execution engine and the optional Duckie AI assistant.
* Explore the file structure of a Duckle workspace.

### 2. [Getting Started Guide](getting-started.md)
* Step-by-step walkthrough to build and execute your first data pipeline (e.g., CSV to Parquet).
* How to use the **Duckie AI Assistant** to generate pipelines from natural language prompts.
* Learn about live previews, generated SQL plans, environment context variables, and saved credentials.

### 3. [Connectors & Sources / Sinks](connectors.md)
* Overview of the **290+ connectors** available at install time.
* Learn how the `SchemaInspector` trait works behind the scenes.
* Deep dive into the **CSV Connector** configuration, format options, and auto-detection mechanism.

### 4. [Transforms & Data Quality](transforms.md)
* Detailed catalog of the **120+ transformations** (Fields, Rows, Aggregates, Joins, Windowing, Shape/Pivot, CDC/SCD, and AI/Search).
* How to enforce data quality using visual validators (Not-Null, Range, Uniqueness) and the **Reject Port** flow.
* Writing custom User Defined Functions (UDFs) in JavaScript, WebAssembly, and SQL.

### 5. [Execution Engines](engines.md)
* An overview of Duckle's multi-engine adapter design.
* Understanding the default **DuckDB Engine** (query compiler & thread-pool execution).
* The experimental **SlothDB Engine** adapter.
* Future directions: Native Stream and Transform engines.

### 6. [Scheduler & Triggers](scheduler.md)
* How to schedule pipeline runs using **Cron**, **Interval**, and **File-Watch** triggers.
* Understand schedule persistence (`schedules.json`) and run bookkeeping.
* Running headless pipelines using the Duckle Command Line Interface (CLI).

### 7. [Architecture & Internals](architecture.md)
* Deep dive into the visual DAG editor, Tauri-based IPC bridge, and Shared App State.
* Understanding the logical plan optimization layer of the workflow engine.
* Structure of the `metadata` and `plugin-sdk` contract crates.

---

## Core Philosophy

Duckle is built on three main pillars:
1. **Visual but Transparent**: No black-box configurations. Every visual block compiles down to clean, readable SQL queries that you can preview and verify at any point.
2. **Local-First & Private**: Telemetry-free by design. Workspaces are directories on your local disk containing plain JSON and Markdown files. Git-friendly, branch-friendly, and audit-ready.
3. **Batteries Included**: Includes all connectors and engines out-of-the-box in a ~30 MB desktop app.
