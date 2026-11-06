# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 6 files · ~2,489 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 5 file(s) not represented in the graph (top: (none) 3, .bat 2)

## Summary
- 43 nodes · 59 edges · 7 communities (3 shown, 4 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 2 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `5150be1e`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- kvm_client.py
- kvm_host.py
- send_msg
- on_press
- setup_receiver.sh
- ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland

## God Nodes (most connected - your core abstractions)
1. `send_msg()` - 10 edges
2. `⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland` - 7 edges
3. `handle_client()` - 4 edges
4. `on_press()` - 4 edges
5. `translate_vk()` - 4 edges
6. `main()` - 3 edges
7. `switch_to_local()` - 3 edges
8. `on_move()` - 3 edges
9. `on_release()` - 3 edges
10. `🚀 Setup & Installation` - 3 edges

## Surprising Connections (you probably didn't know these)
- `on_press()` --calls--> `translate_vk()`  [EXTRACTED]
  windows/kvm_host.py → windows/vk_to_evdev.py
- `on_release()` --calls--> `translate_vk()`  [EXTRACTED]
  windows/kvm_host.py → windows/vk_to_evdev.py

## Import Cycles
- None detected.

## Communities (7 total, 4 thin omitted)

### Community 2 - "send_msg"
Cohesion: 0.33
Nodes (6): clipboard_monitor(), connect_to_omarchy(), on_click(), on_scroll(), send_file(), send_msg()

### Community 3 - "on_press"
Cohesion: 0.40
Nodes (4): on_press(), on_release(), switch_to_local(), translate_vk()

### Community 5 - "⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland"
Cohesion: 0.20
Nodes (9): 1. Linux Receiver Setup (Omarchy / Hyprland), 2. Windows Host Setup, 🏗 Architecture & Tech Stack, 🕸 Graphify Knowledge Graph, 🌟 Key Features, 📄 License, 🚀 Setup & Installation, ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (+1 more)

## Knowledge Gaps
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `send_msg()` connect `send_msg` to `kvm_host.py`, `on_press`?**
  _High betweenness centrality (0.020) - this node is a cross-community bridge._