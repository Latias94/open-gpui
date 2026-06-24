# Open GPUI Component Contract

Official Open GPUI components use an adapter-first, productized GPUI shape. A component may render
with GPUI today, but its behavior and semantic state should stay renderer-neutral enough to test
without rewriting the public API. ADR 0008 treats the current UI crates as the active product
boundary; future headless extraction is a deferred option, not the current roadmap.

## Resolved State

Every component should expose a resolved state or descriptor type. The state type is the primary
unit for tests and documentation.

Resolved state should contain:

- semantic input state such as disabled, selected, checked, indeterminate, open, invalid, read-only,
  and required;
- activation or editability rules;
- navigation state for composite widgets such as selected, focused, and tab-stop position;
- accessibility intent such as role, label requirements, value presence, and required actions;
- metrics derived from `open_gpui_ui_core::Size`;
- token intents derived from `open_gpui_ui_core::ThemeTokens`;
- component anatomy metadata when it affects composition.

Resolved state should avoid GPUI render/runtime types such as `Window`, `App`, `Context`,
`RenderOnce`, `IntoElement`, `ElementId`, focus handles, scroll handles, and callback types.

## GPUI Adapter

The concrete component owns the GPUI adapter. This layer may use:

- `div()`, `ElementId`, `RenderOnce`, `IntoElement`, and fluent style calls;
- focus handles, tab stops, focus-visible styles, and focus restoration;
- AccessKit role/action/state mapping;
- hitboxes, pointer events, keyboard actions, cursor behavior, and event propagation;
- scroll handles, overlay anchoring, portals, and deferred rendering;
- concrete color values produced by `open_gpui_ui_components::ThemeResolver`.

The adapter should read from the resolved state rather than duplicating semantic decisions in the
render body.

UI-core adaptive helpers accept neutral `UiPx` values. GPUI adapters that start from concrete
window, viewport, or layout `Pixels` should convert to `UiPx` before calling
`DeviceShellSwitchPolicy`, `DeviceAdaptivePolicy`, `PanelAdaptivePolicy`, or
`device_adaptive_snapshot`. UI core intentionally does not expose `Pixels` compatibility aliases,
because keeping those aliases would preserve a renderer dependency in the neutral crate.

## Overlay Behavior

Overlay component state should use renderer-neutral policy types from `open_gpui_ui_core` before
reaching GPUI adapters. The shared contract distinguishes:

- semantic `open` state from `present` and `interactive` overlay presence;
- layer kind (`Tooltip`, non-modal dismissible, `Modal`, and menu-like surfaces);
- outside-press policy (`ignore`, `consume`, `dismiss + consume`, and `dismiss + pass-through`);
- Escape-key policy and dismiss reason;
- initial focus and focus restoration intent;
- anchor and placement inputs that use `open_gpui_ui_core` neutral geometry (`UiPx`, `UiPoint`,
  `UiSize`, `UiRect`, and `UiEdges`) and do not store `Window`, `Context`, `FocusHandle`,
  `ElementId`, or callback types.

GPUI adapters remain responsible for `deferred` and `anchored` rendering, event subscriptions,
hitboxes, focus handles, concrete focus restoration, and AccessKit relationship wiring.
`open_gpui_ui_components::gpui_adapter` provides the narrow GPUI mapping layer: deferred priority,
snap-to-window margin, GPUI anchor mapping, and open-change decisions derived from the shared
policy. It does not own global overlay ordering, callback storage, or window subscriptions.
`open_gpui_ui_core::overlay` owns renderer-neutral stack ordering through
`resolve_escape_key`, `resolve_outside_press`, and `resolve_focus_restore`, so nested overlay
behavior can be tested without a GPUI window before an adapter wires concrete events and focus
handles.

Interactive overlay adapters should bind persistent trigger/content focus handles in keyed runtime
state instead of allocating them from rebuilt `RenderOnce` values. Resolved state declares
`InitialFocusIntent` and `FocusRestoreIntent`; the adapter must apply those intents when opening or
dismissing concrete GPUI layers. Non-modal pass-through dismissal may need deferred trigger focus
restoration so later underlay mouse events do not overwrite the restored focus.

`TooltipState` is the first descriptive overlay component contract. It records content kind,
disabled/open state, hover/focus/manual open intent, placement preference, delay policy, resolved
metrics, token intents, and tooltip layer state. The first slice is intentionally non-interactive:
hover/focus subscriptions, focus handles, timing execution, and anchored/deferred rendering remain
GPUI adapter responsibilities. Rich hover cards and action-bearing tooltip content should not reuse
the descriptive tooltip contract as-is.

`PopoverState` is the first interactive non-modal overlay contract. It records controlled versus
uncontrolled open mode, default-open state, trigger expanded/selected intent, placement preference,
outside-press policy, initial focus intent, focus restore intent, resolved metrics, token intents,
and non-modal dismissible layer state. The GPUI adapter owns the concrete trigger/content elements,
`deferred`/`anchored` rendering, outside-press subscription, and focus handles. Popover defaults to
the non-modal overlay initial-focus policy (`InitialFocusIntent::None`); callers must opt in when
content should receive focus. Nested popovers, modal popover variants, and full focus-scope
coordination remain follow-up work.

`DialogState` is the first modal overlay contract. It records controlled versus uncontrolled open
mode, default-open state, title and description metadata, Escape policy, outside-press policy,
initial focus intent, focus restore intent, resolved metrics, token intents, and modal layer state.
The GPUI adapter owns the barrier, concrete dialog surface, close callbacks, keyboard events,
deferred rendering, and focus handles. Alert dialogs, nested modal stacking, and full focus-trap
derivatives build on this contract; nested modal stacking and full focus-trap coordination remain
follow-up work.

`AlertDialogState` is the action-critical modal derivative. It records required title and
description text, cancel and primary action metadata, destructive intent, action disabled state,
initial focus preference, Escape policy, outside-press policy, focus restore intent, token intents,
and modal layer state. Alert dialogs default to consuming outside press without dismissing so the
underlay stays inert and critical decisions require an explicit action. The primary destructive
action is represented as metadata, while the cancel action remains the default initial focus target.
The GPUI adapter owns concrete button rendering, callbacks, keyboard handling, deferred rendering,
and focus handles.

`SheetState` is the edge-attached overlay contract. It records controlled versus uncontrolled open
mode, default-open state, attached side, modal versus non-modal mode, close affordance visibility,
title and optional description metadata, Escape policy, outside-press policy, initial focus intent,
focus restore intent, resolved metrics, token intents, and layer state. Modal sheets block underlay
input and default to dismissing while consuming outside press. Non-modal sheets use the same
surface anatomy while mapping to the non-modal dismissible layer kind and defaulting to
dismiss-and-pass-through outside behavior without installing a blocking barrier. The GPUI adapter
owns the barrier for modal sheets, edge positioning, concrete close control, callbacks, keyboard
handling, deferred rendering, and focus handles.

`HoverCardState` is the interactive hover/focus overlay contract. It records controlled versus
uncontrolled open mode, default-open state, hover/focus/manual open intent, placement preference,
open and close delay policy, outside-press policy, initial focus intent, focus restoration intent,
resolved metrics, token intents, and non-modal dismissible layer state. Hover cards are not
descriptive tooltips: their surfaces may contain interactive content, default to no initial focus
or focus restoration, and use dismiss-and-pass-through outside behavior so underlay interaction can
continue after dismissal. The GPUI adapter owns hover timers, focus handles, keyed runtime open
state, deferred anchored rendering, Escape/outside event wiring, and pointer/focus lifetime
coordination.

`MenuState` and `ContextMenuState` are the first menu overlay contracts. `MenuState` records
controlled versus uncontrolled open mode, action, checkbox, radio, separator, and submenu items,
caller-owned checked state, disabled item state, stable item paths, visible submenu rows, roving
focus, pure typeahead targets, keyboard submenu open/close targets, activation payloads with item
kind/path/checked-at-activation, local scrollability, Escape policy, outside-press policy,
placement preference, resolved metrics, token intents, and menu layer state. `ContextMenuState`
reuses the same item, submenu, typeahead, scrollability, and roving focus model while adding a point
anchor and renderer-neutral placement input sized from the visible menu surface. Keyboard and
pointer activation both invoke item-level selection handlers before component-level selection
handlers. Hover corridor submenu opening, menu bars, application menu integration, global command
dispatch, and native OS menu bridging remain follow-up work.

The Overlay page has its own product catalog instead of being merged into the Components page.
`open_gpui_ui_foundation_gallery::pages::overlay::OVERLAY_CATALOG` lists Tooltip, HoverCard,
Popover, Dialog, AlertDialog, Sheet, Menu, and ContextMenu with official status, family metadata,
resolved-state type, rendered sample selector, coverage summary, and behavior-gate labels.
`overlay_sample_selector_pairs()` is the focused selector contract for rendered overlay samples.
Default-open overlay samples may expose default-open metadata, but the gallery must keep them
visually non-blocking at page load so modal barriers and floating layers do not prevent scrolling
or navigation.

`ListboxState` is the renderer-neutral collection choice contract. It records grouped and
standalone option descriptors, separator rows, disabled option state, selected value, active
descendant value, tab-stop value, APG-style Up/Down/Home/End navigation, Enter/Space activation
payloads, typeahead target metadata, resolved metrics, token intents, and listbox/listbox-option
roles. It does not own popup state, selection persistence outside the adapter runtime, scroll
handles, focus handles, callbacks, or GPUI element ids.

`SelectState` composes a trigger, non-modal dismissible overlay, scroll viewport metadata, and a
nested `ListboxState`. It records controlled versus uncontrolled open mode, default-open state,
placeholder and selected trigger label, selected and active option values, placement preference,
outside-press policy, initial focus intent, focus restoration intent, resolved metrics, token
intents, and the listbox content role. The GPUI `Select` adapter owns trigger/content rendering,
keyed runtime open/selected/active state, callbacks, outside-press and Escape wiring, deferred
anchored rendering, and concrete focus handles.

`ComboboxState` composes an editable text input, non-modal dismissible popup, scroll viewport
metadata, and nested `ListboxState`. It records controlled versus uncontrolled open mode,
default-open state, required/disabled metadata, query text, selected value and label, active option
value, filtered and total option counts, empty-state label, placement preference, outside-press
policy, initial focus intent, focus restoration intent, resolved metrics, token intents, and
editable-combobox/listbox roles. Filtering controls only the visible list: the selected value is
resolved from the unfiltered descriptors and is not cleared just because the current query hides
that option. The GPUI adapter owns the `TextInputController`, keyed runtime query/open/selection
state, callbacks, outside-press and Escape wiring, deferred anchored rendering, scroll handles, and
concrete focus handles.

`CommandState` composes a search text input, ranked grouped command results, optional dialog
wrapper, loading metadata, selected chips, a virtualized result window, and nested `ListboxState`.
It records controlled versus uncontrolled open and query modes, default-open/default-query seed
state, single-select or multi-select behavior, selected and active command values, query text,
filtered and total command counts, standalone/grouped command anatomy, shortcut labels, disabled
command state, deterministic match source/score metadata, app-owned index revision/mode metadata,
empty-state label, Escape policy, focus restoration intent, resolved metrics, token intents,
non-modal inline overlay state, and modal dialog overlay state when dialog presentation is enabled.
`CommandIndexSnapshot` lets applications pass indexed, pre-ranked, or pre-filtered descriptor
snapshots with loading metadata, while keeping command discovery, global registries, keybinding
resolution, dispatch, enablement policy, and async task ownership outside `ui_components`. The GPUI
adapter owns the `TextInputController`, keyed runtime query/open/selection state, callbacks,
outside-press and Escape wiring, deferred dialog rendering, concrete focus handles, and scroll
handles; the renderer-neutral state owns ranking, selection projection, snapshot metadata, and the
virtualized result render plan.

`SeparatorState`, `KbdState`, `ProgressState`, and `SkeletonState` are low-state primitives. They
still expose resolved state, metrics, token intents, and stable rendered debug selectors rather
than relying on ad hoc styled `div()` call sites. `SeparatorState` owns orientation and decorative
mode; semantic separators use the neutral `Role::Separator`, while decorative separators expose no
role. The current GPUI AccessKit adapter maps the neutral separator role through the nearest
available GPUI role because the bundled AccessKit role enum does not expose a separator role yet.
`KbdState` is display-only shortcut text with muted surface/text/border intents. `ProgressState`
owns determinate versus indeterminate progress, clamps determinate values to `0..=100`, exposes a
normalized `0..=1` value for determinate rendering, and maps to `Role::ProgressIndicator`.
Indeterminate progress uses `ProgressVisualMode::Indeterminate` and renders a short non-percentage
segment instead of a left-anchored fixed fill. `SkeletonState` is a non-interactive static loading
placeholder with muted surface token intent; animation remains a future adapter enhancement, not
part of the first resolved-state contract.

`AvatarState` is the identity primitive contract. It resolves display name, fallback initials or
explicit fallback text, optional renderer-neutral `AvatarSource` metadata, accessible label,
metrics, token intents, and `Role::Image`. The first slice intentionally does not own async image
loading status, retry policy, cache state, grouped avatar overlap layout, or fallback delay timers;
callers can model those outside the primitive and pass only the current source/fallback intent into
the GPUI adapter.

## Focus Rings

Interactive component state should expose `FocusRing` metadata instead of rendering focus by
changing border width. `FocusRing` keeps the focus color as a `ColorIntent`, records the paint
width as neutral `UiPx`, and documents that it does not change layout.

The GPUI adapter should apply the ring inside `focus_visible` using
`open_gpui_ui_components::gpui_adapter::focus_ring_shadow`. This paints an outer box shadow, so
keyboard focus visibility does not resize or move the focused component. `focus_ring_shadow` is
available only through `open_gpui_ui_components::gpui_adapter` because its `BoxShadow` return type
is renderer-specific.

## Public API

Prefer Rust builder-style APIs with explicit enums and semantic event names. Public interaction
builders should fall into one of four ownership buckets:

- **render input**: the caller supplies a plain render prop that does not represent adapter-owned
  runtime state. Examples include visible labels, descriptions, variants, tokens, and static
  source metadata.
- **controlled runtime input**: the caller supplies the current render-frame value for state the
  adapter may also mutate. Direct semantic names such as `value`, `open`, `selected`, `active`,
  `focused`, `checked`, `pressed`, `collapsed`, `active_index`, and `selected_index` belong here.
- **default seed**: the caller supplies the first value for adapter-owned runtime state. These
  builders must use `default_*` and document the runtime value they seed, such as
  `default_open -> open`.
- **policy hint**: the caller describes adapter behavior without transferring value ownership.
  Examples include `initial_focus_intent`, `focus_restore_intent`, `outside_press_policy`,
  `escape_key_policy`, placement inputs, scroll reset policy, and externally supplied adapter
  handles.

Callbacks should use a small semantic vocabulary: `on_change` for scalar value changes,
`on_open_change` for overlay visibility requests, `on_selection_change` for persistent selection
state, `on_select` for committed item selection or action-like choice, `on_activate` for activation
without persistent selection ownership, and `on_toggle` for expansion or tri-state toggle payloads.
Seed-shaped runtime builders must stay explicit in the API inventory. Current examples include
`Tabs::default_selected`, `RadioGroup::default_selected`, `Toolbar::default_focused`,
`Sidebar::default_focused`, `Tree::default_selected`, `Tree::default_focused`,
`VirtualizedList::default_active_index`, `VirtualizedList::default_selected_index`,
`Combobox::default_query`, `Command::default_query`, `Menu::default_focused_value`, and
`ContextMenu::default_focused_value`. Direct names such as `Sidebar::selected`,
`Listbox::selected`, `Select::selected`, `Combobox::selected`, and `Command::selected` remain
reserved for caller-owned render-frame inputs. `Switch::on_change`, `Toggle::on_change`, and
`TextInput::on_change` are scalar value-change callbacks. Bootstrap callback exceptions such as
`Button::on_click`, `AlertDialog::on_action`, `AlertDialog::on_cancel`, `Sheet::on_close`, and
`Table::on_sort_requested` must stay explicit in the API inventory because they represent command
activation, modal action outcomes, close affordances, or table sort requests rather than scalar
value changes.

Keep crate-root exports explicit. Do not use wildcard public re-exports in component crates.
GPUI-specific helpers that remain public for concrete applications must be reachable through
`open_gpui_ui_components::gpui_adapter`; current examples include `TextInputController`,
`init_text_input`, `focus_ring_shadow`, accessibility mapping helpers, geometry conversion helpers,
and GPUI overlay scheduling helpers. The crate root and prelude default interface are reserved for
official components and renderer-neutral contracts.

## Official Component Completion

A component is official only when it satisfies the current-crate completion contract:

- it has a public resolved-state or descriptor type that avoids GPUI runtime/rendering types;
- crate-root and prelude exports are explicit and covered by public export tests;
- its public interaction API has a `COMPONENT_API_INVENTORY` row classifying render inputs,
  controlled runtime inputs, `default_*` seeds, policy hints, callbacks, callback payload types,
  and renderer-neutral resolved-state ownership;
- metrics, sizes, colors, focus rings, and accessibility metadata use foundation vocabulary;
- callbacks, focus handles, scroll handles, image loading, deferred rendering, and subscriptions
  stay in the GPUI adapter layer;
- the Components gallery exposes real samples, stable sample ids, and resolved-state metadata;
- every official catalog entry has matching `SIGNALS` entries for its component type and resolved
  state type, plus at least one rendered `gallery:component-*-sample:{id}` selector;
- every official overlay family has a matching `OVERLAY_CATALOG` row with component/state
  `SIGNALS`, at least one rendered `gallery:overlay-*-sample:{id}` selector, and named behavior
  gates;
- focused tests cover state contracts, and rendered runtime tests cover behavior that state tests
  cannot prove;
- `docs/verification.md` names any manual or automated gate added by the component.

`examples/ui-foundation-gallery::pages::components::COMPONENT_CATALOG` is the current visible
catalog for this contract. Entries marked `official` satisfy the checklist above. Entries marked
`adapter-only` are public GPUI helper surfaces such as `TextInputController`, not standalone
components. Entries marked `internal-anatomy` are public parts of a component family, such as
toolbar or listbox item descriptors, and should not be promoted to standalone components without a
new resolved-state contract. Entries marked `state-contract` are public renderer-neutral contracts
with gallery readouts and signal coverage, but they are not themselves rendered GPUI components.
They may sit beside an official adapter, as `TreeState` does for `Tree`. They must use
`state_contract_selector`, not the official `sample_selector`, and they must not satisfy the
official rendered-component gate by accident. Entries marked `deferred` are planned components
that must not be treated as shipped API until they satisfy the checklist.

## Theme Resolution

Component state should expose `ColorIntent` values rather than concrete GPUI colors. A color intent
keeps the semantic `TokenKey`, `ColorState`, and fallback RGB visible for tests, documentation, and
future adapter work.

The GPUI adapter should resolve intents through `ThemeResolver` immediately before calling style
APIs such as `bg`, `border_color`, and `text_color`. `ThemeResolver::resolve` uses the default
light `ThemeSnapshot` for compatibility. New code that has an explicit theme should call
`ThemeResolver::resolve_with(intent, snapshot)` so `(TokenKey, ColorState)` lookups come from the
runtime theme table before falling back to the intent RGB.

`ThemeSnapshot` is an immutable table view with a `ThemeMode`, `revision`, and color entries. The
revision is the cache invalidation hook for future app-level theme providers. Components should not
read global theme state directly; keep the resolved component state renderer-neutral and pass theme
snapshots at the adapter edge.

## Accessibility References

Adapters may wire explicit AccessKit relationships such as controls, labelled-by, active descendant,
and popup references, but those references must point to nodes that are present in the current
accessibility tree update. GPUI defensively strips invalid cross-node references before handing the
tree update to the platform adapter, because AccessKit consumers may panic when explicit labels or
other references target missing nodes.

Component adapters should still prefer stable element IDs for both the referring node and referenced
node. The repair layer is a crash barrier, not a substitute for correct IDs.

## Toolbar Contract

`ToolbarState` describes renderer-neutral command grouping: stable toolbar label, orientation,
foundation size, disabled state, action/toggle/separator items, pressed toggle metadata, focused
item, tab stop, shared button metrics, and focus-ring/color intents. Separators are visual only and
must not participate in roving focus or activation.

The GPUI `Toolbar` adapter owns focus handles, keyboard/click dispatch, and concrete item rendering.
It should expose `Role::Toolbar`, `aria_orientation`, explicit item labels, button roles for action
and toggle items, and toggled metadata for pressed toggle items. It should reuse the shared
roving-focus helpers so arrow keys, Home, and End skip disabled items and separators consistently
with Tabs, RadioGroup, and Menu.

Toolbar v1 is a primitive command surface, not an application command registry. Automatic overflow
menus, shortcut rendering, command enablement policies, persisted customization, and icon asset
resolution remain app/adapter responsibilities until the command and sidebar work proves a common
contract.

## Sidebar Contract

`SidebarState` describes renderer-neutral shell navigation: side, variant, size, collapse mode,
effective collapsed state, accessible label, sections, flattened navigation items, disabled state,
selected item, focused item, tab stop, scrollability, metrics, colors, and focus-ring intent. It
keeps selection app-owned; activating an item produces a `SidebarSelection` payload but does not
own routing or persistent preferences.

Icon collapse keeps navigation items visible and focusable while hiding visible text; item labels
remain explicit accessibility labels. Offcanvas collapse removes items from roving focus by making
them invisible and non-focusable. `SidebarCollapseMode::None` ignores collapsed input and keeps the
expanded width. Disabled items are skipped by the shared vertical roving-focus helper and cannot
produce activation payloads.

The GPUI `Sidebar` adapter owns focus handles, click and keyboard dispatch, concrete rendering,
scroll handles through `ScrollArea`, and AccessKit mapping. It should expose `Role::Navigation` on
the container, `Role::Section` for groups, explicit item labels, selected and disabled metadata,
and set-position metadata for focusable items.

Sidebar v1 is a bounded navigation primitive, not a full application shell. Provider contexts,
mobile sheet routing, nested submenus, route integration, keyboard shortcut toggles, persisted
layout preferences, animated offcanvas unmounting, and command registry integration remain
follow-up work.

## Scroll Viewports

`ScrollAreaState` describes renderer-neutral viewport intent: stable viewport id, axis
(`vertical`, `horizontal`, or `both`), reset policy, optional reset key, foundation size, and
scrollbar metrics. It does not store `ScrollHandle`, current offset, child bounds, window bounds, or
event callbacks.

The GPUI `ScrollArea` adapter owns `ScrollHandle`, maps axis intent to GPUI overflow style, reserves
scrollbar width from the resolved metrics, and performs reset-on-key-change by mutating the concrete
scroll handle after the component has a keyed runtime. Layout shells should pass an externally owned
handle when another view needs to inspect or manipulate scroll state; resolved state remains the
testable contract for docs and future adapter work.

The default `ScrollHandle` must live in the adapter's keyed runtime, not in the `ScrollArea::new`
builder value. Render code commonly reconstructs `RenderOnce` component values every frame, so a
handle allocated by the builder would reset the scroll offset on every notify/redraw and make the
viewport appear non-scrollable. An explicitly supplied external handle remains caller-owned, but the
default path must preserve offset across reconstructed component values.

## Table and Virtualizer Contracts

`TableState` describes renderer-neutral table behavior: stable row ids, nested source rows, row
lookup, row-model stage vocabulary, selection keyed by row id, column visibility and ordering,
pinned column regions, row pinning, sorting, filtering, grouping, built-in aggregation, expansion,
column groups, nested headers, and pagination. The official table contract now resolves the full
pipeline core -> filtered -> grouped -> sorted -> expanded -> paginated -> row-region split ->
final. Source tree rows remain
distinct from synthetic group rows: `TableRow` may own child rows, resolved source rows expose
depth, parent id,
branch/leaf state, descendant counts, and expansion metadata through `TableTreeRow`, and collapsed
source descendants stay addressable by stable row id. `TableRow` can also be marked expandable
before children are loaded, and `TableRowChildrenLoadState` carries caller-owned idle, loading, or
failed child-load metadata into resolved tree rows. `TableExpansionMode::Client` keeps the normal
client-pruned source tree behavior, while `TableExpansionMode::Manual` preserves the
caller-supplied source snapshot for ungrouped tree rows so applications can own server/manual
expansion, child fetches, cancellation, and cache policy. `TableStageMode` lets filtering and
sorting stay client-owned or become manual independently, and `TablePagination` supports the same
manual ownership mode with server-known `row_count` / `page_count` metadata. Manual row-model
stages preserve the caller-supplied snapshot while still keeping row lookup, selection, grouping,
expansion, and stable row ids intact; the table cache key includes those ownership modes and
pagination totals. Group rows may expose aggregate cells through `TableAggregation` using the
built-in `count`, `sum`, `min`, `max`, and `average` kinds;
the active grouping column still displays the grouping value instead of an aggregate payload.
Named custom aggregate callbacks are also supported through `TableState::with_aggregation_fn`,
with named specs resolving through the registered callback map and safely falling back to empty
cells when a callback name is unknown. Grouping plus source-tree composition remains deferred
until a later policy slice defines mixed filtering, sorting, and expansion semantics.
Custom aggregation rows stay renderer-neutral and are surfaced in the Components gallery through
the focused `grouped-custom-aggregation` sample.
`TableColumnPinning` is caller-owned state that splits
resolved visible columns into `left`, `center`, and `right` `TableColumnRegions` after visibility
and explicit ordering have been applied; unknown or invisible pinned ids are ignored. `TableColumn`
also carries preferred width, min/max width, and resizable metadata, while `TableColumnSizing` is
the caller-owned committed width map keyed by `TableColumnId`. `TableState::resolve` exposes
`TableResolvedColumnSizingRegions` after visibility, ordering, and pinning have resolved so
renderers can read per-column width, min/max bounds, region, start/after offsets, resize
capability, and region/all-column totals without owning adapter state.
`TableRowPinning` is caller-owned state with ordered top and bottom row ids. The default
`TableRowPinningPolicy::KeepPinnedRows` resolves pinned rows from the expanded pre-pagination row
model so a pinned row can remain visible while the current page changes; `PageOnly` limits pinned
rows to ids present in the current paginated model. Unknown ids, filtered-out rows, and collapsed
descendants are ignored, and overlapping raw top/bottom inputs resolve without duplicate final
rows. `TableResolvedState` exposes `TableRowRegions` plus top, center, and bottom row accessors;
the final visual model is top + center + bottom while row lookup remains stable for resolved rows.

`VirtualizerState` describes renderer-neutral viewport calculation inputs and outputs rather than a
concrete scroll element. The neutral contract accepts item count, viewport extent, scroll offset,
estimated item size, measurements keyed by stable item key, overscan, gap, and scroll margin. It
returns visible and overscan ranges, item measurements, total size, and snapshot/restore metadata.
GPUI adapters own `ScrollHandle`, wheel events, pointer hitboxes, and any concrete scroll offset
mutation. The first `Table` adapter consumes virtualizer snapshots as measurement-cache seeds; live
scroll offset from the adapter runtime wins during render, and one-shot scroll-position restoration
remains a future adapter-runtime policy.

The GPUI `Table` adapter resolves table state and virtualizer ranges before rendering. The adapter
owns the element tree, concrete scroll viewport, wheel containment, header/body drawing, sortable
header activation callbacks, row focus handles, source-tree disclosure affordances for loaded,
unloaded, loading, and failed branches, controlled row activation / expansion-request payloads,
callback-backed column resize handles, and AccessKit mapping. Table accessibility metadata includes
table, row, column-header, and cell roles, row and
column position metadata, sort metadata for sortable headers, grouped-row and source-tree depth /
parent metadata, selected state, and branch `aria-expanded` state keyed by stable row id. The
adapter keeps row activation independent from selection and expansion; callers decide whether a
click, double-click, Enter, Space, Left, or Right payload changes app-owned `TableState`. The
render plan exposes `TableColumnRegionRenderPlan` entries and every rendered header/body row has
stable `left`, `center`, and `right` region debug selectors.
Region render plans expose summed widths, and header/body cells read the same resolved column
widths. For pinned tables, `TableCenterColumnWindowPlan` virtualizes the shared horizontal center
lane from adapter-owned horizontal scroll input: it exposes visible and overscan ranges, rendered
center columns, total center width, and leading/trailing spacer widths. The adapter keeps left/right
pinned lanes fully mounted while mounting only the rendered center-column window, so the center can
scroll without moving pinned columns or the outer page. `TableRenderPlan` also exposes the current
filtering, sorting, pagination, and faceting ownership modes plus pagination row/page totals and
per-column facet metadata so gallery readouts and consumers can distinguish local row-model
transforms from app-owned server snapshots. Facet metadata covers deterministic unique value/count
entries, numeric min/max ranges, and explicit manual/server payloads keyed by column id; concrete
`TableFacetedFilter` is the official single-column categorical filter recipe over that metadata:
it reads `TableColumnFacets`, renders a searchable `Popover` with checkbox facet rows, keeps query
and popup runtime adapter-owned, and emits controlled `TableFacetedFilterChange` payloads that add,
remove, or clear exact stable tokens while resetting pagination to the first page.
`TableRangeFilter` is the sibling single-column numeric range recipe: it reads the same
`TableColumnFacets::numeric_range()` metadata, renders minimum and maximum `TextInput` fields in a
`Popover`, preserves partially typed endpoint text in adapter runtime, and emits controlled
`TableRangeFilterChange` payloads with parsed finite endpoints, clear state, and an `apply_to`
helper that replaces only the target column's range filter while resetting pagination to the first
page. `TablePredicateFilter` is the general single-column leaf-predicate recipe for text and
numeric comparisons: it renders a controlled operator selector plus value input, exposes
`TablePredicateFilterOperator` options for text contains / equality / prefix / suffix and numeric
greater-than / less-than comparisons, and emits `TablePredicateFilterChange` payloads that replace
only the target column's predicate filters while preserving categorical facets, numeric ranges,
and unrelated `TableState` slices. Nested AND/OR predicate builders, global faceting, async option
search, and fetching/cache lifecycles remain application-owned or follow-up work.
`TableColumnVisibility` is the sibling official column-visibility recipe: it reads renderer-neutral
column descriptors plus runtime visibility overrides, renders hideable columns inside a `Popover`
with checkbox rows and show-all / reset actions, keeps locked identity columns disabled, and emits
controlled `TableColumnVisibilityChange` payloads whose `apply_to` helper updates only visibility
overrides while preserving the rest of `TableState`. Saved views, URL sync, persistence, and
server-side capability negotiation remain application-owned or follow-up work.
Nested header groups are resolved as renderer-neutral row families rather than data columns.
`TableRenderPlan` exposes nested header-group rows for the left, center, and right regions, with
stable row counts, summed widths, and depth-specific group metadata. Pinned regions split group
families when visibility or pinning crosses region boundaries, while group headers continue to stay
leaf-column-driven for sort, resize, visibility, and selection behavior. Flat tables still resolve
to a single header row.
Text cell editing is the official inline-edit recipe over table column metadata:
`TableCellEditor::Text` and `TableColumn::text_editable` opt columns into editable leaf cells,
while synthetic group rows and missing source cells stay display-only. The GPUI adapter renders the
existing controlled `TextInput` path inside rendered body cells and emits
`TableCellEditChange` through `Table::on_cell_edit_change`; applications keep row data app-owned and
feed back a changed `TableState`. The helper `TableCellEditChange::apply_to` updates the matching
stable source row id while preserving unrelated row-model inputs such as sorting, filters,
pagination, selection, pinning, expansion, faceting, and sizing. Rich editor variants, validation,
dirty-state tracking, commit/cancel workflows, clipboard range editing, and server persistence
remain application-owned or follow-up work.
For row-pinned tables, `TableRenderPlan` exposes top, center, and bottom `TableRowRenderPlan`
regions with neutral `TableRowRegion` metadata, while the vertical virtualizer consumes only the
center region. The GPUI adapter renders top and bottom row bands outside the center body
`ScrollArea`, keeps `table:{id}:body:{top|center|bottom}` debug selectors stable, and reuses the
normal row renderer so focus, activation, expansion, pinned-column lanes, and accessibility row
indexes keep the same payload shape across pinned and center rows. `TableRenderPlan` also exposes
`GridViewport2D` when both a vertical row window and a horizontal center-column window are
available, so the adapter can report the combined two-axis viewport without merging the row and
column virtualizer contracts into a new standalone grid engine.

An official Table entry must satisfy the normal component completion gate: `Table` and `TableState`
exports at the crate root and prelude, matching `SIGNALS` entries, a `COMPONENT_CATALOG` official
entry, at least one `gallery:component-table-sample:{id}` rendered selector, state tests for row
identity, grouping, source-tree expansion, row interaction payloads, and virtualizer behavior, and
gallery runtime tests for nested scroll containment, faceted-filter row updates, predicate-filter
row updates, editable text-cell updates, and nested header gallery proof. Sticky headers,
autosize-by-content, data-source fetch/cache orchestration, global faceting, richer editor families,
and deeper two-axis grid virtualization beyond the pinned center-column window remain follow-up
capabilities.

## Splitter Constraints

`SplitterState` describes renderer-neutral resize constraints: stable group id, orientation, panel
fractions, per-panel min/max bounds, collapsible/collapsed metadata, handle adjacency, disabled
state, and handle metrics. The state owns the constraint solver for normalizing fractions and
clamping handle deltas; tests should exercise those rules without a GPUI window.

The GPUI `Splitter` adapter renders resolved panel fractions and resize handles from that state and
wires pointer dragging through keyed runtime state. Drag move events use the root splitter bounds to
translate pixels into fraction deltas, then feed those deltas through `SplitterState::resized_by`.
Dragging a collapsible panel past its restore threshold clears its collapsed state and resumes
normal min/max resizing; dragging below that threshold keeps the collapsed fraction stable.
The adapter may use GPUI layout primitives, cursor styles, drag callbacks, and `Entity` runtime
state, but it should not invent sizing rules in the render body. Keyboard splitter resizing,
controlled resize callbacks, application-level layout persistence, RTL behavior, and nested
splitter arbitration should build on `SplitterState::resized_by` instead of duplicating
min/max/collapse logic in adapter code.

## Gallery Conformance Surface

`examples/ui-foundation-gallery` is the durable conformance surface for official UI components. It
should expose stable sample ids, real resolved state, and a short gate list that names the
regression-prone behaviors each slice must keep covered.

The Components page should keep the official component catalog visible and distinguish shipped
components from adapter-only helpers, internal anatomy, and deferred primitives. It has two
supported inspection modes: the full all-components conformance page, and a focused
component-family view entered from official catalog cards. Focused mode may hide unrelated
sections, but it must keep the section directory available, expose an explicit `All components`
control, reset the page viewport when the family changes, and keep nested sample scrolling local to
the sample viewport. Directory chips remain anchor jumps inside the current page mode; they must
not implicitly change the focused family. The page should also keep these gates visible:

- crate-root and prelude exports stay explicit;
- adapter-only helper exports stay grouped under `open_gpui_ui_components::gpui_adapter`;
- every official catalog entry keeps matching component/state signals and a rendered sample
  selector;
- every official overlay entry keeps matching catalog metadata, component/state signals, rendered
  sample selectors, and visible catalog cards on the Overlay page;
- gallery samples continue to show real resolved state for each shipped component;
- all-components and focused component-family modes preserve the catalog, section directory, page
  scroll reset, and nested scroll containment contracts;
- the gallery navigation rail and page viewport stay independently scrollable on compact windows;
- ScrollArea redraws preserve the default keyed runtime handle;
- Table and virtualizer samples keep long table scrolling inside the table viewport;
- Splitter runtime fractions continue to share one constraint solver;
- Tabs keep overflow and roving-focus behavior visible in the page;
- icon-only affordances and labels keep their accessible metadata explicit.

## Headless Readiness Checkpoint

ADR 0008 makes current-crate productization the active roadmap. The boundary rules below remain
useful hygiene for tests and future adapter work, but they are not a directive to create
`open-gpui-ui-headless` in the current branch.

The current component catalog has enough repeated behavior to keep a future extraction possible:
overlay policy resolution, roving focus, listbox collection navigation, scroll viewport intent, and
splitter resize constraints are all renderer-neutral candidates if that work is reopened.

Before extraction, keep these boundary rules explicit:

- public resolved-state structs must continue to avoid GPUI runtime/rendering types, concrete
  element ids, focus handles, scroll handles, and callbacks;
- public contract guard tests now treat those runtime/rendering leaks as hard failures, while a
  separate extraction-blocker inventory pins any remaining public-state GPUI geometry usage until
  the extraction-prep series removes or classifies it;
- overlay placement, `ContextMenuState`, UI-core sizing, adaptive viewport policies, and public
  component metrics now use neutral UI-core geometry. The strict crate gate is closed:
  `open-gpui-ui-core` has no `open_gpui` dependency, source reference, or `UiPx` GPUI
  style-conversion impl;
- `open_gpui_ui_core` now exposes neutral `Role`, `Toggled`, `Orientation`, `AccessibleAction`,
  and `FocusTargetId`; GPUI/AccessKit conversion is publicly exposed through
  `open_gpui_ui_components::gpui_adapter`;
- component resolved state now exposes `OverlayResolvedState` for overlay policy/presence/focus
  data. `GpuiOverlayState` remains a GPUI adapter helper for deferred priority, snap margins, and
  renderer scheduling, and should not be stored in public `*State` contracts;
- `TextInputController`, externally supplied `ScrollHandle`, `focus_ring_shadow`, and GPUI overlay
  scheduling helpers are adapter-only public surfaces. They are intentionally grouped under
  `open_gpui_ui_components::gpui_adapter`; a future headless crate needs smaller neutral models or
  an explicit rule that these capabilities remain framework-specific.

## Current Known Gaps

The runtime theme table currently covers semantic component colors for light, dark, and
high-contrast snapshots, but there is not yet an app-level theme registry, user theme loading, or
JSON schema. Single-line editable text input now uses GPUI's `EntityInputHandler`/
`ElementInputHandler` path through `TextInputController`. Applications can either supply an
adapter-owned controller directly or use the standard controlled shape
`TextInput::value(...).on_change(...)`; the latter creates a keyed adapter controller internally,
emits sanitized single-line values, and expects callers to feed the accepted value back through
`value` on the next render. Richer editor behavior such as multiline input, password masking,
undo/redo, and completion remains out of scope. `Field` still stays separate from the editing
controller and remains composition-only. `focus_ring_shadow` is GPUI-adapter code and should stay
out of a future headless crate if `FocusRing` is extracted.
ADR 0008 keeps current-crate productization as the active roadmap. ADR 0006 keeps
`open-gpui-ui-headless` deferred after the strict boundary checkpoint, and ADR 0007 records the
post-boundary extraction design without creating the behavior crate.
The project now has repeated reusable behavior across overlay, roving focus, listbox navigation,
scroll viewports, and splitter constraints, and component tests guard public resolved-state structs
against GPUI runtime/rendering type leaks. Public component metrics now use neutral `UiPx`
instead of GPUI `Pixels`, and direct GPUI focus/a11y re-exports have been replaced by UI-core
semantic facades with GPUI adapter mapping exposed through
`open_gpui_ui_components::gpui_adapter`. Component overlay
state now uses neutral `OverlayResolvedState`; `GpuiOverlayState` is adapter-only scheduling
state. Extraction is no longer blocked by UI-core GPUI dependencies or `UiPx` style conversion
impls; the remaining non-headless surfaces are GPUI-owned adapter APIs such as
`TextInputController`, externally supplied `ScrollHandle`, `focus_ring_shadow`, adapter geometry
conversion helpers, and GPUI overlay scheduling helpers. These public adapter APIs are now grouped
under `open_gpui_ui_components::gpui_adapter`. Shared roving-focus helpers now live in
`open_gpui_ui_components::roving_focus`, with `Tabs` preserving compatibility re-exports.
`open_gpui_ui_core` now owns `UiPx`, `UiPoint`, `UiSize`, `UiRect`, and `UiEdges`, and
`ContextMenuState` stores a neutral point anchor plus renderer-neutral `OverlayPlacementInput`.
GPUI placement is resolved only inside the adapter/render boundary. Overlay stack Escape,
outside-press, and focus-restore ordering now have window-free tests in `open_gpui_ui_core`.
`Checkbox` now exposes checked, unchecked, and indeterminate resolved state plus theme intents for
the box, indicator, label, and focus ring. `Label` now exposes control-association metadata at the
resolved-state layer while keeping the visual adapter small. `Tabs` now keeps the roving-focus
contract in resolved state, with orientation, activation mode, selected/focused/tab-stop metadata,
while the GPUI adapter owns the focus handles and `aria` wiring. `RadioGroup` reuses the shared
roving-focus helpers, exposes group required/disabled metadata plus per-item
selected/focused/tab-stop state, and maps items through `Role::RadioButton` with `aria_selected`
because the current AccessKit surface exposed by GPUI does not provide a separate checked
property. `Toggle` is button-like: it exposes `pressed`, maps to `Role::Button` with
`aria_toggled`, and intentionally stays separate from `Checkbox` tri-state semantics. `Badge` is
display-only and exposes no role in resolved state. `IconButton` reuses Button visual variants and
focus-ring color intents, but requires an explicit accessible label because the visible icon glyph
is not a reliable accessible name. `Tooltip` is descriptive-only and currently maps its surface to
`Role::Label` until the public GPUI/AccessKit role wrapper exposes a tooltip role; trigger
association and timed hover/focus execution stay in the adapter layer. `Popover` currently covers
basic non-modal dismissible surfaces with default-open and controlled-open state; nested popover
coordination, modal popover barriers, and a full reusable focus-scope runtime remain deferred.
`HoverCard` covers interactive hover/focus/manual non-modal surfaces with delayed open/close,
pass-through dismissal, and trigger/content focus lifetime tracking; safe pointer corridors,
arrows, text-selection leases, and richer focus-scope traversal remain deferred.
`ScrollArea` covers viewport overflow, axis metadata, scrollbar width metrics, and explicit
reset-on-key-change semantics. It intentionally does not yet expose custom scrollbar anatomy,
nested scroll arbitration, or Radix-style hover/auto scrollbar visibility.
`Table` covers stable row ids, row-model ordering, grouping, expansion, built-in group-row
aggregate cells, source-tree branches with manual expansion and child-load metadata, pinned
left/center/right column regions, runtime column visibility overrides, locked column hideability,
manual filtering/sorting/pagination modes with pagination totals, committed column sizing state,
clamped width resolution with region totals/offsets, row pinning with top/center/bottom regions,
sortable header action payloads, crate-root/prelude
exports, table/cell roles, and a vertically virtualized GPUI recipe whose body scroll stays inside
the table viewport.
For pinned samples, the adapter renders fixed left/right lanes plus a shared horizontal center lane
backed by `TableCenterColumnWindowPlan`, so off-window center headers and cells are unmounted while
spacer geometry preserves the full scrollable width. It also ships GPUI resize handles with
controlled commit callbacks and on-end/on-change resize mode support.
For row-pinned samples, top and bottom row bands render outside the center vertical scroll area,
and the center virtualizer counts only center rows.
Table faceting is a metadata sidecar over configured columns: client facets derive unique
value/count entries and numeric ranges from the source snapshot while excluding the target column's
own local filter, and manual facet payloads can replace client-derived summaries for server-owned
counts without giving the component crate fetch/cache responsibility. `TableFacetedFilter` turns
one categorical facet column into an official searchable Popover recipe with controlled
`TableFacetedFilterChange` payloads and stable option selectors. `TableRangeFilter` turns one
numeric facet column into an official min/max Popover recipe with controlled
`TableRangeFilterChange` payloads, finite-bound parsing, and stable min/max input selectors.
`TablePredicateFilter` turns one text or numeric column into an official operator/value recipe
with controlled `TablePredicateFilterChange` payloads and stable operator/value selectors.
`TableColumnVisibility` turns configured columns and sparse visibility overrides into an official
Popover recipe with controlled `TableColumnVisibilityChange` payloads, locked identity rows, and
stable column-row / action selectors.
`VirtualizerState` covers one-dimensional range math, stable item keys, measurement idempotence,
overscan, total size, and snapshot/restore data in `ui_core`; the Table adapter restores snapshot
measurements but not captured scroll offsets. Sticky headers, autosize-by-content, data-source
orchestration, global faceting, richer editor families, synthetic summary rows, and deeper
two-axis grid virtualization remain follow-up
work.
`StatusCue` and `EmptyState` are official feedback components. They expose resolved feedback
intent, size, role, metrics, and token intents, while the GPUI adapters own concrete styling and
rendered debug selectors. `Tree` is now an official rendered component backed by `TreeState`.
Its adapter owns keyed GPUI runtime state, focus handles, expansion overrides, selection/toggle
callbacks, and a persistent inner `ScrollHandle`. `TreeState` remains the renderer-neutral
hierarchy contract and gallery readout for visible flattening, selected/focused metadata,
disabled-item skipping, expansion toggle payloads, tree/tree-item roles, and keyboard
selection/focus/toggle actions.
`VirtualizedList` is now an official rendered component. Its adapter resolves a
`VirtualizedListRenderPlan` from stable descriptors, owns a keyed GPUI runtime plus persistent
`ScrollHandle`, and keeps row rendering inside its viewport. `VirtualizedListState` remains the
renderer-neutral keyboard/navigation contract: active/selected indices, page navigation,
activation payloads, viewport item count, fixed row metrics, overscan, and semantic scroll
strategy labels. Rendered range calculation remains owned by
`open_gpui_ui_core::VirtualizerState`.
`Splitter` covers panel fraction normalization, min/max constraints, collapsed-panel metadata,
stable handle anatomy, and local pointer dragging through keyed runtime state. Keyboard resizing,
controlled resize callbacks, persisted layouts, RTL behavior, and nested splitter arbitration
remain follow-up work.
