# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 7 files · ~3,085 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 4 file(s) not represented in the graph (top: (none) 3, .toml 1)

## Summary
- 77 nodes · 97 edges · 16 communities (5 shown, 11 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `90f49b38`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- main
- AtomicI32
- windows-host/src/main.rs
- handle_connection
- ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)
- keyboard_hook_proc
- AtomicBool
- HHOOK
- TcpStream
- tailscale-kvm-host
- tailscale-kvm-receiver
- main

## God Nodes (most connected - your core abstractions)
1. `⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)` - 8 edges
2. `mouse_hook_proc()` - 6 edges
3. `keyboard_hook_proc()` - 6 edges
4. `handle_connection()` - 5 edges
5. `main()` - 4 edges
6. `send_fast_bytes()` - 4 edges
7. `start_receiver_listener()` - 4 edges
8. `main()` - 4 edges
9. `STREAM_SENDER` - 3 edges
10. `switch_to_local()` - 3 edges

## Surprising Connections (you probably didn't know these)
- `main()` --calls--> `handle_connection()`  [EXTRACTED]
  linux-receiver/src/main.rs → linux-receiver/src/main.rs  _Bridges community 4 → community 13_
- `start_receiver_listener()` --references--> `TcpStream`  [EXTRACTED]
  windows-host/src/main.rs →   _Bridges community 10 → community 0_
- `keyboard_hook_proc()` --calls--> `switch_to_local()`  [EXTRACTED]
  windows-host/src/main.rs → windows-host/src/main.rs  _Bridges community 0 → community 7_

## Import Cycles
- None detected.

## Communities (16 total, 11 thin omitted)

### Community 0 - "main"
Cohesion: 0.50
Nodes (4): get_config_ip(), main(), start_receiver_listener(), switch_to_local()

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.18
Nodes (4): CENTER_X, CENTER_Y, SCREEN_HEIGHT, SCREEN_WIDTH

### Community 5 - "⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)"
Cohesion: 0.18
Nodes (10): 1. Linux Receiver (Omarchy), 2. Windows Host (`not-a-gabe-cube`), 🏗 Architecture & Tech Stack, 🕸 Graphify Knowledge Graph, 🎮 How to Control, 🌟 Key Features, 📄 License, 🚀 Setup & Usage (+2 more)

### Community 7 - "keyboard_hook_proc"
Cohesion: 0.36
Nodes (5): keyboard_hook_proc(), mouse_hook_proc(), send_fast_bytes(), start_clipboard_sync(), switch_to_remote()

### Community 13 - "main"
Cohesion: 0.67
Nodes (3): get_hyprland_left_bound(), main(), start_cursorpos_worker()

## Knowledge Gaps
- **2 isolated node(s):** `tailscale-kvm-host`, `tailscale-kvm-receiver`
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 40 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **11 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `keyboard_hook_proc()` connect `keyboard_hook_proc` to `main`, `windows-host/src/main.rs`?**
  _High betweenness centrality (0.030) - this node is a cross-community bridge._
- **What connects `tailscale-kvm-host`, `tailscale-kvm-receiver` to the rest of the system?**
  _2 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `linux-receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10526315789473684 - nodes in this community are weakly interconnected._
- **Why does `mouse_hook_proc()` connect `keyboard_hook_proc` to `windows-host/src/main.rs`?**
  _High betweenness centrality (0.029) - this node is a cross-community bridge._
- **Why does `handle_connection()` connect `handle_connection` to `linux-receiver/src/main.rs`, `main`?**
  _High betweenness centrality (0.021) - this node is a cross-community bridge._