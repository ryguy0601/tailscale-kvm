<#
.SYNOPSIS
    Single unified launcher for Tailscale Universal KVM.
.DESCRIPTION
    Runs both the Receiver daemon (so this machine can receive remote control,
    clipboard, and files from any peer) AND the Host controller with the Web GUI
    (so this machine can also control any connected peer screens).
    
    Any computer running 'run-kvm' can be either a Host, a Receiver, or both!

.PARAMETER InstallStartup
    Registers Tailscale Universal KVM to run automatically on Windows startup.
.PARAMETER UninstallStartup
    Removes the Windows startup registration.
#>
[CmdletBinding()]
param (
    [Parameter(Position = 0)]
    [ValidateSet("all", "host", "receiver", "both")]
    [string]$Mode = "all",

    [switch]$InstallStartup,
    [switch]$UninstallStartup,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ExtraArgs
)

$ErrorActionPreference = "Stop"

$repoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $repoDir

$StartupFolder = [Environment]::GetFolderPath("Startup")
$ShortcutPath = Join-Path $StartupFolder "TailscaleUniversalKVM.lnk"

if ($UninstallStartup) {
    if (Test-Path $ShortcutPath) {
        Remove-Item -Path $ShortcutPath -Force
        Write-Host "[OK] Removed Tailscale Universal KVM from Windows Startup." -ForegroundColor Green
    } else {
        Write-Host "[*] No startup shortcut found to remove." -ForegroundColor Yellow
    }
    exit 0
}

if ($InstallStartup) {
    $WScriptShell = New-Object -ComObject WScript.Shell
    $Shortcut = $WScriptShell.CreateShortcut($ShortcutPath)
    $Shortcut.TargetPath = "powershell.exe"
    $Shortcut.Arguments = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$repoDir\run-kvm.ps1`""
    $Shortcut.WorkingDirectory = $repoDir
    $Shortcut.WindowStyle = 7
    $Shortcut.Description = "Tailscale Universal KVM Unified Node (Host & Receiver)"
    $Shortcut.Save()
    Write-Host "[OK] Tailscale Universal KVM registered to run automatically on startup!" -ForegroundColor Green
    Write-Host "     Shortcut: $ShortcutPath" -ForegroundColor Cyan
    exit 0
}

$hostExe = Join-Path $repoDir "target\release\tailscale-kvm-host.exe"
$recvExe = Join-Path $repoDir "target\release\tailscale-kvm-receiver.exe"

# Check if binaries are missing OR outdated compared to source code git HEAD
$needsBuild = $false
if ((-not (Test-Path $hostExe)) -or (-not (Test-Path $recvExe))) {
    $needsBuild = $true
} else {
    try {
        $gitCommitUnix = [int64](git log -1 --format="%ct" 2>$null)
        if ($gitCommitUnix -gt 0) {
            $commitDate = [DateTimeOffset]::FromUnixTimeSeconds($gitCommitUnix).UtcDateTime
            $hostDate = (Get-Item $hostExe).LastWriteTimeUtc
            $recvDate = (Get-Item $recvExe).LastWriteTimeUtc
            if ($hostDate -lt $commitDate -or $recvDate -lt $commitDate) {
                Write-Host "[*] Source code updated since last build. Recompiling..." -ForegroundColor Cyan
                $needsBuild = $true
            }
        }
    } catch {
        # Fallback if git is unavailable
    }
}

if ($needsBuild) {
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
