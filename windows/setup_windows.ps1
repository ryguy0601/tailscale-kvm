[CmdletBinding()]
param (
    [string]$OmarchyIP = "100.99.142.70",
    [switch]$NoStartup
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Tailscale KVM: Windows Host Setup & Startup Installer  " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host ""

# 1. Check Python
Write-Host "[1/4] Checking Python installation..." -ForegroundColor Yellow
$PythonCmd = Get-Command python.exe -ErrorAction SilentlyContinue
if (-not $PythonCmd) {
    Write-Host "[!] Python is not installed or not in PATH." -ForegroundColor Red
    Write-Host "    Please install Python 3 (check 'Add python.exe to PATH' during install) from https://python.org" -ForegroundColor Red
    Pause
    exit 1
}
$PythonVersion = & python.exe --version 2>&1
Write-Host "      Found: $PythonVersion ($($PythonCmd.Source))" -ForegroundColor Green

# 2. Install Python Dependencies
Write-Host ""
Write-Host "[2/4] Installing required packages (pynput, pyperclip)..." -ForegroundColor Yellow
& python.exe -m pip install --upgrade pip --quiet
& python.exe -m pip install pynput pyperclip --quiet
if ($LASTEXITCODE -eq 0) {
    Write-Host "      Python dependencies successfully installed." -ForegroundColor Green
} else {
    Write-Host "      Warning: pip install returned non-zero code. Attempting to continue..." -ForegroundColor DarkYellow
}

# 3. Update config.txt
Write-Host ""
Write-Host "[3/4] Updating configuration..." -ForegroundColor Yellow
$ConfigFile = Join-Path $ScriptDir "config.txt"
$ConfigLines = @(
    "# Omarchy KVM Configuration",
    "OMARCHY_IP=$OmarchyIP",
    "PORT=9876"
)
Set-Content -Path $ConfigFile -Value $ConfigLines -Encoding Ascii
Write-Host "      Saved target Omarchy IP: $OmarchyIP" -ForegroundColor Green

# 4. Configure Windows Startup
if (-not $NoStartup) {
    Write-Host ""
    Write-Host "[4/4] Configuring Windows Auto-Startup on login..." -ForegroundColor Yellow
    $StartupFolder = [Environment]::GetFolderPath("Startup")
    $ShortcutPath = Join-Path $StartupFolder "TailscaleKVMHost.lnk"

    # Locate pythonw.exe to run without a console window, fallback to python.exe
    $PythonDir = Split-Path -Parent $PythonCmd.Source
    $PythonW = Join-Path $PythonDir "pythonw.exe"
    if (-not (Test-Path $PythonW)) {
        $PythonW = $PythonCmd.Source
    }

    $KvmHostScript = Join-Path $ScriptDir "kvm_host.py"

    $WScriptShell = New-Object -ComObject WScript.Shell
    $Shortcut = $WScriptShell.CreateShortcut($ShortcutPath)
    $Shortcut.TargetPath = $PythonW
    $Shortcut.Arguments = "`"$KvmHostScript`""
    $Shortcut.WorkingDirectory = $ScriptDir
    $Shortcut.Description = "Tailscale KVM Windows to Omarchy Host Controller"
    $Shortcut.WindowStyle = 7
    $Shortcut.Save()

    Write-Host "      Shortcut created in Windows Startup:" -ForegroundColor Green
    Write-Host "      $ShortcutPath" -ForegroundColor DarkCyan
} else {
    Write-Host ""
    Write-Host "[4/4] Skipping startup registration (-NoStartup specified)." -ForegroundColor Gray
}

# Tailscale reachability test
Write-Host ""
Write-Host "[*] Testing connection to Omarchy ($OmarchyIP)..." -ForegroundColor Yellow
$PingResult = Test-Connection -ComputerName $OmarchyIP -Count 1 -Quiet -ErrorAction SilentlyContinue
if ($PingResult) {
    Write-Host "    [OK] Omarchy is REACHABLE over Tailscale!" -ForegroundColor Green
} else {
    Write-Host "    [!] Note: $OmarchyIP did not respond to ICMP ping." -ForegroundColor DarkYellow
    Write-Host "        Ensure Tailscale is connected and the Linux receiver daemon is running." -ForegroundColor DarkYellow
}

Write-Host ""
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Setup Complete!                                        " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "To start the controller immediately, run:" -ForegroundColor White
Write-Host "    .\run_kvm.bat" -ForegroundColor Green
Write-Host "Or:" -ForegroundColor White
Write-Host "    python kvm_host.py" -ForegroundColor Green
Write-Host ""
Write-Host "It is also set to launch automatically whenever you log into Windows." -ForegroundColor Cyan
Write-Host ""
