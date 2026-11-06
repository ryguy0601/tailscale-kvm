# ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland

A lightweight, zero-latency software KVM switch bridging Windows workstations to Omarchy Linux (Hyprland / Wayland) over an encrypted Tailscale mesh network with kernel-level `/dev/uinput` synthetic event injection.

<div align="center">

![Python](https://img.shields.io/badge/Python-3.10%2B-3776AB?style=for-the-badge&logo=python&logoColor=white)
![Tailscale](https://img.shields.io/badge/Tailscale-Mesh%20VPN-4F46E5?style=for-the-badge&logo=tailscale&logoColor=white)
![Wayland](https://img.shields.io/badge/Wayland-Hyprland%20%2F%20Omarchy-0A84FF?style=for-the-badge&logo=linux&logoColor=white)
![Graphify](https://img.shields.io/badge/Graphify-Knowledge%20Graph-00C7B7?style=for-the-badge&logo=diagram-next&logoColor=white)
![License](https://img.shields.io/badge/License-MIT-green.style=for-the-badge)

</div>

---

## 🌟 Key Features

- **Kernel-Level Input Injection (`/dev/uinput`):** Uses **`python-evdev`** to emit hardware-level mouse movements, relative scrolls, clicks, and scancodes directly into the Linux input subsystem, seamlessly bypassing Wayland security boundaries and compositor synthetic event blocking.
- **Low-Latency Encrypted Mesh Transport:** Communicates via a dedicated TCP stream with **`TCP_NODELAY`** enabled over a private **Tailscale** peer-to-peer mesh VPN, guaranteeing sub-millisecond edge input transmission without public exposure or port forwarding.
- **Auto-Reconnecting Resilient Connection:** Background health worker automatically retries socket reconnection if either peer reboots or restarts networking.
- **Seamless Edge-Triggered Boundary Transition:** Captures Windows cursor positioning with Win32 **`ClipCursor`** API locks at the physical monitor edge, redirecting inputs until hotkey release (**`F12`**) returns focus to the local desktop.
- **Bidirectional Clipboard & File Streaming:** Automates background clipboard synchronization via **`wl-clipboard`** (`wl-copy`/`wl-paste`) and Win32 clipboard APIs, paired with high-throughput binary chunk file transfer straight into `~/Downloads` with desktop notifications.
- **Automated Windows Startup & Zero-Click Install:** Single-click installer sets up dependencies, writes configuration, and creates a background startup shortcut in `shell:startup` powered by `pythonw.exe`.
- **Graphify Code Graph Integration:** Features automated AST structural code analysis, call flow mapping, and visual knowledge graphs generated via **Graphify**.

---

## 🏗 Architecture & Tech Stack

```text
+-----------------------------------------------------------------------------------+
|                            WINDOWS HOST CONTROLLER                                |
|  Win32 User32 API (ClipCursor / GetSystemMetrics) + pynput (Mouse & Keyboard Hooks)|
|  Pyperclip Clipboard Monitor  +  File Streaming Protocol                          |
|  Auto-Reconnect Loop  +  Silent Startup Daemon (pythonw.exe)                      |
+------------------------------------------+----------------------------------------+
                                           |
                                  TCP Stream (TCP_NODELAY)
                                  Encrypted Tailscale Mesh
                                           |
                                           v
+-----------------------------------------------------------------------------------+
|                        OMARCHY / HYPRLAND CLIENT RECEIVER                         |
|  AsyncIO Socket Server (Port 9876)  -->  JSON Packet & Binary Chunk Dispatcher    |
|                                          |                                        |
|             +----------------------------+----------------------------+           |
|             v                                                         v           |
|  Kernel /dev/uinput (python-evdev)                           wl-clipboard & notify|
|  [Virtual Hardware Mouse / Keyboard]                         [Clipboard & Files]  |
|             |                                                         |           |
|             v                                                         v           |
|  Wayland Compositor (Hyprland Core)                          Omarchy Desktop UI   |
+-----------------------------------------------------------------------------------+
```

- **Host (Windows):** Python 3 (`pythonw.exe`), `pynput`, `pyperclip`, Win32 API (`ctypes`), Virtual Keycode (`VK`) scancode translation table, PowerShell & Batch automated installers.
- **Client (Omarchy / Hyprland Linux):** Python 3 `asyncio`, `python-evdev` (`/dev/uinput`), `wl-clipboard`, `libnotify`, `systemd` user daemon.
- **Networking:** Tailscale Mesh VPN, WireGuard overlay routing, low-latency raw TCP sockets with disabled Nagle algorithm (`TCP_NODELAY`).
- **Code Intelligence & Graphing:** Graphify AST semantic engine, interactive D3 graph (`graph.html`), and Mermaid call flow mapping (`tailscale-kvm-callflow.html`).

---

## 📊 Visual Overview & Architecture Graph

Visual demonstration assets and AST dependency graphs generated for this repository:

<div align="center">

| Architecture Knowledge Graph (`graphify`) | Call Flow & Component Sequence |
| :---: | :---: |
| ![Architecture Graph](demoPics/01-architecture-graph.png) | ![Call Flow Diagram](demoPics/02-callflow-diagram.png) |

</div>

---

## 🚀 Setup & Installation

### 1. Linux Receiver Setup (Omarchy / Hyprland)

Clone the repository and run the automated receiver setup script:

```bash
git clone https://github.com/ryguy0601/tailscale-kvm.git
cd tailscale-kvm/linux

# Run automated dependency installation & udev rule setup
./setup_receiver.sh

# Apply input group membership immediately without rebooting
newgrp input

# Enable and start the systemd user service
systemctl --user enable --now kvm-client.service

# Check receiver service status
systemctl --user status kvm-client.service
```

### 2. Windows Host Setup (`not-a-gabe-cube`)

On the Windows workstation:

```powershell
git clone https://github.com/ryguy0601/tailscale-kvm.git
cd tailscale-kvm\windows

# Run automated setup (installs pynput/pyperclip & sets up Windows startup)
.\setup_windows.bat
```

Or run directly from PowerShell:

```powershell
.\setup_windows.ps1 -OmarchyIP "100.99.142.70"
```

> **What the Windows Setup does:**
> - Verifies Python installation.
> - Installs `pynput` and `pyperclip`.
> - Saves target IP (`100.99.142.70`) to `windows/config.txt`.
> - Creates a background startup shortcut (`TailscaleKVMHost.lnk`) in your Windows Startup directory (`shell:startup`) using `pythonw.exe`, so it starts automatically whenever you log in.

> **Usage:**
> - Move your mouse cursor against the **right edge of your Windows display** to engage remote control on Omarchy.
> - Press **`F12`** anytime to release the cursor back to Windows.
> - Any text copied to the clipboard will automatically synchronize across machines.

---

## 🕸 Graphify Knowledge Graph

This repository is indexed with **Graphify** for code comprehension and architecture analysis:

```bash
# Update AST analysis and graph reports
graphify update .

# Generate interactive Mermaid call flow HTML
graphify export callflow-html

# Open interactive visualization
xdg-open graphify-out/graph.html
```

---

## 📄 License

Distributed under the [MIT License](LICENSE).
