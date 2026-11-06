# Tailscale KVM (Rust Edition)
============================

High-performance, zero-overhead Rust implementation of the cross-platform Tailscale software KVM between Windows and Omarchy / Hyprland Linux.

## Architecture

- **`windows-host/`**: 
  - Raw Win32 low-level hooks (`WH_MOUSE_LL` and `WH_KEYBOARD_LL`) via `windows-sys`.
  - Swallows local mouse clicks and keystrokes while in remote mode to eliminate duplication.
  - Center-warp relative cursor deltas for 360-degree free mouse movement.
  - Bidirectional clipboard sync via `arboard`.
  - Receptive TCP reader for automatic monitor edge return (`SwitchToLocal`).
  - Emergency break hotkeys (`F12`, `Pause`, `ScrollLock`).

- **`linux-receiver/`**:
  - Asynchronous event server built on `tokio`.
  - Kernel-level synthetic hardware injection into `/dev/uinput` via `evdev::uinput`.
  - Dynamic Hyprland monitor coordinate polling via `hyprctl cursorpos` to signal seamless cursor return when crossing the left edge.
  - Native Wayland clipboard sink with `wl-copy`.

## Building & Running

### Windows Host (`windows-host`)
```powershell
cd rust\windows-host
cargo build --release
.\target\release\tailscale-kvm-host.exe
```

### Linux Receiver (`linux-receiver`)
```bash
cd rust/linux-receiver
cargo build --release
./target/release/tailscale-kvm-receiver
```
