# Updating Windows Agents Through Active Directory

This project deploys the Windows agent as the `PakMonitorAgent` service. The
standard rollout source is the NETLOGON share configured in
[agent/deploy-agent.ps1](./agent/deploy-agent.ps1).

## What the deployment script currently updates

The script compares both the source and installed executable and
`agent.toml` with SHA-256 hashes. When either file changes, it:

1. Stops `PakMonitorAgent`.
2. Copies each changed file.
3. Starts the service again.

Therefore:

- Changing only `pak-monitor-agent.exe` updates the executable and restarts the
  service.
- Changing only the NETLOGON `agent.toml` updates the local configuration and
  restarts the service.
- Changing both files updates both files and restarts the service once.
- `state.json` is not replaced. This preserves the enrolled agent ID and token.

## Recommended rollout for the current script

### 1. Build the new agent

Build the Windows release binary from the agent project:

```powershell
cd agent
cargo build --release --target x86_64-pc-windows-gnu
```

Copy the resulting binary to the NETLOGON deployment directory using the
filename expected by the script:

```text
\\<domain-controller>\NETLOGON\pak-monitor\pak-monitor-agent.exe
```

Keep the source `agent.toml` available there with the desired defaults, for
example:

```toml
[server]
url = "http://<monitor-server>"
enrollment_secret = "<shared-enrollment-secret>"

[interval]
heartbeat_secs = 180
report_secs = 86400

[log]
level = "info"
```

Do not commit real enrollment secrets to source control.

### 2. Update the executable through GPO

Use the existing computer startup GPO that runs:

```text
\\<domain-controller>\NETLOGON\pak-monitor\deploy-agent.ps1
```

On the next computer policy refresh or reboot, the script detects the changed
binary, restarts the service, and preserves each machine's `state.json`.

To trigger a policy refresh on a test machine:

```powershell
gpupdate /force
Get-Service -Name PakMonitorAgent
Get-Content C:\ProgramData\PakMonitor\deploy.log -Tail 30
```

### 3. Apply an `agent.toml` change

Publish the changed `agent.toml` beside the executable on NETLOGON, then run
the same computer startup GPO or `gpupdate /force`. The updated script copies
the config by hash and restarts the service. The agent reads `agent.toml` only
at startup.

Do not overwrite `state.json`; doing so would cause the machine to enroll
again and create a new server-side agent record.

## Rollout checklist

- [ ] Build and publish the new Windows executable with the expected filename.
- [ ] Verify the NETLOGON share is readable by computer accounts.
- [ ] Confirm the source `agent.toml` has the intended server URL and intervals.
- [ ] Test one workstation before linking or expanding the GPO scope.
- [ ] Confirm `PakMonitorAgent` is running after `gpupdate /force`.
- [ ] Confirm `deploy.log` records the update.
- [ ] Confirm `agent.log` shows the agent ready and a successful report.
- [ ] Confirm the existing server record remains associated with the same agent ID.
- [ ] Roll out to the remaining computers.

## Server migration reminder

The recent reliability changes add server migrations:

```text
server/migrations/0002_report_refresh_reliability.sql
server/migrations/0003_adjust_offline_threshold.sql
```

Apply the server migration before relying on refresh generations and the
24-hour server-side report interval. Updating only the Windows agent does not
apply server database changes.
