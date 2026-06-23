---
type: "Subagent Finding"
title: "Docking multiviewport authority audit"
timestamp: 2026-06-19T00:00:00Z
status: "complete"
subagent_ids:
  - "019edbe7-908a-7623-9d80-648065922565"
  - "019edbe7-af58-7e40-9872-57bedc108be4"
tags: ["docking", "multiviewport", "imgui", "authority"]
---

# Finding

- `active_window` and `focused_window` are not used as drop commit authority.
- `last_focused` stamps still exist, but they only support fallback ordering and focus recovery diagnostics.
- `window_stack` is the backend fallback authority when trusted hovered-window data is unavailable.
- `UniqueGeometryHit` remains a valid unique-hit fallback and matches the ImGui-style non-overlap case.

# Evidence

- `viewport_target_resolver.rs`
- `viewport_drop_route.rs`
- `viewport_runtime.rs`
- `viewport_registry.rs`

# Recommendation

- Keep backend hovered/window-stack authority explicit.
- Keep unique-geometry fallback only where the route is unambiguous.
- Avoid reintroducing active-window or last-focused as commit authority.

# Disposition

- Applied: renamed the old `SingleGeometryHit` label to `UniqueGeometryHit` and removed stale `active-window` wording from crate docs.
