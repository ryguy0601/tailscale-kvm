[CmdletBinding()]
param (
    [string]$TargetIP = ""
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Tailscale KVM: Windows Host Startup Service Installer  " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

Write-Host "[*] Stopping any running tailscale-kvm-host instances..." -ForegroundColor Yellow
Stop-Process -Name "tailscale-kvm-host" -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 400

Set-Location $RepoRoot
Write-Host "[*] Building optimized release workspace..." -ForegroundColor Yellow
cargo build --release -p tailscale-kvm-host
if ($LASTEXITCODE -ne 0) {
    Write-Host "[!] Cargo build failed." -ForegroundColor Red
    exit 1
}

$ExePath = (Resolve-Path (Join-Path $RepoRoot "target\release\tailscale-kvm-host.exe")).Path

if ($TargetIP -ne "") {
    $LayoutFile = Join-Path $RepoRoot "kvm_layout.json"
    Set-Content -Path $LayoutFile -Value "{`n  `"right`": `"$TargetIP`",`n  `"left`": `"`",`n  `"up`": `"`",`n  `"down`": `"`"`n}" -Encoding UTF8
    Write-Host "[+] Seeded initial Right slot in kvm_layout.json -> $TargetIP" -ForegroundColor Green
}

$StartupFolder = [Environment]::GetFolderPath("Startup")
Get-ChildItem -Path $StartupFolder -Filter "*Tailscale*KVM*" -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue
$ShortcutPath = Join-Path $StartupFolder "TailscaleKVMRustHost.lnk"

$WScriptShell = New-Object -ComObject WScript.Shell
$Shortcut = $WScriptShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = "powershell.exe"
$Shortcut.Arguments = "-NoProfile -WindowStyle Hidden -Command `"Start-Process -FilePath '$ExePath' -ArgumentList '--no-browser' -WorkingDirectory '$RepoRoot' -WindowStyle Hidden`""
$Shortcut.WorkingDirectory = $RepoRoot
$Shortcut.WindowStyle = 7
$Shortcut.Description = "Tailscale Universal KVM Host Background Service"
$Shortcut.Save()

Write-Host "[+] Startup shortcut registered: $ShortcutPath" -ForegroundColor Green
Start-Process -FilePath $ExePath -WorkingDirectory $RepoRoot -WindowStyle Hidden
Write-Host "[OK] Tailscale KVM Host is running! Open http://localhost:9877 anytime to arrange screens." -ForegroundColor Green
