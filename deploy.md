# Final deployment — port 80, single nginx config, systemd

**Yes, port 80-only is fine.** Bind the backend to `127.0.0.1:8080` (loopback, not reachable from LAN), nginx owns port 80 and proxies both frontend + API. When you add HTTPS later, you just add a second `listen 443 ssl` block — nothing else changes.

---

## 1. Pre-flight

On the Debian server:

```bash
# nginx installed?
nginx -v 2>&1

# postgres running?
sudo systemctl status postgresql --no-pager | head -3

# current server still running on 8080?
ss -tlnp | grep 8080

# free port 80?
ss -tlnp | grep ':80 '
```

If nginx isn't installed:

```bash
sudo apt install nginx
```

Then disable the default site so it doesn't clash:

```bash
sudo rm -f /etc/nginx/sites-enabled/default
```

---

## 2. Build both (on your Manjaro box)

### Backend

```bash
cd ~/Desktop/systems-monitor/server
cargo build --release
ls -lh target/release/pak-monitor-server
```

### Frontend

First, one small `api.js` fix so prod uses relative URLs behind nginx. Edit `dashboard/src/api.js`, **replace** this line:

```js
export const API_URL = import.meta.env.VITE_API_URL || 'http://172.16.235.235:8080'
```

with:

```js
export const API_URL = import.meta.env.VITE_API_URL ?? 'http://172.16.235.235:8080'
```

(`||` → `??` — so an **empty** env var stays empty and produces relative paths.)

Create `dashboard/.env.production`:

```bash
cat > ~/Desktop/systems-monitor/dashboard/.env.production <<'EOF'
VITE_API_URL=
EOF
```

Empty on purpose. Behind nginx, `fetch('/api/v1/systems')` hits the same origin. `.env.local` (with the full URL) still applies during `npm run dev`.

Then:

```bash
cd ~/Desktop/systems-monitor/dashboard
npm run build
ls -lh dist/
# → dist/index.html, dist/assets/...
```

---

## 3. File layout on Debian

```bash
# --- backend ---
sudo mkdir -p /opt/pak-monitor
sudo cp ~/Desktop/systems-monitor/server/target/release/pak-monitor-server /opt/pak-monitor/
sudo chmod 755 /opt/pak-monitor/pak-monitor-server
sudo chown root:root /opt/pak-monitor/pak-monitor-server

# server.toml — contains DB password, so 640 root:pak
sudo cp ~/Desktop/systems-monitor/server/server.toml /opt/pak-monitor/
sudo chown root:pak /opt/pak-monitor/server.toml
sudo chmod 640 /opt/pak-monitor/server.toml

# --- frontend ---
sudo mkdir -p /var/www/pak-monitor
sudo cp -r ~/Desktop/systems-monitor/dashboard/dist/* /var/www/pak-monitor/
sudo chown -R www-data:www-data /var/www/pak-monitor
sudo chmod -R 755 /var/www/pak-monitor
```

**Critical:** change `server.toml` to bind loopback only:

```bash
sudo nano /opt/pak-monitor/server.toml
```

Set:

```toml
bind = "127.0.0.1:8080"
database_url = "postgres://pak:changeme@localhost/pak_monitor"
log_level = "info"
```

That's the whole security win: backend listens only on `127.0.0.1`, so nothing on the LAN can hit it directly. All external traffic goes through nginx on 80.

---

## 4. systemd unit

```bash
sudo tee /etc/systemd/system/pak-monitor.service > /dev/null <<'EOF'
[Unit]
Description=PAK Monitor Server
After=network-online.target postgresql.service
Wants=network-online.target postgresql.service

[Service]
Type=simple
User=pak
Group=pak
WorkingDirectory=/opt/pak-monitor
ExecStart=/opt/pak-monitor/pak-monitor-server
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal

# hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/pak-monitor
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
LockPersonality=true
RestrictRealtime=true

[Install]
WantedBy=multi-user.target
EOF
```

Stop the manual instance first, then enable:

```bash
# kill any manual run
pkill -f pak-monitor-server || true

sudo systemctl daemon-reload
sudo systemctl enable --now pak-monitor
sudo systemctl status pak-monitor --no-pager
```

Expected: `Active: active (running)`.

Check logs:

```bash
sudo journalctl -u pak-monitor -n 30 --no-pager
```

You should see `INFO starting…`, `INFO database connected`, `INFO migrations applied`, `INFO listening on 127.0.0.1:8080`. **No** API-key block (DB already populated).

---

## 5. nginx — one single file, no includes except mime

Replace `/etc/nginx/nginx.conf` entirely:

```bash
sudo tee /etc/nginx/nginx.conf > /dev/null <<'EOF'
user  www-data;
worker_processes  auto;
pid   /run/nginx.pid;

events {
    worker_connections  2048;
    multi_accept        on;
}

http {
    # --- basics ---
    sendfile            on;
    tcp_nopush          on;
    tcp_nodelay         on;
    keepalive_timeout   65;
    types_hash_max_size 2048;
    server_tokens       off;

    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    # --- logging ---
    access_log /var/log/nginx/access.log;
    error_log  /var/log/nginx/error.log warn;

    # --- compression ---
    gzip on;
    gzip_vary on;
    gzip_min_length 1000;
    gzip_types
        text/plain text/css text/javascript
        application/json application/javascript application/xml
        application/vnd.openxmlformats-officedocument.spreadsheetml.sheet
        image/svg+xml;

    # --- uploads ---
    client_max_body_size 10m;
    client_body_timeout  30s;
    send_timeout         30s;

    # ============================================================
    # PAK Monitor — dashboard + API on port 80
    # ============================================================
    server {
        listen      80 default_server;
        listen      [::]:80 default_server;
        server_name _;

        # --- SPA static ---
        root  /var/www/pak-monitor;
        index index.html;

        location / {
            try_files $uri $uri/ /index.html;
        }

        # --- SSE stream (must come before /api/) ---
        location = /api/v1/stream {
            proxy_pass         http://127.0.0.1:8080;
            proxy_http_version 1.1;
            proxy_set_header   Host              $host;
            proxy_set_header   X-Real-IP         $remote_addr;
            proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
            proxy_set_header   X-Forwarded-Proto $scheme;
            proxy_set_header   Connection        "";
            proxy_buffering    off;
            proxy_cache        off;
            proxy_read_timeout 24h;
            chunked_transfer_encoding on;
        }

        # --- API (agents + dashboard + export) ---
        location /api/ {
            proxy_pass         http://127.0.0.1:8080;
            proxy_http_version 1.1;
            proxy_set_header   Host              $host;
            proxy_set_header   X-Real-IP         $remote_addr;
            proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
            proxy_set_header   X-Forwarded-Proto $scheme;
            proxy_set_header   Connection        "";
            proxy_buffering    off;
            proxy_read_timeout 120s;
            proxy_connect_timeout 10s;
        }

        # --- health ---
        location = /health {
            proxy_pass http://127.0.0.1:8080/health;
            access_log off;
        }

        # --- security headers ---
        add_header X-Content-Type-Options "nosniff"      always;
        add_header X-Frame-Options        "SAMEORIGIN"   always;
        add_header Referrer-Policy        "no-referrer"  always;

        # --- static caching ---
        location ~* \.(js|css|woff2?|svg|png|jpg|jpeg|gif|ico)$ {
            expires 7d;
            add_header Cache-Control "public, immutable";
        }
    }
}
EOF
```

Key points:
- **Single file**, no `sites-enabled/` split
- `/api/v1/stream` block listed **before** `/api/` so SSE gets the special timeout
- `/api/v1/agent/*` (agent traffic) also passes through `/api/` — no extra config needed
- `proxy_buffering off` on `/api/` — important for SSE and for large JSON responses
- `client_max_body_size 10m` — agents send ~20 KB, but with 500+ apps someone could hit ~100 KB; 10 MB is way more than enough

Test and reload:

```bash
sudo nginx -t
# → syntax is ok / test is successful

sudo systemctl reload nginx
sudo systemctl status nginx --no-pager | head -5
```

---

## 6. Firewall

Only port 80 for the LAN, SSH for you:

```bash
sudo ufw allow from 172.16.0.0/16 to any port 80 proto tcp
sudo ufw allow from 172.16.0.0/16 to any port 22 proto tcp
sudo ufw --force enable
sudo ufw status verbose
```

**Important:** remove the previous 8080 rule if it exists:

```bash
sudo ufw status numbered
# find the line with 8080, then:
sudo ufw delete <N>
```

Now 8080 is inaccessible from LAN — only nginx (on the same box) can reach it.

---

## 7. Update the test agent config

`C:\ProgramData\PakMonitor\agent.toml` on the Windows test machine — **remove `:8080`**:

```toml
[server]
url = "http://172.16.235.235"
enrollment_secret = "c5fd8dc4fea225a2fc31e924bd66426ac5c266255ab7ee3f3c3777d284eeb09f"

[interval]
heartbeat_secs = 180
report_secs = 86400

[log]
level = "info"
```

Restart the agent (`--console` for now):

```powershell
C:\pak-monitor-agent.exe --console
```

Expected: `report sent` within 5 seconds. Server journal shows `INFO ingested`.

---

## 8. Verify the full stack

From any LAN machine:

```bash
# dashboard
curl -s -o /dev/null -w "%{http_code}\n" http://172.16.235.235/
# → 200

# health through nginx
curl -s http://172.16.235.235/health
# → ok

# API through nginx (same as before, no port)
KEY='b79022b68a526d75e6540d68e9b28e26487c605bc61c45868bfccdbdd4b17f63'
curl -s -H "X-API-Key: $KEY" http://172.16.235.235/api/v1/stats | jq
# → {"online":1,...}

# SSE through nginx
curl -N "http://172.16.235.235/api/v1/stream?key=$KEY"
# → stream opens, keepalives every 15s

# 8080 should now REFUSE from LAN
curl -s --max-time 3 http://172.16.235.235:8080/health
# → curl: (7) Failed to connect
```

Open `http://172.16.235.235/` in a browser → login with API key → table loads → click row → drawer opens → force refresh works → SSE flashes the row on the next ingest. If all that works, the stack is deployed.

---

## 9. Agent deployment via GPO (400 machines)

### Folder structure on the DC

```
\\<dc>\netlogon\pak-monitor\
├── pak-monitor-agent.exe
├── agent.toml
└── deploy-agent.ps1
```

`agent.toml` — same for every machine (shared enrollment secret):

```toml
[server]
url = "http://172.16.235.235"
enrollment_secret = "c5fd8dc4fea225a2fc31e924bd66426ac5c266255ab7ee3f3c3777d284eeb09f"

[interval]
heartbeat_secs = 180
report_secs = 86400

[log]
level = "info"
```

`deploy-agent.ps1`:

```powershell
$ErrorActionPreference = 'Stop'

$srcDir     = '\\<dc>\netlogon\pak-monitor'
$installDir = 'C:\ProgramData\PakMonitor'
$exeName    = 'pak-monitor-agent.exe'
$svcName    = 'PakMonitorAgent'

# 1. ensure install dir
New-Item -ItemType Directory -Force -Path $installDir | Out-Null

# 2. copy/update exe if newer or missing
$srcExe = Join-Path $srcDir $exeName
$dstExe = Join-Path $installDir $exeName
$needCopy = $true
if (Test-Path $dstExe) {
    if ((Get-Item $srcExe).LastWriteTime -le (Get-Item $dstExe).LastWriteTime) {
        $needCopy = $false
    }
}
if ($needCopy) {
    Copy-Item $srcExe $dstExe -Force
}

# 3. copy config only if missing (preserve enrollment + state.json)
$srcCfg = Join-Path $srcDir 'agent.toml'
$dstCfg = Join-Path $installDir 'agent.toml'
if (-not (Test-Path $dstCfg)) {
    Copy-Item $srcCfg $dstCfg -Force
}

# 4. register service
$svc = Get-Service -Name $svcName -ErrorAction SilentlyContinue
if (-not $svc) {
    New-Service -Name $svcName `
        -BinaryPathName $dstExe `
        -DisplayName 'PAK Monitor Agent' `
        -Description 'Reports system telemetry to PAK Monitor server' `
        -StartupType Automatic
}

# 5. ensure running
$svc = Get-Service -Name $svcName
if ($svc.Status -ne 'Running') {
    Start-Service -Name $svcName
}
```

### AD Group Policy

1. **Copy the three files** to `\\<dc>\netlogon\pak-monitor\`
2. **Group Policy Management Console** → new GPO "PAK Monitor Agent" → link at the OU containing your 400 computer accounts
3. **Computer Configuration** → Policies → Windows Settings → Scripts (Startup/Shutdown) → Startup → Add:
   - Script Name: `deploy-agent.ps1`  
   - Script Parameters: *(none)*
   - Wait: **No**
4. **Computer Configuration** → Policies → Windows Settings → Security Settings → Local Policies → **User Rights Assignment** → `Log on as a service` — ensure `NT AUTHORITY\SYSTEM` is present (it is by default)
5. Apply: on any target machine run `gpupdate /force` then reboot. Startup scripts fire at boot as SYSTEM.

### Verify after rollout

After 30–60 min the dashboard should show machines coming online. Check by querying:

```bash
curl -s -H "X-API-Key: $KEY" http://172.16.235.235/api/v1/agents | jq '.items | length'
# → number grows as machines enroll
```

Any machine that fails appears in `C:\ProgramData\PakMonitor\agent.log` on that machine. Use SCCM/RMM or `Invoke-Command` to fetch logs at scale if needed.

**Enrollment secret exposure note:** `\\<dc>\netlogon\` is readable by `Domain Users` (standard share permissions). If that's a concern, put the files on a **custom share** with:
- Read: `Domain Computers`
- Read: `Domain Admins`
- Nothing for `Domain Users`

Then reference the UNC path in the PS1. This matters if a compromised user account could exfiltrate the enrollment secret.

---

## 10. Optional — HTTPS later (ready-to-drop)

When you have a cert, only **add** a second `server` block; nothing existing changes:

```nginx
server {
    listen      443 ssl http2 default_server;
    listen      [::]:443 ssl http2 default_server;
    server_name pak-monitor.pak.com;      # or the IP

    ssl_certificate     /etc/ssl/pak-monitor/fullchain.pem;
    ssl_certificate_key /etc/ssl/pak-monitor/privkey.pem;
    ssl_protocols       TLSv1.2 TLSv1.3;
    ssl_ciphers         HIGH:!aNULL:!MD5;
    ssl_session_cache   shared:SSL:10m;
    ssl_session_timeout 10m;

    # same location blocks as port 80 server, copy verbatim:
    root  /var/www/pak-monitor;
    index index.html;
    location /                { try_files $uri $uri/ /index.html; }
    location = /api/v1/stream { proxy_pass http://127.0.0.1:8080; proxy_http_version 1.1; proxy_buffering off; proxy_read_timeout 24h; proxy_set_header Host $host; }
    location /api/            { proxy_pass http://127.0.0.1:8080; proxy_http_version 1.1; proxy_set_header Host $host; proxy_set_header X-Forwarded-Proto $scheme; proxy_buffering off; }
    location = /health        { proxy_pass http://127.0.0.1:8080/health; access_log off; }
}
```

And in the port 80 block, add a permanent redirect from 80 to 443:
```nginx
location / { return 301 https://$host$request_uri; }
```

Then open 443 in ufw and switch `agent.toml` to `url = "https://pak-monitor.pak.com"`. **Nothing in the backend, dashboard, or agent code changes** — that's the point of the proxy-first design.

---

## Post-deploy checklist

```
[ ] systemctl status pak-monitor        → active (running)
[ ] systemctl status nginx              → active (running)
[ ] curl http://<server>/               → 200, dashboard HTML
[ ] curl http://<server>/health         → ok
[ ] curl http://<server>:8080/health    → connection refused (backend not exposed)
[ ] Browser loads dashboard, login works
[ ] Live SSE flashes a row when agent reports
[ ] Force refresh from drawer → row updates in ≤30s
[ ] Maintenance toggle → dot turns amber
[ ] Note posted → appears in tab
[ ] Calendar toggle → Jalali timestamps
[ ] Theme toggle → light theme
[ ] ufw status → only 80 + 22 open
[ ] journalctl -u pak-monitor → no errors
[ ] agent.toml on test machine → url without :8080
[ ] Agent restart → still reaches server
```

If every box is ticked, you're done with the test machine. Then push the GPO and watch the table fill up over the next hour.

Paste the outputs of `systemctl status`, `nginx -t`, and one `curl` from another machine — I'll confirm everything is wired correctly before you roll to 400.