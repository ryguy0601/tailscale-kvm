# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 13 files · ~6,087 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 5 file(s) not represented in the graph (top: (none) 3, .bat 2)

## Summary
- 129 nodes · 162 edges · 13 communities (9 shown, 4 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 2 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `9637a21d`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- kvm_client.py
- kvm_host.py
- windows-host/src/main.rs
- linux-receiver/src/main.rs
- setup_receiver.sh
- ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland
- keyboard_hook_proc
- IncomingMessage
- Building & Running
- HostMessage
- tailscale-kvm-host
- tailscale-kvm-receiver

## God Nodes (most connected - your core abstractions)
1. `HostMessage` - 7 edges
2. `keyboard_hook_proc()` - 7 edges
3. `⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland` - 7 edges
4. `IncomingMessage` - 6 edges
5. `handle_connection()` - 6 edges
6. `mouse_hook_proc()` - 6 edges
7. `send_msg()` - 6 edges
8. `send_payload()` - 5 edges
9. `handle_client()` - 4 edges
10. `start_receiver_listener()` - 4 edges

## Surprising Connections (you probably didn't know these)
- `keyboard_hook_proc()` --calls--> `translate_vk()`  [EXTRACTED]
  rust/windows-host/src/main.rs → rust/windows-host/src/vk_map.rs
- `win32_keyboard_filter()` --calls--> `translate_vk()`  [EXTRACTED]
  windows/kvm_host.py → windows/vk_to_evdev.py

## Import Cycles
- None detected.

## Communities (13 total, 4 thin omitted)

### Community 0 - "kvm_client.py"
Cohesion: 0.24
Nodes (4): get_cursor_x(), get_hyprland_left_bound(), handle_client(), main()

### Community 1 - "kvm_host.py"
Cohesion: 0.16
Nodes (9): clipboard_monitor(), connect_to_omarchy(), listen_to_receiver(), send_msg(), switch_to_local(), switch_to_remote(), win32_keyboard_filter(), win32_mouse_filter() (+1 more)

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.08
Nodes (14): CENTER_X, CENTER_Y, get_config_ip(), IGNORE_NEXT_WARP, IS_REMOTE, KEYBOARD_HOOK, main(), MOUSE_HOOK (+6 more)

### Community 3 - "linux-receiver/src/main.rs"
Cohesion: 0.11
Nodes (6): get_cursor_x(), get_hyprland_left_bound(), handle_connection(), main(), OutgoingMessage, SwitchToLocal

### Community 5 - "⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland"
Cohesion: 0.20
Nodes (9): 1. Linux Receiver Setup (Omarchy / Hyprland), 2. Windows Host Setup (`not-a-gabe-cube`), 🏗 Architecture & Tech Stack, 🕸 Graphify Knowledge Graph, 🌟 Key Features, 📄 License, 🚀 Setup & Installation, ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (+1 more)

### Community 7 - "keyboard_hook_proc"
Cohesion: 0.24
Nodes (7): keyboard_hook_proc(), mouse_hook_proc(), send_payload(), start_clipboard_sync(), switch_to_local(), switch_to_remote(), translate_vk()

### Community 8 - "IncomingMessage"
Cohesion: 0.33
Nodes (6): IncomingMessage, Clipboard, Key, MouseClick, MouseMove, MouseScroll

### Community 9 - "Building & Running"
Cohesion: 0.33
Nodes (5): Architecture, Building & Running, Linux Receiver (`linux-receiver`), Tailscale KVM (Rust Edition), Windows Host (`windows-host`)

### Community 10 - "HostMessage"
Cohesion: 0.33
Nodes (6): HostMessage, Clipboard, Key, MouseClick, MouseMove, MouseScroll

## Knowledge Gaps
- **14 isolated node(s):** `tailscale-kvm-receiver`, `MouseMove`, `MouseClick`, `MouseScroll`, `Key` (+9 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 69 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `IncomingMessage` connect `IncomingMessage` to `linux-receiver/src/main.rs`?**
  _High betweenness centrality (0.045) - this node is a cross-community bridge._
- **What connects `tailscale-kvm-receiver`, `MouseMove`, `MouseClick` to the rest of the system?**
  _14 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `windows-host/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.08172043010752689 - nodes in this community are weakly interconnected._
- **Why does `HostMessage` connect `HostMessage` to `windows-host/src/main.rs`, `keyboard_hook_proc`?**
  _High betweenness centrality (0.045) - this node is a cross-community bridge._
- **Should `linux-receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.1067193675889328 - nodes in this community are weakly interconnected._
- **Why does `keyboard_hook_proc()` connect `keyboard_hook_proc` to `windows-host/src/main.rs`?**
  _High betweenness centrality (0.031) - this node is a cross-community bridge._