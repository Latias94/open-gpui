---
type: "Subagent Finding"
title: "Gallery architecture review 2026-06-18"
description: "Subagent finding on remaining deletion seams in the UI foundation gallery."
timestamp: 2026-06-18T00:00:00Z
tags: ["open-gpui", "gallery", "architecture", "subagent"]
---

# Finding
- The `Listbox` / `Select` / `Combobox` / `Command` slices do not currently expose another obvious deletion seam beyond the state helpers already moved into `ListboxState` and `CommandState`.
- `ScrollAreaState` is already deep enough; do not split scroll policy further in the gallery.
- The next likely shared-rule seam is `Menu` / `ContextMenu` entry-focus handling, analogous to the `repo-ref/fret` `entry_focus` helper pattern.

# Evidence
- `repo-ref/fret` uses thin public forwarders and pure helper modules for behavior math.
- The gallery now reconstructs choice and command shells from resolved state views rather than from duplicate sample-side contract copies.
- A focused architecture review did not find another evidence-backed deletion seam in the current choice/command state layer.

# Recommendation
- Keep the current `ListboxState` / `CommandState` shape.
- If the menu path is refined next, extract shared entry-focus rules only when they remove duplicate branching and not just rename existing state.

# Disposition
- Accepted as the next candidate for the architecture loop.
