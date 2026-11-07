[CmdletBinding()]
param (
    [string]$OmarchyIP = "100.99.142.70"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Tailscale KVM: Windows Rust Host Background Installer  " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Update config.txt
$ConfigFile = Join-Path $ScriptDir "config.txt"
Set-Content -Path $ConfigFile -Value "OMARCHY_IP=$OmarchyIP`nPORT=9876" -Encoding Ascii
Write-Host "[+] Configured target IP: $OmarchyIP" -ForegroundColor Green

# 2. Build release binary
Write-Host "[*] Building release binary..." -ForegroundColor Yellow
Set-Location $ScriptDir
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "[!] Cargo build failed." -ForegroundColor Red
    exit 1
}

$ExePath = Join-Path $ScriptDir "target\release\tailscale-kvm-host.exe"
if (-not (Test-Path $ExePath)) {
    # Check parent workspace target directory
    $ExePath = Join-Path $ScriptDir "..\target\release\tailscale-kvm-host.exe"
}

# 3. Register in Windows Startup folder (shell:startup)
$StartupFolder = [Environment]::GetFolderPath("Startup")
$ShortcutPath = Join-Path $StartupFolder "TailscaleKVMRustHost.lnk"

$WScriptShell = New-Object -ComObject WScript.Shell
$Shortcut = $WScriptShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = (Resolve-Path $ExePath).Path
$Shortcut.WorkingDirectory = $ScriptDir
$Shortcut.Description = "Tailscale KVM Rust Host Background Service"
$Shortcut.Save()

Write-Host "[+] Background startup shortcut registered:" -ForegroundColor Green
Write-Host "    $ShortcutPath" -ForegroundColor DarkCyan

# 4. Start the background process immediately
Write-Host "[*] Launching background process now..." -ForegroundColor Yellow
Start-Process -FilePath $ExePath -WorkingDirectory $ScriptDir

Write-Host ""
Write-Host "[OK] Tailscale KVM Rust Host is now running silently in the background!" -ForegroundColor Green
Write-Host "     It will also start automatically on boot." -ForegroundColor Green
