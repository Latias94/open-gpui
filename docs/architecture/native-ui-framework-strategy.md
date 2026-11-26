# Native UI Framework Strategy

`open-gpui-base` provides unstyled controls for Open GPUI applications. It uses GPUI's native
focus, keyboard, input, rendering, and accessibility services. Applications supply styling and
own their commands, validation, and document state.

## Using the Controls

- Use `Button`, `IconButton`, `Checkbox`, and `Row` with GPUI's `Styled` methods. Bind actions
  to application callbacks, and pass a checked value into each `Checkbox` render.
- Store `TextInput` as a GPUI entity. Subscribe to `TextInputEvent::Change` for text edits and
  `TextInputEvent::Submit` for Enter. IME preedit stays inside the field. Call `set_value` to
  synchronize an application-owned value without emitting a change.
- Supply stable keys to `VirtualList` and `VirtualTree`. The controls manage row interaction
  and report selection, expansion, activation, and reorder intent to the application.

Read the [public control exports](../../crates/gpui_base/src/lib.rs) and the [dockspace editor](../../examples/dockspace-editor/README.md) for a retained input example.
