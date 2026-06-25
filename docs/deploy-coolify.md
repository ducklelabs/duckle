# Deploy Duckle on a VPS with Coolify

[Coolify](https://coolify.io) is the open-source, self-hosted PaaS that powers
the one-click app stores on Hetzner, Hostinger, OVH and most other VPS
providers. This guide installs Duckle's web editor as a one-click Coolify app:
a prebuilt image (no heavy build on your server), an auto-assigned domain with
HTTPS, and persistent storage for your pipelines and data.

What you get: the full Duckle studio in the browser - build, run, and schedule
pipelines, with the DuckDB engine running inside the container. Local-first:
everything stays on your server.

## Prerequisites

- A VPS with Coolify already installed (the provider one-click image, or the
  Coolify install script). A 1 vCPU / 1 GB instance is enough to start.
- A domain or subdomain you can point at the server (optional - Coolify can give
  you a `sslip.io` URL to start with).

## Install (one click)

1. In Coolify: **+ New** -> **Resource** -> **Docker Compose**.
2. Choose either:
   - **Paste**: paste the contents of
     [`docker-compose.coolify.yml`](../docker-compose.coolify.yml), or
   - **From repository**: point Coolify at this repository and select
     `docker-compose.coolify.yml`.
3. (Optional) Under **Domains**, set your own domain; otherwise Coolify assigns
   one. HTTPS is provisioned automatically.
4. Click **Deploy**.

Coolify pulls `ghcr.io/slothflowlabs/duckle-web:latest`, starts it, waits for the
container healthcheck, and routes your domain to it. Open the URL and you are in
the Duckle studio.

## Using Duckle once it is live

Open the Coolify-assigned URL and you are in the full Duckle studio - the same
app as the desktop edition, just in your browser. A typical first session:

1. **Connect your data.** Anything reachable over the network works directly:
   Postgres, MySQL, SQL Server, Oracle, Snowflake, BigQuery, Redshift, S3 / GCS /
   Azure, REST APIs, Kafka, and so on - add a source node and fill in the
   connection. For local files (CSV, Parquet, Excel, ...), upload them into the
   workspace with the built-in file browser, or drop them on the server under the
   `duckle-workspace` volume; reference them with `${workspace}/yourfile.csv` so
   the pipeline stays portable.
2. **Build a pipeline** on the canvas: drag source -> transforms -> sink, wire
   them together, and configure each node in the properties panel.
3. **Run it.** Click Run; the DuckDB engine executes inside the container and you
   get live per-node progress (over SSE), row counts, and a result preview.
   Run-to-here lets you execute up to a single node while iterating.
4. **Schedule it.** The built-in interval scheduler runs saved pipelines on a
   cadence; the management console (Operations / Runs) shows history and logs.
5. **Everything persists.** Pipelines, connections, dives and run history live in
   the `duckle-workspace` volume on your server, so they survive redeploys and
   image updates.

Because it is single-tenant with no built-in login, treat the URL as admin
access - keep Basic Auth on (see Security) if it is reachable from the internet.

## How it works

The compose is intentionally tiny:

```yaml
services:
  duckle:
    image: ghcr.io/slothflowlabs/duckle-web:latest
    environment:
      - SERVICE_FQDN_DUCKLE_8080   # Coolify generates the URL + wires its proxy to :8080
    expose:
      - "8080"
    volumes:
      - duckle-workspace:/workspace
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "curl -fsS http://localhost:8080/ >/dev/null 2>&1 || exit 1"]
      interval: 30s
      timeout: 5s
      retries: 5
      start_period: 30s
volumes:
  duckle-workspace:
```

- `SERVICE_FQDN_DUCKLE_8080` is a Coolify "magic" variable: Coolify fills it in,
  generates the public URL, terminates HTTPS at its proxy, and forwards to the
  container's port 8080.
- `duckle-workspace` is a managed volume mounted at `/workspace` - your
  pipelines, connections and data survive redeploys and image updates.
- The healthcheck lets Coolify show the app as healthy only once the editor is
  actually serving.

## Security

Duckle's web editor is single-tenant and has no built-in login - anyone who can
reach the URL has full access to the workspace and can run pipelines. Before
exposing it publicly:

- Turn on **Basic Auth** in Coolify for this resource, or put an auth
  middleware (Authelia, Authentik, Cloudflare Access) in front, and/or
- Restrict access by IP / keep it on a private network or VPN.

## Updating

- **Pin a version** by replacing `:latest` with a release tag in the compose,
  e.g. `image: ghcr.io/slothflowlabs/duckle-web:v0.5.2`, then Redeploy.
- **Stay on latest** by clicking **Redeploy** (or enabling Coolify's automatic
  updates) to pull the newest image. Your `duckle-workspace` volume is kept.

## Feature parity with the desktop app

The web image runs the same frontend and the same engine as the desktop app, so
the studio you get in the browser is the studio you get on the desktop: the
drag-and-drop canvas, dives + dashboards, the column-lineage viewer, the
data-quality / governance nodes, and the AI transforms are all there. The image
also bundles the runtime pieces those features need:

- the DuckDB engine + DuckDB CLI (DuckDB, DuckLake, files, REST, cloud
  warehouses, etc.),
- the LanceDB / Vortex sidecar (`src.lancedb` / `snk.lancedb`, `src.vortex` /
  `snk.vortex`), and
- a Python 3 interpreter for the `code.python` UDF.

Not bundled (keeps the image small; opt in if you need them): the local AI-chat
model and the dbt Fusion runtime, which the desktop app downloads on first use.

Plain Docker (without Coolify) works too:

```
docker run -p 8080:8080 -v duckle-workspace:/workspace ghcr.io/slothflowlabs/duckle-web:latest
```
