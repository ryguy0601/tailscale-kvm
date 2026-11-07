# ⚡ Tailscale Software KVM: Universal Multi-Directional & Multi-Platform Mesh (Pure Rust)

A high-performance, zero-overhead software KVM switch written in 100% native Rust, seamlessly bridging any device combination (**Windows, Linux, macOS**) over an encrypted Tailscale mesh network with configurable multi-directional edge transitions (**Right, Left, Up, Down**).

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

- **100% Native Rust Architecture:** Engineered as a unified Cargo workspace with zero runtime interpreter dependencies, providing sub-millisecond input response, zero garbage collection pauses, and minimal memory footprint.
- **Universal Multi-Directional Edge Positioning:**
  - Place your receiver in **any direction relative to the host**: `--direction right`, `--direction left`, `--direction up`, or `--direction down`.
  - Reciprocal return transitions: Exiting the matching opposite boundary on the receiver instantly hands control back to the host.
  - Active visual edge glow and corner anti-slip safety locks dynamically adapt to whatever edge direction is selected.
- **Native Tailscale Mesh Integration:**
  - Automatic peer discovery via local `tailscale status --json` query: automatically locates active online peers on your tailnet without manual IP configuration.
  - Full support for Tailscale 100.x.y.z addresses and MagicDNS node names.
- **Kernel-Level Input Injection (`/dev/uinput`):** Uses pure-Rust **`evdev`** to inject hardware-level relative mouse movements, clicks, wheel scrolls, and scancodes directly into the Linux input subsystem, completely bypassing Wayland compositor synthetic event blocking.
- **Complete Event Suppression (No Ghost Clicks):** Raw Win32 low-level hooks (`WH_MOUSE_LL` and `WH_KEYBOARD_LL`) swallow inputs on Windows while in remote mode, eliminating duplicate clicks and phantom keystrokes.
- **Clipboard & Media Synchronization:** Real-time text and high-resolution image clipboard synchronization with streaming chunk buffers.
- **Cross-Platform Compatibility:** Designed to support any device combination across Windows, Linux, and macOS.
- **Graphify Code Graph Integration:** Features automated AST structural code analysis, call flow mapping, and visual knowledge graphs generated via **Graphify**.

---

## 🏗 Architecture & Tech Stack

```text
+-----------------------------------------------------------------------------------+
|                        UNIVERSAL HOST CONTROLLER (Rust)                           |
|  Low-Level Event Hooks (Win32 LL Hooks / CoreGraphics Tap / evdev grab)            |
|  Directional Edge Detector: [Right | Left | Up | Down]                             |
|  Event Suppression Engine  +  Relative Center-Warp Delta Tracker                  |
|  Bidirectional Clipboard Engine (arboard / Win32 / Wayland wl-clipboard)         |
+------------------------------------------+----------------------------------------+
                                           |
                                  TCP Stream (TCP_NODELAY)
                                  Encrypted Tailscale Mesh
                                           |
                                           v
+-----------------------------------------------------------------------------------+
|                     UNIVERSAL CLIENT RECEIVER (Rust)                             |
|  Async Tokio Multi-Threaded TCP Server (Port 9876)                                |
|  Multi-Edge Return Monitor: [Left | Right | Top | Bottom]                          |
|                                          |                                        |
|             +----------------------------+----------------------------+           |
|             v                                                         v           |
|  Kernel /dev/uinput (evdev) / SendInput / CGEventPost       Wayland / Native Clip |
|  [Virtual Mouse & Keyboard Device]                           [Clipboard Sync]     |
|             |                                                         |           |
|             v                                                         v           |
|  Desktop Compositor & Window Manager                         Desktop Display      |
+-----------------------------------------------------------------------------------+
```

---

## 🚀 Setup & Usage

### 1. Starting the Receiver (Client Machine)

On the receiver computer (e.g. Omarchy Linux, macOS, or Windows):

```bash
cd linux-receiver
cargo build --release

# Run receiver specifying which edge returns cursor back to host:
# Options: --edge left | right | top | bottom (default: left)
./target/release/tailscale-kvm-receiver --edge left
```

### 2. Starting the Host Controller

On the host machine (where your physical mouse and keyboard are connected):

```bash
cd windows-host
cargo build --release

# Run host specifying where the receiver is located:
# Options: --direction right | left | up | down (default: right)
# Target automatically discovers active online Tailscale peer if omitted!
.\target\release\tailscale-kvm-host.exe --direction right --target 100.x.y.z
```

### Configuration File (`config.txt`)
Alternatively, create a `config.txt` in the program folder:
```ini
DIRECTION=right
EDGE=left
TARGET_IP=100.x.y.z
```

---

## 🎮 How to Control

- **Switch to Receiver:** Move your mouse cursor past the configured edge on your host display (**Right**, **Left**, **Up**, or **Down**).
- **Switch back to Host:** Move your mouse cursor past the opposite return edge on the receiver display.
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
