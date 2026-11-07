# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 18 files · ~23,950 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 4 file(s) not represented in the graph (top: (none) 3, .toml 1)

## Summary
- 436 nodes · 848 edges · 32 communities (22 shown, 10 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 10 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `607beeb6`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- AtomicBool
- handle_connection
- windows-host/src/main.rs
- receiver/src/main.rs
- spawn_slot_worker
- ⚡ Tailscale Software KVM: Universal Multi-Directional & Multi-Platform Mesh (Pure Rust)
- execute_handoff
- macos_host.rs
- start_windows_clipboard_sync
- HHOOK
- linux_host.rs
- tailscale-kvm-host
- tailscale-kvm-receiver
- OnceLock
- EdgeDirection
- execute_reverse_handoff
- MonitorBounds
- update_current_monitor_bounds
- ReturnEdge
- ActiveApp
- input_adapter.rs
- start_clipboard_monitor
- web_gui.rs
- RETURN_EDGE
- main
- send_bytes_to_slot
- build.sh script

## God Nodes (most connected - your core abstractions)
1. `handle_connection()` - 25 edges
2. `execute_handoff()` - 14 edges
3. `main()` - 14 edges
4. `switch_to_local()` - 13 edges
5. `spawn_slot_worker()` - 12 edges
6. `ReturnEdge` - 12 edges
7. `run_linux_host_loop()` - 10 edges
8. `macos_tap_callback()` - 10 edges
9. `run_macos_host_loop()` - 10 edges
10. `SlotChannel` - 10 edges

## Surprising Connections (you probably didn't know these)
- `run_linux_host_loop()` --calls--> `monotonic_ms()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `run_linux_host_loop()` --calls--> `send_to_active_remote()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `run_linux_host_loop()` --calls--> `switch_to_local()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `run_linux_host_loop()` --calls--> `switch_to_remote_slot()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `macos_tap_callback()` --calls--> `send_to_active_remote()`  [INFERRED]
  host/src/macos_host.rs → host/src/main.rs

## Import Cycles
- None detected.

## Communities (32 total, 10 thin omitted)

### Community 0 - "AtomicBool"
Cohesion: 0.18
Nodes (10): ALT_DOWN, CTRL_DOWN, HANDOFF_BUSY, IGNORE_WARP, IS_REMOTE, LBUTTON_DOWN, OVERLAY_ACTIVE, SLOT_CONNECTED (+2 more)

### Community 1 - "handle_connection"
Cohesion: 0.16
Nodes (12): copy_text_to_system_clipboard(), decode_base64(), downloads_dir(), handle_connection(), notify_files_received(), open_terminal_at_path(), open_url_in_browser(), receive_binary_file() (+4 more)

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.05
Nodes (17): ACTIVE_REMOTE_SLOT, CF_DIB_FMT, CF_DIBV5_FMT, CF_HDROP_FMT, CF_UNICODETEXT_FMT, DRAG_CLICK_HWND, FILE_CHUNK_SIZE, LAST_CLIP_SEQ (+9 more)

### Community 3 - "receiver/src/main.rs"
Cohesion: 0.08
Nodes (19): ACTIVE_CONTROL_CLIENTS, ACTIVE_SESSION_ID, CURRENT_BOUND_MAX_X, CURRENT_BOUND_MAX_Y, CURRENT_BOUND_MIN_X, CURRENT_BOUND_MIN_Y, CURSOR_X, CURSOR_Y (+11 more)

### Community 4 - "spawn_slot_worker"
Cohesion: 0.14
Nodes (13): write_linux_clipboard_text(), write_macos_clipboard_text(), get_host_downloads_dir(), open_terminal_on_host(), open_url_on_host(), receive_binary_file_host(), sanitize_relative_path(), spawn_slot_worker() (+5 more)

### Community 5 - "⚡ Tailscale Software KVM: Universal Multi-Directional & Multi-Platform Mesh (Pure Rust)"
Cohesion: 0.15
Nodes (12): 1. Build the Workspace, 2. Launch the Receiver (Each device to be controlled), 3. Launch the Host (The device with your physical keyboard & mouse), 🏗 Architecture & Multi-Screen Mesh, 🎮 Controls & Shortcuts, 🕸 Graphify Knowledge Graph, 🌟 Key Features, 📄 License (+4 more)

### Community 6 - "execute_handoff"
Cohesion: 0.12
Nodes (20): cancel_active_drag_and_release_mouse(), classify_window(), DragKind, Browser, Explorer, Other, execute_handoff(), first_connected_slot() (+12 more)

### Community 7 - "macos_host.rs"
Cohesion: 0.07
Nodes (50): warp_linux_cursor(), CFMachPortCreateRunLoopSource(), CFRunLoopAddSource(), CFRunLoopGetCurrent(), CFRunLoopRun(), CGAssociateMouseAndMouseCursorPosition(), CGDisplayPixelsHigh(), CGDisplayPixelsWide() (+42 more)

### Community 8 - "start_windows_clipboard_sync"
Cohesion: 0.22
Nodes (9): any_slot_connected(), collect_transfer_entries(), read_clipboard_dib_bytes(), read_clipboard_unicode_text(), resolve_target_addr(), slot_target_ip(), start_windows_clipboard_sync(), stream_files_to_receiver() (+1 more)

### Community 10 - "linux_host.rs"
Cohesion: 0.09
Nodes (16): fcntl(), libc_fcntl_getfl(), libc_fcntl_setfl(), open_physical_input_devices(), query_linux_cursor_pos(), run_linux_host_loop(), start_linux_clipboard_sync(), update_linux_screen_bounds() (+8 more)

### Community 14 - "EdgeDirection"
Cohesion: 0.18
Nodes (10): EdgeDirection, Down, Left, Right, Up, explicit_cli_target(), get_config_direction(), legacy_env_or_config_target() (+2 more)

### Community 15 - "execute_reverse_handoff"
Cohesion: 0.25
Nodes (7): ACTIVE_HOST_PEER, collect_transfer_entries(), execute_reverse_handoff(), looks_like_url(), parse_file_uris(), percent_decode_str(), stream_files_to_host()

### Community 16 - "MonitorBounds"
Cohesion: 0.22
Nodes (9): ACTIVE_MONITORS, compute_entry_position(), is_at_outer_return_edge(), LAST_SYNCED_CLIP_TEXT, MonitorBounds, monitors_list(), select_entry_monitor(), START_INSTANT (+1 more)

### Community 17 - "update_current_monitor_bounds"
Cohesion: 0.29
Nodes (7): ACTIVE_CONTROL_WRITER, apply_single_monitor_if_docked_async(), get_linux_monitors(), mark_control_session(), monitor_counts(), restore_docked_monitors_if_needed_async(), update_current_monitor_bounds()

### Community 18 - "ReturnEdge"
Cohesion: 0.19
Nodes (10): main(), monotonic_ms(), parse_return_edge(), ReturnEdge, Bottom, Left, Right, Top (+2 more)

### Community 19 - "ActiveApp"
Cohesion: 0.40
Nodes (6): ActiveApp, Browser, FileManager, Other, Terminal, get_active_hyprland_app()

### Community 20 - "input_adapter.rs"
Cohesion: 0.07
Nodes (35): CFRelease(), CGDisplayPixelsHigh(), CGDisplayPixelsWide(), CGEventCreate(), CGEventCreateKeyboardEvent(), CGEventCreateMouseEvent(), CGEventCreateScrollWheelEvent(), CGEventGetLocation() (+27 more)

### Community 21 - "start_clipboard_monitor"
Cohesion: 0.67
Nodes (3): escape_text_for_transmission(), read_windows_clipboard_text(), start_clipboard_monitor()

### Community 22 - "web_gui.rs"
Cohesion: 0.14
Nodes (20): build_status_json(), CACHED_PEERS, get_all_slots(), GUI_SLOTS, handle_http_client(), index_to_slot_name(), normalize_slot_ip(), open_browser() (+12 more)

### Community 24 - "main"
Cohesion: 0.40
Nodes (6): init_edge_overlay(), main(), print_usage(), start_all_slot_workers(), start_windows_bounds_refresher(), update_windows_screen_bounds()

### Community 25 - "send_bytes_to_slot"
Cohesion: 0.19
Nodes (8): apply_slot_layout(), broadcast_clipboard_text(), LAST_LOCAL_RETURN_MS, pool(), save_layout_file(), send_bytes_to_slot(), SlotChannel, test_layout_deduplication()

### Community 28 - "build.sh script"
Cohesion: 0.33
Nodes (3): build.sh script, run-host.sh script, run-receiver.sh script

## Knowledge Gaps
- **16 isolated node(s):** `tailscale-kvm-host`, `Right`, `Left`, `Up`, `Down` (+11 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 122 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **10 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `InputAdapter` connect `input_adapter.rs` to `handle_connection`, `receiver/src/main.rs`?**
  _High betweenness centrality (0.053) - this node is a cross-community bridge._
- **Are the 2 inferred relationships involving `switch_to_local()` (e.g. with `run_linux_host_loop()` and `macos_tap_callback()`) actually correct?**
  _`switch_to_local()` has 2 INFERRED edges - model-reasoned connections that need verification._
- **What connects `tailscale-kvm-host`, `Right`, `Left` to the rest of the system?**
  _16 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `windows-host/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.05204872646733112 - nodes in this community are weakly interconnected._
- **Why does `EdgeDirection` connect `EdgeDirection` to `linux_host.rs`, `windows-host/src/main.rs`?**
  _High betweenness centrality (0.025) - this node is a cross-community bridge._
- **Should `receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07807807807807808 - nodes in this community are weakly interconnected._
- **Why does `ReturnEdge` connect `ReturnEdge` to `MonitorBounds`, `receiver/src/main.rs`?**
  _High betweenness centrality (0.021) - this node is a cross-community bridge._