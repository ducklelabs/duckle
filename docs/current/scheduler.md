# Scheduler & Automation

Duckle features a lightweight, background-ready orchestration engine (`duckle-scheduler`) that automates pipeline execution using cron schedules, time intervals, or file state changes.

---

## 1. Orchestration Model

The scheduler runs as an active thread inside the desktop application context, persisting configurations to a plain JSON file in the active workspace (`schedules.json`).

### The Polling Loop
* **Cadence**: A background poller wakes up every **15 seconds** (`TICK_INTERVAL`).
* **Due Check**: The poller evaluates all active schedules, checks the current time against `next_run_at`, and spawns execution threads for any tasks that are due.
* **Overrun Prevention**: When a schedule triggers, its `next_run_at` timestamp is advanced *immediately* before starting the pipeline execution. This "claims" the occurrence and ensures that a pipeline taking longer than 15 seconds is not re-triggered by subsequent ticks.

---

## 2. Trigger Types

Schedules are defined using one of three orchestrator patterns:

### Cron Triggers
* **Syntax**: Standard 5-field (minute, hour, day-of-month, month, day-of-week) or 6-field (including seconds) expressions.
* **Resolution**: Recomputed using the Rust `cron` library.
* **Example**: `0 2 * * *` runs a pipeline every night at 2:00 AM.

### Interval Triggers
* **Syntax**: Executes at a fixed frequency of seconds, minutes, hours, or days.
* **Resolution**: Recomputed by adding the target duration to the completion timestamp of the last successful run (`last_run_at`).
* **Example**: `every 15 minutes`.

### File-Watch Triggers
* **Syntax**: Watches a specified local directory or file path using the system's native event notifications.
* **Resolution**: Implemented using the `notify` and `notify-debouncer-mini` crates.
* **Debounce Window**: Aggregates and debounces file-system events for **2 seconds** (`WATCH_DEBOUNCE`). This prevents multiple write updates (such as during a slow file copy) from triggering redundant, overlapping pipeline runs.
* **Parameters**: Supports optional recursive directory scanning.

---

## 3. History & Bookkeeping

Each scheduled run records detailed metadata about execution results:

* **Persistence**: Updated records are written back to `workspace/schedules.json` and saved in the pipeline's run logs.
* **Log Entry Details**:
  * `last_run_at`: Timestamp indicating when the execution started.
  * `last_run_status`: Status outcome (e.g., `"success"`, `"failed"`).
  * `last_run_duration_ms`: Duration of the execution in milliseconds.
  * `last_run_error`: The error message if the run failed.
  * `next_run_at`: Next planned execution timestamp (cleared for File-Watch triggers).

---

## 4. Headless Execution (CLI Mode)

While schedules run automatically inside the desktop app, you may want to run pipelines on headless servers without keeping the GUI open. You can execute pipelines via your system's cron scheduler or systemd timers using the Duckle CLI:

```bash
duckle run --workspace /path/to/my-workspace --pipeline orders_etl
```

This runs the compiled SQL pipeline directly on the DuckDB engine in a headless environment, printing run summaries and writing execution logs to `<workspace>/logs/orders_etl/runtime.log`.
