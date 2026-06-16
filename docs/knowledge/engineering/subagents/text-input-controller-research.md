---
type: "Subagent Finding"
title: "Text input controller research"
description: "Subagent research on the minimal editable TextInputController slice."
tags: ["ui-components", "text-input", "controller", "subagent"]
timestamp: 2026-06-15T16:30:00Z
subagent_id: "/root/text_input_controller_research"
related_plan: docs/plans/2026-06-15-004-feat-ui-component-roadmap-plan.md
---

# Finding

The minimal editable TextInput slice should be a single-line controller plus a GPUI adapter. It
needs GPUI's `EntityInputHandler` / `ElementInputHandler` path, UTF-16 selection conversion, IME
marked text, grapheme-aware movement/deletion, clipboard commands, and paint-time input
registration. It should not import editor-grade concepts such as multiline buffers, password
masking, completion, LSP, undo/redo, or popover menus.

# Evidence

- `crates/gpui/examples/input.rs` shows the smallest end-to-end editable input pattern.
- `crates/gpui/src/input.rs` defines the platform input handler contract used by IME and native
  text services.
- `repo-ref/gpui-component/crates/ui/src/input/state.rs` separates editable input state from
  visual chrome, but carries broader editor behavior that is too large for this slice.
- `repo-ref/fret/crates/fret-ui/src/text/input/widget.rs` and related UTF helpers are useful for
  edge-case thinking but should remain references rather than runtime dependencies.

# Recommendation

Ship a renderer-neutral `TextInputState` plus a GPUI-backed `TextInputController` entity. Keep
single-line normalization explicit, cover UTF-16 conversion and grapheme deletion in tests, and
dogfood one controller-backed sample in the gallery. Defer multiline/password/editor features until
the official component contracts repeat across more controls.

# Disposition

Accepted for the editable TextInput controller slice. The implementation keeps `Field`
composition-only, exports `TextInputController` and `init_text_input`, and wires the default gallery
TextInput sample to a controller-backed editable path.

# Citations

[1] [Official UI component roadmap](../../plans/2026-06-15-004-feat-ui-component-roadmap-plan.md)
[2] [Component contract guide](../../ui/component-contract.md)
