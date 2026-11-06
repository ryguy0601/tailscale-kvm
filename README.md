# ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)

A high-performance, zero-overhead software KVM switch written in 100% native Rust, bridging Windows workstations to Omarchy Linux (Hyprland / Wayland) over an encrypted Tailscale mesh network with kernel-level `/dev/uinput` synthetic event injection.

<div align="center">

![Rust](https://img.shields.io/badge/Rust-2021%20Edition-DEA584?style=for-the-badge&logo=rust&logoColor=black)
![Tailscale](https://img.shields.io/badge/Tailscale-Mesh%20VPN-4F46E5?style=for-the-badge&logo=tailscale&logoColor=white)
![Wayland](https://img.shields.io/badge/Wayland-Hyprland%20%2F%20Omarchy-0A84FF?style=for-the-badge&logo=linux&logoColor=white)
![Tokio](https://img.shields.io/badge/Async-Tokio-000000?style=for-the-badge&logo=tokio&logoColor=white)
![Graphify](https://img.shields.io/badge/Graphify-85%20Nodes-00C7B7?style=for-the-badge&logo=diagram-next&logoColor=white)
![License](https://img.shields.io/badge/License-MIT-green.style=for-the-badge)

</div>

---

## 🌟 Key Features

- **100% Native Rust Architecture:** Engineered as a unified Cargo workspace with zero runtime interpreter dependencies, providing sub-millisecond input response, zero garbage collection pauses, and minimal memory footprint.
- **Kernel-Level Input Injection (`/dev/uinput`):** Uses the pure-Rust **`evdev`** crate to inject hardware-level relative mouse movements, clicks, wheel scrolls, and scancodes directly into the Linux input subsystem, completely bypassing Wayland security boundaries and compositor synthetic event blocking.
- **Low-Latency Encrypted Mesh Transport:** Communicates via dedicated TCP streams with **`TCP_NODELAY`** enabled over a private **Tailscale** peer-to-peer mesh VPN, guaranteeing sub-millisecond edge input transmission without public exposure or port forwarding.
- **Bi-Directional Seamless Monitor Edge Transitions:** 
  - Move cursor past the **right edge of Windows** $\rightarrow$ Seamlessly enters Omarchy.
  - Move cursor past the **left edge of Omarchy** $\rightarrow$ Automatically returns to Windows without pressing any hotkeys!
- **Complete Event Suppression (No Ghost Clicks):** Uses raw Win32 low-level hooks (`WH_MOUSE_LL` and `WH_KEYBOARD_LL`) via `windows-sys` to swallow inputs on Windows while in remote mode, eliminating duplicate clicks and phantom keystrokes.
- **Comprehensive Keycode & Hotkey Support:** Fast pattern-matching translation table for Hyprland hotkeys (`Super / Windows Key`, `Alt`, `Shift`, `Backspace`, function keys, and symbols).
- **Clipboard Synchronization:** Background worker keeps clipboards in sync between Windows and Omarchy using `arboard` and Wayland `wl-copy`.
- **Graphify Code Graph Integration:** Features automated AST structural code analysis, call flow mapping, and visual knowledge graphs generated via **Graphify**.

---

## 🏗 Architecture & Tech Stack

```text
+-----------------------------------------------------------------------------------+
|                        WINDOWS HOST CONTROLLER (Rust)                             |
|  Raw Win32 Hooks (WH_MOUSE_LL / WH_KEYBOARD_LL) via windows-sys                   |
|  Event Suppression Engine  +  Relative Center-Warp Delta Tracker                  |
|  Bidirectional Clipboard Engine (arboard)                                         |
+------------------------------------------+----------------------------------------+
                                           |
                                  TCP Stream (TCP_NODELAY)
                                  Encrypted Tailscale Mesh
                                           |
                                           v
+-----------------------------------------------------------------------------------+
|                     OMARCHY / HYPRLAND CLIENT RECEIVER (Rust)                     |
|  Async Tokio Multi-Threaded TCP Server (Port 9876)                                |
|                                          |                                        |
|             +----------------------------+----------------------------+           |
|             v                                                         v           |
|  Kernel /dev/uinput (evdev)                                 Wayland wl-clipboard  |
|  [Virtual Mouse & Keyboard Device]                           [Clipboard Sync]     |
|             |                                                         |           |
|             v                                                         v           |
|  Wayland Compositor (Hyprland Core)                          Omarchy Desktop UI   |
|  [Dynamic Left Edge Boundary Monitor (hyprctl)]              [Screen Sync]        |
+-----------------------------------------------------------------------------------+
```

- **Host (Windows):** Rust 2021, `windows-sys`, `arboard`, `serde`, `serde_json`.
- **Receiver (Omarchy / Hyprland Linux):** Rust 2021, `tokio` (async network I/O), `evdev` (`/dev/uinput`), `notify-rust`, `serde`, `serde_json`.
- **Networking:** Tailscale Mesh VPN, WireGuard overlay routing, low-latency raw TCP sockets with disabled Nagle algorithm (`TCP_NODELAY`).
- **Code Intelligence & Graphing:** Graphify AST semantic engine, interactive D3 graph (`graph.html`), and Mermaid call flow mapping (`tailscale-kvm-callflow.html`).

---

## 📊 Visual Overview & Architecture Graph

Visual demonstration assets and AST dependency graphs generated for this repository:

<div align="center">

| Architecture Knowledge Graph (`graphify` - 85 Nodes) | Call Flow & Component Sequence |
| :---: | :---: |
| ![Architecture Graph](demoPics/01-architecture-graph.png) | ![Call Flow Diagram](demoPics/02-callflow-diagram.png) |

</div>

---

## 🚀 Setup & Usage

### 1. Linux Receiver (Omarchy)

```bash
cd linux-receiver
cargo build --release
./target/release/tailscale-kvm-receiver
```

### 2. Windows Host (`not-a-gabe-cube`)

```powershell
cd windows-host
cargo build --release
.\target\release\tailscale-kvm-host.exe 100.99.142.70
```

---

## 🎮 How to Control

- **Switch to Omarchy:** Move your mouse cursor against the **right edge of your Windows display**.
- **Switch back to Windows:** Move your mouse cursor against the **left edge of your Omarchy display**.
- **Emergency Release Hotkeys:** Press **`F12`**, **`Pause / Break`**, or **`Scroll Lock`** anytime to instantly release the cursor back to Windows.

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
