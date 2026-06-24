# Design: Live data views ("Dives")

Status: proposed (design pass done, not yet built)
Origin: community request - a local-first take on the "dives" concept (ask a question, get a live, shareable chart). "If Duckle had this, I wouldn't need any other tool."

## Goal

AI-generated, interactive, **live-querying**, shareable data views, fully **local-first**. A user asks a question in plain English over a workspace table; Duckie generates a SQL query plus a chart; it is saved as a persistent artifact that **re-runs its SQL against DuckDB on every open** (never a cached result) and renders an interactive chart. Sharing is a same-network URL or a self-contained HTML export - no cloud, no account, no tenancy.

Clean-room: our own JSON format invented on open libraries (DuckDB + Vega-Lite). No third-party product code or spec is referenced or copied.

## Why this is mostly assembly, not new infrastructure

It rides seams that already exist:

- **Dual backend for free.** `frontend/src/web-shim/tauri-core.ts` aliases `invoke(cmd, args)` to `POST /api/cmd/<cmd>` under `DUCKLE_WEB`. Implementing dives as `invoke` commands (a `#[tauri::command]` for desktop + a `dispatch_cmd` arm in `crates/duckle-runner/src/serve.rs`) gives desktop and the web editor parity automatically - the same mechanism `run_pipeline` / `compile_pipeline` use.
- **The read path is an existing pipeline run.** A dive is conceptually a one-node `code.sql` (View) with no input. v1 synthesizes that pipeline doc from the dive's SQL and runs it through the existing `runPipeline`, inheriting SSE progress, cancel, `${workspace}` resolution, the concurrent-drain spawn (avoids the #4 pipe-buffer deadlock), and `CREATE_NO_WINDOW`. No new Rust engine code for v1.
- **Generation reuses Duckie.** The OpenAI-compatible chat path (`chat_send` + `llama_chat::chat_stream`, already endpoint-configurable via #92) generates `{sql, chart}` from a new system prompt.
- **Storage reuses workspace artifacts.** `saveItemPayload` / `PAYLOAD_DIR_BY_TYPE` / `repository.json` with a new `"dive"` type and a `dives/` dir; `*.dive.json` so discovery filters cheaply, and `dives` is added to the pipeline-discovery skip list.

## The artifact (ours, hand-editable, git-committed)

`<workspace>/dives/<id>.dive.json`:

```json
{
  "diveSchemaVersion": 1,
  "id": "rev_by_region",
  "title": "Revenue by region",
  "question": "total revenue by region this year",
  "source": { "kind": "duckdb", "database": "out/warehouse.duckdb", "table": "sales" },
  "query": { "sql": "SELECT region, sum(amount) AS revenue FROM sales GROUP BY 1 ORDER BY 2 DESC",
             "params": [] },
  "chart": { "mark": "bar", "encoding": { "x": {"field":"region"}, "y": {"field":"revenue","type":"quantitative"} } },
  "meta": { "createdAt": "...", "rev": 1, "generator": "duckie" }
}
```

Only the SQL and the chart spec are stored - **never a result set** - which is what makes "never stale" automatic. The `chart` is a Vega-Lite spec with **no inline data**; rows are injected at runtime.

Source is always a **persistent** store (a `.duckdb` / parquet / DuckLake sink, or a loose data file), never an ephemeral pipeline view (the run's temp DB is deleted by `TempDbGuard`). "Dive on a pipeline output" means dive on that pipeline's durable sink.

## Visualization: Vega-Lite

`vega` + `vega-lite` + `vega-embed` (BSD-3-Clause), bundled from npm (no runtime CDN), lazy-loaded via dynamic `import()` so the editor hot path pays nothing until a dive opens. Decisive reason: the AI emits **one JSON object that is simultaneously the render contract and the stored spec**, and Vega-Lite is a serializable JSON grammar with an official JSON Schema to validate against. (ECharts `option` has no schema; Observable Plot marks are JS calls, not serializable.) Data stays out of the spec via a named dataset (`data:{name:"dive"}` + `view.data("dive", rows)`), so the file stays tiny and refresh is automatic. The same bundle is what gets inlined into the HTML export.

## AI grounding

A dedicated `DIVE_SYSTEM_PROMPT` (separate from the pipeline prompt) grounded on a **schema card** built from `inspect()`'s `DESCRIBE` plus a capped distinct-value sample (so the model cannot hallucinate columns or filter literals). Generated SQL is validated with DuckDB's own `EXPLAIN` / `DESCRIBE` against the real target (true bind-time column resolution) plus a read-only keyword denylist, with one error-fed repair retry. A deterministic `suggest_chart` heuristic (time+measure -> line, category+measure -> bar, two measures -> scatter, else table) is the fallback, and chart fields are cross-checked against the projected columns (fall back to a table on mismatch).

## Security (a cross-cutting fix this feature forces)

The `duckle-runner` web server currently has **no Origin / Host / CSRF check** on its POST routes - it is CSRF / DNS-rebinding exposed today, independent of dives. This feature lands the fix:

- `guard_local` on **all** state-changing POST routes: reject when `Host` is not the bind host or loopback, or when `Origin` (if present) is not same-origin.
- A same-origin CSRF token minted at boot, embedded in served HTML, required as `X-Duckle-Token` on POSTs.
- The live dive-run endpoint (v2) executes SQL **only** from the stored slug-addressed `.dive.json`, **never** from the request body (the #96 RCE shape must never exist).
- Params resolve to **safe typed literals** in the loader (numbers/dates/bools coerced, strings escaped, restricted to the declared param set) - never free-text concatenation. AI-generated SQL is treated as untrusted input to the same gate.
- Non-loopback bind requires an explicit `--allow-remote-exec` to enable execution; optional `--token` for same-network sharing.

## Phased plan

- **Phase 0 - format + read path** (the spine; proves never-stale end to end): `dive-types.ts` + validator, `dive-io.ts` (list/read/write on the existing fs bridges), `dive-run.ts` (synthesize one-node `code.sql` -> `runPipeline` -> `{columns, rows}`), `"dive"` workspace type + `dives/` skip-list line. Verify a hand-authored dive round-trips and re-queries in **both** desktop and `duckle-runner web`.
- **Phase 1 - viz**: add vega deps, `VegaChart.tsx` (named-dataset rebind, brand-token theme, lazy import), `DivePanel.tsx` (chart + reuse the preview table; field cross-check -> table fallback). v1 marks: table, bar, line, metric. Verify in WebView2 **and** the browser editor.
- **Phase 2 - AI generation**: `crates/duckdb-engine/src/dive.rs` (schema card, read-only assert, `EXPLAIN` validate), `DIVE_SYSTEM_PROMPT` + `parse_dive`, `dive_generate` command (card -> chat -> validate -> one repair), source picker + question box. Verify a hallucinated-column case is caught and auto-repaired.
- **Phase 3 - share + hardening**: `dive_export` (run once, escape, inline rows + pinned vega + strict CSP). Cross-cutting: `guard_local` + CSRF token on all POST routes. Verify the exported HTML opens offline and a cross-origin POST is rejected 403.
- **Phase 4 (deferred)**: live `GET /dive/<id>` URL (SQL only from the stored file), param controls UI, iframe/postMessage embedding, web-editor generation wiring, and the optional lock-free `Engine::query` read primitive if the `run_lock` serialization or 100-row preview cap bites.

## Known v1 limits (documented, not blockers)

- v1 reads ride `run_lock` (a dive load queues behind an in-flight pipeline run) and the 100-row `PREVIEW_ROW_LIMIT` (fine for aggregated charts; do not raise the global cap). The deferred `Engine::query` lock-free path is the clean fix if needed.
- Web-editor generation needs a configured external AI endpoint (no bundled model in the browser edition); fail loudly with a "configure AI in Settings" message.
