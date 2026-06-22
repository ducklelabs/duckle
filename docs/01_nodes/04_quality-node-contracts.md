# Quality Node Contracts

This note documents `qa.*` quality-node behavior at the contract level. It is intended for agents that need to build validation/profiling workflows or add/modify quality nodes.

Authoritative files:

- Palette entries: `frontend/src/workflow-ui/palette-data.ts`
- Form contracts: `frontend/src/workflow-ui/fields/manifest-synth.ts`
- Quality SQL builders: `crates/duckdb-engine/src/plan/builders.rs`
- Reject-output logic: `crates/duckdb-engine/src/plan/builders.rs`
- Schema/port validation: `crates/duckdb-engine/src/plan/graph.rs`

## Common Quality Contract

Quality nodes are mostly SQL-backed transforms with explicit validation/profiling intent.

| Aspect | Contract |
|---|---|
| Input | Usually one `main` input. Link/reconcile/refintegrity also use a reference/lookup input. |
| Main output | Pass rows, scorecard rows, profile rows, or cleansed rows depending node. |
| Reject output | Validation nodes can emit reject rows to `<node>__reject` when a reject port is wired. |
| Runtime | Currently pure DuckDB SQL builder paths. |
| Failure behavior | Some nodes report rows, some gate/fail, and some route rejects. Verify node semantics before using in CI. |

## Validation Nodes

| Nodes | Input | Main output | Reject output | Notes |
|---|---|---|---|---|
| `qa.schemavalidate` | Main | Rows matching expected schema/null rules | Invalid rows | Current implementation is closer to expected-column/null validation than full structural schema enforcement. |
| `qa.regex` | Main | Rows where column matches pattern | Non-matching rows | Use for IDs, codes, emails, known formats. |
| `qa.range` | Main | Rows inside min/max bounds | Out-of-range rows | Numeric/date-like comparisons depend on DuckDB type behavior. |
| `qa.notnull` | Main | Rows where required columns are non-null | Rows with nulls | Useful before sinks with NOT NULL constraints. |
| `qa.unique` | Main | First row per key | Duplicate rows | Use stable upstream ordering if survivor row matters. |
| `qa.outlier` | Main | Inlier rows | Outlier rows | Supports IQR/z-score style detection. Nulls/zero-spread data generally pass. |

Agent rule: validation nodes are best when paired with `ctl.deadletter`, `ctl.die`, `ctl.warn`, or `xf.diffsummary`/`xf.count` so failures become visible.

## Profiling Nodes

| Nodes | Input | Output | Notes |
|---|---|---|---|
| `qa.profile` | Main | Per-column stats | Count, nulls, distinct, min/max/quartile-style profile. |
| `qa.profile.adv` | Main | Long-form metric rows for one column | Includes null pct, approx distinct, pattern fractions, top values. |
| `qa.describe` | Main | Column names/types | Good for schema inspection and debugging. |
| `qa.histogram` | Main | Value/count rows for one column | Good for categorical distributions. |

Profiling nodes are report producers, not row-pass validators. Do not wire them before a sink expecting the original data unless that is intentional.

## Cleansing and Governance Nodes

| Nodes | Input | Output | Notes |
|---|---|---|---|
| `qa.standardize` | Main | Same rows with standardized text columns | Trim/case/collapse whitespace. |
| `qa.mask` | Main | Same rows with masked/anonymized column | Hash, partial mask, null-out, or constant replacement. |
| `qa.dedupe` | Main | Fewer rows after fuzzy dedupe | String similarity over selected columns. |
| `qa.match` | Main | Candidate match pairs with score | Self-match style output, not original row passthrough. |
| `qa.survivor` | Main | One golden record per group | Uses most frequent/recent/oldest/max/min rule. |
| `qa.matchgroup` | Match-pair rows | Record id to cluster id rows | Builds connected components from matched pairs. |
| `qa.expect` | Main | Scorecard rows per rule | Rules: not_null, unique, non_negative, in_set, in_range, regex. |
| `qa.contract` | Main | Original rows when all rules pass; run failure when any rule breaks | Gate node for CI/scheduled loads. |
| `qa.freshness` | Main | Original rows in gate mode, or one-row report in report mode | Checks `now - max(timestamp)` against SLA. |
| `qa.sample.adv` | Main | Reproducible percent sample | Supports reservoir/Bernoulli and optional seed. |
| `qa.refintegrity` | Main plus reference | Rows whose FK exists in reference | Reject output contains orphan rows when wired. |
| `qa.link` | Main plus reference | Candidate cross-dataset match pairs | Uses string similarity and threshold. |
| `qa.reconcile` | Main plus target/reference | Metric rows comparing source and target | Row counts, only-in-source/target, matched keys, optional measure sums. |
| `qa.classify` | Main | Column classification/PII report | Heuristic regex/statistics; no LLM. |

## Ports and Shape Changes

| Pattern | Nodes |
|---|---|
| Pass/Reject validators | `qa.schemavalidate`, `qa.regex`, `qa.range`, `qa.notnull`, `qa.unique`, `qa.outlier`, `qa.refintegrity` |
| Report producers | `qa.profile`, `qa.profile.adv`, `qa.describe`, `qa.histogram`, `qa.expect`, `qa.reconcile`, `qa.classify`, `qa.match`, `qa.link`, `qa.matchgroup` |
| Row-preserving cleansers | `qa.standardize`, `qa.mask`, `qa.contract` when passing, `qa.freshness` in gate mode |
| Row-reducing cleansers | `qa.dedupe`, `qa.survivor`, `qa.sample.adv` |

Agent rule: before connecting a quality node downstream, know whether it preserves original rows or emits a report. This is the main source of workflow-shape mistakes.

## Common Quality Patterns

| Pattern | Suggested graph |
|---|---|
| Block bad load | Source -> transforms -> `qa.contract` -> sink |
| Send rejects to file | Source -> `qa.notnull`/`qa.regex` with reject edge -> `ctl.deadletter` |
| Fail when rejects exist | Validator reject edge -> `xf.count` -> `ctl.die` with `has-rows` |
| Profile incoming data | Source -> `qa.profile`/`qa.describe` -> `snk.json` or preview |
| Migration reconciliation | Source A + Source B -> `qa.reconcile` -> report sink/assertion |
| PII governance | Source -> `qa.classify` report, then source -> `qa.mask` for selected columns |

## Agent Rules

- Use quality nodes before sinks, not after, unless the sink output is being re-read.
- Keep report-producing quality nodes on separate branches from row-preserving load branches.
- Wire reject outputs intentionally; otherwise validation failures may only affect the main pass rows.
- Use `qa.contract` for CI gates and `qa.expect` for scorecards.
- Use `qa.reconcile`, `qa.diffsummary`, and `xf.assert` together for migration validation.
- Verify exact rule syntax in `builders.rs` before generating expectation/contract JSON.

## Adding a New Quality Node

Minimum implementation checklist:

1. Add the palette entry in `palette-data.ts`.
2. Add or route a form manifest in `manifest-synth.ts`.
3. Decide whether the node is row-preserving, row-filtering, reject-producing, or report-producing.
4. Add SQL builder support in `builders.rs`.
5. If it has rejects, add reject SQL handling and document the reject schema.
6. Add column-reference validation in `plan/graph.rs` when props reference upstream columns.
7. Add tests for pass output, reject output if applicable, and missing required props.
8. Update this doc and `00_node-inventory.md`.
