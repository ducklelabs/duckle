# Local Studio Surfaces

These notes map the current browser-accessible surfaces, backend/debug APIs,
and running services in the Duckle fork, then sketch a possible Stitchly v2
local studio topology.

## Current Shape

Duckle is not currently a classic web app with a frontend talking to an HTTP
backend. It has several local surfaces:

- Vite serves the React app during development.
- Tauri IPC is the primary desktop API.
- `duckle-runner serve` exposes a small HTTP operations console.
- DuckDB runs as a CLI subprocess.
- Duckie AI runs through a local `llama-server` subprocess when installed and
  first used.
- MCP runs as a stdio server, not an HTTP server.
- The scheduler runs inside the desktop app or inside the runner web panel
  process, depending on which surface is active.

This distinction matters for Stitchly v2 because "browser-accessible" can mean
two different things:

1. A browser UI only, backed by Tauri IPC when running inside the desktop shell.
2. A true local HTTP API and browser UI that can run outside Tauri.

## Current Browser Routes

The React frontend appears to be a single-page app without a real URL router.
In development, Vite serves:

```text
GET http://localhost:5173/
GET http://localhost:5173/index.html
```

All important app views are currently internal React state:

- Canvas
- Plan tab
- Run tab
- History tab
- Project/palette sidebar
- Properties panel
- Bottom output/problems/console panel
- Connection, context, routine, schedule, settings, Git, MCP, build, and chat
  modals/panels

When opened in a normal browser, real local execution features degrade because
`@tauri-apps/api` is unavailable. Browser mode is useful for UI development,
but it is not currently a full local studio.

## Current Desktop API

The desktop app exposes backend behavior through Tauri commands in
`apps/desktop/src/lib.rs`. These are IPC commands, not HTTP routes.

Main command groups:

```text
ping

autodetect_schema
compile_pipeline
run_pipeline
run_pipeline_partial
cancel_pipeline
run_history

watermark_list
watermark_set
watermark_clear

schedule_set_workspace
schedule_list
schedule_upsert
schedule_delete
schedule_run_now

engine_status
engine_install
dbt_status
dbt_install

chat_send
chat_extract_pipeline

workspace_git_status
workspace_git_init
workspace_git_commit
workspace_git_push
workspace_git_pull
workspace_git_branches
workspace_git_branch_create
workspace_git_branch_checkout
workspace_git_remote_set
workspace_git_save_pat
workspace_git_clear_pat
workspace_ci_status

connection_encrypt_payload
connection_decrypt_payload

settings_get_proxy
settings_set_proxy

build_capabilities
build_pipeline_bundle

mcp_connection_info
connect_claude_code
mcp_inject_config

open_web_panel
check_for_update
self_update
```

For Stitchly, these commands are the current "backend API" surface for the
desktop studio. If we want a browser-only local studio, we need to wrap or
mirror part of this surface over HTTP.

## Current HTTP Backend Routes

The existing HTTP backend is the runner web panel, implemented in
`crates/duckle-runner/src/serve.rs`.

It can be launched manually:

```bash
duckle-runner serve --host 127.0.0.1 --port 8080 --workspace /path/to/workspace --duckdb /path/to/duckdb
```

The desktop app can also spawn it through the `open_web_panel` Tauri command.
It prefers port `8080`, then falls back to an OS-assigned free port.

Default browser URL:

```text
http://127.0.0.1:8080/
```

Current routes:

```text
GET  /
GET  /index.html

GET  /api/summary
GET  /api/pipelines
GET  /api/pipeline?file=<workspace-relative-json>
GET  /api/runs
GET  /api/runs?id=<pipeline-id>
GET  /api/log?id=<pipeline-id>&tail=200
GET  /api/schedules

POST /api/schedules
POST /api/run
```

This is closer to an operations console than a full studio. It discovers
pipeline JSON files in a workspace, lists run history, shows logs, stores
simple interval schedules in `panel-schedules.json`, and can manually run a
pipeline.

There is no authentication. It should bind to localhost unless intentionally
used on a trusted network.

## Running Services

Current services/processes to know about:

```text
5173       Vite React dev server
8080       duckle-runner web panel, when started
dynamic    llama-server for local AI chat
stdio      duckle-mcp server
subprocess DuckDB CLI per run/stage/batch
in-process scheduler in desktop app
in-process scheduler in runner serve mode
```

Potential Stitchly v2 local development topology:

```text
5173  Vite React dev server
3000  Stitchly local HTTP API server, if we add one
8080  Runner operations web panel, existing
dynamic llama-server for local AI
stdio  MCP server
subprocess DuckDB CLI
```

## Proposed Studio Routes

If we make Stitchly v2 browser-addressable, a useful route map could be:

```text
/                                      main studio
/workspaces                            workspace picker/manage
/workspaces/:workspaceId               workspace shell
/workspaces/:workspaceId/pipelines/:pipelineId
/workspaces/:workspaceId/pipelines/:pipelineId/plan
/workspaces/:workspaceId/pipelines/:pipelineId/runs
/workspaces/:workspaceId/pipelines/:pipelineId/history
/workspaces/:workspaceId/connections
/workspaces/:workspaceId/contexts
/workspaces/:workspaceId/routines
/workspaces/:workspaceId/schedules
/workspaces/:workspaceId/settings
/debug
/debug/services
/debug/engines
/debug/logs
/debug/ipc
```

These routes would make deep links and debugging easier than the current
all-state-inside-one-page model.

## Proposed Local HTTP API

If we add a real browser-accessible local backend, a first API shape could be:

```text
GET  /api/health
GET  /api/services
GET  /api/engines
POST /api/engines/:engineId/install

GET  /api/workspaces
POST /api/workspaces/open
GET  /api/workspaces/:workspaceId
GET  /api/workspaces/:workspaceId/files

GET  /api/workspaces/:workspaceId/pipelines
POST /api/workspaces/:workspaceId/pipelines
GET  /api/workspaces/:workspaceId/pipelines/:pipelineId
PUT  /api/workspaces/:workspaceId/pipelines/:pipelineId
DELETE /api/workspaces/:workspaceId/pipelines/:pipelineId

POST /api/workspaces/:workspaceId/pipelines/:pipelineId/compile
POST /api/workspaces/:workspaceId/pipelines/:pipelineId/run
POST /api/workspaces/:workspaceId/pipelines/:pipelineId/run-partial

GET  /api/runs/:runId
GET  /api/runs/:runId/events
GET  /api/runs/:runId/logs
POST /api/runs/:runId/cancel

GET  /api/workspaces/:workspaceId/connections
POST /api/workspaces/:workspaceId/connections
PUT  /api/workspaces/:workspaceId/connections/:connectionId
DELETE /api/workspaces/:workspaceId/connections/:connectionId

GET  /api/workspaces/:workspaceId/contexts
POST /api/workspaces/:workspaceId/contexts
PUT  /api/workspaces/:workspaceId/contexts/:contextId
DELETE /api/workspaces/:workspaceId/contexts/:contextId

GET  /api/workspaces/:workspaceId/schedules
POST /api/workspaces/:workspaceId/schedules
DELETE /api/workspaces/:workspaceId/schedules/:scheduleId

POST /api/inspect/schema
POST /api/compile/sql
```

For live run events, use SSE or WebSocket:

```text
GET /api/runs/:runId/events
```

SSE is probably enough for one-way run event streaming and simpler to debug
with curl/browser tooling. WebSocket may be useful later if the browser needs
bidirectional control during a run.

## Two Possible Modes

### Mode 1: Desktop Studio

Keep the Tauri app as the full-power local studio.

- Vite is only a dev server.
- Real filesystem/process/secret access stays behind Tauri IPC.
- The browser cannot run everything by itself.
- This is closest to the current architecture and likely fastest.

### Mode 2: Browser Debug/Operations Studio

Expand `duckle-runner serve` into a stronger browser-accessible local console.

- Keep it localhost-only by default.
- Add richer debugging APIs for services, engines, logs, workspace files, and
  pipeline compile/run.
- Avoid porting the entire desktop app all at once.
- Useful for server deployments and local debugging.

## Near-Term Recommendation

For early Stitchly v2, separate the terms clearly:

- "Desktop Studio" means full Tauri-powered authoring and execution.
- "Web Panel" means browser-accessible operations/debugging over HTTP.
- "Local API" means a future HTTP service that may wrap the Tauri/engine
  command surface.

The most practical first step is to improve/document the existing runner web
panel and use it as the browser-accessible debug surface. A full browser-native
studio can come later if we decide the product should run outside Tauri.

## Open Questions

- Do we want deep-linkable routes in the React app even inside Tauri?
- Should the local HTTP API be a new crate/server or an expansion of
  `duckle-runner serve`?
- Should Tauri commands and HTTP handlers share a common application-service
  layer to avoid duplicating behavior?
- How much of workspace editing should be exposed over HTTP?
- What authentication, if any, is required once a local API exists?
- Should the debug panel be developer-only, or a normal user-facing operations
  console?
