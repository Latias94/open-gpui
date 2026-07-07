# Open GPUI UI Components

`open-gpui-ui-components` contains the concrete GPUI component library for Open GPUI. It sits above
`open-gpui-ui-core`: `ui-core` owns renderer-neutral contracts and geometry, while this crate owns
styled GPUI elements, runtime adapters, public component builders, and component-contract evidence.

Use this crate when an application wants first-party Open GPUI controls rather than raw GPUI
elements.

## What This Crate Owns

- Action and form controls: buttons, icon buttons, checkboxes, switches, radios, sliders, text
  input, textarea, number input, fields, labels, tags, badges, progress, skeleton, and feedback
  surfaces.
- Choice and overlay surfaces: Listbox, Select, Combobox, Menu, ContextMenu, Command, Popover,
  HoverCard, Tooltip, Dialog, AlertDialog, Sheet, and shared overlay placement/runtime helpers.
- Layout and navigation components: Toolbar, Sidebar, Tabs, Breadcrumb, Accordion, Collapsible,
  ScrollArea, Splitter, Tree, Table, and VirtualizedList.
- Theme and contract surfaces: shared theme recipes, GPUI accessibility adapters, public API
  inventory, component-contract rows, and conformance evidence used by the gallery and tests.

## VirtualizedList

`VirtualizedList` is the general flat-list primitive for large collections. It uses stable item
keys and typed row descriptors instead of index-only labels. The renderer owns the scroll handle,
focus target, row measurement cache, active-descendant indicator, and input adapter; public state
stays in `VirtualizedListState` and behavior snapshots.

Supported behavior includes:

- Key-based active and selected state.
- Item, section, separator, initial loading, append loading, prepend loading, exhausted, empty,
  error, and retry row descriptors.
- Printable-key typeahead over `text_value`, skipping disabled, structural, status, and
  duplicate-key rows.
- Replacement-style multi-select range selection with a stable key anchor.
- Measured-row virtualizer snapshots and estimated reveal targets.
- Keyed measured-row reveal after prepends.
- `VirtualizedListBehaviorSnapshot::sticky_section` and presentation-only `sticky_overlay`
  metadata for grouped lists.
- Theme-backed color recipes through `VirtualizedListColors`.
- A constrained `render_row` hook that replaces row content while the outer row keeps layout,
  accessibility, focus, hit testing, and selection behavior.
- Optional host-owned viewport control through `VirtualizedList::scroll_handle`, so application
  shells can share a GPUI `ScrollHandle` with surrounding chrome while the list keeps semantic row
  ownership.

`VirtualizedList` does not currently animate row enter/exit or expose a public presence API. The
sticky overlay and active indicator are paint-only chrome and must not mutate selection, focus
order, roles, or row geometry.

The public API is intentionally key-first. Applications should use `VirtualizedListState` methods
such as `navigation_target`, `scroll_target_for_key`, and `scroll_target_for_key_with_snapshot`
rather than index-first helper functions.

Host code should compute reveal targets from stable keys and then drive the host-owned scroll
handle. Nested row actions should use the component event containment APIs so click, key, and wheel
behavior stay explicit without replacing the list's outer row focus, hit-testing, and selection
contract.

Internally, VirtualizedList is split by responsibility: descriptors describe rows, model/state owns
semantic identity, runtime plans resolve input and viewport facts, render modules assemble GPUI
rows, style modules resolve theme-backed colors, and motion modules keep paint-only active chrome
on the shared frame-demand protocol.

## Action Projection

Command and component actions share a typed projection layer:

- `ActionDescriptor` carries id, label, shortcut, disabled reason, tooltip, accessibility
  description, and an optional renderer-neutral `ActionIconDescriptor`.
- `ActionDescriptor::from_command_descriptor` preserves `open_gpui_command::CommandDescriptor`
  metadata, including `CommandIconDescriptor`.
- `ResolvedActionState` resolves icon metadata once and feeds Button, IconButton, Toolbar, Menu,
  ContextMenu, Command, and Sidebar surfaces.
- `ActionIconDiagnostic` records missing icon resolution in a stable form for tests and tooling.

Prefer passing `ResolvedActionState` into component builders over reconstructing label, icon,
shortcut, and disabled metadata separately for each surface.

## Demos

Run the normal-checkout component gallery:

```sh
cargo run -p open-gpui-ui-foundation-gallery
```

The gallery is the durable visual and conformance surface for official components. It includes
focused component-family views, state-contract readouts, runtime smoke probes, and examples for
Tree, Table, Splitter, overlays, and VirtualizedList.

## Imports

Most applications should use the default public surface:

```rust
use open_gpui_ui_components::{
    ActionDescriptor, Button, ResolvedActionState, VirtualizedList,
    VirtualizedListItemDescriptor, VirtualizedListSelectionMode,
};
```

For GPUI-specific adapter helpers that are not renderer-neutral component contracts, use
`open_gpui_ui_components::gpui_adapter`.

## Verification

For focused changes in this crate, run:

```sh
cargo fmt -p open-gpui-ui-components
cargo check -p open-gpui-ui-components --tests --locked
cargo nextest run -p open-gpui-ui-components --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast --locked
```

For VirtualizedList-specific work, the fast local gates are:

```sh
cargo test -p open-gpui-ui-components --locked --lib virtualized_list_
cargo test -p open-gpui-ui-components --locked --test layout virtualized_list_runtime
cargo nextest run -p open-gpui-ui-foundation-gallery virtualized_list --no-fail-fast --locked
```

See `docs/ui/component-contract.md` and `docs/verification.md` for the full component contract and
workspace verification matrix.
