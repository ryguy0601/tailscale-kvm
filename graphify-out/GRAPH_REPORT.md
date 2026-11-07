# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 15 files · ~12,516 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 4 file(s) not represented in the graph (top: (none) 3, .toml 1)

## Summary
- 264 nodes · 430 edges · 33 communities (19 shown, 14 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `a921be05`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- AtomicBool
- AtomicI32
- windows-host/src/main.rs
- linux-receiver/src/main.rs
- start_windows_server_listener
- ⚡ Tailscale Software KVM: Universal Multi-Directional & Multi-Platform Mesh (Pure Rust)
- execute_handoff
- mouse_hook_proc
- main
- HHOOK
- handle_connection
- tailscale-kvm-host
- tailscale-kvm-receiver
- AtomicI32
- EdgeDirection
- execute_reverse_handoff
- AtomicIsize
- update_current_monitor_bounds
- ReturnEdge
- ActiveApp
- main
- web_gui.rs
- resolve_target_addr
- STREAM_SENDER
- EDGE_DIRECTION
- RETURN_EDGE
- build.sh script
- run-host.sh script
- run-receiver.sh script

## God Nodes (most connected - your core abstractions)
1. `handle_connection()` - 18 edges
2. `execute_handoff()` - 14 edges
3. `mouse_hook_proc()` - 10 edges
4. `main()` - 9 edges
5. `⚡ Tailscale Software KVM: Universal Multi-Directional & Multi-Platform Mesh (Pure Rust)` - 9 edges
6. `keyboard_hook_proc()` - 8 edges
7. `EdgeDirection` - 7 edges
8. `send_fast_bytes()` - 7 edges
9. `ReturnEdge` - 7 edges
10. `start_windows_server_listener()` - 6 edges

## Surprising Connections (you probably didn't know these)
- `mouse_hook_proc()` --calls--> `get_slot_peer()`  [EXTRACTED]
  host/src/main.rs → host/src/web_gui.rs
- `keyboard_hook_proc()` --calls--> `translate_vk()`  [EXTRACTED]
  host/src/main.rs → host/src/vk_map.rs
- `main()` --calls--> `set_slot_peer()`  [EXTRACTED]
  host/src/main.rs → host/src/web_gui.rs
- `main()` --calls--> `start_gui_server()`  [EXTRACTED]
  host/src/main.rs → host/src/web_gui.rs

## Import Cycles
- None detected.

## Communities (33 total, 14 thin omitted)

### Community 0 - "AtomicBool"
Cohesion: 0.18
Nodes (10): ALT_DOWN, CTRL_DOWN, HANDOFF_BUSY, IGNORE_WARP, IS_CONNECTED, IS_REMOTE, LBUTTON_DOWN, OVERLAY_ACTIVE (+2 more)

### Community 1 - "AtomicI32"
Cohesion: 0.22
Nodes (8): CURRENT_BOUND_MAX_X, CURRENT_BOUND_MAX_Y, CURRENT_BOUND_MIN_X, CURRENT_BOUND_MIN_Y, CURSOR_X, CURSOR_Y, REMOTE_DRAG_START_X, REMOTE_DRAG_START_Y

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.06
Nodes (9): CF_DIB_FMT, CF_DIBV5_FMT, CF_HDROP_FMT, CF_UNICODETEXT_FMT, FILE_CHUNK_SIZE, LAST_CLIP_SEQ, MAX_CLIPBOARD_TEXT_BYTES, MAX_IMAGE_BYTES (+1 more)

### Community 3 - "linux-receiver/src/main.rs"
Cohesion: 0.08
Nodes (7): ACTIVE_CONTROL_CLIENTS, MAX_IMAGE_BYTES, MAX_SINGLE_FILE_BYTES, REMOTE_HANDOFF_BUSY, REMOTE_MOUSE_LBUTTON, STREAM_CHUNK_SIZE, SWITCHED_SINGLE_MONITOR

### Community 4 - "start_windows_server_listener"
Cohesion: 0.29
Nodes (7): get_windows_downloads_dir(), open_terminal_on_windows(), open_url_on_windows(), receive_binary_file_windows(), sanitize_relative_path(), start_windows_server_listener(), write_clipboard_dib_bytes()

### Community 5 - "⚡ Tailscale Software KVM: Universal Multi-Directional & Multi-Platform Mesh (Pure Rust)"
Cohesion: 0.15
Nodes (12): 1. Build the Workspace, 2. Launch the Receiver (The device being controlled), 3. Launch the Host (The device with your keyboard & mouse), 🏗 Architecture & Multi-Screen Mesh, 🎮 Controls & Shortcuts, 🕸 Graphify Knowledge Graph, 🌟 Key Features, 📄 License (+4 more)

### Community 6 - "execute_handoff"
Cohesion: 0.17
Nodes (14): cancel_active_drag_and_release_mouse(), classify_window(), DragKind, Browser, Explorer, Other, execute_handoff(), looks_like_url() (+6 more)

### Community 7 - "mouse_hook_proc"
Cohesion: 0.23
Nodes (11): hide_edge_overlay(), keyboard_hook_proc(), mouse_hook_proc(), overlay_wnd_proc(), send_fast_bytes(), show_edge_overlay(), start_connection_manager(), switch_active_peer() (+3 more)

### Community 8 - "main"
Cohesion: 0.29
Nodes (7): auto_discover_peer_ip(), get_config_ip(), init_edge_overlay(), main(), read_clipboard_dib_bytes(), read_clipboard_unicode_text(), start_clipboard_sync()

### Community 10 - "handle_connection"
Cohesion: 0.16
Nodes (11): copy_text_to_wayland_clipboard(), decode_base64(), downloads_dir(), handle_connection(), notify_files_received(), open_terminal_at_path(), open_url_in_browser(), receive_binary_file() (+3 more)

### Community 13 - "AtomicI32"
Cohesion: 0.29
Nodes (6): CENTER_X, CENTER_Y, DRAG_START_X, DRAG_START_Y, SCREEN_HEIGHT, SCREEN_WIDTH

### Community 14 - "EdgeDirection"
Cohesion: 0.33
Nodes (6): EdgeDirection, Down, Left, Right, Up, get_config_direction()

### Community 15 - "execute_reverse_handoff"
Cohesion: 0.17
Nodes (9): ACTIVE_CONTROL_WRITER, ACTIVE_WINDOWS_PEER, collect_transfer_entries(), execute_reverse_handoff(), LAST_WAYLAND_CLIP_TEXT, looks_like_url(), parse_file_uris(), percent_decode_str() (+1 more)

### Community 17 - "update_current_monitor_bounds"
Cohesion: 0.33
Nodes (7): apply_single_monitor_if_docked(), get_hyprland_bounds(), mark_control_session(), monitor_counts(), MonitorBounds, restore_docked_monitors_if_needed(), update_current_monitor_bounds()

### Community 18 - "ReturnEdge"
Cohesion: 0.33
Nodes (6): parse_return_edge(), ReturnEdge, Bottom, Left, Right, Top

### Community 19 - "ActiveApp"
Cohesion: 0.40
Nodes (6): ActiveApp, Browser, FileManager, Other, Terminal, get_active_hyprland_app()

### Community 21 - "main"
Cohesion: 0.50
Nodes (4): escape_text_for_transmission(), main(), start_cursorpos_worker(), start_wayland_clipboard_monitor()

### Community 22 - "web_gui.rs"
Cohesion: 0.15
Nodes (11): get_slot(), get_slot_peer(), get_tailscale_peers_json(), GUI_ACTIVE_DOWN, GUI_ACTIVE_LEFT, GUI_ACTIVE_RIGHT, GUI_ACTIVE_UP, handle_http_client() (+3 more)

### Community 24 - "resolve_target_addr"
Cohesion: 0.40
Nodes (4): collect_transfer_entries(), resolve_target_addr(), stream_files_to_omarchy(), stream_image_to_omarchy()

## Knowledge Gaps
- **16 isolated node(s):** `tailscale-kvm-host`, `Right`, `Left`, `Up`, `Down` (+11 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 94 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **14 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ReturnEdge` connect `ReturnEdge` to `linux-receiver/src/main.rs`?**
  _High betweenness centrality (0.031) - this node is a cross-community bridge._
- **What connects `tailscale-kvm-host`, `Right`, `Left` to the rest of the system?**
  _16 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `windows-host/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.057057057057057055 - nodes in this community are weakly interconnected._
- **Why does `EdgeDirection` connect `EdgeDirection` to `windows-host/src/main.rs`?**
  _High betweenness centrality (0.031) - this node is a cross-community bridge._
- **Should `linux-receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07586206896551724 - nodes in this community are weakly interconnected._
- **Why does `ActiveApp` connect `ActiveApp` to `linux-receiver/src/main.rs`?**
  _High betweenness centrality (0.024) - this node is a cross-community bridge._