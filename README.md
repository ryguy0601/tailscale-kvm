# ⚡ Tailscale Software KVM: Universal Multi-Directional & Multi-Platform Mesh (Pure Rust)

A high-performance, zero-overhead software KVM switch written in 100% native Rust, seamlessly bridging any device combination (**Windows, Linux, macOS**) over an encrypted Tailscale mesh network with configurable multi-directional edge transitions (**Right, Left, Up, Down**) and an interactive **Multi-Screen Visual Web Dashboard**.

<div align="center">

![Rust](https://img.shields.io/badge/Rust-2021%20Edition-DEA584?style=for-the-badge&logo=rust&logoColor=black)
![Tailscale](https://img.shields.io/badge/Tailscale-Mesh%20VPN-4F46E5?style=for-the-badge&logo=tailscale&logoColor=white)
![Platforms](https://img.shields.io/badge/Platforms-Windows%20%7C%20Linux%20%7C%20macOS-blue?style=for-the-badge)
![Wayland](https://img.shields.io/badge/Wayland-Hyprland%20%2F%20Omarchy-0A84FF?style=for-the-badge&logo=linux&logoColor=white)
![Tokio](https://img.shields.io/badge/Async-Tokio-000000?style=for-the-badge&logo=tokio&logoColor=white)
![Graphify](https://img.shields.io/badge/Graphify-Indexed-00C7B7?style=for-the-badge&logo=diagram-next&logoColor=white)
![License](https://img.shields.io/badge/License-MIT-green.style=for-the-badge)

</div>

---

## 🌟 Key Features

- **100% Native Rust Architecture:** Engineered as a unified Cargo workspace with clean `host` and `receiver` crates and zero runtime interpreter dependencies. Provides sub-millisecond input response, zero garbage collection pauses, and minimal memory footprint.
- **Universal Multi-Directional Edge Positioning:**
  - Place your receiver in **any direction relative to the host**: `--direction right`, `--direction left`, `--direction up`, or `--direction down`.
  - Reciprocal return transitions: Exiting the matching opposite boundary on the receiver instantly hands control back to the host.
  - Active visual edge glow and corner anti-slip safety locks dynamically adapt to whatever edge direction is selected.
- **Interactive Multi-Receiver Visual Web Dashboard (`http://localhost:9877`):**
  - Built-in zero-dependency web GUI dashboard that automatically opens in your default browser upon host launch.
  - 2D Drag-and-Drop Canvas: drag detected Tailscale peer devices onto **Top**, **Bottom**, **Left**, or **Right** screen slots to dynamically re-route boundaries in real time.
- **Native Tailscale Mesh Integration:**
  - Automatic peer discovery via local `tailscale status --json` query: automatically locates active online peers on your tailnet without manual IP configuration.
  - Full support for Tailscale 100.x.y.z addresses and MagicDNS node names.
- **Kernel-Level Input Injection (`/dev/uinput`):** Uses pure-Rust **`evdev`** to inject hardware-level relative mouse movements, clicks, wheel scrolls, and scancodes directly into the Linux input subsystem, completely bypassing Wayland compositor synthetic event blocking.
- **Complete Event Suppression (No Ghost Clicks):** Raw Win32 low-level hooks (`WH_MOUSE_LL` and `WH_KEYBOARD_LL`) swallow inputs on Windows while in remote mode, eliminating duplicate clicks and phantom keystrokes.
- **Clipboard & Media Synchronization:** Real-time text and high-resolution image clipboard synchronization with streaming chunk buffers.
- **Cross-Platform Compatibility:** Universal architecture designed for any combination of hosts and receivers across Windows, Linux, and macOS.
- **Graphify Code Graph Integration:** Features automated AST structural code analysis, call flow mapping, and visual knowledge graphs generated via **Graphify**.

---

## 📁 Repository Structure

```text
tailscale-kvm/
├── host/                    # Universal Host Controller crate (manages mouse/keyboard, web GUI & mesh routing)
│   ├── gui_static/          # Embedded HTML5 drag-and-drop multi-screen dashboard
│   ├── src/                 # Host logic, Win32/native event hooks, web_gui.rs & vk_map.rs
│   └── Cargo.toml
├── receiver/                # Universal Receiver Daemon crate (injects synthetic inputs & clipboard)
│   ├── src/                 # /dev/uinput evdev injection, boundary return monitor & Wayland clip
│   └── Cargo.toml
├── build.sh / build.ps1     # One-click universal build scripts (Linux/macOS & Windows)
├── run-host.sh / .ps1       # Universal host launchers (starts controller + auto-opens web GUI)
├── run-receiver.sh / .ps1   # Universal receiver launchers (starts receiver daemon)
└── Cargo.toml               # Unified workspace manifest
```

---

## 🏗 Architecture & Multi-Screen Mesh

```text
               +---------------------------+
               |        RECEIVER 1         |
               |   "linux-workstation"     |
               |        (Top Edge)         |
               +---------------------------+
                             ^
                             |
+---------------------+      |      +---------------------+
|     RECEIVER 2      | <----+----> |     RECEIVER 3      |
|    "macbook-air"    |             |  "windows-desktop"  |
|     (Left Edge)     |             |    (Right Edge)     |
+---------------------+             +---------------------+
                             |
                             v
               +---------------------------+
               |     HOST CONTROLLER       |
               | (Physical Mouse/Keyboard) |
               | Web GUI @ localhost:9877  |
               +---------------------------+
```

---

## 🚀 Quick Start

### 1. Build the Workspace

- **Linux / macOS:**
  ```bash
  ./build.sh
  ```
- **Windows (PowerShell):**
  ```powershell
  .\build.ps1
  ```

---

### 2. Launch the Receiver (Each device to be controlled)

Run on any receiver machine — no flags needed (its return edge automatically syncs whenever you arrange screens in the Host GUI):

- **Linux / macOS:**
  ```bash
  ./run-receiver.sh
  ```
- **Windows (PowerShell):**
  ```powershell
  .\run-receiver.ps1
  ```

*(Optional CLI override: `--edge left|right|top|bottom`.)*

---

### 3. Launch the Host (The device with your physical keyboard & mouse)

Run on your main machine:

- **Linux / macOS:**
  ```bash
  ./run-host.sh
  ```
- **Windows (PowerShell):**
  ```powershell
  .\run-host.ps1
  ```

*The host probes your Tailscale peers for active KVM receivers (`:9876`), restores your saved `kvm_layout.json` arrangement (if any), and opens the **Visual Management Dashboard** at `http://localhost:9877`. You can also pass optional CLI flags (`--target <IP> --direction right|left|up|down`, `--no-browser`).*

---

## 🖥️ Visual Web Dashboard & Multi-Receiver Layout

When the host is running, `http://localhost:9877` lets you control everything live:

1. **Auto-Discovered Peer Pool:** Lists your Tailscale peers and badges which ones have an active KVM Receiver listening on `:9876`.
2. **Drag & Reposition Anywhere:** Drag peers from the pool onto **Top**, **Left**, **Right**, or **Bottom** — and drag **already-connected screens** directly from one slot to another (moving or swapping them).
3. **Two-Way Live Edge Sync:** Dropping a screen onto a new side immediately updates **both** the Host edge trigger and the Receiver's reciprocal return edge without restarting either daemon, and persists your layout to `kvm_layout.json`.

---

## 🎮 Controls & Shortcuts

- **Switch to Receiver:** Move your mouse cursor past the configured screen edge on your host display (**Right**, **Left**, **Up**, or **Down**).
- **Switch back to Host:** Move your mouse cursor past the matching return edge on the receiver display.
- **Emergency Release Hotkeys:** Press **`F12`**, **`Pause / Break`**, or **`Scroll Lock`** anytime to instantly release the cursor back to the host.

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
