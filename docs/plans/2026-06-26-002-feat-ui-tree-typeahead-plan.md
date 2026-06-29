---
title: "feat: Add Tree typeahead navigation"
type: feat
date: 2026-06-26
---

# feat: Add Tree typeahead navigation

## Summary

Deepen the official `Tree` component with APG-style typeahead over visible, focusable rows. The
state layer provides deterministic prefix matching, while the GPUI adapter owns printable-key
buffering and timeout reset. The slice does not search collapsed descendants or introduce an
application-wide tree search model.

---

## Requirements

- R1. `TreeState::typeahead_target(query)` returns the next visible, focusable row whose label
  starts with the caller-owned query.
- R2. Matching starts after the currently focused row and wraps through the visible row list.
- R3. Disabled rows are ignored. Collapsed descendants are not considered because they are not in
  the visible `TreeState` list.
- R4. Empty or whitespace-only queries return no target.
- R5. The GPUI adapter captures printable key presses into an adapter-owned buffer and moves focus
  to the pure target when one exists.
- R6. Modifier shortcuts, navigation keys, expansion keys, and Enter/Space selection keep their
  existing behavior.
- R7. The Components gallery proves typeahead through a rendered Tree sample and runtime focus
  selectors.

---

## Non-Goals

- Searching unloaded or collapsed descendants.
- Highlighting matched text.
- Exposing a controlled typeahead query builder.
- App-owned search indexes.
- Virtualized tree data or async search.

---

## Implementation Units

### U1. Pure TreeState helper

- Add `TreeState::typeahead_target(query)`.
- Add unit coverage for wraparound, disabled-row skipping, and collapsed descendant exclusion.

### U2. GPUI adapter keyboard buffer

- Add adapter-owned typeahead buffer and reset deadline to `TreeRuntime`.
- Route printable key events to `TreeState::typeahead_target`.
- Move focus and scroll the matched row into view without selecting it.

### U3. Gallery proof and docs

- Add or extend a Tree gallery smoke that focuses a row and types a prefix to reach a visible
  sibling.
- Update component contract, verification docs, and engineering memory.
