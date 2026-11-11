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

`heartbeat_secs` is configured on each agent and controls how often it contacts the server. A forced refresh is processed on the next heartbeat, not immediately. Set the server's offline threshold above the agent heartbeat interval to allow for network delays.

Agents observe uptime and console or remote sessions at each heartbeat. The dashboard retains the last user seen and marks the session as signed in, signed out, or unknown. Windows disconnected sessions are still signed in; Linux relies on sessions reported by `who`. Status is unknown when observation fails, the agent is offline, or an older agent has not sent session information. On first enrollment, status may remain unknown until the heartbeat following the first report. Changes between heartbeats may not be observed, and a user already cleared from the database cannot be recovered. Uptime is estimated from the most recent boot observation and stops advancing once the agent is offline.

On Linux, build and run the agent with:

```sh
cd agent
cargo run --release
```

On Windows, run the installed service for normal operation. To run interactively for troubleshooting, pass `--console` to the agent executable.

Windows full reports include present Plug and Play monitors (with EDID model, serial and native resolution) and imaging devices (which can include scanners and cameras). The report also includes printer queues visible to the agent's account, including network/shared queues, except queues Windows reports as offline via PrinterStatus. Printer queue status does not reliably establish whether the physical printer is connected; virtual queues may also appear. Network scanners not exposed as present Plug and Play imaging devices are not discovered. Per-user printer connections belonging to other accounts may not be visible to the service. Missing or failed collection is displayed as unavailable rather than an empty inventory. The inventory reflects the last full report, not a live device scan. Linux and older agents do not report peripherals.

The server stores searchable peripheral rows in `system_peripherals`, alongside the original full report. Migration 0006 backfills existing peripheral reports and migration 0007 clears stored peripheral data (including keyboards and mice, which are no longer collected); subsequent reports replace the rows for that system, including removing rows when inventory is unavailable. The dashboard can filter systems by peripheral type and case-insensitive name prefix; both criteria must match the same peripheral. The systems API supports the same filters using `peripheral_kind` (`printer`, `monitor`, or `imaging`) and `peripheral_name`. Results follow the existing systems-list pagination and dashboard limit.

### Dashboard

Configure the dashboard's `VITE_API_URL` environment variable to the server API origin. For local development, for example:

```sh
cd dashboard
VITE_API_URL=http://127.0.0.1:8080 npm run dev
```

In production, set `VITE_API_URL` to the browser-accessible API origin when building the dashboard, then serve the build and route API requests through a reverse proxy to the server. Keep the dashboard and API on the same origin when practical. The current dashboard falls back to a configured default when `VITE_API_URL` is empty, so do not leave it empty for a deployment.

The dashboard's event stream is a persistent connection shared by updates from all agents. It sends a keepalive comment every 15 seconds when idle; heartbeat events arrive whenever any agent reports in, so the stream's event frequency is not the heartbeat interval of a single agent.

## Deployment

For production, run the server as a managed service, use PostgreSQL with appropriate backups, and place a reverse proxy in front of the API and dashboard. Bind the server to loopback when the proxy is on the same host, and configure TLS at the proxy. Restrict network access to only the required services.

Install agents as managed services so they start automatically and can recover after a restart. On Windows, the included deployment script can be used with a computer startup policy to distribute agent updates. Keep the agent's local `state.json` when updating the executable or configuration; it contains the existing enrollment identity. Test updates on a small group of computers before a broad rollout.

After an update, deploy the rebuilt server and dashboard and restart the server to apply any new migrations. Deploy updated agents separately; they read their heartbeat interval at startup.

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



## License

This project is licensed under the **GNU General Public License v3.0**.

See the [LICENSE](LICENSE) file for the full text.
