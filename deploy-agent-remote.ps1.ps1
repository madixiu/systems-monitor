<#
Deploy PAK Monitor Agent to a list of Windows machines.
Usage:
  .\deploy-agent-remote.ps1 -Computers @("IT-28","PAK-PC01") -Force
  .\deploy-agent-remote.ps1 -OU "OU=Workstations,DC=pak,DC=com" -Force
  .\deploy-agent-remote.ps1 -AllDomainComputers -Exclude @("PDC","DC02") -Force
#>

param(
    [Parameter(Mandatory = $false)]
    [string[]] $Computers,

    [Parameter(Mandatory = $false)]
    [string] $OU,

    [Parameter(Mandatory = $false)]
    [switch] $AllDomainComputers,

    [Parameter(Mandatory = $false)]
    [string[]] $Exclude = @(),

    [Parameter(Mandatory = $false)]
    [switch] $Force,   # force restart service even if already installed

    [Parameter(Mandatory = $false)]
    [string] $SharePath = "\\pak.com\netlogon\pak-monitor"
)

$ErrorActionPreference = 'Stop'

# ---- build target list ----
if ($AllDomainComputers) {
    Write-Host "Discovering all domain computers..." -ForegroundColor Cyan
    $Computers = (Get-ADComputer -Filter * -Properties Name).Name
} elseif ($OU) {
    Write-Host "Discovering computers in $OU..." -ForegroundColor Cyan
    $Computers = (Get-ADComputer -SearchBase $OU -Filter * -Properties Name).Name
}

if (-not $Computers -or $Computers.Count -eq 0) {
    Write-Error "No computers specified. Use -Computers, -OU, or -AllDomainComputers."
}

$Computers = $Computers | Where-Object { $_ -notin $Exclude } | Sort-Object -Unique
Write-Host "Targets: $($Computers.Count) machines" -ForegroundColor Cyan
Write-Host "Share:   $SharePath" -ForegroundColor Cyan
Write-Host ""

# ---- remote payload ----
$remoteScript = {
    param($SharePath, $Force)

    $installDir = 'C:\ProgramData\PakMonitor'
    $exeName    = 'pak-monitor-agent.exe'
    $svcName    = 'PakMonitorAgent'

    $result = [PSCustomObject]@{
        Host        = $env:COMPUTERNAME
        Installed   = $false
        ServiceOK   = $false
        Started     = $false
        Error       = $null
    }

    try {
        # 1. ensure share reachable
        if (-not (Test-Path "$SharePath\$exeName")) {
            throw "share not reachable: $SharePath"
        }

        # 2. create install dir
        New-Item -ItemType Directory -Force -Path $installDir | Out-Null

        # 3. copy exe if newer or missing
        $srcExe = Join-Path $SharePath $exeName
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
        $result.Installed = $true

        # 4. copy config only if missing (do NOT overwrite state.json / enrollment)
        $srcCfg = Join-Path $SharePath 'agent.toml'
        $dstCfg = Join-Path $installDir 'agent.toml'
        if (-not (Test-Path $dstCfg)) {
            Copy-Item $srcCfg $dstCfg -Force
        }

        # 5. register service if not present
        $svc = Get-Service -Name $svcName -ErrorAction SilentlyContinue
        if (-not $svc) {
            New-Service -Name $svcName `
                -BinaryPathName $dstExe `
                -DisplayName 'PAK Monitor Agent' `
                -Description 'Reports system telemetry to PAK Monitor server' `
                -StartupType Automatic | Out-Null
            $svc = Get-Service -Name $svcName
        }
        $result.ServiceOK = $true

        # 6. start or restart
        if ($Force -and $svc.Status -eq 'Running') {
            Restart-Service -Name $svcName -Force
            $result.Started = $true
        } elseif ($svc.Status -ne 'Running') {
            Start-Service -Name $svcName
            $result.Started = $true
        }
    } catch {
        $result.Error = $_.Exception.Message
    }

    $result
}

# ---- run remotely in parallel chunks ----
$results = @()
$chunkSize = 20    # how many machines at a time
$total = $Computers.Count
$done = 0

foreach ($chunk in ($Computers | ForEach-Object -Begin { $i=0 } -Process { 
    [PSCustomObject]@{ Name = $_; Idx = [math]::Floor($i / $chunkSize) }; $i++ 
} | Group-Object Idx | ForEach-Object { $_.Group.Name })) {
    # (loop body below — grouping trick above just batches)
}

# Simpler approach: process in chunks manually
for ($i = 0; $i -lt $total; $i += $chunkSize) {
    $batch = $Computers[$i..([math]::Min($i + $chunkSize - 1, $total - 1))]
    Write-Host "[$($i+1)-$($i+$batch.Count)/$total] Deploying..." -ForegroundColor Cyan

    $batchResults = Invoke-Command `
        -ComputerName $batch `
        -ScriptBlock $remoteScript `
        -ArgumentList $SharePath, $Force `
        -ErrorAction SilentlyContinue `
        -ThrottleLimit $chunkSize

    foreach ($r in $batchResults) {
        if ($r) { $results += $r }
    }

    # Also mark unreachable machines
    $reached = $batchResults | ForEach-Object { $_.Host }
    foreach ($c in $batch) {
        if ($c -notin $reached) {
            $results += [PSCustomObject]@{
                Host = $c; Installed = $false; ServiceOK = $false; Started = $false
                Error = "unreachable (WinRM or firewall)"
            }
        }
    }
}

# ---- summary ----
$ok    = $results | Where-Object { $_.ServiceOK -and $_.Started -and -not $_.Error }
$partial = $results | Where-Object { $_.ServiceOK -and $_.Error }
$failed  = $results | Where-Object { -not $_.ServiceOK }

Write-Host ""
Write-Host "========= SUMMARY =========" -ForegroundColor Cyan
Write-Host "Success:  $($ok.Count)" -ForegroundColor Green
Write-Host "Partial:  $($partial.Count)" -ForegroundColor Yellow
Write-Host "Failed:   $($failed.Count)" -ForegroundColor Red
Write-Host "===========================" -ForegroundColor Cyan

if ($failed.Count -gt 0) {
    Write-Host ""
    Write-Host "Failed machines:" -ForegroundColor Red
    $failed | Format-Table Host, Error -AutoSize
}

# Export full results
$outFile = "C:\pak-monitor-deploy-$(Get-Date -Format yyyyMMdd-HHmmss).csv"
$results | Export-Csv -Path $outFile -NoTypeInformation
Write-Host ""
Write-Host "Full log: $outFile" -ForegroundColor Cyan