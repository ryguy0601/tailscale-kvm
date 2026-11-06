@echo off
setlocal EnableDelayedExpansion

title Tailscale KVM Windows Setup
cd /d "%~dp0"

echo ========================================================
echo  Tailscale KVM: Automated Windows Setup
echo ========================================================
echo.

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup_windows.ps1" %*

echo.
echo Press any key to exit...
pause >nul
