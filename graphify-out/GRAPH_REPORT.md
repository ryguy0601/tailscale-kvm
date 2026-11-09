# Graph Report - tailscale-kvm  (2026-11-08)

## Corpus Check
- 23 files · ~39,640 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 4 file(s) not represented in the graph (top: (none) 3, .toml 1)

## Summary
- 551 nodes · 1083 edges · 47 communities (26 shown, 21 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 12 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `2a4fff61`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- AtomicBool
- handle_connection
- windows-host/src/main.rs
- receiver/src/main.rs
- execute_handoff
- ⚡ Tailscale Software KVM — Universal Multi-Directional Mesh (Pure Rust)
- monotonic_ms
- macos_host.rs
- start_windows_clipboard_sync
- HHOOK
- linux_host.rs
- tailscale-kvm-host
- tailscale-kvm-receiver
- screen_record.rs
- ActiveApp
- mark_control_session
- ReturnEdge
- send_bytes_to_slot
- input_adapter.rs
- main
- web_gui.rs
- handle_receiver_record_command
- spawn_slot_worker
- Mutex
- stitch-demo.sh script
- build.sh script
- EdgeDirection
- AtomicU8
- main
- AtomicIsize
- AtomicU64

## God Nodes (most connected - your core abstractions)
1. `handle_connection()` - 28 edges
2. `execute_handoff()` - 16 edges
3. `send_bytes_to_slot()` - 14 edges
4. `spawn_slot_worker()` - 13 edges
5. `ReturnEdge` - 12 edges
6. `switch_to_local()` - 12 edges
7. `monotonic_ms()` - 12 edges
8. `SlotChannel` - 11 edges
9. `main()` - 11 edges
10. `rec_coord()` - 11 edges

## Surprising Connections (you probably didn't know these)
- `run_linux_host_loop()` --calls--> `monotonic_ms()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `run_linux_host_loop()` --calls--> `send_to_active_remote()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `run_linux_host_loop()` --calls--> `switch_to_local()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `run_linux_host_loop()` --calls--> `switch_to_remote_slot()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `macos_tap_callback()` --calls--> `switch_to_local()`  [INFERRED]
  host/src/macos_host.rs → host/src/main.rs

## Import Cycles
- None detected.

## Communities (47 total, 21 thin omitted)

### Community 0 - "AtomicBool"
Cohesion: 0.18
Nodes (10): ALT_DOWN, CTRL_DOWN, HANDOFF_BUSY, IGNORE_WARP, IS_REMOTE, LBUTTON_DOWN, OVERLAY_ACTIVE, SLOT_CONNECTED (+2 more)

### Community 1 - "handle_connection"
Cohesion: 0.15
Nodes (13): copy_text_to_system_clipboard(), decode_base64(), downloads_dir(), handle_connection(), lock_receiver_workstation(), notify_files_received(), open_terminal_at_path(), open_url_in_browser() (+5 more)

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.05
Nodes (12): CF_DIB_FMT, CF_DIBV5_FMT, CF_HDROP_FMT, CF_UNICODETEXT_FMT, FILE_CHUNK_SIZE, LAYOUT_FILE_NAME, MAX_CLIPBOARD_TEXT_BYTES, MAX_IMAGE_BYTES (+4 more)

### Community 3 - "receiver/src/main.rs"
Cohesion: 0.07
Nodes (28): ACTIVE_CONTROL_CLIENTS, ACTIVE_SESSION_ID, collect_transfer_entries(), CURRENT_BOUND_MAX_X, CURRENT_BOUND_MAX_Y, CURRENT_BOUND_MIN_X, CURRENT_BOUND_MIN_Y, CURSOR_X (+20 more)

### Community 4 - "execute_handoff"
Cohesion: 0.09
Nodes (25): cancel_active_drag_and_release_mouse(), classify_window(), DragKind, Browser, Explorer, Other, enum_monitors_callback(), execute_handoff() (+17 more)

### Community 5 - "⚡ Tailscale Software KVM — Universal Multi-Directional Mesh (Pure Rust)"
Cohesion: 0.13
Nodes (14): 1. Build the Workspace, 2. Enable Boot-Time Linux `/dev/uinput` Access & Receiver Auto-Start, 3. Run Everywhere (Unified Node), 🏗 Architecture & Tech Stack, 🎮 Controls & Shortcuts, 🎬 Dual-Computer Demo Recording & Portfolio Sync, 🕸 Graphify Knowledge Graph, 🌟 Key Features (+6 more)

### Community 6 - "monotonic_ms"
Cohesion: 0.32
Nodes (10): build_recording_status_json(), export_latest_recording(), first_connected_slot(), HostRecState, monotonic_ms(), on_rec_control_line(), rec_coord(), slot_caps_store() (+2 more)

### Community 7 - "macos_host.rs"
Cohesion: 0.07
Nodes (45): CFMachPortCreateRunLoopSource(), CFRunLoopAddSource(), CFRunLoopGetCurrent(), CFRunLoopRun(), CGAssociateMouseAndMouseCursorPosition(), CGDisplayPixelsHigh(), CGDisplayPixelsWide(), CGEventGetIntegerValueField() (+37 more)

### Community 8 - "start_windows_clipboard_sync"
Cohesion: 0.33
Nodes (6): collect_transfer_entries(), resolve_target_addr(), slot_target_ip(), stream_files_to_receiver(), stream_image_to_connected_receivers(), stream_recording_file_to_receiver()

### Community 10 - "linux_host.rs"
Cohesion: 0.09
Nodes (16): fcntl(), libc_fcntl_getfl(), libc_fcntl_setfl(), open_physical_input_devices(), query_linux_cursor_pos(), run_linux_host_loop(), start_linux_clipboard_sync(), update_linux_screen_bounds() (+8 more)

### Community 13 - "screen_record.rs"
Cohesion: 0.08
Nodes (35): active_captures(), ACTIVE_CAPTURES, ActiveCaptureProcess, build_stitch_filtergraph(), build_windows_mjpeg_avi_ps1(), CaptureStartInfo, command_exists(), CREATE_NO_WINDOW (+27 more)

### Community 15 - "ActiveApp"
Cohesion: 0.40
Nodes (6): ActiveApp, Browser, FileManager, Other, Terminal, get_active_hyprland_app()

### Community 16 - "mark_control_session"
Cohesion: 0.17
Nodes (13): ACTIVE_MONITORS, apply_single_monitor_if_docked_async(), control_active_notify(), CONTROL_ACTIVE_NOTIFY, get_linux_monitors(), mark_control_session(), monitor_counts(), MonitorBounds (+5 more)

### Community 18 - "ReturnEdge"
Cohesion: 0.20
Nodes (9): compute_entry_position(), is_at_outer_return_edge(), parse_return_edge(), ReturnEdge, Bottom, Left, Right, Top (+1 more)

### Community 19 - "send_bytes_to_slot"
Cohesion: 0.15
Nodes (13): apply_slot_layout(), broadcast_sys_lock(), get_edge_filters(), get_slot_rtt_ms(), LAST_CLIP_SEQ, pool(), save_layout_file(), send_bytes_to_slot() (+5 more)

### Community 20 - "input_adapter.rs"
Cohesion: 0.08
Nodes (36): CFRelease(), CGDisplayPixelsHigh(), CGDisplayPixelsWide(), CGEventCreate(), CGEventCreateKeyboardEvent(), CGEventCreateMouseEvent(), CGEventCreateScrollWheelEvent(), CGEventGetLocation() (+28 more)

### Community 21 - "main"
Cohesion: 0.31
Nodes (8): escape_text_for_transmission(), main(), monotonic_ms(), read_windows_clipboard_text(), start_bounds_refresh_worker(), start_clipboard_monitor(), start_cursorpos_worker(), wait_until_control_active()

### Community 22 - "web_gui.rs"
Cohesion: 0.10
Nodes (22): build_status_json(), CACHED_PEERS, get_all_slots(), GUI_SLOTS, handle_http_client(), index_to_slot_name(), LAST_GUI_ACTIVITY_MS, mark_gui_active() (+14 more)

### Community 23 - "handle_receiver_record_command"
Cohesion: 0.31
Nodes (6): ACTIVE_CONTROL_WRITER, ACTIVE_HOST_PEER, handle_receiver_record_command(), LAST_SYNCED_CLIP_TEXT, send_receiver_rec_caps(), stream_recording_file_to_host()

### Community 24 - "spawn_slot_worker"
Cohesion: 0.17
Nodes (12): get_host_downloads_dir(), on_rec_binary_received(), open_terminal_on_host(), open_url_on_host(), receive_binary_file_host(), sanitize_relative_path(), spawn_slot_worker(), start_reverse_transfer_listener() (+4 more)

### Community 25 - "Mutex"
Cohesion: 0.24
Nodes (8): detected_monitors(), DETECTED_MONITORS, HOST_REC_COORD, HostMonitorInfo, RecvRecCaps, SLOT_POOL, SLOT_REC_CAPS, START_INSTANT

### Community 26 - "stitch-demo.sh script"
Cohesion: 0.83
Nodes (3): escape_dt(), make_filter(), stitch-demo.sh script

### Community 28 - "build.sh script"
Cohesion: 0.22
Nodes (4): build.sh script, run-host.sh script, run-kvm.sh script, run-receiver.sh script

### Community 30 - "EdgeDirection"
Cohesion: 0.18
Nodes (10): EdgeDirection, Down, Left, Right, Up, explicit_cli_target(), get_config_direction(), legacy_env_or_config_target() (+2 more)

### Community 31 - "AtomicU8"
Cohesion: 0.22
Nodes (8): ACTIVE_REMOTE_SLOT, EDGE_DWELL_SLOT, EDGE_FILTER_DOWN, EDGE_FILTER_LEFT, EDGE_FILTER_RIGHT, EDGE_FILTER_UP, LAST_EXIT_MONITOR, OVERLAY_DIR

### Community 40 - "main"
Cohesion: 0.22
Nodes (10): any_slot_connected(), broadcast_clipboard_text(), init_edge_overlay(), main(), print_usage(), read_clipboard_dib_bytes(), read_clipboard_unicode_text(), start_windows_bounds_refresher() (+2 more)

## Knowledge Gaps
- **18 isolated node(s):** `tailscale-kvm-host`, `tailscale-kvm-receiver`, `Horizontal`, `Vertical`, `Browser` (+13 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 149 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **21 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `InputAdapter` connect `input_adapter.rs` to `handle_connection`, `receiver/src/main.rs`?**
  _High betweenness centrality (0.039) - this node is a cross-community bridge._
- **What connects `tailscale-kvm-host`, `tailscale-kvm-receiver`, `Horizontal` to the rest of the system?**
  _18 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `windows-host/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.05263157894736842 - nodes in this community are weakly interconnected._
- **Why does `EdgeDirection` connect `EdgeDirection` to `linux_host.rs`, `windows-host/src/main.rs`?**
  _High betweenness centrality (0.020) - this node is a cross-community bridge._
- **Should `receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.0743321718931475 - nodes in this community are weakly interconnected._
- **Why does `ReturnEdge` connect `ReturnEdge` to `mark_control_session`, `receiver/src/main.rs`, `main`?**
  _High betweenness centrality (0.017) - this node is a cross-community bridge._
- **Should `execute_handoff` be split into smaller, more focused modules?**
  _Cohesion score 0.0873440285204991 - nodes in this community are weakly interconnected._