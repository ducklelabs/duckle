# Local Studio Quickstart

This is the working setup path for getting the current Duckle fork running as
the Stitchly v2 local studio.

## Goal

Get the desktop studio running locally with:

- React/Vite frontend in dev mode.
- Tauri desktop shell.
- Embedded `duckle-runner` available for desktop build/runtime features.
- DuckDB engine installed through first-launch setup.
- Optional browser-accessible operations panel.

## Prerequisites

Install:

- Rust via `rustup`.
- Node.js 20+.
- npm 10+.
- Tauri 2 CLI.
- OS-specific Tauri webview dependencies.

Install the Tauri CLI:

```bash
cargo install tauri-cli --version "^2"
```

On Linux, install the Tauri system dependencies for your distribution. On an
Ubuntu/Debian-style setup, this is typically in the family of:

```bash
sudo apt install build-essential curl wget file libssl-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
```

Use the official Tauri prerequisites page if a package name differs on the
target machine.

## One-Time Repo Setup

From the repo root:

```bash
npm --prefix frontend install
```

Do not run this with `sudo`. If the normal user install succeeds, it is done.
On some machines `sudo npm ...` fails because root's PATH does not include the
user-managed Node/npm install.

If npm reports audit findings, inspect them from the frontend package context:

```bash
npm --prefix frontend audit
```

Avoid running `npm audit fix` blindly during setup. If needed, run it with the
same prefix:

```bash
npm --prefix frontend audit fix
```

Running bare `npm audit fix` from the repo root can fail with `ENOLOCK`
because the lockfile lives under `frontend/package-lock.json`.

Build the embedded headless runner before starting the desktop app:

```bash
cargo build --profile release-runner -p duckle-runner
```

This matters because `apps/desktop/build.rs` embeds `duckle-runner` into the
desktop binary at compile time.

For `cargo tauri dev`, stage the built runner where the desktop build script
checks first:

```bash
cp target/release-runner/duckle-runner apps/desktop/bin/duckle-runner
```

Alternative: build a debug-profile runner with `cargo build -p duckle-runner`,
which creates `target/debug/duckle-runner`. The staged `apps/desktop/bin/`
approach is more explicit and matches the build script's preferred lookup path.

## Run The Desktop Studio

From the repo root:

```bash
cd apps/desktop
cargo tauri dev
```

This starts:

- Vite dev server at `http://localhost:5173`.
- Tauri desktop shell pointed at that dev server.

Use `cargo tauri dev` for desktop development. Do not use
`cargo run -p duckle-desktop` as the normal dev command; that starts the Rust
shell without Vite and can leave the window showing a localhost connection
error.

## First Launch Setup

Inside the app:

1. Install the DuckDB engine when prompted. This is required for pipeline
   execution.
2. Choose or create a workspace folder.
3. Install Duckie AI only if local AI chat is needed. It is optional and much
   larger than the core DuckDB engine.

The DuckDB CLI is downloaded into the app data directory. The desktop shell
publishes its path as `DUCKLE_DUCKDB_BIN` for engine calls that need it.

## Browser-Accessible Operations Panel

The existing HTTP web panel is provided by `duckle-runner serve`.

From the desktop app, it can be launched through the `open_web_panel` command.
It prefers:

```text
http://127.0.0.1:8080/
```

If port `8080` is busy, the app picks another local port.

Manual runner mode is also possible once a runner binary and DuckDB CLI are
available:

```bash
target/release-runner/duckle-runner serve \
  --host 127.0.0.1 \
  --port 8080 \
  --workspace /path/to/workspace \
  --duckdb /path/to/duckdb
```

The operations panel is not the full authoring studio. It is a browser
accessible view for pipeline discovery, run history, logs, schedules, and manual
runs.

## Useful Checks

Light Rust smoke test:

```bash
cargo test -p duckle-metadata
```

Frontend typecheck/lint:

```bash
npm --prefix frontend run lint
```

Full DuckDB engine integration tests need a DuckDB CLI path:

```bash
DUCKLE_DUCKDB_BIN=/path/to/duckdb cargo test -p duckle-duckdb-engine
```

## Day-One Path

For the fastest path to seeing the local studio alive:

```bash
npm --prefix frontend install
cargo build --profile release-runner -p duckle-runner
cp target/release-runner/duckle-runner apps/desktop/bin/duckle-runner
cd apps/desktop
cargo tauri dev
```

Then install DuckDB from the app's first-launch setup modal.

## Expected Services

During desktop development:

```text
5173       Vite dev server
dynamic    Tauri desktop shell process
subprocess DuckDB CLI when pipelines run
dynamic    llama-server only when Duckie AI is installed and used
8080       runner operations panel only when opened
stdio      MCP server only when configured/launched
```

## Common Failure Modes

- `frontend/node_modules` missing:
  run `npm --prefix frontend install`.

- `sudo npm --prefix frontend install` says `npm: command not found`:
  ignore it and use the normal user command. The root environment may not have
  npm on PATH.

- `npm audit fix` says `ENOLOCK`:
  run audit commands with `--prefix frontend`, or skip them until dependency
  updates are being handled intentionally.

- Desktop build cannot find `duckle-runner`:
  run `cargo build --profile release-runner -p duckle-runner`, then stage it
  with `cp target/release-runner/duckle-runner apps/desktop/bin/duckle-runner`.
  For dev builds, simply building the release-runner profile is not enough
  unless the binary is staged, because the desktop build script checks
  `apps/desktop/bin/` first and then the current Cargo profile path.

- Desktop window shows localhost refused:
  run through `cargo tauri dev` from `apps/desktop`, not raw `cargo run`.

- Pipeline execution says DuckDB is not installed:
  install DuckDB through the app setup modal, or set `DUCKLE_DUCKDB_BIN` when
  running headless tools.

- Web panel cannot bind `8080`:
  use another port, or let the desktop `open_web_panel` command choose a free
  one automatically.
