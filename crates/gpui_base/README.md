# Open GPUI Base

Open GPUI Base provides unstyled controls built on Open GPUI. Use `Styled` on immediate elements and `StyleRefinement` setters on entity-backed controls. Applications supply colors, spacing, icons, and domain state.

`Button`, `IconButton`, `Checkbox`, `Row`, `Toolbar`, `Collapsible`, `ScrollArea`, and `Tooltip` compose directly with `div()`. `TextInput`, `NumericScrubber`, `Slider`, `VirtualList`, `Select`, `Menu`, `Split`, `Modal`, and `ColorPicker` are `Render` entities.

## Editing

```rust
use open_gpui_base::{TextInput, TextInputEvent};

let input = cx.new(|cx| TextInput::new("Initial name", cx));
let subscription = cx.subscribe(&input, |view, _, event, cx| {
    if let TextInputEvent::Submit(value) = event {
        view.rename(value.as_ref(), cx);
    }
});
```

Keep the subscription with the owning view and render the `input` entity as a child. `Change` reports committed text, including an IME commit. `Submit` reports Enter, `Cancel` reports Escape, and `Blur` lets the application choose its own focus-loss policy. `set_value` synchronizes external changes silently. Use `set_value_in` when native text services also need notice in the active window.

`Slider` and `NumericScrubber` emit `NumericEvent::Change` during a drag and `Commit` or `Cancel` when it ends. The scrubber opens its text field on double-click or Enter. Use `with_text_entry(false)` when a separate field owns the text draft. Shift makes dragging finer. Control on Windows and Linux, or Command on macOS, snaps to a step.

## Virtual rows

Create a `VirtualList<K>` or `VirtualTree<K>` entity and supply `TreeRow::new(key, parent, label)` values with unique keys. Input order defines sibling order. Give the list a bounded height so GPUI can render the visible range. `set_row_renderer` receives selection, focus, depth, and expansion state for each requested row.

Shift extends a visible selection range. The platform's primary shortcut modifier toggles an individual row. Enable drag reordering with `set_reorder_enabled`. Handle `ReorderRequested` in the application model, or apply it to a list-owned model with `apply_reorder`.
