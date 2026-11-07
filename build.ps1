# PowerShell Universal Build Script for Tailscale KVM
$ErrorActionPreference = "Stop"

$repoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $repoDir

Write-Host "=== Building Tailscale Universal KVM ===" -ForegroundColor Cyan
cargo build --release --workspace

Write-Host "`nBuild complete! Executables available in target\release\:" -ForegroundColor Green
Write-Host " - target\release\tailscale-kvm-host.exe (Universal Host Controller)"
Write-Host " - target\release\tailscale-kvm-receiver.exe (Universal Receiver Daemon)"
