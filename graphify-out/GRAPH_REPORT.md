# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 8 files · ~4,441 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 4 file(s) not represented in the graph (top: (none) 3, .toml 1)

## Summary
- 104 nodes · 148 edges · 14 communities (7 shown, 7 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `1f4aee96`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- main
- AtomicI32
- windows-host/src/main.rs
- handle_connection
- ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)
- main
- keyboard_hook_proc
- AtomicBool
- HHOOK
- save_transferred_file
- tailscale-kvm-host
- tailscale-kvm-receiver

## God Nodes (most connected - your core abstractions)
1. `handle_connection()` - 9 edges
2. `keyboard_hook_proc()` - 8 edges
3. `⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)` - 8 edges
4. `send_fast_bytes()` - 6 edges
5. `handle_edge_drag_or_transfer()` - 6 edges
6. `mouse_hook_proc()` - 6 edges
7. `get_hyprland_left_bound()` - 4 edges
8. `main()` - 4 edges
9. `send_files_to_omarchy()` - 4 edges
10. `fling_browser_to_omarchy()` - 4 edges

## Surprising Connections (you probably didn't know these)
- `keyboard_hook_proc()` --calls--> `translate_vk()`  [EXTRACTED]
  windows-host/src/main.rs → windows-host/src/vk_map.rs

## Import Cycles
- None detected.

## Communities (14 total, 7 thin omitted)

### Community 0 - "main"
Cohesion: 0.40
Nodes (3): start_receiver_listener(), STREAM_SENDER, switch_to_local()

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.10
Nodes (5): CENTER_X, CENTER_Y, CF_HDROP_FORMAT, SCREEN_HEIGHT, SCREEN_WIDTH

### Community 4 - "handle_connection"
Cohesion: 0.24
Nodes (7): apply_single_monitor_if_docked(), get_hyprland_left_bound(), handle_connection(), main(), open_url_in_browser(), restore_docked_monitors_if_needed(), start_cursorpos_worker()

### Community 5 - "⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)"
Cohesion: 0.18
Nodes (10): 1. Linux Receiver (Omarchy), 2. Windows Host (`not-a-gabe-cube`), 🏗 Architecture & Tech Stack, 🕸 Graphify Knowledge Graph, 🎮 How to Control, 🌟 Key Features, 📄 License, 🚀 Setup & Usage (+2 more)

### Community 6 - "main"
Cohesion: 0.67
Nodes (3): get_config_ip(), main(), start_clipboard_sync()

### Community 7 - "keyboard_hook_proc"
Cohesion: 0.19
Nodes (11): encode_base64(), fling_browser_to_omarchy(), get_clipboard_files(), handle_edge_drag_or_transfer(), keyboard_hook_proc(), mouse_hook_proc(), send_fast_bytes(), send_files_to_omarchy() (+3 more)

### Community 8 - "AtomicBool"
Cohesion: 0.29
Nodes (6): ALT_DOWN, CTRL_DOWN, IGNORE_WARP, IS_REMOTE, LBUTTON_DOWN, WIN_DOWN

## Knowledge Gaps
- **2 isolated node(s):** `tailscale-kvm-receiver`, `tailscale-kvm-host`
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 49 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **7 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `keyboard_hook_proc()` connect `keyboard_hook_proc` to `main`, `windows-host/src/main.rs`?**
  _High betweenness centrality (0.058) - this node is a cross-community bridge._
- **What connects `tailscale-kvm-receiver`, `tailscale-kvm-host` to the rest of the system?**
  _2 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `windows-host/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.1 - nodes in this community are weakly interconnected._
- **Why does `mouse_hook_proc()` connect `keyboard_hook_proc` to `windows-host/src/main.rs`?**
  _High betweenness centrality (0.023) - this node is a cross-community bridge._
- **Should `linux-receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.09090909090909091 - nodes in this community are weakly interconnected._
- **Why does `handle_connection()` connect `handle_connection` to `save_transferred_file`, `linux-receiver/src/main.rs`?**
  _High betweenness centrality (0.019) - this node is a cross-community bridge._