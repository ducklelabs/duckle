# Duckle - Groundbreaking feature ideas

A shortlist of differentiating features to pull in more users, chosen to lean on
Duckle's unique edge that cloud ETL tools structurally cannot match:

- **Local-first**: data, schemas and credentials never leave the machine.
- **DuckDB engine**: columnar, out-of-core, parquet-native speed on a laptop.
- **On-device AI**: embeddings, LLM, classify, PII and a bundled local chat
  model (llama.cpp) already ship - so "AI features" run with zero data exfil.
- **Visual + code**: a canvas that compiles to inspectable SQL/code.

The market is converging on AI-assisted, self-healing, lineage-aware pipelines
(see Sources). Almost every vendor delivers these as cloud SaaS. Duckle's wedge
is delivering the same magic **privately, on-device, for free**.

---

## Tier 1 - headline differentiators (build first)

### 1. Natural-language -> pipeline ("describe it, get a DAG")
Type "join orders with customers, flag refunds over $500, write parquet" and the
bundled local LLM wires up the canvas (sources, joins, filters, sink) using the
existing component manifests. Competitors charge for cloud NL->pipeline; Duckle
does it **fully offline**, so no schema or sample row ever leaves the laptop.
- *Builds on:* the existing AI chat + llama runtime + component manifest registry.
- *Lure:* the single biggest "wow" in 2026 data tooling, minus the privacy cost.
- *Effort:* medium-high.

### 2. Column-level lineage + impact analysis (built in, visual)
Click a column on any sink and light up every upstream node/column that feeds it;
ask "what breaks if I drop this field?" and get the blast radius before you run.
The planner already propagates schemas (`derive_output_columns`); extend that to
column-level edges and overlay it on the canvas.
- *Lure:* audit/governance catnip (regulated industries demand provable lineage),
  and it is genuinely useful day-to-day. Cloud tools sell this as a separate
  product; Duckle gets it for free from its own compiler.
- *Effort:* medium.

### 3. AI self-healing / one-click fix
When a run fails (schema drift, type mismatch, the null-cluster class of bugs),
the local LLM proposes a concrete fix as a canvas edit - insert a Cast, Fill,
Rename or a `responsePath` - with one-click apply and a diff preview.
- *Builds on:* structured run errors + the on-device LLM.
- *Lure:* "self-healing pipelines" is a top 2026 differentiator; doing it as
  reviewable canvas suggestions (not opaque auto-magic) builds trust.
- *Effort:* medium.

## Tier 2 - strong, feasible

### 4. Run diff / data time-travel
Snapshot each run's outputs to parquet (the checkpoint plumbing exists) and show
a row-level diff between run N and N-1 per node: added / removed / changed rows,
"what changed since yesterday." Observability built in, computed locally by
DuckDB's own `EXCEPT`/anti-join in milliseconds.
- *Effort:* medium.

### 5. Incremental / CDC subscriptions
Nodes subscribe to upstream output changes; only the affected downstream re-runs
(keyed on changed partitions). Pairs with the DuckLake connector for change feeds.
This was already floated for the ducklake-cdc direction.
- *Lure:* makes large local datasets practical; turns Duckle from batch to
  near-incremental.
- *Effort:* high.

### 6. Data contracts + assertions as first class
Promote the `qa.*` validators into named contracts per node: row-count bounds,
null %, uniqueness, value ranges, freshness - validated each run, blocking on
violation, with a pass/fail history panel.
- *Effort:* medium.

### 7. Semantic catalog / "search your data lake"
Point Duckle at a folder or lake; it auto-profiles and embeds each schema with
on-device embeddings so you can search "where is the customer email column?"
across hundreds of parquet/csv files - a private semantic catalog.
- *Builds on:* `xf.ai.embed` + DuckDB profiling.
- *Effort:* medium-high.

## Tier 3 - quick wins / positioning

### 8. "Ship as" - pipeline to portable artifact
Author visually, then export one-click to: a runnable Python+duckdb script, a
headless CLI invocation, a cron/scheduled run, or a tiny Docker image. The SQL
export already exists; this closes the "prototype visually, run anywhere" loop.
- *Effort:* medium.

### 9. In-canvas "explain this" + null-doctor
Select any node -> live sample preview -> ask the local LLM "explain this
transform" or "why are there nulls here?" Inline, private, zero setup.
- *Effort:* low-medium.

### 10. Template gallery
A browsable gallery of ready-made pipelines (CSV cleanup, API->parquet,
SCD2 load, dedupe+embed) installable in one click via the existing in-app Git.
- *Effort:* low-medium.

---

## Recommended first three
1. **Natural-language -> pipeline** (the marquee lure; maximizes the local-AI edge)
2. **Column-level lineage + impact analysis** (unique-from-the-compiler, governance pull)
3. **AI self-healing fixes** (turns the existing correctness work into a visible superpower)

Together they tell one story: *the visual, private, on-device data tool that
understands your pipeline well enough to build it, explain it, and fix it for you.*

---

## Sources
- [How AI is transforming modern data pipelines - dbt Labs](https://www.getdbt.com/blog/how-ai-changes-data-pipelines)
- [AI ETL: how AI automates data pipelines - Databricks](https://www.databricks.com/blog/ai-etl-how-artificial-intelligence-automates-data-pipelines)
- [Top AI ETL tools for data teams - Airbyte](https://airbyte.com/data-engineering-resources/ai-etl-tools)
- [Data lineage: why it is essential in 2026 - BuzzClan](https://buzzclan.com/data-engineering/data-lineage/)
- [AI-powered ETL market projections - Integrate.io](https://www.integrate.io/blog/ai-powered-etl-market-projections/)
