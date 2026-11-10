Since the files are in `C:\ProgramData\PakMonitor`, the copy and service paths need to match. Here are the corrected commands.

## 1. Copy the files to ProgramData

smbclient reaches `C$` as the share root, so the path is `ProgramData\PakMonitor` inside it:

```bash
TARGET=PA140317
DOMAIN=PAK
USER_NAME=mor
PASS='YOUR_PASSWORD'
SRC=/home/moradi/Desktop/agent

ls -lh "$SRC"/pak-monitor-agent.exe "$SRC"/agent.toml

smbclient "//$TARGET/C\$" -U "$DOMAIN\\$USER_NAME%$PASS" -c "
    cd ProgramData;
    mkdir PakMonitor;
    cd PakMonitor;
    lcd $SRC;
    put pak-monitor-agent.exe;
    put agent.toml;
    ls;
"
```

Notes:
- `mkdir` fails with an error if the folder already exists. That's harmless, so you can ignore it.
- Use `-U "$DOMAIN\\$USER_NAME%$PASS"` instead of `--password=`. This keeps the password out of the process list and avoids the `YOUR_PASSWORD` placeholder being sent literally.


## 1.2 Create Service
```bash
wmiexec.py "$DOMAIN/$USER_NAME:$PASS@$TARGET" \
  'sc.exe create PakMonitor binPath= "C:\ProgramData\PakMonitor\pak-monitor-agent.exe" start= auto DisplayName= "PAK Monitor Agent"'
```
## 2. Service status

Your service was created with the correct path, so you only need to start it:

```bash
wmiexec.py "$DOMAIN/$USER_NAME:$PASS@$TARGET" 'sc.exe start PakMonitor'
wmiexec.py "$DOMAIN/$USER_NAME:$PASS@$TARGET" 'sc.exe query PakMonitor'
```

If you want the computer name in the service name, as in the earlier loop, create it again as `PakMonitor-PA140317`. Keep the same binary path.

## 3. Quick check that the file landed

```bash
wmiexec.py "$DOMAIN/$USER_NAME:$PASS@$TARGET" 'dir C:\ProgramData\PakMonitor'
```

## 4. Deploy loop with the new paths

```bash
#!/usr/bin/env bash
DOMAIN=PAK
USER_NAME=mor
PASS='YOUR_PASSWORD'
SRC=/home/moradi/Desktop/agent
BIN='C:\ProgramData\PakMonitor\pak-monitor-agent.exe'

while read -r H; do
  [ -z "$H" ] && continue
  SVC="PakMonitor-${H}"
  echo "=== $H ==="

  # 1. Copy files
  smbclient "//$H/C\$" -U "$DOMAIN\\$USER_NAME%$PASS" -c "
    cd ProgramData;
    mkdir PakMonitor;
    cd PakMonitor;
    lcd $SRC;
    put pak-monitor-agent.exe;
    put agent.toml;
  " || { echo "copy failed on $H"; continue; }

  # 2. Create service (error is expected if it already exists)
  wmiexec.py "$DOMAIN/$USER_NAME:$PASS@$H" \
    "sc.exe create $SVC binPath= \"$BIN\" start= auto DisplayName= \"PAK Monitor Agent\"" || true

  # 3. Start and verify
  wmiexec.py "$DOMAIN/$USER_NAME:$PASS@$H" "sc.exe start $SVC" || true
  wmiexec.py "$DOMAIN/$USER_NAME:$PASS@$H" "sc.exe query $SVC"
done < hosts.txt
```

Run it with `bash deploy.sh`, with one hostname per line in `hosts.txt`.

## Remember

The `sc.exe create` success only means the service was registered. Starting it can still fail with error 1053 if the exe isn't a real Windows service. If `sc.exe start` fails, check the status with `sc.exe query`, or wrap the agent with NSSM as described earlier.