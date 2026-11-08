# Build the Linux agent
```bash
cd ~/Desktop/systems-monitor/agent
rustup target add x86_64-unknown-linux-gnu  # usually already installed on Linux
cargo build --release
# → target/release/pak-monitor-agent
```
### 1. create config dir
`sudo mkdir -p /etc/pak-monitor`

###  2. copy the binary and config
```bash
sudo cp pak-monitor-agent /usr/local/bin/
```
```bash
sudo cp agent.toml /etc/pak-monitor/agent.toml
```

###  3. edit config
```bash
sudo nano /etc/pak-monitor/agent.toml
```
 ```
url = "http://192.168.12.200"
 enrollment_secret = "c5fd8dc4..."
```

###  4. run once to enroll + verify
```bash
sudo pak-monitor-agent  
```
##### *Ctrl+C after 10s if it enrolls cleanly*


### 5. make it a systemd service
```bash
sudo tee /etc/systemd/system/pak-monitor-agent.service > /dev/null <<'EOF'
[Unit]
Description=PAK Monitor Agent
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
ExecStart=/usr/local/bin/pak-monitor-agent
Restart=on-failure
RestartSec=15
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
```
```bash
sudo systemctl daemon-reload
```
```bash
sudo systemctl enable --now pak-monitor-agent
```
```bash
sudo systemctl status pak-monitor-agent
```
