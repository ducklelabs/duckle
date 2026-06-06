# Transforms & Data Quality

Duckle features a rich transformation library (over 120 visual and SQL operations) paired with inline data validation rules.

---

## 1. Transformation Catalog

Transforms are compiled by the execution engine directly into logical stages or SQL clauses (e.g. `SELECT`, `JOIN`, `GROUP BY`, `WINDOW` functions).

### Field Operations
* **Map**: A multi-functional visual mapper node. It joins a main input stream with up to 3 separate lookup inputs (using left or inner joins) and generates output columns using visual mapping expressions and filters.
* **Project / Cast / Rename**: Restructure table schemas, alter column types, or rename fields.
* **UUID / Coalesce**: Generate UUID v4 identifiers or coalesce null columns.

### Row & Aggregate Operations
* **Row Filters**: Visual expression builders or raw SQL filter predicates.
* **Fills**: Forward-fill, backward-fill, or constant-value fill for missing data.
* **Grouping**: Group By, Rollup, Cube, and window aggregates.
* **Approximate Functions**: HyperLogLog for unique counts; t-digest for approximate quantiles.

### Advanced Joins & Windows
* **Joins**: Inner, Left, Right, Full Outer, Cross, Lookup, Semi, Anti, and Spatial Joins.
* **Window Functions**: Row Number, Rank, Dense Rank, Lead, Lag, First/Last Value, and NTile.

### CDC & Change Data Capture
* **Incremental Load**: Watermark tracking. Saves the high-water mark to the local workspace state, only advancing it if the entire pipeline run finishes successfully.
* **SCD (Slowly Changing Dimensions)**: Built-in nodes for SCD Type 1 (overwrite) and SCD Type 2 (generating `valid_from`, `valid_to`, and `is_current` columns).
* **Merge/Upsert**: Upsert modifications into destinations (databases, MongoDB, etc.) with optional delete propagation driven by a CDC change-type column.

### AI & Text Search
* **Similarity Search**: Vector Similarity Search (cosine, L2, inner-product) using DuckDB `vss`.
* **Full-Text Search**: BM25 keyword matching via DuckDB `fts`.
* **AI Embeddings**: Connects to OpenAI-compatible `/v1/embeddings` providers (such as local Ollama, OpenAI, Cohere).
* **LLM UDFs**: Runs text completion models per row using custom prompting templates (e.g. `Translate {product_description} to Spanish`).
* **Text Chunker**: RAG-ready text segmentation running completely locally.
* **PII Redaction**: Regex-driven redaction of sensitive identifiers (emails, phone numbers, credit card numbers).

---

## 2. Data Quality & Visual Validation

Data Quality nodes in Duckle are designed to split data streams. Passing records continue out of the `main` output port, while invalid rows are routed to a **reject** port.

```text
               ┌──────────┐
               │  Source  │
               └────┬─────┘
                    │
              ┌─────▼────────┐      Reject Port
              │ Not-Null QA  ├─────────────────► [ Dead Letter Queue ]
              └─────┬────────┘
                    │ Pass Port (Main)
              ┌─────▼────────┐
              │ Transform/   │
              │ Destination  │
              └──────────────┘
```

### Key Validators
* **Not-Null Check**: Routes rows containing null values in specified fields to the reject port.
* **Range Check**: Asserts numeric ranges (inclusive or exclusive).
* **Regex Match**: Inspects fields against standard regular expression patterns.
* **Uniqueness Check**: Retains the first encountered row for a given key; sends duplicate records to the reject port.
* **Schema Validate**: Verifies that all expected columns are present.

### Profiling Tools
* **Column Profile**: Computes statistics (`SUMMARIZE` under the hood) including null percentage, min, max, average, and quantiles.
* **Describe**: Outputs the active column list, types, and nullability attributes of the input stream.
* **Histogram**: Lists top value frequencies for a single column.
* **Fuzzy Deduplicate / Record Match**: Uses text distance measures (Jaro-Winkler / Levenshtein) to locate near-duplicate rows.

---

## 3. Custom Code & User-Defined Functions (UDFs)

For logic that cannot be expressed visually, Duckle provides sandboxed custom programming blocks.

### Inline SQL & Templates
* **Inline SQL**: Write a raw SQL query referencing upstream nodes as the table `input`. materializes as a subquery stage in the compiled output.
* **SQL Templates**: Run queries parameterized with workspace context variables using `${context.var}`.

### JavaScript UDFs
* **Interpreter**: Executed using a sandboxed interpreter (`boa` crate) written in pure Rust.
* **Execution Model**: Runs a `transform(row)` script on each incoming row.
* **Security**: Sandbox environment has no access to the filesystem, network, or command execution shell.

### WebAssembly UDFs
* **Runtime**: Runs compiled WebAssembly binaries using the `wasmi` crate.
* **Usage**: Embed optimized custom libraries written in Rust, TinyGo, assemblyscript, or C to run row-by-row logic inside a fast sandbox.

### Shell Execution
* **Usage**: Runs custom shell scripts and returns a row containing `{stdout, stderr, exit_code, duration_ms}`. Includes a customizable timeout constraint to automatically kill long-running subprocesses.
