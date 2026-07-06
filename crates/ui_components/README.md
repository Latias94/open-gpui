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

Supported v0.2.0 behavior includes:

- Key-based active and selected state.
- Item, section, separator, loading, empty, and error row descriptors.
- Printable-key typeahead over `text_value`, skipping disabled, structural, and duplicate-key rows.
- Replacement-style multi-select range selection with a stable key anchor.
- Measured-row virtualizer snapshots and estimated reveal targets.
- `VirtualizedListBehaviorSnapshot::sticky_section` metadata for grouped lists.
- A constrained `render_row` hook that replaces row content while the outer row keeps layout,
  accessibility, focus, hit testing, and selection behavior.

`VirtualizedList` does not currently render a sticky overlay, animate row enter/exit, or expose a
public presence API. The active indicator is paint-only motion chrome and must not mutate selection,
focus order, roles, or row geometry.

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
    Button, VirtualizedList, VirtualizedListItemDescriptor, VirtualizedListSelectionMode,
};
```

For GPUI-specific adapter helpers that are not renderer-neutral component contracts, use
`open_gpui_ui_components::gpui_adapter`.

## Verification

For focused changes in this crate, run:

```sh
cargo fmt -p open-gpui-ui-components
cargo check -p open-gpui-ui-components --tests --locked
cargo nextest run -p open-gpui-ui-components --no-fail-fast
cargo test -p open-gpui-ui-components --test public_surface --locked
```

For VirtualizedList-specific work, the fast local gates are:

```sh
cargo test -p open-gpui-ui-components --locked --lib virtualized_list_
cargo test -p open-gpui-ui-components --locked --test layout virtualized_list_runtime
```

See `docs/ui/component-contract.md` and `docs/verification.md` for the full component contract and
workspace verification matrix.
