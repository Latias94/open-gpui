# Verification

Run the local Open GPUI gate with:

```sh
cargo run -p xtask -- verify
```

The gate runs:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo check -p open-gpui-smoke-native`
- `cargo nextest run -p open-gpui-ui-core`
- `cargo nextest run -p open-gpui-ui-components`
- `cargo nextest run -p open-gpui-ui-foundation-gallery`
- `cargo run -p xtask -- scan-import-boundary`

For focused `open-gpui-canvas` work, run:

```sh
cargo fmt -p open-gpui-canvas
cargo check -p open-gpui-canvas --benches
cargo nextest run -p open-gpui-canvas
cargo check -p open-gpui-smoke-native
```

The canvas crate also has a large-canvas Criterion baseline:

```sh
cargo bench -p open-gpui-canvas --bench large_canvas
```

Use the benchmark to compare spatial-index, visible-query, and paint-frame culling changes. It is
not part of the default CI gate because benchmark timing is runner-dependent.

For focused `open-gpui-ui-core`, `open-gpui-ui-components`, or UI foundation gallery work, run:

```sh
cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-ui-core
cargo check -p open-gpui-ui-components
cargo check -p open-gpui-ui-foundation-gallery
cargo nextest run -p open-gpui-ui-core
cargo nextest run -p open-gpui-ui-components
cargo nextest run -p open-gpui-ui-foundation-gallery
```

The gallery package includes Components-page runtime smoke coverage for regressions that state-only
tests can miss: short-viewport page scrolling and navigation reset, navigation rail scrolling,
Select popup outside dismissal, nested ScrollArea wheel scrolling, vertical Tabs rail scrolling,
horizontal plus vertical Splitter pointer dragging, Table column resize dragging, and long Sidebar
internal navigation scrolling. Run the gallery package tests before relying on manual dogfood for
those paths.
The Components-page ScrollArea regressions also cover release-queue wheel isolation so scroll
gestures on the sample card chrome do not leak to the page shell.
Because the Components page now carries more depth samples, the longer-section smokes also rely on
catalog directory jumps and page-scroll handle alignment instead of only raw page wheel motion;
that keeps the focused inspection paths stable even as the page grows.
The Components page has two inspection modes: the full all-components conformance page, and a
catalog-driven focused component-family view. Directory chips remain pure anchor jumps. Focused
mode is entered from catalog cards and restored through the explicit `All components` control. The
focused-view proof includes a catalog-driven matrix that opens every focusable official or
state-contract catalog entry, plus focused runtime smokes for scroll reset and nested scroll
containment:

```powershell
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focuses_every_focusable_catalog_entry
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focuses_catalog_family_and_restores_all_mode
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_table_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_mode_resets_page_on_family_change
```

Table gallery gates now follow the same split: `open-gpui-ui-core` tests prove row-model,
virtualizer, column sizing, column-window, and resize-math contracts without rendering, including
grouped row ids, expansion lookup behavior, built-in group-row aggregate cells, pinned-column
region splitting, center-column virtual windows, and on-end/on-change resize deltas.
`open-gpui-ui-components` tests prove adapter exports, state metadata, resize callback wiring,
center-window header/body mounting, and scroll ownership; gallery smokes prove long table scroll
input stays inside the table viewport, `release-resize` column dragging updates the controlled
sample without moving the outer Components page, and wide center lanes scroll independently from
fixed left/right pinned lanes. The focused proofs are:

`components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample` is the focused
sticky-pinned Table proof: it enters the Table family view, scrolls the `release-rollup` center
lane horizontally, and asserts left/right pinned lanes plus the outer Components page stay fixed.

`components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample` is the focused
center-column virtualization proof: it enters the Table family view, scrolls the `release-matrix`
center lane horizontally, verifies far center metric cells are unmounted before scrolling and
mounted after scrolling, and asserts left/right pinned lanes plus the outer Components page stay
fixed.

```powershell
cargo nextest run -p open-gpui-ui-core table
cargo nextest run -p open-gpui-ui-components table
cargo nextest run -p open-gpui-ui-foundation-gallery table
```

`VirtualizedList` follows the same split at component scale: `open-gpui-ui-components` tests prove
render-plan rows, scroll-target math, PageDown reveal, and Enter/Space activation payloads, while
the gallery metadata and smoke tests prove the official catalog entry, 10k-item rendered sample,
and inner scroll containment inside the overflowing Components page. The focused proof is:

```powershell
cargo nextest run -p open-gpui-ui-components virtualized_list
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_card_wheel_does_not_leak_to_page
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_keyboard_reveals_and_activates
```

The components package includes runtime smoke coverage for Switch, TextInput, RadioGroup, Listbox,
Select, Combobox, Command, Tabs, and Toolbar keyboard navigation. The focused Switch test renders
a controlled switch, clicks its real root selector, verifies `on_change` receives the next checked
value, and confirms disabled switches do not emit changes. The focused TextInput test renders a
standalone controller-backed input, clicks its real root, accepts simulated platform text, sanitizes
single-line input, and verifies the controller caret ends at the inserted text. The focused
RadioGroup test renders real radio
items, rejects disabled clicks, skips disabled items with arrow navigation, verifies default
selection seeding, click and arrow-selection payloads, and confirms Space on an already selected radio does not emit a duplicate
selection change. The focused Listbox test renders real standalone, separator, and grouped options,
rejects disabled clicks, keeps arrow navigation selection-free, skips disabled/separator rows, and
verifies Enter and Space dispatch both option-level and listbox-level selection callbacks. The
focused Select test opens the real trigger, rejects disabled popup option clicks, verifies click and
keyboard selection payloads, closes after selection, and confirms popup Listbox arrow navigation
skips disabled rows. The focused Combobox tests click the controller-backed text input, type a
query, open the filtered popup by trigger and keyboard paths, verify filtered Listbox options, and
select filtered options with ordered select/open callbacks. The focused Command tests cover
renderer-neutral ranking, controlled and default query ownership, multi-select selected chips,
virtualized result render plans, app-owned index snapshots, inline and dialog command filtering,
keyboard activation, shortcut payloads, non-dialog content persistence, and dialog Escape/outside
press dismissal. The focused gallery Command smoke renders ranked, multi-select, virtualized, and
indexed/loading samples in focused family mode, verifies selected chips and snapshot metadata are
inspectable, and confirms wheel input on the virtualized sample does not move the surrounding card.
Run the focused proof with:

```powershell
cargo nextest run -p open-gpui-ui-components command
cargo nextest run -p open-gpui-ui-foundation-gallery command
```

The focused Tabs test renders real tabs,
preserves the `default_selected` seed on the first frame, rejects disabled tab clicks, keeps manual
arrow navigation as focus-only, and activates focused tabs with Enter and Space. The focused
Toolbar test renders real toolbar items, moves roving focus with arrow/Home keys, skips disabled and
separator items, and activates the focused item with Enter.

The components package also includes low-state primitive coverage for Separator, Kbd, Progress,
Skeleton, and Avatar. Those tests verify resolved state branches, explicit root/prelude exports,
theme color intents, stable rendered debug selectors, decorative separator semantics, progress
clamping, indeterminate progress, Avatar fallback initials, explicit accessible labels, size
metrics, `Role::Image`, and source metadata staying outside image-loading ownership. The gallery metadata and
short-viewport smoke tests also verify those primitives are listed as official catalog entries and
render visible samples with stable debug selectors.
The public API inventory gate lives in `crates/ui_components/tests/components.rs` as
`component_api_inventory_covers_official_gallery_catalog` and
`component_api_inventory_uses_stable_ownership_vocabulary`. Run the focused proof with:

```sh
cargo nextest run -p open-gpui-ui-components component_api_inventory
```

That gate checks that every official Components catalog entry has a matching API inventory row,
that overlay families are explicitly listed, that public method baselines catch top-level builder
drift, that render/controlled/default/policy vocabulary stays consistent, and that
renderer-neutral resolved state remains free of GPUI runtime types.
Feedback coverage now promotes `StatusCue` and `EmptyState` as official rendered Components
catalog entries. The focused component tests verify root/prelude exports, feedback intent labels,
resolved roles, metrics, and theme color intents. The gallery metadata tests require their
component/state `SIGNALS` entries and stable `gallery:component-status-cue-sample:{id}` /
`gallery:component-empty-state-sample:{id}` selectors, while the short-viewport smoke verifies the
real `status-cue:*:root` and `empty-state:*:root` debug selectors render.
`official_component_catalog_entries_have_signals_and_sample_selectors` is the gallery contract
gate for catalog drift: every official `COMPONENT_CATALOG` entry must have matching component and
resolved-state `SIGNALS` entries plus one rendered `gallery:component-*-sample:{id}` selector in
the Components page.
`state_contract_catalog_entries_have_signals_and_readout_selectors` is the companion pre-renderer
contract gate. Entries marked `state-contract` must declare `state_contract_selector`, must not
declare official `sample_selector`, and must stay disjoint from `official_sample_selector_pairs`.
The current state contracts are `TreeState` and `VirtualizedListState`; their signals cover state,
descriptor, action/result, helper, and payload types. `TreeState` remains a reusable hierarchy
contract even though `Tree` is now an official rendered component, matching the
`VirtualizedListState` / `VirtualizedList` split. The Components page smoke also verifies every
`state_contract_readout_pairs()` selector is visible.
The official Table gate requires `Table`, `TableState`, `VirtualizerState`, role signals for table
rows and cells, and at least one `gallery:component-table-sample:{id}` selector. Table smokes and
state tests assert that rendered row selectors stay bounded by the virtualizer's visible rows plus
overscan, scroll input stays inside the table viewport, sortable header actions emit state-update
payloads, controlled column resize callbacks carry stable sizing payloads, sort/filter state
follows stable row ids rather than numeric positions, and grouped / expanded row models keep
collapsed descendants addressable by stable row id. The Components gallery now carries
`release-rollup`, a grouped Table sample that mixes expanded and collapsed team groups, exposes
aggregate count and score cells, pins the identifier and status columns, and has its own
inner-scroll smoke. It also carries `release-resize`, a controlled column-sizing sample whose
resize smoke drags the `name` handle, records the app-owned committed width, and verifies header
and first-row cell widths stay aligned. `release-matrix` is the wide center-column virtualization
sample: it pins the identity and status lanes, exposes fourteen center metrics, and has a focused
smoke that proves off-window center columns unmount/remount while horizontal wheel input remains
inside the sample. Core table tests also assert that `TableAggregation` exposes
stable built-in aggregate labels and resolves count, sum, min, max, and average cells for grouped
rows without hiding the grouping column value. Core and component tests assert that
`TableColumnPinning` splits visible columns into left, center, and right regions after
visibility/order resolution, ignores unknown or invisible pinned ids, removes moved columns from
their previous pinned side, and exposes matching header/body region metadata and debug selectors.
The official Tree gate requires `Tree`, `TreeState`, `TreeMetrics`, tree/tree-item role signals,
and at least one `gallery:component-tree-sample:{id}` selector. Component runtime tests verify
expansion, reveal, and selection payloads; gallery smokes verify keyboard expansion/selection
through the sample runtime log and prove Tree wheel input stays inside the sample viewport.
Tree and virtualized-list state-contract samples are verified through
`components_page_samples_expose_component_metadata`: Tree readouts assert visible flattening,
disabled-row position skipping, navigation skipping, toggle payloads, and Enter/Space selection
actions; virtualized-list state-contract readouts assert active/selected indices, PageUp/PageDown
clamping, activation payloads, viewport item count, overscan, and semantic scroll strategy labels.
The same metadata test now also checks the official `Tree` sample's role metadata and keyboard
toggle payload, plus the official `VirtualizedList` sample's 10k item count, listbox roles,
active/selected state, visible range, and overscan summary.

The gallery package also includes a compact-shell runtime smoke that switches the gallery to the
compact viewport policy, verifies the derived mobile shell and compact density, scrolls the left
navigation rail to deep pages, and confirms switching away and back resets the page scroll position.

The gallery package also includes Overlay-page runtime smoke coverage for popover, modal dialog,
alert dialog, non-modal sheet, menu, and ContextMenu right-click hotspot opening plus Escape
dismissal. Popover and Dialog smokes open the real component trigger, assert Dialog initial focus,
and assert focus restoration to the trigger after outside press, modal barrier dismissal, and
Escape dismissal. The AlertDialog smoke opens the real trigger, confirms the cancel action gets the
default focus, verifies the primary action closes the dialog, and confirms Escape dismissal
restores focus to the trigger. The Overlay gallery intentionally keeps default-open contract
samples visually closed at page load so modal barriers and floating layers do not block page
scrolling; the metadata rows still report each sample's resolved default-open contract.

The focused Overlay catalog gates are:

```powershell
cargo nextest run -p open-gpui-ui-foundation-gallery overlay_page_catalog_entries_have_signals_and_sample_selectors
cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_renders_catalog_entries_and_official_samples
```

The `open-gpui-ui-core` overlay tests are the renderer-neutral gate for shared overlay behavior.
They should cover layer kind, presence, outside-press policy, Escape policy, focus restore intent,
initial focus intent, and placement input without opening a GPUI window.
The `open-gpui-ui-components` overlay helper tests should cover the GPUI adapter mapping for
deferred priority, snap margin, anchor conversion, outside-press open-change, and Escape
open-change without introducing a global overlay runtime.
For GPUI runtime focus assertions, `VisualTestContext::debug_selector_is_focused` and
`VisualTestContext::focused_debug_selector` are the preferred test hooks. They use test-only
debug-selector-to-focus-handle data and keep focus checks independent from component internals.
The `open-gpui-ui-components` public contract tests should also keep
`public_resolved_state_contracts_avoid_gpui_runtime_types` passing. That test is the hard
headless-readiness guard for public resolved-state structs: it prevents `Window`, `App`,
`Context`, `RenderOnce`, `IntoElement`, `ElementId`, `Entity`, focus handles, scroll handles, and
callback storage from entering state contracts. The companion extraction-blocker inventory tests in
`open-gpui-ui-components` and `open-gpui-ui-core` pin the extraction gate deliberately. Component
public-state blockers are currently empty: resolved overlay contracts expose `OverlayResolvedState`, while
`GpuiOverlayState` stays in the GPUI adapter helper surface for deferred priority and snap margin.
Public component metrics and accessibility state now use neutral UI-core vocabulary; adding public
GPUI `Pixels`, `Bounds`, `Point`, or `Size` aliases to resolved-state contracts should fail the
guard inventory. `open-gpui-ui-core` is now renderer-neutral: it has no `open_gpui` dependency,
no UI-core source references to `open_gpui`, and no `UiPx` conversion impls for GPUI style types.
Adaptive policies accept neutral `UiPx` thresholds and inputs instead of GPUI `Pixels`; GPUI
callers should convert their concrete window or viewport width at the adapter boundary before
invoking UI-core adaptive helpers. The companion strict-boundary inventory must stay empty.
`adapter_only_public_surfaces_match_allowlist` and
`gpui_adapter_exports_group_runtime_specific_surfaces` guard the intentionally public GPUI helper
surface: `TextInputController`, externally supplied `ScrollHandle`, `focus_ring_shadow`,
`GpuiOverlayState`, the adapter accessibility/geometry conversions, and related adapter scheduling
helpers must stay classified under `open_gpui_ui_components::gpui_adapter` instead of drifting into
the crate root, prelude default interface, or resolved state. `FocusRing` itself uses neutral
`UiPx`; only `focus_ring_shadow` returns a GPUI `BoxShadow`.

When changing GPUI accessibility repair or component metadata that creates explicit cross-node
relationships, also run:

```sh
cargo check -p open-gpui
cargo nextest run -p open-gpui --lib window::a11y::tests::repair_tree_update
```

Manual UI foundation dogfood should use the dedicated gallery after the automated checks pass:

```sh
cargo run -p open-gpui-ui-foundation-gallery
cargo run -p open-gpui-ui-foundation-gallery -- --page components
```

1. Open `Tokens` and confirm the semantic token registry shows surface, text, accent, focus ring,
   destructive, overlay, and modal overlay keys without introducing a styled component layer.
2. Open `Sizing & Density`, switch between compact and desktop from the summary panel, and confirm
   the highlighted density and default size change with the foundation policies.
3. Open `Adaptive`, use the same compact/desktop switch, and confirm device samples show mobile /
   desktop shell mode, compact / regular / expanded class, and panel samples show compact / medium /
   wide classes.
4. Open `Focus & A11y`, tab through the focusable controls, confirm the focus-visible outline is
   visible, click the counter and reset controls, and toggle the switch. The visible counter and
   switch state should match the accessible role/state vocabulary shown by the page.
5. Open `Overlay`, click `open overlay`, confirm the anchored popover appears from the trigger, then
   close it from the popover or press Escape. The geometry readout should keep anchor, layout,
   visual, preferred, and safe-window rectangles visible. The behavior contract matrix should show
   distinct tooltip, popover, dialog, and menu policies for presence, outside press, Escape, focus,
   underlay blocking, and GPUI adapter fields such as deferred priority and snap margin. In the
   Tooltip samples, hover `Hover or focus`, tab to `Focus only`, and confirm each reveals
   descriptive tooltip content while `Disabled` remains unfocusable and closed; `Manual delayed`
   should stay visible and report its custom delay policy. In the HoverCard samples, confirm
   `Profile preview` reports its default-open interactive contract without visually blocking the
   page at load, `Focus preview` opens only from keyboard focus, and `Manual card` opens and closes
   from its gallery control with pass-through or consume outside-press metadata shown in the state
   row. In the Popover samples, confirm `Default open` reports the default-open contract
  without visually blocking the page at load, `Controlled` opens and closes from its gallery
  control, Escape closes the controlled popover, outside press closes the visible popovers, and the
  `Consume outside` sample reports a consuming outside-press policy while `Disabled` remains
  closed. In the Dialog samples, open and close `Controlled modal`, confirm Escape and the modal
  barrier can close it without activating underlay controls, confirm `Default open` reports a
  blocking modal layer without visually blocking the page at load, confirm `Outside ignored`
  reports the sticky outside policy, and confirm `Disabled` stays closed. In the AlertDialog
  samples, open `Delete project`, confirm the destructive action is explicit, cancel receives the
  default focus, outside press is consumed without dismissing, Escape closes it, and focus returns
  to the trigger; confirm the safe cancel sample reports its default-open and modal-underlay
  contract without visually blocking the page at load. In the Sheet samples, confirm the left modal
  sheet reports blocking underlay input, the right non-modal sheet opens from its gallery control
  and reports pass-through outside behavior without a blocking modal barrier, and the bottom sticky
  sheet reports bottom-edge attachment, hidden close affordance, and ignored outside press. In the
  Menu samples, confirm arrow keys move roving focus over enabled
   action items while skipping separators and disabled items, Enter/Space activates the focused
   action and closes the menu, Escape closes the controlled menu, and `Outside ignored` keeps its
   explicit outside policy. In the ContextMenu samples, right-click the hotspot and confirm the
   menu opens from the pointer point, snaps inside the window near edges, and closes on outside
   press or Escape.
6. Open `Components`, or start there directly with
   `cargo run -p open-gpui-ui-foundation-gallery -- --page components`, and confirm Button, Badge,
   IconButton, Separator, Kbd, Progress, Skeleton, Avatar, ScrollArea, Splitter, Switch, Checkbox,
   RadioGroup, Toggle, Label, TextInput, Field, Tabs, Toolbar, Sidebar, Listbox, Select, Combobox,
   Command, Table, and VirtualizedList samples render with enabled, disabled, selected, checked, unchecked,
   indeterminate, pressed, invalid, required, read-only, placeholder, value, help, error,
   control-association, decorative, semantic, indeterminate-progress, fallback-initial,
   source-metadata, roving-focus, popup, overflow-axis, scroll-reset, resize-constraint, row-model,
   and virtualized-viewport states. The Badge, Kbd, and Skeleton samples should remain display-only.
   Use a few catalog cards, such as Table, Tree, and VirtualizedList, to enter focused
   component-family mode; confirm unrelated samples are hidden, the section directory stays
   available, nested sample scrolling still stays inside the sample, and `All components` restores
   the full conformance page with the page scroll reset. The Separator samples should distinguish semantic and
   decorative roles. The Progress samples should cover determinate and indeterminate values, with
   indeterminate progress rendering as a short non-percentage segment rather than a fixed 33% fill.
   The Avatar samples should show derived fallback initials, explicit fallback text, explicit
   accessible labels, and source metadata without owning image loading. The IconButton samples
   should be square controls with visible focus and explicit accessible labels. The ScrollArea samples should cover vertical overflow, horizontal overflow,
   and two-axis overflow; wheel or trackpad scrolling should stay inside each constrained viewport
   while the state readout reports the expected axis and reset policy. Scroll each constrained
   ScrollArea once, then continue scrolling the same viewport after the content has moved; it should
   keep moving instead of snapping back to the origin after the redraw caused by the first scroll.
   The gallery navigation rail should also scroll independently inside its own viewport so deep
   sections remain reachable on compact windows. The vertical Tabs sample should keep its tab rail
   scrollable inside the constrained gallery card.
   The Splitter samples should
   show horizontal and vertical panel groups, stable handle affordances, min/max fraction readouts,
   collapsed-panel metadata, and pointer-drag resizing without changing surrounding layout. Drag the
   vertical collapsed sample far enough to restore the collapsed panel, then confirm subsequent
   dragging resizes it normally. The RadioGroup samples should
   cover vertical required selection and horizontal navigation that skips disabled items. The Toggle
   samples should expose button-like pressed state without behaving like a checkbox. The Tabs
   samples should cover horizontal automatic activation and vertical manual activation; use arrow
   keys, Home/End, Enter, and Space to confirm focus movement and activation behavior. The vertical
    sample should keep its tab rail scrollable inside the constrained gallery card. The Toolbar
    samples should expose horizontal and vertical command groups; use arrow keys plus Home/End to
    confirm roving focus skips disabled items and separators, and use Enter/Space to activate
    action/toggle items. The component runtime smoke now verifies the rendered Toolbar keyboard path
    for disabled-item/separator skipping and activation payloads. The Sidebar samples should expose
   expanded, icon-collapsed, and long scrollable navigation; icon collapse should hide visible labels
   while keeping item labels
   explicit, disabled items should be skipped, and the long sidebar should scroll inside its sample
   frame. The gallery smoke now verifies the long sidebar's internal viewport moves relative to its
   sample card. The Listbox samples should expose
   grouped options, disabled option skipping, selected and active descendant metadata, empty-state
   behavior, and keyboard navigation/activation with Up/Down/Home/End plus Enter/Space. The
   component runtime smoke now verifies rendered Listbox disabled clicks, selection-free arrow
   navigation, disabled/separator skipping, and option/listbox callback parity for keyboard
   activation. The Select
   samples should expose closed, controlled-open, and disabled states; confirm the trigger label
   reflects the selected option, the open sample uses a non-modal dismissible listbox popup with a
   scrollable long option set, Escape/outside press dismisses it, and disabled empty select remains
   closed. The component runtime smoke now verifies rendered Select trigger opening, disabled popup
   option rejection, click selection, keyboard popup selection that skips disabled rows, selection
   payloads, and ordered popup close callbacks. The Combobox samples should expose editable
   filtering, selected value metadata that does not disappear when the query hides the selected
   option, an empty filtered state, and disabled input/popup suppression. The component runtime
   smoke now verifies real Combobox text-input editing, filtered popup options, filtered option
   click selection, and close callbacks. The Command samples should expose ranked search results,
   selected chips for multi-select, a 10k-item virtualized command result window, app-owned
   indexed/loading metadata, shortcut labels, inline and dialog-backed presentation, and modal
   dialog outside/Escape dismissal while preserving the Components page scrollability. The component
   runtime smoke now verifies real Command text-input editing, inline filtering, keyboard
   activation, shortcut payloads, non-dialog content persistence, multi-select toggling, virtualized
   scrolling/reveal behavior, and app-owned index snapshot state. The default TextInput
    sample should accept real text editing through the
    controller-backed path, while the gallery remains scrollable and keeps focus visible when the
    page overflows. The Table samples should expose the `release-queue` 10k-row virtualized window,
    the filtered/sorted/paginated `filter-board` model, the controlled `release-resize` sizing
    sample, the grouped and sticky pinned `release-rollup` model with left/right fixed lanes and a
    horizontally scrollable center lane, the wide `release-matrix` center-column window, stable
    selected row ids, table/row/cell accessibility metadata, sortable header metadata, resize
    handle metadata, and internal body viewports that scroll without moving the outer Components
    page.
    The Tree sample should expose `document-outline`,
    tree/tree-item accessibility metadata, expandable `Paper` children, a state readout, an inner
    viewport that scrolls without moving the outer Components page, and selection/toggle events
    through the gallery sample runtime log. The VirtualizedList sample should expose the
    `release-navigation` 10k-item window, listbox/listbox-option roles, active/selected
    metadata, visible/overscan readouts, an internal viewport that scrolls without moving the
    outer Components page, card-chrome wheel containment, and PageDown plus Enter/Space activation
    through the gallery sample runtime log. The app should stay open after opening `Components`;
    an `accesskit_consumer`
   panic during that navigation is a
   regression in the accessibility repair gate. The Components page also serves as a conformance
   surface: confirm the visible component catalog distinguishes official components from
    adapter-only helpers and internal anatomy, and confirms Separator, Kbd, Progress, Skeleton, and
    Avatar are official entries with state types, then confirm the visible gate cards for explicit
    crate exports, gallery metadata, ScrollArea redraw persistence, Splitter runtime constraints,
    Tabs overflow, `table-virtualization`, `tree-renderer`, `virtualized-list-renderer`, and
    explicit accessible metadata on icon-only and label-association samples.
   The Overlay Menu and ContextMenu samples should expose action, checkbox, radio, separator,
   disabled, submenu, typeahead, controlled-open, outside-policy, and point-anchor variants. Use
   `cargo nextest run -p open-gpui-ui-components menu` and `cargo nextest run -p
   open-gpui-ui-components context_menu` to verify rich item payloads, pure typeahead,
   visible-submenu keyboard navigation, local menu scrollability, and context-menu reuse. Use
   `cargo nextest run -p open-gpui-ui-foundation-gallery
   overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
   overlay_page_context_menu_samples_expose_point_anchor_contracts
   overlay_page_catalog_entries_have_signals_and_sample_selectors
   overlay_gallery_smoke_closes_menu_from_escape_and_outside_press
   overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses` plus `cargo check -p
   open-gpui-ui-foundation-gallery --tests` after changing the overlay menu family.
7. Re-run `cargo nextest run -p open-gpui-ui-components` and `cargo nextest run -p
   open-gpui-ui-foundation-gallery` if a manual check exposes a component or gallery regression.

For UI component productization checkpoint work, additionally review
`docs/adr/0008-open-gpui-ui-component-productization-roadmap.md` after the automated component
tests pass. If a future task explicitly reopens extraction, also review
`docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md` and
`docs/adr/0007-open-gpui-ui-headless-boundary-design.md`. The checkpoint should continue to
identify which behavior is neutral, which behavior remains GPUI adapter-owned, and why the current
crates remain the active product boundary.

CI runs a three-platform matrix for pushes to `master` / `main`, pull requests, and manual workflow
dispatches:

- Windows runs the same local gate, `cargo nextest run -p xtask`,
  `cargo nextest run -p open-gpui-docking-native --no-fail-fast`, and
  `cargo check -p open-gpui-windows --all-features --locked`.
- Linux runs `cargo check -p open-gpui-linux --all-features --locked` after installing the system
  headers needed for Wayland, X11, fontconfig, freetype, and pkg-config.
- macOS runs `cargo check -p open-gpui-macos --features font-kit --locked`.
- All three platforms run `cargo check -p open-gpui-wgpu --features font-kit --locked`.

Run the native renderer smoke explicitly with:

```sh
cargo run -p xtask -- renderer-smoke
```

That command runs the focused `open-gpui-wgpu` smoke test that requests a real native `wgpu` adapter and
device, creates the renderer bind group layouts, and builds the core render pipelines. It is not
part of the default `verify` gate because it depends on local GPU, driver, and session availability.

Run the docking smoke surface explicitly after changing `open-gpui-docking`:

```sh
cargo nextest run -p open-gpui-docking
cargo nextest run -p open-gpui-docking-native --no-fail-fast
cargo check -p open-gpui-docking-native
cargo run -p open-gpui-docking-native
```

The docking native example exercises the public multi-window setup: applications build one
`DockController`, wrap it in a `DockViewportRuntimeHandle`, register window-close cleanup, and open
controller-backed primary and secondary `DockHost` viewports.

Manual native docking dogfood should use the same example after the automated checks pass:

1. Launch `cargo run -p open-gpui-docking-native` and confirm the app opens `Docking demo`,
   `Docking preview`, and `Empty central dogfood` windows.
2. Drag a primary-class tab from `Docking demo` into another primary-compatible target; the preview
   must appear in the destination window and release must select the moved item there.
3. Drag the `Preview` / `Diff` secondary-class stack from `Docking preview` back into `Docking demo`;
   item order and the active tab must be preserved.
4. Drag `Preview` / `Diff` over `Empty central dogfood`; the route must render as rejected and
   release must not mutate the graph because the central space only accepts central-class panels.
5. Use `Restore central note` from the runtime status panel; the `Central note` panel must reopen in
   the empty central window and recover the central-region identity instead of becoming ordinary
   root-only content.
6. Drag a tab or stack outside every docking window; a new runtime-backed viewport must open before
   the graph moves the source payload.
7. Dock the torn-off viewport content back into an existing window; the destination window must
   activate and the moved item must become the selected tab.
8. Move runtime-opened windows across displays, choose `Save placement`, then use `Reopen closed
   demo viewports`; restored placement should use saved bounds only as placement input while live
   drag routing continues to use current viewport bounds. On macOS, windows on a secondary display
   should keep non-overlapping desktop-space bounds while routing between viewports.
9. Exercise the runtime panel close-policy controls for prevent, retain, and merge-back behavior;
   closing a viewport must match the selected policy without losing descriptor-backed panel restore
   or leaving a stale cross-window route preview in another viewport.
10. Start a cross-window drag, hover a valid target, then move to an area of the same viewport with
   no current dock target before releasing; the previous preview must not commit from stale target
   state.
11. Drag over the empty central dogfood window; empty central-space preview, rejection, and
   passthrough behavior must match the visible policy state.

Current platform caveats for docking multi-viewport dogfood:

- Windows mixed-DPI displays and Wayland global toplevel positions are not yet normalized into one
  explicit GPUI coordinate type. Treat cross-display routing results on those backends as areas for
  follow-up platform API work, not as proof of full ImGui PlatformIO parity.
- No-input, no-focus-on-appearing, alpha, topmost, and no-taskbar viewport flags are not modeled in
  GPUI's platform trait yet.

Before publishing a crate, confirm that the packaged archive carries the expected attribution files:

```sh
cargo package -p open-gpui --list --allow-dirty
```

For the canvas crate specifically, run:

```sh
cargo package -p open-gpui-canvas --list --allow-dirty
cargo publish -p open-gpui-canvas --dry-run --allow-dirty
```

Every published Open GPUI crate should include `README.md`, `LICENSE-APACHE`, and `NOTICE`. Cargo
does not package files outside a crate root through `include`, so each publishable crate root keeps
its own `NOTICE` copy.

The import-boundary scan rejects dependency files that reintroduce Zed's GPL tracing stack
(`ztracing`, `ztracing_macro`, `zlog`), the old `zed-sum-tree` dependency, the Zed monorepo as a
Cargo git dependency, retired Zed Git fork sources that have already been migrated, or the removed
Zed `perf` crate dependency. The retired `zed-scap` package and `zed-industries/scap` Git source
are also rejected now that screen capture resolves through the Open GPUI-owned
`open-gpui-scap` fork. The old crates.io `zed-font-kit` package is retired and should not be
reintroduced; font-kit resolves through the Open GPUI-owned fork configured in the crate manifests.
