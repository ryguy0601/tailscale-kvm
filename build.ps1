# PowerShell Universal Build Script for Tailscale KVM
$ErrorActionPreference = "Stop"

$repoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $repoDir

Write-Host "=== Stopping any running Tailscale KVM processes before build ===" -ForegroundColor Yellow
Stop-Process -Name "tailscale-kvm-host" -Force -ErrorAction SilentlyContinue
Stop-Process -Name "tailscale-kvm-receiver" -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 250

Write-Host "=== Building Tailscale Universal KVM ===" -ForegroundColor Cyan
cargo build --release --workspace
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "`nBuild complete! Executables available in target\release\:" -ForegroundColor Green
Write-Host " - target\release\tailscale-kvm-host.exe (Universal Host Controller)"
Write-Host " - target\release\tailscale-kvm-receiver.exe (Universal Receiver Daemon)"
