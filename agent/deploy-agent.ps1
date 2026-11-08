$ErrorActionPreference = 'Stop'

$srcDir     = '\\pak.com\NETLOGON\pak-monitor'
$installDir = 'C:\ProgramData\PakMonitor'
$exeName    = 'pak-monitor-agent.exe'
$svcName    = 'PakMonitorAgent'

New-Item -ItemType Directory -Force -Path $installDir | Out-Null
$log = Join-Path $installDir 'deploy.log'
function Log($m) { "$(Get-Date -Format s) $m" | Add-Content $log }

try {
    $srcExe = Join-Path $srcDir $exeName
    $dstExe = Join-Path $installDir $exeName
    $srcCfg = Join-Path $srcDir 'agent.toml'
    $dstCfg = Join-Path $installDir 'agent.toml'

    if (-not (Test-Path $srcExe)) { Log "source not reachable: $srcExe"; exit 0 }

    $svc = Get-Service -Name $svcName -ErrorAction SilentlyContinue

    # Compare both centrally managed files by content hash.
    $needExeCopy = -not (Test-Path $dstExe) -or
                   ((Get-FileHash $srcExe).Hash -ne (Get-FileHash $dstExe).Hash)
    $needCfgCopy = (Test-Path $srcCfg) -and (
                       -not (Test-Path $dstCfg) -or
                       ((Get-FileHash $srcCfg).Hash -ne (Get-FileHash $dstCfg).Hash)
                   )

    if ($needExeCopy -or $needCfgCopy) {
        if ($svc -and $svc.Status -ne 'Stopped') {
            Stop-Service -Name $svcName -Force
            $svc.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(30))
        }

        if ($needExeCopy) {
            Copy-Item $srcExe $dstExe -Force
            Log "executable installed/updated"
        }

        if ($needCfgCopy) {
            Copy-Item $srcCfg $dstCfg -Force
            Log "config installed/updated"
        }
    }

    if (-not $svc) {
        New-Service -Name $svcName -BinaryPathName "`"$dstExe`"" `
            -DisplayName 'PAK Monitor Agent' `
            -Description 'Reports system telemetry to PAK Monitor server' `
            -StartupType Automatic | Out-Null
        sc.exe failure $svcName reset= 86400 actions= restart/60000/restart/60000/restart/60000 | Out-Null
        Log "service created"
    }

    if ((Get-Service -Name $svcName).Status -ne 'Running') {
        Start-Service -Name $svcName
        Log "service started"
    }
}
catch { Log "ERROR: $_" }
