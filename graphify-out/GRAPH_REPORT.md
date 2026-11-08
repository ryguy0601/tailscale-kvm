# Graph Report - tailscale-kvm  (2026-11-08)

## Corpus Check
- 21 files · ~33,974 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 4 file(s) not represented in the graph (top: (none) 3, .toml 1)

## Summary
- 513 nodes · 1040 edges · 33 communities (25 shown, 8 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 12 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `be9a009d`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- AtomicBool
- handle_connection
- windows-host/src/main.rs
- receiver/src/main.rs
- spawn_slot_worker
- ⚡ Tailscale Software KVM — Universal Multi-Directional Mesh (Pure Rust)
- execute_handoff
- macos_host.rs
- start_windows_clipboard_sync
- HHOOK
- linux_host.rs
- tailscale-kvm-host
- tailscale-kvm-receiver
- screen_record.rs
- AtomicI32
- execute_reverse_handoff
- ACTIVE_MONITORS
- update_current_monitor_bounds
- ReturnEdge
- ActiveApp
- input_adapter.rs
- main
- web_gui.rs
- handle_receiver_record_command
- main
- spawn_slot_worker
- stitch-demo.sh script
- build.sh script

## God Nodes (most connected - your core abstractions)
1. `handle_connection()` - 27 edges
2. `execute_handoff()` - 14 edges
3. `main()` - 14 edges
4. `switch_to_local()` - 13 edges
5. `spawn_slot_worker()` - 13 edges
6. `send_bytes_to_slot()` - 12 edges
7. `ReturnEdge` - 12 edges
8. `rec_coord()` - 11 edges
9. `start_dual_recording()` - 11 edges
10. `run_linux_host_loop()` - 10 edges

## Surprising Connections (you probably didn't know these)
- `run_linux_host_loop()` --calls--> `monotonic_ms()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `run_linux_host_loop()` --calls--> `send_to_active_remote()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `run_linux_host_loop()` --calls--> `switch_to_local()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `run_linux_host_loop()` --calls--> `switch_to_remote_slot()`  [INFERRED]
  host/src/linux_host.rs → host/src/main.rs
- `macos_tap_callback()` --calls--> `monotonic_ms()`  [INFERRED]
  host/src/macos_host.rs → host/src/main.rs

## Import Cycles
- None detected.

## Communities (33 total, 8 thin omitted)

### Community 0 - "AtomicBool"
Cohesion: 0.18
Nodes (10): ALT_DOWN, CTRL_DOWN, HANDOFF_BUSY, IGNORE_WARP, IS_REMOTE, LBUTTON_DOWN, OVERLAY_ACTIVE, SLOT_CONNECTED (+2 more)

### Community 1 - "handle_connection"
Cohesion: 0.16
Nodes (12): copy_text_to_system_clipboard(), decode_base64(), downloads_dir(), handle_connection(), notify_files_received(), open_terminal_at_path(), open_url_in_browser(), receive_binary_file() (+4 more)

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.05
Nodes (18): ACTIVE_REMOTE_SLOT, CF_DIB_FMT, CF_DIBV5_FMT, CF_HDROP_FMT, CF_UNICODETEXT_FMT, DRAG_CLICK_HWND, FILE_CHUNK_SIZE, LAST_CLIP_SEQ (+10 more)

### Community 3 - "receiver/src/main.rs"
Cohesion: 0.09
Nodes (20): ACTIVE_CONTROL_CLIENTS, ACTIVE_SESSION_ID, CURRENT_BOUND_MAX_X, CURRENT_BOUND_MAX_Y, CURRENT_BOUND_MIN_X, CURRENT_BOUND_MIN_Y, CURSOR_X, CURSOR_Y (+12 more)

### Community 4 - "spawn_slot_worker"
Cohesion: 0.22
Nodes (8): get_host_downloads_dir(), on_rec_binary_received(), open_terminal_on_host(), open_url_on_host(), receive_binary_file_host(), sanitize_relative_path(), start_reverse_transfer_listener(), write_clipboard_dib_bytes()

### Community 5 - "⚡ Tailscale Software KVM — Universal Multi-Directional Mesh (Pure Rust)"
Cohesion: 0.13
Nodes (14): 1. Build the Workspace, 2. Enable Boot-Time Linux `/dev/uinput` Access & Receiver Auto-Start, 3. Run Manually (Host & Receiver), 🏗 Architecture & Tech Stack, 🎮 Controls & Shortcuts, 🎬 Dual-Computer Demo Recording & Portfolio Sync, 🕸 Graphify Knowledge Graph, 🌟 Key Features (+6 more)

### Community 6 - "execute_handoff"
Cohesion: 0.17
Nodes (14): cancel_active_drag_and_release_mouse(), classify_window(), DragKind, Browser, Explorer, Other, execute_handoff(), looks_like_url() (+6 more)

### Community 7 - "macos_host.rs"
Cohesion: 0.07
Nodes (47): CFMachPortCreateRunLoopSource(), CFRunLoopAddSource(), CFRunLoopGetCurrent(), CFRunLoopRun(), CGAssociateMouseAndMouseCursorPosition(), CGDisplayPixelsHigh(), CGDisplayPixelsWide(), CGEventGetIntegerValueField() (+39 more)

### Community 8 - "start_windows_clipboard_sync"
Cohesion: 0.18
Nodes (11): any_slot_connected(), broadcast_clipboard_text(), collect_transfer_entries(), read_clipboard_dib_bytes(), read_clipboard_unicode_text(), resolve_target_addr(), slot_target_ip(), start_windows_clipboard_sync() (+3 more)

### Community 10 - "linux_host.rs"
Cohesion: 0.11
Nodes (18): fcntl(), libc_fcntl_getfl(), libc_fcntl_setfl(), open_physical_input_devices(), query_linux_cursor_pos(), run_linux_host_loop(), start_linux_clipboard_sync(), update_linux_screen_bounds() (+10 more)

### Community 13 - "screen_record.rs"
Cohesion: 0.08
Nodes (35): active_captures(), ACTIVE_CAPTURES, ActiveCaptureProcess, build_stitch_filtergraph(), build_windows_mjpeg_avi_ps1(), CaptureStartInfo, command_exists(), CREATE_NO_WINDOW (+27 more)

### Community 14 - "AtomicI32"
Cohesion: 0.22
Nodes (8): CENTER_X, CENTER_Y, DRAG_START_X, DRAG_START_Y, HOST_MAX_X, HOST_MAX_Y, HOST_MIN_X, HOST_MIN_Y

### Community 15 - "execute_reverse_handoff"
Cohesion: 0.33
Nodes (6): collect_transfer_entries(), execute_reverse_handoff(), looks_like_url(), parse_file_uris(), percent_decode_str(), stream_files_to_host()

### Community 16 - "ACTIVE_MONITORS"
Cohesion: 0.33
Nodes (4): ACTIVE_MONITORS, control_active_notify(), CONTROL_ACTIVE_NOTIFY, START_INSTANT

### Community 17 - "update_current_monitor_bounds"
Cohesion: 0.40
Nodes (6): apply_single_monitor_if_docked_async(), get_linux_monitors(), mark_control_session(), monitor_counts(), restore_docked_monitors_if_needed_async(), update_current_monitor_bounds()

### Community 18 - "ReturnEdge"
Cohesion: 0.19
Nodes (12): compute_entry_position(), is_at_outer_return_edge(), MonitorBounds, monitors_list(), parse_return_edge(), ReturnEdge, Bottom, Left (+4 more)

### Community 19 - "ActiveApp"
Cohesion: 0.40
Nodes (6): ActiveApp, Browser, FileManager, Other, Terminal, get_active_hyprland_app()

### Community 20 - "input_adapter.rs"
Cohesion: 0.07
Nodes (36): CFRelease(), CGDisplayPixelsHigh(), CGDisplayPixelsWide(), CGEventCreate(), CGEventCreateKeyboardEvent(), CGEventCreateMouseEvent(), CGEventCreateScrollWheelEvent(), CGEventGetLocation() (+28 more)

### Community 21 - "main"
Cohesion: 0.31
Nodes (8): escape_text_for_transmission(), main(), monotonic_ms(), read_windows_clipboard_text(), start_bounds_refresh_worker(), start_clipboard_monitor(), start_cursorpos_worker(), wait_until_control_active()

### Community 22 - "web_gui.rs"
Cohesion: 0.10
Nodes (23): test_layout_deduplication(), build_status_json(), CACHED_PEERS, get_all_slots(), GUI_SLOTS, handle_http_client(), index_to_slot_name(), LAST_GUI_ACTIVITY_MS (+15 more)

### Community 23 - "handle_receiver_record_command"
Cohesion: 0.31
Nodes (6): ACTIVE_CONTROL_WRITER, ACTIVE_HOST_PEER, handle_receiver_record_command(), LAST_SYNCED_CLIP_TEXT, send_receiver_rec_caps(), stream_recording_file_to_host()

### Community 24 - "main"
Cohesion: 0.20
Nodes (10): explicit_cli_target(), init_edge_overlay(), legacy_env_or_config_target(), load_layout_file(), main(), print_usage(), resolve_initial_layout(), start_all_slot_workers() (+2 more)

### Community 25 - "spawn_slot_worker"
Cohesion: 0.08
Nodes (30): warp_linux_cursor(), apply_slot_layout(), build_recording_status_json(), export_latest_recording(), first_connected_slot(), HOST_REC_COORD, HostRecState, keyboard_hook_proc() (+22 more)

### Community 26 - "stitch-demo.sh script"
Cohesion: 0.83
Nodes (3): escape_dt(), make_filter(), stitch-demo.sh script

### Community 28 - "build.sh script"
Cohesion: 0.33
Nodes (3): build.sh script, run-host.sh script, run-receiver.sh script

## Knowledge Gaps
- **18 isolated node(s):** `tailscale-kvm-host`, `Right`, `Left`, `Up`, `Down` (+13 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 130 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **8 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `InputAdapter` connect `input_adapter.rs` to `handle_connection`, `receiver/src/main.rs`?**
  _High betweenness centrality (0.042) - this node is a cross-community bridge._
- **Are the 2 inferred relationships involving `switch_to_local()` (e.g. with `run_linux_host_loop()` and `macos_tap_callback()`) actually correct?**
  _`switch_to_local()` has 2 INFERRED edges - model-reasoned connections that need verification._
- **What connects `tailscale-kvm-host`, `Right`, `Left` to the rest of the system?**
  _18 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `windows-host/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.04810360777058279 - nodes in this community are weakly interconnected._
- **Why does `EdgeDirection` connect `linux_host.rs` to `main`, `spawn_slot_worker`, `windows-host/src/main.rs`?**
  _High betweenness centrality (0.021) - this node is a cross-community bridge._
- **Should `receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.0873440285204991 - nodes in this community are weakly interconnected._
- **Why does `ReturnEdge` connect `ReturnEdge` to `receiver/src/main.rs`, `main`?**
  _High betweenness centrality (0.018) - this node is a cross-community bridge._