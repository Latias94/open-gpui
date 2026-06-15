---
type: "Subagent Finding"
title: "Text input patterns research"
description: "Subagent research on GPUI text input primitives and TextInput/Field component boundaries."
tags: ["ui-components", "text-input", "field", "subagent"]
timestamp: 2026-06-15T09:30:00Z
subagent_id: "/root/research_text_input_patterns"
related_plan: docs/plans/2026-06-15-003-feat-ui-text-field-slice-plan.md
---

# Finding

GPUI's real text editing path is entity-based. Editable inputs should be built around
`EntityInputHandler` and `ElementInputHandler`, with registration during paint through
`Window::handle_input`. A `RenderOnce` field shell cannot safely implement IME, UTF-16 selection,
marked text, grapheme navigation, clipboard, cursor bounds, and selection painting by handling
ordinary key events.

# Evidence

- `crates/gpui/src/input.rs` defines the reusable `EntityInputHandler` and `ElementInputHandler`
  bridge.
- `crates/gpui/examples/input.rs` shows the minimal single-line input mechanics: UTF-8/UTF-16
  conversion, grapheme movement, marked ranges, cursor/selection painting, clipboard actions, and
  `Window::handle_input`.
- `repo-ref/gpui-component/crates/ui/src/input/input.rs` and
  `repo-ref/gpui-component/crates/ui/src/input/state.rs` split editable state from visual input
  chrome, but include editor-grade complexity that should not be copied wholesale.
- `repo-ref/gpui-component/crates/ui/src/form/field.rs` keeps Field as label/description/control
  composition instead of owning text editing.

# Recommendation

Keep the current TextInput/Field slice scoped to resolved state, token intent, metrics,
accessibility role/label intent, display value/placeholder, and Field composition. Plan full
editable text input as a follow-up controller slice that reuses GPUI's input handler path instead
of faking text entry in the component adapter.

# Disposition

Accepted for the 2026-06-15 TextInput/Field slice. The implementation intentionally ships a
display/semantic `TextInput` shell and `Field` composition component, while deferring full editing,
IME, selection, and `described-by`/`labelled-by` accessibility mapping to follow-up work.

# Citations

[1] [TextInput/Field plan](../../plans/2026-06-15-003-feat-ui-text-field-slice-plan.md)
[2] [Component contract guide](../../ui/component-contract.md)
[3] [ADR 0005](../../adr/0005-open-gpui-official-component-architecture.md)
