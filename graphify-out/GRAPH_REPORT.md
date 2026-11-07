# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 8 files · ~6,491 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 4 file(s) not represented in the graph (top: (none) 3, .toml 1)

## Summary
- 159 nodes · 246 edges · 20 communities (10 shown, 10 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `a80c4fa1`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- main
- AtomicI32
- windows-host/src/main.rs
- linux-receiver/src/main.rs
- apply_single_monitor_if_docked
- ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)
- execute_handoff
- keyboard_hook_proc
- AtomicBool
- HHOOK
- handle_connection
- tailscale-kvm-host
- tailscale-kvm-receiver
- AtomicI32
- resolve_target_addr
- AtomicIsize
- AtomicU32
- AtomicBool
- OnceLock

## God Nodes (most connected - your core abstractions)
1. `handle_connection()` - 15 edges
2. `execute_handoff()` - 14 edges
3. `keyboard_hook_proc()` - 8 edges
4. `⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)` - 8 edges
5. `send_fast_bytes()` - 6 edges
6. `mouse_hook_proc()` - 6 edges
7. `receive_binary_file()` - 5 edges
8. `DragKind` - 5 edges
9. `send_inputs()` - 5 edges
10. `tap_key()` - 5 edges

## Surprising Connections (you probably didn't know these)
- `keyboard_hook_proc()` --calls--> `translate_vk()`  [EXTRACTED]
  windows-host/src/main.rs → windows-host/src/vk_map.rs

## Import Cycles
- None detected.

## Communities (20 total, 10 thin omitted)

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.07
Nodes (4): CF_HDROP_FMT, CF_UNICODETEXT_FMT, FILE_CHUNK_SIZE, MAX_CLIPBOARD_TEXT_BYTES

### Community 3 - "linux-receiver/src/main.rs"
Cohesion: 0.07
Nodes (3): ACTIVE_CONTROL_CLIENTS, MAX_SINGLE_FILE_BYTES, STREAM_CHUNK_SIZE

### Community 4 - "apply_single_monitor_if_docked"
Cohesion: 0.33
Nodes (7): apply_single_monitor_if_docked(), get_hyprland_left_bound(), main(), mark_control_session(), monitor_counts(), restore_docked_monitors_if_needed(), start_cursorpos_worker()

### Community 5 - "⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)"
Cohesion: 0.18
Nodes (10): 1. Linux Receiver (Omarchy), 2. Windows Host (`not-a-gabe-cube`), 🏗 Architecture & Tech Stack, 🕸 Graphify Knowledge Graph, 🎮 How to Control, 🌟 Key Features, 📄 License, 🚀 Setup & Usage (+2 more)

### Community 6 - "execute_handoff"
Cohesion: 0.17
Nodes (14): cancel_active_drag_and_release_mouse(), classify_window(), DragKind, Browser, Explorer, Other, execute_handoff(), looks_like_url() (+6 more)

### Community 7 - "keyboard_hook_proc"
Cohesion: 0.18
Nodes (11): get_config_ip(), keyboard_hook_proc(), main(), mouse_hook_proc(), read_clipboard_unicode_text(), send_fast_bytes(), start_clipboard_sync(), start_connection_manager() (+3 more)

### Community 8 - "AtomicBool"
Cohesion: 0.20
Nodes (9): ALT_DOWN, CTRL_DOWN, HANDOFF_BUSY, IGNORE_WARP, IS_CONNECTED, IS_REMOTE, LBUTTON_DOWN, SUPPRESS_CLIPBOARD_SYNC (+1 more)

### Community 10 - "handle_connection"
Cohesion: 0.18
Nodes (9): copy_text_to_wayland_clipboard(), decode_base64(), downloads_dir(), handle_connection(), notify_files_received(), open_url_in_browser(), receive_binary_file(), sanitize_relative_path() (+1 more)

### Community 13 - "AtomicI32"
Cohesion: 0.29
Nodes (6): CENTER_X, CENTER_Y, DRAG_START_X, DRAG_START_Y, SCREEN_HEIGHT, SCREEN_WIDTH

### Community 15 - "resolve_target_addr"
Cohesion: 0.50
Nodes (3): collect_transfer_entries(), resolve_target_addr(), stream_files_to_omarchy()

## Knowledge Gaps
- **5 isolated node(s):** `tailscale-kvm-receiver`, `tailscale-kvm-host`, `Browser`, `Explorer`, `Other`
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 63 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **10 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `keyboard_hook_proc()` connect `keyboard_hook_proc` to `windows-host/src/main.rs`, `execute_handoff`?**
  _High betweenness centrality (0.040) - this node is a cross-community bridge._
- **What connects `tailscale-kvm-receiver`, `tailscale-kvm-host`, `Browser` to the rest of the system?**
  _5 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `windows-host/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07142857142857142 - nodes in this community are weakly interconnected._
- **Why does `DragKind` connect `execute_handoff` to `windows-host/src/main.rs`?**
  _High betweenness centrality (0.034) - this node is a cross-community bridge._
- **Should `linux-receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07407407407407407 - nodes in this community are weakly interconnected._
- **Why does `mouse_hook_proc()` connect `keyboard_hook_proc` to `windows-host/src/main.rs`?**
  _High betweenness centrality (0.016) - this node is a cross-community bridge._