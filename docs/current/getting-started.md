# Getting Started

This guide walks you through building your first visual pipeline, executing it, and using the built-in Duckie AI Assistant.

---

## Tutorial: Build a CSV Cleanup Pipeline

We will build a simple pipeline that reads a CSV file containing order data, filters out orders that are not paid, and saves the output to a Parquet file.

### Step 1: Add and Configure a CSV Source
1. Open the **Components** drawer on the left sidebar.
2. Click **Sources** -> **Files** -> **CSV** and drag the node onto the canvas.
3. Select the node to open the **Properties Panel** on the right.
4. Set the **Path** properties to a CSV file (e.g., `samples/orders.csv`).
5. Click the **Autodetect schema** button.
   * Duckle will scan a sample of the file, infer column names/types, and update the **Schema** tab.
   * The **Preview** tab will display the first few rows of the CSV.

### Step 2: Add a Filter Transform
1. Open the **Components** drawer.
2. Go to **Transforms** -> **Rows** -> **Filter** and drag it onto the canvas.
3. Drag a line from the CSV source's `main` output port to the Filter's `main` input port.
4. In the Filter node properties, set the **Predicate** to:
   ```sql
   status = 'paid'
   ```
   * *Note: The Filter node has two output ports: `pass` (rows matching your predicate) and `reject` (rows that failed the predicate).*

### Step 3: Add a Parquet Sink
1. Open the **Components** drawer.
2. Go to **Sinks** -> **Files** -> **Parquet** and drag it onto the canvas.
3. Wire the Filter's `pass` output port to the Parquet sink's input port.
4. In the Parquet properties:
   * **Path**: Set to `paid_orders.parquet`.
   * **Write Mode**: Select `overwrite`.
   * **Compression**: Choose `zstd`.

### Step 4: Run and Inspect the Pipeline
1. Click the **Run** button in the top toolbar.
2. The nodes will light up green stage-by-stage. A live row counter will appear under each edge indicating data flow.
3. Click any node after execution:
   * **Plan Tab**: Shows the exact, compile-time SQL query Duckle generated for that node.
   * **Preview Tab**: Shows a live tabular view of data at that step.

---

## Asking Duckie (AI Assistant)

Instead of dragging and wiring nodes manually, you can describe your pipeline in plain English and let **Duckie** build it for you.

1. Click the **Sparkles** icon in the top-right corner of the toolbar to open the AI Sidebar.
2. Type a natural language prompt, such as:
   > "read orders.csv, filter where status is paid, and write to paid.parquet"
3. Duckie will stream back a visual pipeline definition.
4. Click the **Insert into canvas** button. The canvas will immediately populate with the configured nodes, positioned and wired automatically.

*Duckie runs entirely locally on your CPU via `llama-server`. No telemetry or prompt content is sent to external networks.*

---

## Workspace Features

### Context Variables
If you need to deploy pipelines across multiple environments (e.g., development, staging, production), use **Context Variables**.

1. Create a context variable in your active context configuration (e.g., `S3_BUCKET = "my-dev-bucket"`).
2. Switch any property input from **Manual** to **Context** mode.
3. Pick your variable, or reference it directly in text inputs using `${S3_BUCKET}`.
4. When you switch context from the topbar dropdown, all references automatically resolve to the new values at runtime.

### Connection Manager
Rather than hardcoding credentials (such as PostgreSQL passwords or AWS access keys) in node properties:
1. Save credentials in the **Connection Manager** (accessed via the topbar key icon).
2. The credentials are encrypted using a workspace-local key.
3. Select the saved connection from the dropdown in a node properties panel.
4. At run time, Duckle registers these as DuckDB `SECRET` structures, keeping pipeline JSON files completely safe to check into Git.
