# Graph Report - tailscale-kvm  (2026-11-06)

## Corpus Check
- 4 files · ~1,130 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 30 nodes · 46 edges · 6 communities (2 shown, 4 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 1 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- kvm_client.py
- send_msg
- on_press
- setup_receiver.sh
- on_move

## God Nodes (most connected - your core abstractions)
1. `send_msg()` - 8 edges
2. `handle_client()` - 4 edges
3. `on_press()` - 4 edges
4. `translate_vk()` - 4 edges
5. `main()` - 3 edges
6. `on_move()` - 3 edges
7. `on_release()` - 3 edges
8. `send_file()` - 2 edges
9. `clipboard_monitor()` - 2 edges
10. `switch_to_remote()` - 2 edges

## Surprising Connections (you probably didn't know these)
- `on_press()` --calls--> `translate_vk()`  [EXTRACTED]
  windows/kvm_host.py → windows/vk_to_evdev.py
- `on_release()` --calls--> `translate_vk()`  [EXTRACTED]
  windows/kvm_host.py → windows/vk_to_evdev.py

## Import Cycles
- None detected.

## Communities (6 total, 4 thin omitted)

### Community 2 - "send_msg"
Cohesion: 0.40
Nodes (5): clipboard_monitor(), on_click(), on_scroll(), send_file(), send_msg()

### Community 3 - "on_press"
Cohesion: 0.40
Nodes (4): on_press(), on_release(), switch_to_local(), translate_vk()

## Knowledge Gaps
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `send_msg()` connect `send_msg` to `kvm_host.py`, `on_press`, `on_move`?**
  _High betweenness centrality (0.025) - this node is a cross-community bridge._
- **Why does `on_press()` connect `on_press` to `kvm_host.py`, `send_msg`?**
  _High betweenness centrality (0.003) - this node is a cross-community bridge._