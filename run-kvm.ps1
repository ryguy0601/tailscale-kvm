<#
.SYNOPSIS
    Single unified launcher for Tailscale Universal KVM.
.DESCRIPTION
    Runs both the Receiver daemon (so this machine can receive remote control,
    clipboard, and files from any peer) AND the Host controller with the Web GUI
    (so this machine can also control any connected peer screens).
    
    Any computer running 'run-kvm' can be either a Host, a Receiver, or both!

.PARAMETER Mode
    Optional override:
      'all' (default): Runs Receiver daemon in the background and launches Host controller in foreground.
      'host': Runs Host controller only.
      'receiver': Runs Receiver daemon only.
#>
[CmdletBinding()]
param (
    [Parameter(Position = 0)]
    [ValidateSet("all", "host", "receiver", "both")]
    [string]$Mode = "all",

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ExtraArgs
)

$ErrorActionPreference = "Stop"

$repoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $repoDir

$hostExe = Join-Path $repoDir "target\release\tailscale-kvm-host.exe"
$recvExe = Join-Path $repoDir "target\release\tailscale-kvm-receiver.exe"

# If binaries are missing, run the release build first
if ((-not (Test-Path $hostExe)) -or (-not (Test-Path $recvExe))) {
    Write-Host "[*] Release binaries not found. Building workspace first..." -ForegroundColor Cyan
    & "$repoDir\build.ps1"
}

# Cleanly stop any existing instances before starting fresh
Stop-Process -Name "tailscale-kvm-host" -Force -ErrorAction SilentlyContinue
Stop-Process -Name "tailscale-kvm-receiver" -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 250

if ($Mode -eq "receiver") {
    Write-Host "`n=== Launching Tailscale KVM Receiver Daemon ===" -ForegroundColor Green
    & $recvExe @ExtraArgs
    exit $LASTEXITCODE
}

if ($Mode -eq "host") {
    Write-Host "`n=== Launching Tailscale KVM Host Controller ===" -ForegroundColor Cyan
    & $hostExe @ExtraArgs
    exit $LASTEXITCODE
}

# Default: Mode = "all" / "both"
Write-Host "`n========================================================" -ForegroundColor Cyan
Write-Host "         Tailscale Universal KVM Unified Node          " -ForegroundColor White
Write-Host "  Any computer running this can be a Host or Receiver!  " -ForegroundColor Yellow
Write-Host "========================================================`n" -ForegroundColor Cyan

Write-Host "[1/2] Starting Background Receiver Daemon (Port 9876)..." -ForegroundColor Green
$recvProcess = Start-Process -FilePath $recvExe -ArgumentList $ExtraArgs -PassThru -WindowStyle Hidden

Register-EngineEvent -SourceIdentifier ([System.Guid]::NewGuid().ToString()) -Action {
    Stop-Process -Name "tailscale-kvm-receiver" -Force -ErrorAction SilentlyContinue
} | Out-Null

try {
    Write-Host "[2/2] Starting Host Controller & Web GUI (:9877)...`n" -ForegroundColor Cyan
    & $hostExe @ExtraArgs
} finally {
    Write-Host "`nShutting down KVM Receiver daemon..." -ForegroundColor Yellow
    if ($recvProcess -and -not $recvProcess.HasExited) {
        Stop-Process -Id $recvProcess.Id -Force -ErrorAction SilentlyContinue
    }
    Stop-Process -Name "tailscale-kvm-receiver" -Force -ErrorAction SilentlyContinue
}
