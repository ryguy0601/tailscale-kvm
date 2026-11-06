# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 6 files · ~3,342 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 5 file(s) not represented in the graph (top: (none) 3, .bat 2)

## Summary
- 40 nodes · 51 edges · 8 communities (2 shown, 6 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 2 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `c4c8086f`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- kvm_client.py
- send_msg
- translate_vk
- setup_receiver.sh
- ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland
- win32_mouse_filter

## God Nodes (most connected - your core abstractions)
1. `⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland` - 7 edges
2. `send_msg()` - 6 edges
3. `win32_keyboard_filter()` - 4 edges
4. `handle_client()` - 4 edges
5. `switch_to_local()` - 3 edges
6. `win32_mouse_filter()` - 3 edges
7. `main()` - 3 edges
8. `translate_vk()` - 3 edges
9. `🚀 Setup & Installation` - 3 edges
10. `connect_to_omarchy()` - 2 edges

## Surprising Connections (you probably didn't know these)
- `win32_keyboard_filter()` --calls--> `translate_vk()`  [EXTRACTED]
  windows/kvm_host.py → windows/vk_to_evdev.py

## Import Cycles
- None detected.

## Communities (8 total, 6 thin omitted)

### Community 2 - "send_msg"
Cohesion: 0.50
Nodes (5): clipboard_monitor(), connect_to_omarchy(), send_msg(), switch_to_local(), win32_keyboard_filter()

### Community 5 - "⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland"
Cohesion: 0.20
Nodes (9): 1. Linux Receiver Setup (Omarchy / Hyprland), 2. Windows Host Setup (`not-a-gabe-cube`), 🏗 Architecture & Tech Stack, 🕸 Graphify Knowledge Graph, 🌟 Key Features, 📄 License, 🚀 Setup & Installation, ⚡ Tailscale Software KVM: Windows Host to Omarchy / Hyprland (+1 more)

## Knowledge Gaps
- **6 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Not enough signal to generate questions. This usually means the corpus has no AMBIGUOUS edges, no bridge nodes, no INFERRED relationships, and all communities are tightly cohesive. Add more files or run with --mode deep to extract richer edges._