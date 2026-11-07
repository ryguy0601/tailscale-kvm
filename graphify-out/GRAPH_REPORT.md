# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 7 files · ~3,046 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 4 file(s) not represented in the graph (top: (none) 3, .toml 1)

## Summary
- 91 nodes · 110 edges · 18 communities (5 shown, 13 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `a8fd65b8`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- IncomingMessage
- HostMessage
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
- OutgoingMessage
- ReceiverMessage

## God Nodes (most connected - your core abstractions)
1. `⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)` - 8 edges
2. `HostMessage` - 7 edges
3. `mouse_hook_proc()` - 6 edges
4. `keyboard_hook_proc()` - 6 edges
5. `IncomingMessage` - 6 edges
6. `handle_connection()` - 6 edges
7. `send_payload()` - 5 edges
8. `start_receiver_listener()` - 4 edges
9. `main()` - 4 edges
10. `STREAM_SENDER` - 3 edges

## Surprising Connections (you probably didn't know these)
- `send_payload()` --references--> `HostMessage`  [EXTRACTED]
  windows-host/src/main.rs → windows-host/src/main.rs  _Bridges community 1 → community 7_
- `start_receiver_listener()` --references--> `TcpStream`  [EXTRACTED]
  windows-host/src/main.rs →   _Bridges community 10 → community 7_
- `main()` --calls--> `handle_connection()`  [EXTRACTED]
  linux-receiver/src/main.rs → linux-receiver/src/main.rs  _Bridges community 13 → community 4_

## Import Cycles
- None detected.

## Communities (18 total, 13 thin omitted)

### Community 0 - "IncomingMessage"
Cohesion: 0.33
Nodes (6): IncomingMessage, Clipboard, Key, MouseClick, MouseMove, MouseScroll

### Community 1 - "HostMessage"
Cohesion: 0.33
Nodes (6): HostMessage, Clipboard, Key, MouseClick, MouseMove, MouseScroll

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.14
Nodes (4): CENTER_X, CENTER_Y, SCREEN_HEIGHT, SCREEN_WIDTH

### Community 5 - "⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (Pure Rust)"
Cohesion: 0.18
Nodes (10): 1. Linux Receiver (Omarchy), 2. Windows Host (`not-a-gabe-cube`), 🏗 Architecture & Tech Stack, 🕸 Graphify Knowledge Graph, 🎮 How to Control, 🌟 Key Features, 📄 License, 🚀 Setup & Usage (+2 more)

### Community 7 - "keyboard_hook_proc"
Cohesion: 0.23
Nodes (9): get_config_ip(), keyboard_hook_proc(), main(), mouse_hook_proc(), send_payload(), start_clipboard_sync(), start_receiver_listener(), switch_to_local() (+1 more)

## Knowledge Gaps
- **14 isolated node(s):** `MouseMove`, `MouseClick`, `MouseScroll`, `Key`, `Clipboard` (+9 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 53 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **13 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `HostMessage` connect `HostMessage` to `windows-host/src/main.rs`, `keyboard_hook_proc`?**
  _High betweenness centrality (0.087) - this node is a cross-community bridge._
- **What connects `MouseMove`, `MouseClick`, `MouseScroll` to the rest of the system?**
  _14 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `windows-host/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.14285714285714285 - nodes in this community are weakly interconnected._
- **Why does `IncomingMessage` connect `IncomingMessage` to `linux-receiver/src/main.rs`?**
  _High betweenness centrality (0.087) - this node is a cross-community bridge._
- **Should `linux-receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.125 - nodes in this community are weakly interconnected._
- **Why does `keyboard_hook_proc()` connect `keyboard_hook_proc` to `windows-host/src/main.rs`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._