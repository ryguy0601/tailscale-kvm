[CmdletBinding()]
param (
    [string]$OmarchyIP = "100.99.142.70",
    [switch]$NoStartup
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Tailscale KVM: Windows Host Setup (Miniconda / Python) " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host ""

# 1. Discover Python / Conda Executable
Write-Host "[1/4] Detecting Python / Conda environment..." -ForegroundColor Yellow

function Find-PythonPath {
    # Check if python is already in PATH
    $cmd = Get-Command python.exe -ErrorAction SilentlyContinue
    if ($cmd -and (Test-Path $cmd.Source)) {
        return $cmd.Source
    }

    # Common Miniconda / Anaconda user and system paths
    $candidatePaths = @(
        "$env:USERPROFILE\miniconda3\python.exe",
        "$env:USERPROFILE\Miniconda3\python.exe",
        "$env:LOCALAPPDATA\miniconda3\python.exe",
        "$env:LOCALAPPDATA\Miniconda3\python.exe",
        "$env:USERPROFILE\anaconda3\python.exe",
        "$env:USERPROFILE\Anaconda3\python.exe",
        "C:\ProgramData\miniconda3\python.exe",
        "C:\ProgramData\Miniconda3\python.exe",
        "C:\miniconda3\python.exe",
        "C:\Miniconda3\python.exe",
        "C:\ProgramData\anaconda3\python.exe",
        "C:\anaconda3\python.exe"
    )

    foreach ($path in $candidatePaths) {
        if (Test-Path $path) {
            return $path
        }
    }

    # Query registry for Conda or Python installs
    $regKeys = @(
        "HKCU:\Software\Python\PythonCore",
        "HKLM:\SOFTWARE\Python\PythonCore"
    )
    foreach ($reg in $regKeys) {
        if (Test-Path $reg) {
            Get-ChildItem $reg -ErrorAction SilentlyContinue | ForEach-Object {
                $installPath = (Get-ItemProperty "$($_.PSPath)\InstallPath" -ErrorAction SilentlyContinue)."(default)"
                if ($installPath) {
                    $regPy = Join-Path $installPath "python.exe"
                    if (Test-Path $regPy) {
                        return $regPy
                    }
                }
            }
        }
    }

    return $null
}

$PythonExe = Find-PythonPath

if (-not $PythonExe) {
    Write-Host "[!] Python or Miniconda could not be automatically located." -ForegroundColor Red
    Write-Host "    If you have Miniconda installed, open 'Anaconda Prompt (miniconda3)' or run:" -ForegroundColor Yellow
    Write-Host "    conda init powershell" -ForegroundColor Cyan
    Write-Host "    Or enter the full path to your python.exe:" -ForegroundColor White
    $manualPath = Read-Host "    Path to python.exe"
    if ($manualPath -and (Test-Path $manualPath.Trim('"'))) {
        $PythonExe = $manualPath.Trim('"')
    } else {
        Write-Host "[!] Invalid python executable path. Exiting." -ForegroundColor Red
        Pause
        exit 1
    }
}

$PythonDir = Split-Path -Parent $PythonExe
$PythonVersion = & $PythonExe --version 2>&1
Write-Host "      Found: $PythonVersion" -ForegroundColor Green
Write-Host "      Location: $PythonExe" -ForegroundColor DarkCyan

# Add directory to session PATH so scripts/tools resolve
$env:PATH = "$PythonDir;$PythonDir\Scripts;$PythonDir\Library\bin;$env:PATH"

# 2. Install Python Dependencies
Write-Host ""
Write-Host "[2/4] Installing required packages (pynput, pyperclip)..." -ForegroundColor Yellow
& $PythonExe -m pip install pynput pyperclip --quiet
if ($LASTEXITCODE -eq 0) {
    Write-Host "      Python dependencies successfully installed." -ForegroundColor Green
} else {
    Write-Host "      Trying conda install fallback or non-quiet pip..." -ForegroundColor DarkYellow
    & $PythonExe -m pip install pynput pyperclip
}

# 3. Update config.txt
Write-Host ""
Write-Host "[3/4] Updating configuration..." -ForegroundColor Yellow
$ConfigFile = Join-Path $ScriptDir "config.txt"
$ConfigLines = @(
    "# Omarchy KVM Configuration",
    "OMARCHY_IP=$OmarchyIP",
    "PORT=9876",
    "PYTHON_PATH=$PythonExe"
)
Set-Content -Path $ConfigFile -Value $ConfigLines -Encoding Ascii
Write-Host "      Saved target Omarchy IP: $OmarchyIP" -ForegroundColor Green
Write-Host "      Saved Python executable path to config.txt" -ForegroundColor Green

# 4. Configure Windows Startup
if (-not $NoStartup) {
    Write-Host ""
    Write-Host "[4/4] Configuring Windows Auto-Startup on login..." -ForegroundColor Yellow
    $StartupFolder = [Environment]::GetFolderPath("Startup")
    $ShortcutPath = Join-Path $StartupFolder "TailscaleKVMHost.lnk"

    # Locate pythonw.exe in the same directory as python.exe for silent background run
    $PythonW = Join-Path $PythonDir "pythonw.exe"
    if (-not (Test-Path $PythonW)) {
        $PythonW = $PythonExe
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
    Write-Host "      Target: $PythonW `"$KvmHostScript`"" -ForegroundColor DarkCyan
    Write-Host "      Location: $ShortcutPath" -ForegroundColor DarkCyan
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
Write-Host "    & '$PythonExe' kvm_host.py" -ForegroundColor Green
Write-Host ""
Write-Host "It will also start automatically whenever you log into Windows." -ForegroundColor Cyan
Write-Host ""
