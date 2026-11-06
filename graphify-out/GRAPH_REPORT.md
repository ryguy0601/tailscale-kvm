# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 7 files · ~2,535 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 3 file(s) not represented in the graph (top: (none) 3)

## Summary
- 85 nodes · 107 edges · 16 communities (6 shown, 10 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `5d78a02d`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- IncomingMessage
- HostMessage
- windows-host/src/main.rs
- handle_connection
- Building & Running
- main
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
1. `HostMessage` - 7 edges
2. `keyboard_hook_proc()` - 7 edges
3. `IncomingMessage` - 6 edges
4. `handle_connection()` - 6 edges
5. `mouse_hook_proc()` - 6 edges
6. `send_payload()` - 5 edges
7. `start_receiver_listener()` - 4 edges
8. `main()` - 4 edges
9. `main()` - 3 edges
10. `STREAM_SENDER` - 3 edges

## Surprising Connections (you probably didn't know these)
- `keyboard_hook_proc()` --calls--> `translate_vk()`  [EXTRACTED]
  windows-host/src/main.rs → windows-host/src/vk_map.rs

## Import Cycles
- None detected.

## Communities (16 total, 10 thin omitted)

### Community 0 - "IncomingMessage"
Cohesion: 0.33
Nodes (6): IncomingMessage, Clipboard, Key, MouseClick, MouseMove, MouseScroll

### Community 1 - "HostMessage"
Cohesion: 0.33
Nodes (6): HostMessage, Clipboard, Key, MouseClick, MouseMove, MouseScroll

### Community 2 - "windows-host/src/main.rs"
Cohesion: 0.13
Nodes (4): CENTER_X, CENTER_Y, SCREEN_HEIGHT, SCREEN_WIDTH

### Community 5 - "Building & Running"
Cohesion: 0.33
Nodes (5): Architecture, Building & Running, Linux Receiver (`linux-receiver`), Tailscale KVM (Rust Edition), Windows Host (`windows-host`)

### Community 6 - "main"
Cohesion: 0.40
Nodes (5): get_config_ip(), main(), start_clipboard_sync(), start_receiver_listener(), switch_to_local()

### Community 7 - "keyboard_hook_proc"
Cohesion: 0.31
Nodes (5): keyboard_hook_proc(), mouse_hook_proc(), send_payload(), switch_to_remote(), translate_vk()

## Knowledge Gaps
- **14 isolated node(s):** `tailscale-kvm-receiver`, `MouseMove`, `MouseClick`, `MouseScroll`, `Key` (+9 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 46 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **10 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `IncomingMessage` connect `IncomingMessage` to `linux-receiver/src/main.rs`?**
  _High betweenness centrality (0.105) - this node is a cross-community bridge._
- **What connects `tailscale-kvm-receiver`, `MouseMove`, `MouseClick` to the rest of the system?**
  _14 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `windows-host/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.13333333333333333 - nodes in this community are weakly interconnected._
- **Why does `HostMessage` connect `HostMessage` to `windows-host/src/main.rs`, `keyboard_hook_proc`?**
  _High betweenness centrality (0.105) - this node is a cross-community bridge._
- **Should `linux-receiver/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.125 - nodes in this community are weakly interconnected._
- **Why does `keyboard_hook_proc()` connect `keyboard_hook_proc` to `windows-host/src/main.rs`, `main`?**
  _High betweenness centrality (0.073) - this node is a cross-community bridge._