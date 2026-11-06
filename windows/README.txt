Tailscale KVM Windows Host
===========================

Automated Setup:
----------------
Double-click `setup_windows.bat` (or in PowerShell run `.\setup_windows.ps1`).

This will:
1. Check Python installation.
2. Install dependencies (`pynput`, `pyperclip`).
3. Set the target Omarchy IP in `config.txt` (`100.99.142.70`).
4. Register a shortcut in your Windows Startup folder (`shell:startup`) using `pythonw.exe` so the KVM controller runs silently in the background on boot.

Manual Start:
-------------
- Run `run_kvm.bat` or `python kvm_host.py`.

Usage:
------
- Move cursor to the far right edge of your screen to switch control to Omarchy.
- Press `F12` to release cursor back to Windows.
- Clipboard text automatically syncs between both computers.
