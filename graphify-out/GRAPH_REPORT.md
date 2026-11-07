# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 8 files · ~10,507 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 4 file(s) not represented in the graph (top: (none) 3, .toml 1)

## Summary
- 236 nodes · 379 edges · 29 communities (18 shown, 11 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `669cffac`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- main
- AtomicI32
- windows-host/src/main.rs
- linux-receiver/src/main.rs
- update_current_monitor_bounds
- ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)
- execute_handoff
- main
- AtomicBool
- HHOOK
- handle_connection
- tailscale-kvm-host
- tailscale-kvm-receiver
- AtomicI32
- stream_files_to_windows
- AtomicIsize
- start_windows_server_listener
- AtomicBool
- ReturnEdge
- EdgeDirection
- main
- AtomicU32
- RETURN_EDGE
- OnceLock
- EDGE_DIRECTION
- ActiveApp
- execute_reverse_handoff

## God Nodes (most connected - your core abstractions)
1. `handle_connection()` - 18 edges
2. `execute_handoff()` - 14 edges
3. `mouse_hook_proc()` - 8 edges
4. `⚡ Tailscale Software KVM: Universal Multi-Directional & Multi-Platform Mesh (Pure Rust)` - 7 edges
5. `ReturnEdge` - 7 edges
6. `EdgeDirection` - 7 edges
7. `send_fast_bytes()` - 7 edges
8. `keyboard_hook_proc()` - 7 edges
9. `main()` - 7 edges
10. `ActiveApp` - 6 edges

## Surprising Connections (you probably didn't know these)
- `handle_connection()` --calls--> `restore_docked_monitors_if_needed()`  [EXTRACTED]
  linux-receiver/src/main.rs → linux-receiver/src/main.rs  _Bridges community 4 → community 10_
- `main()` --calls--> `parse_return_edge()`  [EXTRACTED]
  linux-receiver/src/main.rs → linux-receiver/src/main.rs  _Bridges community 19 → community 21_
- `main()` --calls--> `update_current_monitor_bounds()`  [EXTRACTED]
  linux-receiver/src/main.rs → linux-receiver/src/main.rs  _Bridges community 4 → community 21_
- `execute_reverse_handoff()` --calls--> `get_active_hyprland_app()`  [EXTRACTED]
  linux-receiver/src/main.rs → linux-receiver/src/main.rs  _Bridges community 26 → community 27_
- `execute_reverse_handoff()` --calls--> `stream_files_to_windows()`  [EXTRACTED]
  linux-receiver/src/main.rs → linux-receiver/src/main.rs  _Bridges community 15 → community 27_

## Import Cycles
- None detected.

## Communities (29 total, 11 thin omitted)

### Community 1 - "AtomicI32"
Cohesion: 0.22
Nodes (8): CURRENT_BOUND_MAX_X, CURRENT_BOUND_MAX_Y, CURRENT_BOUND_MIN_X, CURRENT_BOUND_MIN_Y, CURSOR_X, CURSOR_Y, REMOTE_DRAG_START_X, REMOTE_DRAG_START_Y

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.06
Nodes (7): CF_DIB_FMT, CF_DIBV5_FMT, CF_HDROP_FMT, CF_UNICODETEXT_FMT, FILE_CHUNK_SIZE, MAX_CLIPBOARD_TEXT_BYTES, MAX_IMAGE_BYTES

### Community 3 - "linux-receiver/src/main.rs"
Cohesion: 0.07
Nodes (4): ACTIVE_CONTROL_CLIENTS, MAX_IMAGE_BYTES, MAX_SINGLE_FILE_BYTES, STREAM_CHUNK_SIZE

### Community 4 - "update_current_monitor_bounds"
Cohesion: 0.33
Nodes (7): apply_single_monitor_if_docked(), get_hyprland_bounds(), mark_control_session(), monitor_counts(), MonitorBounds, restore_docked_monitors_if_needed(), update_current_monitor_bounds()

### Community 5 - "⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)"
Cohesion: 0.18
Nodes (10): 1. Starting the Receiver (Client Machine), 2. Starting the Host Controller, 🏗 Architecture & Tech Stack, Configuration File (`config.txt`), 🕸 Graphify Knowledge Graph, 🎮 How to Control, 🌟 Key Features, 📄 License (+2 more)

### Community 6 - "execute_handoff"
Cohesion: 0.13
Nodes (22): cancel_active_drag_and_release_mouse(), classify_window(), DragKind, Browser, Explorer, Other, execute_handoff(), hide_edge_overlay() (+14 more)

### Community 7 - "main"
Cohesion: 0.17
Nodes (12): auto_discover_peer_ip(), collect_transfer_entries(), get_config_ip(), init_edge_overlay(), main(), read_clipboard_dib_bytes(), read_clipboard_unicode_text(), resolve_target_addr() (+4 more)

### Community 8 - "AtomicBool"
Cohesion: 0.18
Nodes (10): ALT_DOWN, CTRL_DOWN, HANDOFF_BUSY, IGNORE_WARP, IS_CONNECTED, IS_REMOTE, LBUTTON_DOWN, OVERLAY_ACTIVE (+2 more)

### Community 10 - "handle_connection"
Cohesion: 0.16
Nodes (11): copy_text_to_wayland_clipboard(), decode_base64(), downloads_dir(), handle_connection(), notify_files_received(), open_terminal_at_path(), open_url_in_browser(), receive_binary_file() (+3 more)

### Community 13 - "AtomicI32"
Cohesion: 0.29
Nodes (6): CENTER_X, CENTER_Y, DRAG_START_X, DRAG_START_Y, SCREEN_HEIGHT, SCREEN_WIDTH

### Community 15 - "stream_files_to_windows"
Cohesion: 0.25
Nodes (5): ACTIVE_CONTROL_WRITER, ACTIVE_WINDOWS_PEER, collect_transfer_entries(), LAST_WAYLAND_CLIP_TEXT, stream_files_to_windows()

### Community 17 - "start_windows_server_listener"
Cohesion: 0.29
Nodes (7): get_windows_downloads_dir(), open_terminal_on_windows(), open_url_on_windows(), receive_binary_file_windows(), sanitize_relative_path(), start_windows_server_listener(), write_clipboard_dib_bytes()

### Community 18 - "AtomicBool"
Cohesion: 0.50
Nodes (3): REMOTE_HANDOFF_BUSY, REMOTE_MOUSE_LBUTTON, SWITCHED_SINGLE_MONITOR

### Community 19 - "ReturnEdge"
Cohesion: 0.33
Nodes (6): parse_return_edge(), ReturnEdge, Bottom, Left, Right, Top

### Community 20 - "EdgeDirection"
Cohesion: 0.33
Nodes (6): EdgeDirection, Down, Left, Right, Up, get_config_direction()

### Community 21 - "main"
Cohesion: 0.50
Nodes (4): escape_text_for_transmission(), main(), start_cursorpos_worker(), start_wayland_clipboard_monitor()

### Community 26 - "ActiveApp"
Cohesion: 0.40
Nodes (6): ActiveApp, Browser, FileManager, Other, Terminal, get_active_hyprland_app()

### Community 27 - "execute_reverse_handoff"
Cohesion: 0.50
Nodes (4): execute_reverse_handoff(), looks_like_url(), parse_file_uris(), percent_decode_str()

## Knowledge Gaps
- **16 isolated node(s):** `Left`, `Right`, `Top`, `Bottom`, `Browser` (+11 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 88 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **11 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ReturnEdge` connect `ReturnEdge` to `linux-receiver/src/main.rs`?**
  _High betweenness centrality (0.035) - this node is a cross-community bridge._
- **What connects `Left`, `Right`, `Top` to the rest of the system?**
  _16 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `windows-host/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.05555555555555555 - nodes in this community are weakly interconnected._
- **Why does `EdgeDirection` connect `EdgeDirection` to `windows-host/src/main.rs`?**
  _High betweenness centrality (0.035) - this node is a cross-community bridge._
- **Should `linux-receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.06896551724137931 - nodes in this community are weakly interconnected._
- **Why does `ActiveApp` connect `ActiveApp` to `linux-receiver/src/main.rs`?**
  _High betweenness centrality (0.028) - this node is a cross-community bridge._
- **Should `execute_handoff` be split into smaller, more focused modules?**
  _Cohesion score 0.12535612535612536 - nodes in this community are weakly interconnected._