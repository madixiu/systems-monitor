# Systems Monitor

Systems Monitor collects hardware, operating system, storage, network, and installed-software inventory from Windows and Linux computers. A central service stores reports in PostgreSQL and provides a web dashboard for viewing and managing monitored systems.

## Components

- **Agent (`agent/`)** — collects system information, enrolls with the server, sends periodic heartbeats, and submits inventory reports. Linux agents can run as a foreground process or systemd service; Windows agents support running as a Windows service or in console mode.
- **Server (`server/`)** — Rust HTTP API backed by PostgreSQL. It handles agent enrollment and reports, dashboard requests, API-key authentication, live event streaming, and data exports. Database migrations run when the server starts.
- **Dashboard (`dashboard/`)** — React application for browsing system inventory, monitoring agent status, requesting refreshes, managing maintenance state and notes, and exporting data.

## Requirements

- Rust toolchain with Cargo
- Node.js and npm
- PostgreSQL
- Windows or Linux for the corresponding agent build
- A reverse proxy such as nginx for production deployments

## Build

Build the server:

```sh
cd server
cargo build --release
```

Build the agent for the current host platform:

```sh
cd agent
cargo build --release
```

To cross-compile, install the Rust target and any required platform linker/toolchain, then pass the target triple to Cargo:

```sh
cargo build --release --target <rust-target-triple>
```

Build the dashboard:

```sh
cd dashboard
npm ci
npm run build
```

The compiled dashboard is written to `dashboard/dist/`.

## Configuration and local development

### Server

Create `server/server.toml` from `server/server.toml.sample`. Set the listener address, PostgreSQL connection URL, and log level. Keep database credentials out of source control.

The server looks for `server.toml` in its working directory and then beside the executable. Start PostgreSQL, then run the server from the `server/` directory:

```sh
cargo run
```

The server applies database migrations during startup. The health endpoint is available at `/health`.

### Agent

Create an `agent.toml` in the agent's platform configuration directory, using this structure:

```toml
[server]
url = "https://<server-host>"
enrollment_secret = "<shared-enrollment-secret>"

[interval]
heartbeat_secs = 180
report_secs = 86400

[log]
level = "info"
```

Set the server URL to the address reachable by the monitored computers. Use the same enrollment secret configured on the server. Keep this secret private and do not commit it.

On Linux, build and run the agent with:

```sh
cd agent
cargo run --release
```

On Windows, run the installed service for normal operation. To run interactively for troubleshooting, pass `--console` to the agent executable.

### Dashboard

Configure the dashboard's `VITE_API_URL` environment variable to the server API origin. For local development, for example:

```sh
cd dashboard
VITE_API_URL=http://127.0.0.1:8080 npm run dev
```

In production, serve the built dashboard and route API requests through a reverse proxy to the server. Keep the dashboard and API on the same origin when practical.

## Deployment

For production, run the server as a managed service, use PostgreSQL with appropriate backups, and place a reverse proxy in front of the API and dashboard. Bind the server to loopback when the proxy is on the same host, and configure TLS at the proxy. Restrict network access to only the required services.

Install agents as managed services so they start automatically and can recover after a restart. On Windows, the included deployment script can be used with a computer startup policy to distribute agent updates. Keep the agent's local `state.json` when updating the executable or configuration; it contains the existing enrollment identity. Test updates on a small group of computers before a broad rollout.

See the deployment guides for detailed platform-specific procedures:

- [Server and dashboard deployment](./deploy.md)
- [Linux agent deployment](./deploy_linux.md)
- [Windows agent update rollout](./AGENT-UPDATE.md)

## Security notes

- Use strong, unique secrets and API keys. Do not commit credentials or populated configuration files.
- Use HTTPS for agent-to-server and dashboard-to-server traffic outside trusted local development.
- Bind the server to a private interface or loopback when it is behind a reverse proxy; do not expose its application port unnecessarily.
- Restrict access to configuration files containing database credentials, API keys, or the enrollment secret.
- Preserve agent state during upgrades to avoid unnecessary re-enrollment.
