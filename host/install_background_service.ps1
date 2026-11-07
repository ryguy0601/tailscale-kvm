[CmdletBinding()]
param (
    [string]$OmarchyIP = "100.99.142.70"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Tailscale KVM: Windows Rust Host Background Installer  " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Stop any currently running instances so binary lock is released
Write-Host "[*] Checking for running instances..." -ForegroundColor Yellow
$Running = Get-Process -Name "tailscale-kvm-host" -ErrorAction SilentlyContinue
if ($Running) {
    $Pids = ($Running | Select-Object -ExpandProperty Id) -join ", "
    Write-Host "    Found active tailscale-kvm-host process (PID: $Pids). Stopping it..." -ForegroundColor DarkYellow
    Stop-Process -Name "tailscale-kvm-host" -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 700
    Write-Host "[+] Previous instance stopped." -ForegroundColor Green
} else {
    Write-Host "    No existing instance running." -ForegroundColor Gray
}

# Remove any stale pre-workspace binary so we never accidentally launch an old build
$StaleLocalExe = Join-Path $ScriptDir "target\release\tailscale-kvm-host.exe"
$WorkspaceCargo = Join-Path $RepoRoot "Cargo.toml"
if ((Test-Path $WorkspaceCargo) -and (Test-Path $StaleLocalExe)) {
    Remove-Item -Path $StaleLocalExe -Force -ErrorAction SilentlyContinue
}

# 2. Update config.txt
$ConfigFile = Join-Path $ScriptDir "config.txt"
Set-Content -Path $ConfigFile -Value "OMARCHY_IP=$OmarchyIP`nPORT=9876" -Encoding Ascii
Write-Host "[+] Configured target IP: $OmarchyIP" -ForegroundColor Green

# 3. Build release binary
Write-Host "[*] Building optimized lean release binary..." -ForegroundColor Yellow
Set-Location $ScriptDir
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "[!] Cargo build failed." -ForegroundColor Red
    exit 1
}

# Workspace builds output to ..\target\release\tailscale-kvm-host.exe
$WorkspaceExe = Join-Path $RepoRoot "target\release\tailscale-kvm-host.exe"
if (Test-Path $WorkspaceExe) {
    $ExePath = (Resolve-Path $WorkspaceExe).Path
} elseif (Test-Path $StaleLocalExe) {
    $ExePath = (Resolve-Path $StaleLocalExe).Path
} else {
    Write-Host "[!] Could not locate built tailscale-kvm-host.exe!" -ForegroundColor Red
    exit 1
}

$SizeKB = [math]::Round((Get-Item $ExePath).Length / 1KB, 1)
Write-Host "[+] Built binary: $ExePath ($SizeKB KB)" -ForegroundColor Green

# 4. Register in Windows Startup folder (shell:startup) and clean up any legacy shortcuts
$StartupFolder = [Environment]::GetFolderPath("Startup")
Get-ChildItem -Path $StartupFolder -Filter "*Tailscale*KVM*" -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue
$ShortcutPath = Join-Path $StartupFolder "TailscaleKVMRustHost.lnk"

$WScriptShell = New-Object -ComObject WScript.Shell
$Shortcut = $WScriptShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = $ExePath
$Shortcut.WorkingDirectory = $ScriptDir
$Shortcut.Description = "Tailscale KVM Rust Host Background Service"
$Shortcut.Save()

Write-Host "[+] Background startup shortcut registered:" -ForegroundColor Green
Write-Host "    $ShortcutPath" -ForegroundColor DarkCyan

# 5. Start the background process immediately
Write-Host "[*] Launching background process now..." -ForegroundColor Yellow
Start-Process -FilePath $ExePath -WorkingDirectory $ScriptDir

Write-Host ""
Write-Host "[OK] Tailscale KVM Rust Host is now running silently in the background!" -ForegroundColor Green
Write-Host "     - Drag a Firefox window or tab off the right edge -> opens right in Firefox on Omarchy" -ForegroundColor Gray
Write-Host "     - Drag file(s)/folder from Explorer/Desktop off the right edge -> saves to ~/Downloads on Omarchy" -ForegroundColor Gray
Write-Host "     - Or press Win + Right Arrow in Browser/Explorer | F12 to release cursor" -ForegroundColor Gray
