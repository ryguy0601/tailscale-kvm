@echo off
setlocal EnableDelayedExpansion

title Tailscale KVM Windows Host Launcher
cd /d "%~dp0"

set PYTHON_CMD=

:: 1. Check config.txt for saved PYTHON_PATH
if exist "%~dp0config.txt" (
    for /f "tokens=1,2 delims==" %%A in (%~dp0config.txt) do (
        if "%%A"=="PYTHON_PATH" (
            set "CONFIG_PY=%%B"
        )
    )
)

if defined CONFIG_PY (
    if exist "!CONFIG_PY!" (
        set "PYTHON_DIR=!CONFIG_PY:\python.exe=!"
        if exist "!PYTHON_DIR!\pythonw.exe" (
            set "PYTHON_CMD=!PYTHON_DIR!\pythonw.exe"
        ) else (
            set "PYTHON_CMD=!CONFIG_PY!"
        )
    )
)

:: 2. Search Miniconda / Anaconda paths if not in config
if not defined PYTHON_CMD (
    if exist "%USERPROFILE%\miniconda3\pythonw.exe" set "PYTHON_CMD=%USERPROFILE%\miniconda3\pythonw.exe"
    if not defined PYTHON_CMD if exist "%USERPROFILE%\Miniconda3\pythonw.exe" set "PYTHON_CMD=%USERPROFILE%\Miniconda3\pythonw.exe"
    if not defined PYTHON_CMD if exist "%LOCALAPPDATA%\miniconda3\pythonw.exe" set "PYTHON_CMD=%LOCALAPPDATA%\miniconda3\pythonw.exe"
    if not defined PYTHON_CMD if exist "C:\ProgramData\miniconda3\pythonw.exe" set "PYTHON_CMD=C:\ProgramData\miniconda3\pythonw.exe"
)

:: 3. Check system PATH
if not defined PYTHON_CMD (
    where pythonw.exe >nul 2>&1
    if %errorlevel% equ 0 set "PYTHON_CMD=pythonw.exe"
)
if not defined PYTHON_CMD (
    where python.exe >nul 2>&1
    if %errorlevel% equ 0 set "PYTHON_CMD=python.exe"
)

if not defined PYTHON_CMD (
    echo [!] Python or Miniconda could not be located automatically.
    echo     Please run .\setup_windows.bat first.
    pause
    exit /b 1
)

echo [*] Starting Tailscale KVM Host with: !PYTHON_CMD!
start "" "!PYTHON_CMD!" kvm_host.py
echo [+] Host started in background!
timeout /t 2 >nul
