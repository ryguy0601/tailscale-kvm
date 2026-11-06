@echo off
setlocal EnableDelayedExpansion

title Tailscale KVM Windows Host Launcher
cd /d "%~dp0"

echo ========================================================
echo  Tailscale KVM: Windows Host to Omarchy Controller
echo ========================================================
echo.

where pythonw.exe >nul 2>&1
if %errorlevel% equ 0 (
    echo [*] Launching background KVM host via pythonw...
    start "" pythonw.exe kvm_host.py
    echo [+] Host started silently in background!
) else (
    where python.exe >nul 2>&1
    if %errorlevel% equ 0 (
        echo [*] Launching KVM host via python...
        start "" python.exe kvm_host.py
        echo [+] Host started!
    ) else (
        echo [!] Python is not found in PATH!
        pause
        exit /b 1
    )
)

timeout /t 2 >nul
