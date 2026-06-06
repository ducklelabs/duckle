# Installation & Guided Setup

Duckle is compiled as a lightweight, self-contained desktop binary (~24–30 MB depending on the platform). Because it follows a local-first philosophy, it does not include database engines or heavy AI weights inside the initial bundle. Instead, it downloads them into your local application data directory on the first launch.

---

## 1. Downloading the Desktop Binary

Pick the binary matching your operating system from the latest release page:

| Operating System | Asset Name | Run Instructions | Notes |
| :--- | :--- | :--- | :--- |
| **Windows** | `Duckle-windows-x64.exe` | Double-click the installer. | Unsigned binary: Windows SmartScreen will prompt; click **"More info"** -> **"Run anyway"**. |
| **macOS** (Apple Silicon) | `Duckle-macos-arm64` | `chmod +x Duckle-macos-arm64 && ./Duckle-macos-arm64` | Right-click the app icon -> **"Open"** to register a Gatekeeper bypass. |
| **Linux** (x86_64) | `Duckle-linux-x64` | `chmod +x Duckle-linux-x64 && ./Duckle-linux-x64` | Requires WebKitGTK 4.1 (`libwebkit2gtk-4.1-0` on Debian/Ubuntu). |

---

## 2. First-Launch Guided Setup

When you open Duckle for the first time, you will be prompted with a setup modal to install execution engines:

1. **DuckDB CLI (Required)**
   * **Size**: ~30 MB (plus extension libraries).
   * **Role**: Powers the SQL compilation, schema inference, local database tables, and cloud connectors (`httpfs` for S3/GCS).
   * **Setup Time**: ~30 seconds.
2. **Duckie AI Assistant (Optional)**
   * **Size**: ~1.1 GB.
   * **Role**: Downloads the **Qwen 2.5 Coder 1.5B** GGUF model and a compiled **llama-server** binary. This lets you generate pipelines using plain English without external APIs.
   * **Setup Time**: 5–10 minutes depending on internet connection speed.

### Installation Directory
Both engines are saved locally under your platform's app-data directory:
* **Windows**: `%APPDATA%\io.duckle.app\engines\`
* **macOS**: `~/Library/Application Support/io.duckle.app/engines/`
* **Linux**: `~/.config/io.duckle.app/engines/`

> [!TIP]
> If you need to force a fresh install of the engines, simply close the app, delete the `engines/` directory, and restart Duckle.

---

## 3. Selecting a Workspace

After engine setup, Duckle will ask you to select or create a **Workspace Folder** on your local drive. 

A workspace in Duckle is just a plain folder on your machine. Everything you build is stored in a clean, human-readable file structure:

```text
my-workspace/
├── pipelines/
│   ├── orders_etl.pipeline.json     # Node graph layout & properties
│   └── nightly_load.pipeline.json
├── connections/
│   ├── prod-postgres.connection.json # Saved credentials (values encrypted)
│   └── snowflake.connection.json
├── contexts/
│   ├── dev.context.json             # Variables for Dev environment
│   └── prod.context.json
├── routines/
│   └── cleanse-addresses.sql        # Reusable SQL snippets
├── documents/
│   └── runbook.md                   # Markdown notes and run instructions
├── schedules.json                   # Scheduled trigger configurations
└── run-history/
    └── orders_etl/
        └── 2026-05-25T14-30-00.json # Detailed execution report & row counts
```

> [!IMPORTANT]
> Because workspaces are plain directories, you can open them in any standard IDE (like VS Code), manage them under Git, track differences between commits, and push them to GitHub or GitLab.

---

## 4. Building from Source

If you want to build the Duckle desktop application yourself:

### Prerequisites
* **Rust compiler** (Stable toolchain) -> [rustup.rs](https://rustup.rs/)
* **Node.js** (v18+) and **npm** -> [nodejs.org](https://nodejs.org/)
* **cargo-tauri CLI**: `cargo install tauri-cli --version "^2"`
* Platform-specific Webview library dependencies (WebView2 on Windows, WebKitGTK on Linux).

### Steps
1. Clone the repository:
   ```bash
   git clone https://github.com/SouravRoy-ETL/duckle
   cd duckle
   ```
2. Install frontend dependencies:
   ```bash
   npm --prefix frontend install
   ```
3. Run the development server (runs hot-reloading frontend inside Tauri):
   ```bash
   cargo tauri dev
   ```
4. Compile production releases:
   ```bash
   cargo tauri build
   ```
