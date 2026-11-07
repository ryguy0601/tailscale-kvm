# Universal Host Launcher for Windows
$ErrorActionPreference = "Stop"

$repoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $repoDir

$exePath = Join-Path $repoDir "target\release\tailscale-kvm-host.exe"
if (-not (Test-Path $exePath)) {
    Write-Host "[*] Host binary not found. Building release binary first..." -ForegroundColor Cyan
    & "$repoDir\build.ps1"
} else {
    Stop-Process -Name "tailscale-kvm-host" -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 200
}

& $exePath @args
