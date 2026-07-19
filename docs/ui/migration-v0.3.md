# Open GPUI v0.3 UI Migration

Open GPUI v0.3 is an intentionally breaking, pre-release architecture release. Replaced APIs are
deleted rather than retained as aliases. This guide records each migration in the implementation
unit that introduces it.

## Form Lifecycle Authority

`FormStore` now derives `FormStatus` and submission eligibility from active validation plus its
submission phase. Callers no longer coordinate status changes indirectly.

`FormStore` is no longer `Clone`: cloning a live authority made its outstanding tickets ambiguous.
Share `FormSnapshot` or application-owned field values when another consumer needs a copy. Tickets
are scoped to the exact store that created them and are rejected by other stores.

### Async Validation

`ValidationTicket` is opaque and includes the field value revision. Completion returns a typed
result instead of `bool`:

```rust
let ticket = form.begin_async_validation(&email)?;
let completion = form.complete_async_validation(ticket, errors);

match completion {
    ValidationCompletion::Applied => {}
    ValidationCompletion::Stale | ValidationCompletion::Cancelled => return Ok(()),
}
```

An edit, reset, synchronous validation, or newer generation invalidates old work. Starting
validation during submission now returns `FormError::CannotValidateWhileSubmitting`.

Successfully registering a new field also advances the form revision and invalidates an active or
terminal submission because the submitted data shape changed. A rejected duplicate registration
does not mutate lifecycle state.

### Submission

`begin_submit()` now returns `Result<SubmitTicket, FormError>`. Both finish methods require that
ticket and return `SubmitCompletion`:

```rust
let ticket = form.begin_submit()?;

match request_result {
    Ok(()) => {
        let _completion = form.finish_submit_success(ticket);
    }
    Err(error) => {
        let _completion = form.finish_submit_error(ticket, error.to_string());
    }
}
```

Handle `FormError::CannotSubmit { reason }` with `SubmitBlockReason::{Invalid, Validating,
AlreadySubmitting}`. The old free-form rejection reason and ticketless finish methods no longer
exist. Rejected starts do not increment `submit_count`.

### UI Projection

Use `FormProjection::resolve(&snapshot, disabled)` for form-level busy state and submit eligibility.
`FormFieldProjection` now exposes `validating()` and `busy()`, and propagates busy state to Field,
TextInput, Textarea, NumberInput, and Checkbox state. Busy validation does not imply disabled or
read-only input. When rebuilding concrete components from those states, pass `busy(state.busy())`;
all five concrete builders preserve that fact in their resolved `state()`.

### DevTools Boundary

The first-party form adapter no longer forwards caller-selected values, field paths, field ids, or
free-form errors. It emits typed redaction markers, counts, lifecycle facts, and deterministic
opaque field identities before a `DevtoolsCapture` is constructed. Do not parse DevTools payloads
for application form data; application and rendering code should consume `FormSnapshot` directly.

## Theme Scope Authority

Theme selection is no longer owned by a public app-global `ThemeRuntime`. Install definitions and
select app or window authority through the explicit theme-owner API, then resolve the effective
owned context with the window-aware resolver:

```rust
use open_gpui_ui_components::theme::{
    ThemeContext, ThemeResolver, ThemeScope, install_theme_registry, set_app_theme,
    set_window_theme,
};

install_theme_registry(cx, registry, "light")?;
set_app_theme(cx, "dark")?;
set_window_theme(window, cx, "high-contrast")?;

let effective = ThemeResolver::current(window, cx);
let subtree = ThemeScope::new("settings-theme", ThemeContext::dark(), child);
```

The precedence is nearest subtree scope, window selection or explicit override, app selection,
then the built-in light fallback. `ThemeScope::new` now requires a stable element id so a context
change can invalidate cached child-view journals. Window state is owned by the window and is
dropped on close. Unknown ids and equal selections are atomic no-ops and do not refresh unrelated
windows. Replacing the complete registry does not silently demote a window whose selected id is
temporarily absent: it retains the last-known snapshot until that id returns or
`clear_window_theme` explicitly restores app inheritance.

Official overlays freeze the effective context when their lifecycle generation opens. Trigger
styling follows the current context, but surface colors and deferred children retain the opening
context until close; reopening captures again. Delayed Button, IconButton, Menu, Sidebar, and
Toolbar tooltips capture automatically. Direct GPUI tooltip attachment must make the boundary
explicit:

```rust
use open_gpui_ui_components::Tooltip;

let context = ThemeResolver::current(window, cx);
let trigger = div().tooltip(Tooltip::scoped(
    context,
    Tooltip::text("Saved"),
));
```

When both a custom tooltip builder and text fallback are present, official Button, IconButton, and
Toolbar adapters attach only the custom builder. The former `ThemeRuntime`, `ThemeRuntimeError`,
`ThemeResolver::current(&App)`, `ThemeResolver::resolve`, `init_theme_runtime`,
`current_theme_context`, `try_theme_context`, and `set_active_theme*` surfaces are deleted without
aliases. The scope mechanism remains theme-specific because no independent non-theme consumer met
the adoption gate for a public generic inherited-context API.
See [Theme scope resolution and deferred capture](../knowledge/engineering/decisions/theme-scope-resolution.md)
for the render-timing prototype and rejected generic-context decision.

### Complete Theme V1 Replacement

Theme v1 is no longer a color-only definition. `ThemeSnapshot` owns a complete color table plus
typed typography, spacing, radius, elevation, density, and motion-policy scales. `ThemeContext`
adds the runtime-owned effective revision used for invalidation. Serialized `revision` remains
source metadata and cannot force or suppress runtime refreshes.

Code-built definitions must now supply the complete payload. The most direct migration for a
derived theme starts from a complete snapshot:

```rust
use open_gpui_ui_components::theme::{ThemeDefinition, ThemeSnapshot};

let base = ThemeSnapshot::dark();
let definition = ThemeDefinition::from_snapshot("editor-dark", "Editor dark", &base);
```

Use `ThemeDefinition::design_scales(...)` and `.colors(...)` when replacing those complete values.
Registration rejects missing, duplicate, or unsupported color entries and missing design scales
before it mutates the registry. Metadata-only reloads preserve the effective revision; changed
effective content and changed app/window/subtree selection allocate a new monotonic revision.

Theme JSON remains schema version 1 but its shape is intentionally replaced in place. Files must
contain `design.typography`, `design.spacing`, `design.radius`, `design.elevation`,
`design.density`, and `design.motion_policy`, as well as the complete color table. Regenerate or
serialize complete files with `theme_json_string`. The old `fallback_mode`, partial-color fill,
`ThemeRegistrationDiagnostics`, caller-supplied runtime revision, and color-only loader are deleted
without aliases or compatibility parsing.

Component sizing now resolves as explicit `Size` first, then the theme density default. Theme and
component motion preferences merge to the stricter value: either side can request reduced motion,
and an explicit animated request cannot override a reduced theme. Adaptive device density remains
an application-shell recommendation rather than an implicit theme override.

## Deterministic Collection Typeahead

Collection typeahead now uses one crate-private, instance-owned session with GPUI executor time.
Tree, VirtualizedList, Menu, ContextMenu, and standalone Listbox share its printable-input,
composition, modifier, timeout, and repeated-character rules. Select receives the same behavior
only from its mounted popup Listbox; printable input on a closed Select trigger remains inert.
Combobox and Command keep their persistent editor-owned query and filtering paths and do not use
the collection session.

The timeout remains 700ms. Input at exactly the boundary refines the existing prefix; later input
starts a new session. A first character and repeated equal characters scan after the current stable
key and wrap. A different character refines the current prefix while including the current match.
Matching, disabled or structural row filtering, focus, reveal, and selection remain owned by each
component model. Sessions never store row indexes or target keys, so reorder resolves from the
latest stable key and removal falls back safely.

Tree values are stable row identities and must be unique within one resolved tree. Duplicate
values now fail closed: ambiguous rows remain visible but cannot receive focus, selection, or
typeahead targeting. Assign distinct values instead of relying on source order to disambiguate
rows.

`Listbox::typeahead_query`, `ListboxState::typeahead_query`, and the corresponding
`ListboxState::resolve` argument were deleted without aliases. They copied an editable owner's
query into inert Listbox metadata and were not the runtime buffer. Read `ComboboxState::query` or
`CommandState::query` when displaying or inspecting editable search, and call
`ListboxState::typeahead_target` directly when a pure caller-owned query needs model resolution.
There is no public collection-session API.

`Listbox::active`, `Select::active`, `Combobox::active`, and `Command::active` were also deleted.
They behaved like caller-owned values but exposed no active-change intent, so every redraw could
silently overwrite keyboard or typeahead progress. Use `default_active` to seed the adapter-owned
keyed runtime once. Renderer-neutral state requests still accept an `active_value`, and the
editor-owned Combobox adapter projects its current runtime value into its private embedded
Listbox; neither path recreates a public controlled-active API.

## Interactive Subtree Transform And Coordinates

The SVG-only `Transformation`, `TransformationMatrix`, and `Svg::with_transformation` APIs are
deleted without aliases. They changed raster output while leaving descendants, input, IME,
accessibility, and diagnostics at the old geometry. Use the checked layout-neutral subtree
authority for supported axis-aligned scale and translation:

```rust
use open_gpui::{
    SubtreeTransform, SubtreeTransformExt as _, SubtreeTransformOrigin, div, point, px, size,
};

let transform = SubtreeTransform::try_new(
    size(1.25, 0.9),
    point(px(8.0), px(-4.0)),
    SubtreeTransformOrigin::CENTER,
)?;

let element = div().child(content).with_subtree_transform(transform);
```

The contract accepts only finite, strictly positive normal axis scale with a representable reciprocal,
finite logical-pixel translation, and a finite post-layout origin. `TOP_LEFT` and `CENTER` are
built in; `SubtreeTransformOrigin::try_new(anchor, offset)` resolves
`anchor * child_size + offset` after layout. Rotation, skew, arbitrary matrices, reflection, and
3D have no replacement in this API. A numeric or backend conversion failure suppresses the whole
affected subtree transaction; it never clamps or substitutes identity.

Retained geometry or other state published outside the frame journal must use a stable
`PrepaintPublicationId` with `Window::record_prepaint_window_transaction`. GPUI commits it only
after a valid paint and invokes its discard callback when the frame is invalid or the publication
is absent from the next completed frame. Reuse the ID across renders of one logical producer; do
not publish directly from prepaint or preserve last-known state after unmount or rollback.

For a top-left-relative pixel origin, use the checked
`SubtreeTransformOrigin::try_pixels(offset)` constructor. The origin type has no unchecked pixel
conversion or `From<Point<Pixels>>` implementation; invalid input must be handled at the call site.

Raw platform events remain window-space. Interactive listeners now receive `TargetedEvent<E>`
where target geometry is required. Read the original event through `window_event()` and use the
checked target-local or target-layout helpers for control logic:

```rust
div().on_mouse_move(|event, _window, _cx| {
    let raw_window_position = event.window_event().position;
    let Ok(local_position) = event.target_local_position() else {
        return;
    };
    update_selection(local_position, raw_window_position);
})
```

`on_click` and `on_aux_click` receive `TargetedEvent<ClickEvent>`. Drag construction receives
`DragStartGeometry`; `on_drag_move` receives `DragMoveEvent<T>` whose `drag()` reads the captured
payload; and `on_drop` receives `DropEvent<T>` with `value()` plus a targeted `pointer()`. Use
`window_preview_offset()` only for the window-space drag preview. Pixel wheel deltas can be read in
target-local units; line deltas preserve their semantic unit.

For committed geometry, use `Hitbox::geometry()` or wrap content with `measured_element`. Its
listener is now `Fn(MeasuredElementSnapshot, &mut App)`: the snapshot carries frame generation,
semantic/global identity, and one immutable `ElementGeometry` with layout bounds, displayed
bounds, and checked local/layout/window conversions. Failed transform scopes publish no snapshot.

Complex custom consumers must keep visual stacking and pointer ownership in the same committed
coordinate model. Docking now resolves divider junctions separately for the root and each floating
surface, gives every floating container a blocking boundary across its complete bounds, and retains
one stable capture owner for raw and standard GPUI drags. High-level drag sources acquire it after
crossing the drag threshold, so removal of their owner binding produces terminal cancellation
without capturing ordinary clicks. Terminal cancellation keeps the GPUI window-owned payload
visible while each observer clears only its own runtime session, preview, anchor, and outside-release
poll; this lets the true owner observe cancellation even when multiple independent hosts share a
window. Establish component payload state only after policy and checked geometry accept the
underlying drag session, and defer rollback when the framework writes its active drag after the
constructor returns. Do not flatten transformed overlays into one hit list, treat only dividers as
occluding, or leave component drag state dependent on receiving a normal mouse-up.

Motion remains below GPUI. Sample `MotionProjection::try_transform_sample` and convert it in a
consumer that depends on both crates, such as
`open_gpui_ui_components::gpui_adapter::subtree_transform_from_motion_projection`. Do not add a
GPUI type or identity fallback to `open-gpui-motion`.

See [ADR 0021](../adr/0021-open-gpui-interactive-subtree-transform-authority.md) for composition,
cache/deferred, renderer, and failure-ordering details.

## Semantic Activation Authority

Official controls normalize pointer, allowed key-up, AccessKit Click, and programmatic requests
through one activation transaction. `Activation` carries the typed source, while domain payloads
carry only the item or value facts required by the callback. Use an `ActivationHandle` for
application-driven activation instead of synthesizing pointer coordinates or key events.

### Listbox Selection And Activation

`Listbox::selected` now accepts `Option<String>` and is always caller-owned. Use
`Listbox::default_selected` when the Listbox adapter should own subsequent selection commits. Bind
programmatic selection by stable option value with `Listbox::activation_handle`:

```rust
use open_gpui_ui_components::{ActivationHandle, Listbox};
use open_gpui_ui_components::listbox::ListboxOption;

let beta = ActivationHandle::new();
let listbox = Listbox::new("frameworks", "Frameworks")
    .default_selected("alpha")
    .option(ListboxOption::new("alpha", "Alpha"))
    .option(ListboxOption::new("beta", "Beta"))
    .activation_handle("beta", &beta)
    .on_select(|selection, window, cx| {
        record_selection(selection.value(), window, cx);
    });
```

An option-level `ListboxOption::on_select` replaces the Listbox-level public fallback; the two are
not both delivered. When the option is rendered inside Select or Combobox, the owning adapter still
commits its controlled/uncontrolled transaction, input projection, and overlay close before it
delivers the chosen item or family callback. Disabled, structural, and duplicate-value rows reject
pointer, keyboard, AccessKit, and programmatic activation. Separators no longer use reserved String
sentinels, so every caller-provided option value remains legal.

### Select, Combobox, And Command Selection Ownership

`Select::selected`, `Combobox::selected`, and `Command::selected` now match Listbox: they accept
`Option<String>` and always represent the caller-owned render-frame value. Use each component's
`default_selected` builder when its adapter should own later single-selection commits. Command
multi-selection keeps `selected_values(...)` as its caller-owned input and adds
`default_selected_values(...)` for adapter-owned state.

```rust
let controlled = Select::new("framework", "Framework")
    .selected(selected_framework.clone())
    .option(ListboxOption::new("alpha", "Alpha"));

let uncontrolled = Command::new("tools", "Tools")
    .default_selected("search")
    .item(CommandItem::new("search", "Search"));
```

A controlled selection callback emits one intent without changing hidden adapter state. If the
owner refuses it, selection and Combobox input text remain unchanged; the new value or label is
projected only after a later caller prop commit. Selection callbacks may synchronously redraw.
The matching overlay close observer is still delivered exactly once after an ordinary rebind,
while a genuinely superseding request, owner commit, ownership change, or unregister invalidates
the older dispatch.
Command multi-select callbacks toggle the raw caller/runtime value set. Values that are currently
missing, disabled, or filtered out stay in the emitted set even though they do not project as chips
or selected options. Toggling an existing value removes every duplicate occurrence of that value,
so the emitted collection cannot report deselection while retaining the same logical member.

### Toolbar Activation

`ToolbarSelection` and both `Toolbar::on_select` and `ToolbarItem::on_select` are deleted. Replace
them with `ToolbarActivation` and `on_activate`, whose callback also receives `Activation`:

```rust
use open_gpui_ui_components::{ActivationHandle, Toolbar};
use open_gpui_ui_components::toolbar::ToolbarItem;

let save = ActivationHandle::new();
let toolbar = Toolbar::new("editor-toolbar", "Editor actions")
    .item(ToolbarItem::action("save", "Save"))
    .activation_handle("save", &save)
    .on_activate(|item, input, window, cx| {
        dispatch_toolbar_action(item.value(), input.source(), window, cx);
    });
```

Action items activate on unmodified Enter or Space key-up. Toggle items activate on Space key-up
only, and their `pressed()` payload is the caller-owned state from before activation; Toolbar does
not mutate it. Disabled items share the same gate for every source. An item-level `on_activate`
handler overrides the toolbar-level fallback, so registering both does not execute an action twice.
Roving Arrow/Home/End navigation remains a separate focus transaction. Item values must be unique
within one Toolbar. Duplicate values remain visible but fail closed as disabled and reject every
activation source rather than letting render order choose a programmatic target.
Diagnostic selectors for duplicate occurrences end in `:snapshot:<opaque-token>`. Treat them as
current-snapshot probes: do not construct or persist the token, and reacquire the selector after an
authored item reorder.
Custom view tooltip closures are not mounted for duplicate Toolbar values because closure contents
cannot provide stable authored identity. Use a text tooltip or unique item values instead.

### Sidebar Activation

`SidebarSelection`, `Sidebar::on_selection_change`, and `SidebarItem::on_select` are deleted.
Replace them with `SidebarActivation` and `on_activate`; bind application-driven activation by
stable item value:

```rust
use open_gpui_ui_components::{ActivationHandle, Sidebar, SidebarSection};
use open_gpui_ui_components::sidebar::SidebarItem;

let settings = ActivationHandle::new();
let sidebar = Sidebar::new("app-sidebar", "Application navigation")
    .section(
        SidebarSection::new("account", "Account")
            .item(SidebarItem::new("settings", "Settings")),
    )
    .activation_handle("settings", &settings)
    .on_activate(|item, input, window, cx| {
        navigate(item.value(), input.source(), window, cx);
    });
```

Pointer, Enter/Space key-up, AccessKit Click, and programmatic requests now share one transaction.
The callback observes caller-owned `selected()` state from before activation. Item-level handlers
override the Sidebar fallback. Item values must be globally unique across sections; duplicates stay
visible but are disabled, non-focusable, and blocked for programmatic activation. Offcanvas and
disabled Sidebars bind handles as blocked rather than dispatching or selecting an arbitrary item.
Duplicate section and item selectors likewise carry an opaque current-snapshot suffix so a stale
selector, AccessKit node id, or activation state key cannot silently retarget after reorder.

## Federated Component Contract Authority

The component product table now owns only official id, revision, family, and required scenario ids.
The former API inventory, source mapping, public-surface owner map, Gallery/docs status rows,
accessibility evidence rows, and central conformance gates are deleted without compatibility
aliases. Use `ComponentContractEntry` / `ComponentContractMetadata` for product identity and use the
natural owner for every downstream fact:

- public exports are declared together with typed `PublicApiExport` facts;
- Components and Overlay Gallery rows own presentation status, selectors, and Story probes;
- native integration targets own exact coordinates in sibling `*.scenarios.toml` artifacts;
- DevTools semantic payloads carry `contract_id`, `contract_revision`, and `family` from canonical
  metadata;
- `scan-ui-contract` joins and executes these owners but is not another registry.

`ComponentContractEntry` is now immutable canonical metadata rather than a public struct-literal
inventory row. Replace `entry.name` with `entry.id().as_str()`, `entry.family: Option<_>` with
`entry.family().as_str()`, and read the new `entry.revision()` and
`entry.required_scenarios()` projections. Owner, Gallery/docs status, export, and source fields have
no replacement on this row; query their natural owners instead.

Delete imports of `CallbackApi`, `DefaultSeedApi`, `ComponentApiInventoryEntry`,
`ComponentA11yEvidence`, `ComponentConformanceGate`, `PublicSurfaceOwnerClass`,
`PublicSurfaceOwnerEntry`, `SurfaceGalleryStatus`, and `SurfaceDocsStatus`. Also delete uses of
`COMPONENT_API_INVENTORY`, `COMPONENT_A11Y_EVIDENCE`, `COMPONENT_CONFORMANCE_GATES`,
`PUBLIC_SURFACE_OWNER_MAP`, `component_public_methods`, `component_render_inputs`,
`public_owner_for_component_inventory`, `component_recipe_component_rows`,
`default_surface_rows`, `gallery_surface_rows`, `official_component_rows`,
`official_overlay_component_rows`, and `component_source_inputs`. These APIs have no aliases or
method/source manifest replacement; inspect the owning Rust API and use executable tests.

## Accessibility Semantic Projection

Official semantic producers now derive an ephemeral `SemanticDescriptor` from their resolved
render state. The descriptor is projected into GPUI with `UiA11yElementExt::ui_semantics`; it is
not stored as a second component state model. Final `TreeUpdate` assertions and real AccessKit
action dispatch are the executable authority. Static `COMPONENT_A11Y_EVIDENCE` rows, Gallery
`COMPONENT_A11Y_CLAIMS`, and their consumers were deleted rather than retained as transitional or
substitute runtime evidence.

See
[Semantic accessibility and final-tree authority](../knowledge/engineering/decisions/semantic-accessibility-final-tree-authority.md)
for the projection, lifecycle, executable-evidence, and DevTools redaction decision.

IconButton keeps its accessible name separate from its description. Slider and NumberInput expose
`SetValue` only when a change callback exists, consume only numeric AccessKit payloads, and reject
non-finite values. Disabled controls reject focus and mutation actions; read-only NumberInput keeps
focus but rejects value changes. Indeterminate Progress omits the current numeric value while
retaining its supported range.

`NumberInputStepAction` now includes `SetValue`, so exhaustive downstream matches must handle the
new variant. This breaking addition preserves the real source of a `NumberInputChange`; mapping an
explicit accessibility value request to Increment or Decrement would lose behaviorally relevant
information.

TextInput and Textarea now publish their value, required, invalid, busy, read-only, disabled, and
available text actions from an ephemeral descriptor. Textarea uses the multiline text-input role;
password inputs use the password role and expose only a masked value in the final tree. AccessKit
`SetValue` and `ReplaceSelectedText` enter the same controlled text-editing path as platform input.
Plain TextInput publishes one stable `(control id, "text-run")` child. Textarea publishes one stable
TextRun per logical line: its first run uses `(control id, "text-run")`, later runs use stable
`text-run:{line index}` identities, a hard line break belongs to the preceding run, and a trailing
line break creates an empty final run. Directional selection may span Textarea runs. AccessKit
character indices follow Unicode grapheme boundaries, including combining sequences, emoji ZWJ
sequences, and normalized LF line breaks. Read-only controls remain focusable and selectable but
reject value mutation; disabled controls reject both selection and mutation. Password inputs
intentionally publish neither a TextRun child nor text-selection actions, so masked content cannot
be recovered through fine-grained accessibility metadata. If any single grapheme exceeds
AccessKit's `u8` character-length limit, the control omits TextRun and selection metadata and
retains only whole-value accessibility actions.

`Label::for_control` and `LabelState` control-association metadata have been removed because they
never created a real accessibility relation. Compose labels and support text through `Field`, whose
typed control adapter resolves `labelled_by`, `described_by`, and validation error relations from
the actual GPUI element path. Standalone `Label` remains a visible-text semantic primitive.
`Field::new(id, control_id, label)` is replaced by `Field::new(id, label)`, and
`FieldState::control_id()` is removed; the composed control continues to own its own element id.
Custom field controls implement `open_gpui_ui_components::gpui_adapter::FieldControl` rather than
depending on the renderer-neutral prelude.

### Table Public Paths

Table restoration inputs remain common application APIs, while diagnostic readouts now require
their owner modules:

| Removed v0.2 import | v0.3 import |
| --- | --- |
| `open_gpui_ui_components::TableBehaviorSnapshot` and companion behavior snapshot types | `open_gpui_ui_components::table::TableBehaviorSnapshot` and the named companion types in `open_gpui_ui_components::table` |
| `open_gpui_ui_components::common::TableBehaviorSnapshot` and companions | Import the explicit `open_gpui_ui_components::table` types; they are not common-facade APIs |
| `open_gpui_ui_components::prelude::TableBehaviorSnapshot` and companions | Import the explicit `open_gpui_ui_components::table` types; they are not prelude APIs |
| `open_gpui_ui_core::TableStateCacheKey` or `open_gpui_ui_core::table::prelude::TableStateCacheKey` | `open_gpui_ui_core::table::TableStateCacheKey` |

`Table` and `TableVirtualizerSnapshot` remain available from the component root/common prelude.
`TableState`, row-model inputs, and the Table engine remain under `open_gpui_ui_core`; no behavior
contract was deleted merely to narrow an import path.

`TABLE_ROW_MODEL_PIPELINE`, `TABLE_ROW_MODEL_V0_PIPELINE`, and
`TableRowModelStage::implemented_in_v0` were deleted without aliases because they only restated a
version label. Use `TableResolvedState::{core_model, filtered_model, grouped_model, sorted_model,
expanded_model, paginated_model, final_model}` when code needs executable stage output, and use
`TableRowModelStage::as_str()` only for a stable label.

### Table Logical Identity

Table row pinning, expansion, rendering, editing, callbacks, virtualization, and accessibility now
share `TableRowIdentity` as the authoritative logical-row identity. Source rows with duplicate
business `TableRowId` values resolve to distinct source-instance identities, and synthetic group
rows use a separate typed namespace. Pinning and virtual-window movement no longer add region or
slot identity, so the same logical row and cell keep their final AccessKit node ids when they move.

`TableRowPinning::pinned_top` and `pinned_bottom` no longer accept strings or `TableRowId` values.
Pass `TableRowIdentity` for one exact source instance or group row. Use
`TableRowPinTarget::all_source_rows(row_id)` only when pinning every resolved source instance with
the same business id is intentional. Targets retain caller order; bulk matches retain current
model order; top wins logical overlap after target expansion. This is a clean public-contract
replacement with no compatibility alias for the old business-id-only pin state.

`Table::virtualizer_snapshot` now accepts `TableVirtualizerSnapshot` instead of the generic
`VirtualizerSnapshot`. Build retained measurements with `TableVirtualizerSnapshotItem` and a
`TableRowIdentity`; string keys are no longer accepted because they bypass duplicate-source and
group-row identity rules. The Table adapter owns the private conversion to generic virtualizer
keys, so application code should not encode row identities itself.

`TableRowId` remains the application business key, but it is no longer an exact target. Choose the
source identity form deliberately:

```rust
use open_gpui_ui_core::{
    TableRow, TableRowIdentity, TableRowPinTarget, TableSourceRowIdentity, TableState,
};

let state = TableState::new([
    TableRow::new("unique-row"),
    TableRow::new("duplicate"),
    TableRow::new("duplicate"),
    TableRow::new("duplicate").with_instance_id("retained-instance"),
]);
let unique = TableSourceRowIdentity::unique("unique-row");
let occurrence = state
    .source_row_identity_at("duplicate", 1)
    .expect("the current source snapshot contains a second occurrence");
let retained = TableSourceRowIdentity::explicit("duplicate", "retained-instance");
let selected_state = state
    .clone()
    .with_selected_rows([unique.clone(), retained.clone()]);
let exact_pin = TableRowPinTarget::exact(TableRowIdentity::source_instance(
    "duplicate",
    "retained-instance",
));
let bulk_pin = TableRowPinTarget::all_source_rows("duplicate");
```

`TableSourceRowIdentity::unique` means that the business id must be unique in the current source
snapshot; lookup returns `TableSourceRowLookup::Ambiguous` when it is not. An occurrence returned
by `source_row_identity_at` is valid through `TableState` clones and row-model transforms, but any
`with_rows` replacement or reorder creates a different snapshot and lookup returns
`TableSourceRowLookup::StaleSnapshot`. Use `TableRow::with_instance_id` and
`TableSourceRowIdentity::explicit` for identity retained across source replacement or reorder.
Row selection now follows the same rule: `TableState::with_selected_rows` accepts exact
`TableSourceRowIdentity` values, `selected_rows()` returns that exact set, and
`TableRowSelectionChange::current_selection()` returns caller-owned explicit selection roots in
source-model order. Derived descendants are not promoted into explicit state. With
`TableSubRowSelectionPolicy::Descendants`, canceling a selected parent removes its explicit subtree;
canceling an inherited selected descendant removes the explicit ancestor that covers it, so
committing the callback payload makes that descendant unselected. Duplicate business ids no longer
select or deselect every occurrence together. The unimplemented
`TableSelectionScope` surface has been removed; any future business-id bulk selection must use a
separately named target rather than an implicit conversion. `Table::on_row_expansion_request` is
also strictly controlled: a pointer or keyboard request does not render an expanded branch until
the caller commits a changed `TableState`.

Cell edits now target the exact `(TableRowIdentity, TableColumnId)` pair. Construct an
application-owned edit with `TableCellEditRequest::new(TableSourceRowIdentity, ...)`; the
`TableCellEditChange` emitted by `Table::on_cell_edit_change` is reserved for renderer-resolved
callbacks and therefore always carries real `TableRowAction` metadata. `source_row_id()` is a
business-id readout, not target authority. Applying a unique-assumption request to duplicate rows returns
`TableCellEditApplyOutcome::AmbiguousRowId`, and applying an occurrence edit to a newer source
snapshot returns `StaleRowIdentity`. Both outcomes leave the current rows and Table cache identity
unchanged; an exact unique, explicit-instance, or current-snapshot occurrence edit updates only the
intended source row.

The identity-sensitive accessor migration is explicit. A `source_row_id()` result is a business-key
readout and must not be passed back as though it uniquely identified the resolved row:

| Removed or changed v0.2 surface | v0.3 replacement |
| --- | --- |
| `TableResolvedRow::id()` | `identity()` for exact lookup and targeting; optional `source_row_id()` for source-backed display/diagnostics |
| `TableResolvedRow::parent_id()` | `parent_identity()` |
| `TableGroupRow::parent_id()` | `parent_identity()` |
| `TableGroupRow::first_leaf_row_id()` | `first_leaf_identity()` |
| `TableTreeRow::parent_id()` | `parent_identity()` |
| `TableRowModel::rows_by_id()` | No business-id map replacement. Use `rows()` for materialized model order, `lookup_rows()` for every addressable row, or `row(&TableRowIdentity)` for one exact row. |
| `TableRowModel::row(&TableRowId)` | `row(&TableRowIdentity)` for an exact row; `source_rows(&TableRowId)` for every matching source instance; `unique_source_row(&TableRowId)` only when exactly one match is acceptable |
| `TableResolvedState::duplicate_row_ids()` | `row_identity_diagnostics()`, including typed `DuplicateRowId` and `DuplicateSourceInstance` diagnostics |
| `TableRowAction::row_id()` | `identity()` for exact targeting; `source_row_id()` only for display/diagnostics |
| `TableRowActivation::row_id()` | `identity()`; optional `source_row_id()` readout |
| `TableRowExpansionToggle::row_id()` | `identity()`; optional `source_row_id()` readout |
| `TableRowSelectionChange::row_id()` | `identity()`; optional `source_row_id()` readout |
| `TableState::with_selected_rows(TableRowId...)` / `selected_rows() -> BTreeSet<TableRowId>` | Pass exact `TableSourceRowIdentity` values; `selected_rows()` returns `BTreeSet<TableSourceRowIdentity>`. |
| `TableRowSelectionChange::current_selection() -> &[TableRowId]` | `current_selection() -> &[TableSourceRowIdentity]` as caller-owned explicit roots in source-model order. |
| `TableSelectionScope` | Removed. Only exact row selection exists; add an explicitly named bulk target if a real consumer requires one. |
| `TableExpansionState::rows(TableRowId...)` / `is_expanded(&TableRowId)` | `rows(TableRowIdentity...)` / `is_expanded(&TableRowIdentity)` |
| `TableCellEditChange::for_row(...)` / `for_source_identity(...)` | `TableCellEditRequest::new(TableSourceRowIdentity, ...)` for programmatic edits; runtime callbacks receive `TableCellEditChange` |
| `TableCellEditChange::row_id()` | `identity()` for the exact callback row; optional `source_row_id()` readout |
| `TableBehaviorSnapshot::row(&TableRowId)` | `row(&TableRowIdentity)` for one exact rendered row; `source_rows(&TableRowId)` or `unique_source_row(&TableRowId)` for rendered business-id lookup |
| `TableRowBehaviorSnapshot::id()` | `identity()` for the exact rendered row; optional `source_row_id()` readout |
| `TableResolvedHeaderCell::id()` | `identity()` for the typed resolved fragment; `logical_identity()` when pinning-region fragmentation must be ignored |
| `TableResolvedHeaderCell::source_id()` | `source_column_id()` for a leaf, `source_group_path()` for a group, or inspect `logical_identity()` when handling every header kind |
| `TableResolvedHeaderCell::placeholder_id()` | No string-id replacement. Match `TableHeaderIdentity::Placeholder` through `logical_identity()`. |
| `TableResolvedHeaderCell::sub_header_ids()` | `sub_header_identities()` |
| `TableResolvedHeaderGroup::id()` | `identity()` returning `TableHeaderRowIdentity`; read `region()` separately because row identity is region-independent |
| `TableRowPinning::pinned_top(TableRowId...)` / `pinned_bottom(TableRowId...)` | Pass exact `TableRowIdentity` targets, or explicit `TableRowPinTarget::all_source_rows(TableRowId)` bulk targets. |
| `TableRowPinning::top()` / `bottom()` | `top_targets()` / `bottom_targets()` returning ordered `TableRowPinTarget` slices |
| `TableRowAction::render_key()` / `TableCellEditChange::render_key()` | No like-for-like string accessor. Retain `identity()` as authority, use `identity().key()` only when a typed `TableRowIdentityKey` is required, and use `TableDebugSelector` builders for official selectors. |
| `TableDebugSelector::select_editor_option(...)` | Removed without replacement. Table owns the cell-editor identity, while the nested Listbox owns rendered option selectors. Render the editor and query the unique Listbox option within that editor owner instead of synthesizing a cross-component selector. |
| `Table::default_focused_row(TableRowId)` | `Table::default_focused_row(TableRowIdentity)` |
| generic `VirtualizerSnapshot` string keys | `TableVirtualizerSnapshotItem::new(TableRowIdentity, size)` |
| `TableColumnOrderChange::apply_to_order(column_order)` | `apply_to(TableState) -> TableState`; the state supplies the full source-column authority used to normalize partial order before the move |

Column order no longer owns visibility. A partial `TableState::column_order()` puts its known ids
first, and `normalized_column_order()` appends every unlisted source column in source order while
ignoring unknown and duplicate ids. `TableColumnOrderChange::apply_to(TableState)` normalizes that
complete source order before moving either a listed or previously unlisted column, then stores the
full order without changing visibility or pinning.

Virtual Table focus remains logical when a row leaves the rendered overscan window. The stable
Table root becomes the physical and AccessKit focus proxy, publishes no stale row actions, and
continues real Up, Down, Home, End, Enter, and Space behavior against the complete final model. A
row remount reclaims physical focus only while that proxy still owns the claim. If the exact row
leaves the final model, focus falls back to its first remaining row or clears for an empty model;
focus already moved outside the Table is never stolen.

### VirtualizedList Render Identity

`VirtualizedListRowBehaviorSnapshot::render_key()` and
`VirtualizedListRowRenderContext::render_key()` are opaque adapter identities, not domain keys.
Use `VirtualizedListItemDescriptor::key()` for application identity. Duplicate source keys now use
a collision-checked, length-prefixed occurrence encoding so they cannot alias any legal unique
source key. Do not parse, format, or persist the encoding; only round-trip a render key to the
adapter-owned measurement path for the same ordered descriptor snapshot. Reordering duplicate
items creates a new occurrence authority.

### DevTools Semantic Probes

The old `a11y_evidence_probe_snapshot` and `a11y_contracts_probe_snapshot` entry points are deleted
without compatibility aliases. They projected contract evidence and claim rows rather than the
resolved semantics used by the renderer.

Construct the replacement probe from canonical component metadata, an app-assigned opaque identity,
and the ephemeral resolved descriptor:

```rust
use open_gpui_devtools::ui_components::{
    ComponentSemanticIdentity, OpaqueSemanticNodeId, ResolvedSemanticNode,
    resolved_semantics_probe_snapshot,
};

let component = ComponentSemanticIdentity::for_component("TextInput")
    .expect("TextInput must have a canonical component contract row");
let node = ResolvedSemanticNode::new(
    component,
    OpaqueSemanticNodeId::new(42),
    semantic_descriptor,
);
let snapshot = resolved_semantics_probe_snapshot([node]);
```

`OpaqueSemanticNodeId` must not encode a renderer node id, accessible text, or application data.
The new payload replaces `contract_count` and claim rows with a root `node_count` plus typed,
redacted semantic nodes. Each node retains canonical contract/family metadata, role, state, actions,
structural counts, and presence facts; accessible text and numeric values are represented only by
typed redaction markers.

## GPUI Pointer Sessions

Window removal now needs application context so GPUI can deliver terminal pointer cancellation and
clear pressed-button, pointer-capture, and drag state before the window disappears. Replace
`window.remove_window()` with `window.remove_window(cx)`. Removing one window clears only sessions
owned by that window; it does not cancel a drag or capture owned by another window.

The pointer-capture API is a clean break:

| v0.2 | v0.3 |
| --- | --- |
| `window.capture_pointer(hitbox_id)` | Retain a `PointerCaptureHandle`, bind it each frame, then call `window.capture_pointer(&handle, button)?`. |
| `window.release_pointer()` | Call `window.release_pointer(&handle)?`; only that owner can release its capture. |
| `window.captured_hitbox()` | Use `window.captured_pointer()` and inspect `PointerCapture::handle()` plus `PointerCapture::button()`. |
| `hitbox.is_hovered(window)` for captured event routing | Use `hitbox.is_mouse_event_target(window)`; reserve `is_hovered(window)` for physical hover. |

Custom controls that must keep receiving mouse move and button events after the pointer leaves their
hitbox now use a stable, window-owned `PointerCaptureHandle`:

```rust
let capture = window.new_pointer_capture_handle();

// Bind the retained handle while rendering every frame.
let owner = div().track_pointer_capture(&capture);

// Start capture from the owner's mouse-down path after GPUI records the pressed button.
window.capture_pointer(&capture, event.button)?;

// End capture explicitly when custom interaction logic completes.
window.release_pointer(&capture)?;
```

Create and retain the handle once for the control instance, then bind that same handle with
`track_pointer_capture` in every rendered frame. `capture_pointer` requires both a current-frame
binding and an already pressed initiating button; creating a fresh handle during each render or
capturing before the mouse-down dispatch is rejected. Custom elements may use
`Window::bind_pointer_capture` after inserting their hitbox instead of the standard element helper.

`window.release_pointer(&capture)` returns `Result<bool, PointerCaptureError>` for explicit release.
GPUI also releases capture when the initiating button goes up, the owner is absent from the next
frame, the window deactivates, pointer cancellation occurs, or `remove_window(cx)` closes the owner
window. Use
`window.captured_pointer()` when diagnostics need the active handle and initiating button; it
returns `Option<PointerCapture>`, whose `handle()` and `button()` accessors preserve that ownership
identity.

Pointer capture separates event routing from visual hover. Mouse event handlers use
`Hitbox::is_mouse_event_target(window)` (or the equivalent `HitboxId` method) so the captured owner
continues receiving routed events outside its bounds. Visual hover, cursors, tooltips, and drag-over
feedback continue to use `is_hovered(window)`, which follows the physical pointer and input
modality. Do not use physical hover to gate a captured mouse-up, and do not use the captured event
target to paint hover outside the hitbox.

## Window Overlay Runtime

Focus target identity is now owned by `FocusTargetId`. The duplicate `OverlayFocusTarget` type was
deleted without an alias. Update explicit overlay focus policies directly:

```rust
use open_gpui_ui_core::{FocusTargetId, InitialFocusIntent};

let initial_focus = InitialFocusIntent::TargetOrFirstFocusable(
    FocusTargetId::new("dialog.primary-action"),
);
```

An explicit target ID now names a real handle registered by the component adapter. Dialog, Sheet,
and Popover expose the same declaration API:

```rust
use open_gpui_ui_components::dialog::Dialog;
use open_gpui_ui_components::gpui_adapter::FocusTargetRegistration;

let dialog = Dialog::element("confirm", "Open", "Confirm", content)
    .initial_focus_intent(initial_focus)
    .focus_target(FocusTargetRegistration::new(
        "dialog.primary-action",
        &primary_action_focus,
    ));
```

The ID passed to `InitialFocusIntent` and `FocusTargetRegistration` must match exactly and is local
to that overlay layer. Do not include a component instance or layer prefix. `WindowOverlayRuntime`
owns canonical window identity, live availability, rerender rebind, and stale-target removal.

### Canonical Overlay Presence

`OverlayPresence::from_parts(open, present, interactive)` now returns `Option<OverlayPresence>` and
accepts only the three canonical lifecycle states:

- `Hidden`: `(false, false, false)`
- `Open`: `(true, true, true)`
- `Closing`: `(false, true, false)`

Every other flag combination returns `None`; callers must reject inconsistent lifecycle input
instead of relying on normalization. Prefer `OverlayPresence::{hidden, open, closing}` when the
semantic state is already known, and use `OverlayPresence::from_open` only for an instant open/hidden
surface with no exit-presence phase.

### Focus Restore Inputs

`FocusRestoreInput::ancestor_last_targets` was renamed to `ancestor_targets` without a compatibility
field. Update struct literals and helper parameters to the new name. The new name is intentional:
the nearest-first slice may contain both an ancestor scope's last live target and its surface
fallback, rather than exactly one last-live target per ancestor.

```rust
let input = FocusRestoreInput {
    newer_claim,
    saved_target,
    ancestor_targets: &ancestor_candidates,
    window_fallback,
    current_target,
};
```

GPUI elements that combine `track_focus` with `tab_stop` or `tab_index` now apply the element's
declared tab configuration to the explicit handle in either builder order. Code no longer needs to
preconfigure a separate cloned handle merely to enter the rendered tab order.

The preparatory `gpui_adapter::FocusScopeRuntime` constructor and its direct registration methods
were deleted from the public surface. There is no compatibility alias. Focus-scope construction is
now crate-private to the window-owned overlay authority.

Dialog, Popover, and Menu formed the U4A pilot. The completed fleet also includes ContextMenu,
Sheet, AlertDialog, HoverCard, Tooltip, Select, Combobox, and Command overlay mode. Every official
family now obtains the unique runtime for its GPUI window, registers stable layer and parent
identities, and lets that runtime arbitrate Escape, outside press, modal barriers, controlled close
intent, focus claims, and restoration. Applications using official components do not install or
forward an additional runtime.

Dialog's default outside policy changed from `OutsidePressPolicy::Consume`, which only blocked the
press, to `OutsidePressPolicy::DismissAndConsume`, which requests close and prevents underlay
activation. Code that deliberately needs a sticky Dialog must now set `Consume` or `Ignore`
explicitly. Popover keeps pass-through outside dismissal, but it restores its trigger only after
focus entered the Popover surface or a registered descendant. Closing a Popover that never owned
focus therefore preserves the application's newer focus claim.

Mounted Dialog, Popover, and Menu instances may now switch between controlled and uncontrolled
ownership without changing their element ID or forcing a remount. Adding or removing the `.open(...)`
builder on a later render atomically adopts that render's committed presence and clears any pending
intent from the previous ownership mode. In particular, switching away from a controlled
`CloseRequested` state does not leave a stale close request suppressing the new owner.

### Menu Logical Children

Use `Menu::overlay_child` when another overlay must remain a logical child of the Menu root. The
Menu installs a parent scope around each supplied element, so a deferred Dialog or other official
overlay registers with the Menu layer as its parent instead of becoming the Menu's sibling under an
outer Popover. This is the supported composition path for Popover -> Menu -> Dialog dismissal,
ancestor inside-region handling, subtree teardown, and LIFO focus restoration; do not reconstruct
component-generated layer IDs in application code.

### Menu Path-Key Encoding

All six public Menu path-key accessors now percent-encode each path segment before joining segments
with `/`:

- `MenuItemState::path_key`
- `MenuSelection::path_key`
- `MenuSubmenuNavigation::{open_path_key, focused_path_key}`
- `MenuState::{open_path_key, focused_path_key}`

Within a segment, `%` becomes `%25` and `/` becomes `%2F`, in that order. For example, the path for
values `parent/%` and `child/%` is now
`0:parent%2F%25/0:child%2F%25`. This is a breaking public output-format change: update persisted
keys, selector fixtures, caches, and equality assertions. Code that parses a compact key may split
on `/` only as the encoded segment boundary and must percent-decode each segment before treating it
as the prior unescaped path-segment representation. Prefer the slice-returning `path`, `open_path`,
and `focused_path` accessors when unencoded segment strings are required.

Custom GPUI overlay adapters that genuinely need the low-level boundary migrate to the window
runtime registration API:

```rust
use open_gpui_ui_components::gpui_adapter::{
    OverlayLayerRegistration, OverlayOwnership, WindowOverlayRuntime,
};

let runtime = WindowOverlayRuntime::for_window(window, cx);
let registration = OverlayLayerRegistration::new(
    "app.settings-dialog",
    policy,
    OverlayOwnership::Controlled,
);
let binding = runtime.register_layer_for_entity(registration, &owner, window, cx)?;
```

Build `registration` from a stable, instance-qualified layer ID, an `OverlayLayerPolicy`, and
`OverlayOwnership`. Declare parentage on `OverlayLayerRegistration`, retain or entity-bind the
returned `OverlayLayerBinding`, wrap rendered overlay content with `WindowOverlayRuntime::surface`,
and use the binding's runtime-owned trigger/surface handles or explicit focus-target registration.
Rebind the same lease when policy or committed presence changes. Do not recreate a runtime to model
mount, unmount, or rerender; repeated `for_window` calls return handles to the same window state.

The runtime validates both logical ownership and the current GPUI render tree. A named target that
is outside its declared scope, unavailable, stale, unmounted, or owned by an inactive nested scope
is ignored. Initial focus requested before conditional content mounts is retried after the next
completed frame. Use `rebind_layer` and `rebind_focus_target` when stable identities gain current
policy or handles, and use the matching unregister methods only for explicit manual lifetime. The
preferred `register_layer_for_entity` path binds subtree cleanup to owner release.

Official component adapters also handle a new entity mounting with the same stable component ID
before the old owner's deferred release cleanup has settled. When the old owner is gone or stale for
the current frame, the runtime cancels the old subtree's pending restore, tears down its leases and
focus registrations without restoring through stale state, and atomically installs the replacement
binding. The replacement receives a new lease token, so callbacks, geometry journals, and focus
work captured by the old incarnation cannot affect it. This replacement behavior belongs to the
component binding path; low-level manual registrations still report duplicate live IDs.

Target IDs are canonical within one window. Component instances must qualify their IDs so two live
instances do not collide, and one live handle cannot be registered under multiple aliases. Modal
containment intercepts only plain Tab and Shift-Tab; Tab chords with Control, Alt, platform, or
function modifiers continue through normal dispatch. If initial-focus and restoration work become
pending in the same turn, the newest valid initial-focus claim wins so reopening cannot be undone by
an older close. Initial and restoration targets are resolved after the state transition reaches a
completed rendered frame, so a target hidden in the same transaction is treated as unmounted even
when another owner still holds its handle.

The old stack-only `FocusRestoreResolution` and per-layer trigger target bookkeeping were removed.
They could select an unmounted trigger without consulting live registrations. The renderer-neutral
resolver is now exported as `resolve_focus_scope_restore`; the window runtime supplies its
live-handle validation and applies the returned resolution rather than resolving a focus target
from an `OverlayLayer` snapshot.

Controlled close callbacks are intent notifications. Keeping the controlled value open keeps the
layer registered, modal, and focused; accepting the request requires the owner to commit closed
presence on a later render. Do not restore focus or remove a barrier from the callback itself.
Uncontrolled components commit framework state before notifying observers. In either mode, a newer
focus claim made by a callback wins over an older deferred restore.

`Sheet::on_close` was removed without a compatibility alias. Replace it with
`Sheet::on_open_change`, whose callback receives an `OverlayOpenIntent`; run close-request behavior
only when `intent.desired_open()` is false. The typed callback also identifies the dismissal reason
and, for controlled state, represents an intent rather than proof that the owner committed closed
presence.

The crate-private `OverlayLayerHost`, `OverlayOpenRuntimeRequest`, and their forwarding helpers were
deleted after the final fleet caller moved to `WindowOverlayRuntime`. There is no compatibility
facade. Official adapters no longer own parallel Escape, outside-press, initial-focus, or
focus-restoration tails.

HoverCard and Tooltip retain component-owned delay/epoch policy but register every visible surface
with the window runtime. They never claim or restore focus. Outside press is transparent and maps to
`OutsidePressPolicy::Ignore`; it is not a HoverCard or Tooltip ownership event. HoverCard still
allows Escape dismissal, while Tooltip retains its descriptive default Escape policy. The removed
HoverCard `.outside_press_policy(...)`, `.initial_focus_intent(...)`, and
`.focus_restore_intent(...)` builders must not be replaced with local event handlers.

### DevTools Window Focus Projection

Construct the GPUI runtime focus projection from the rendered window instead of assembling focus
facts beside the runtime:

```rust
use open_gpui_devtools::gpui::GpuiRuntimeFocusSnapshot;

let focus = GpuiRuntimeFocusSnapshot::from_window(window_id, window, cx);
```

The snapshot now carries optional `focused_element_rendered`, opaque `focus_claim_revision`, and
opaque `rendered_frame_revision` facts. `focus_scope_count` and `focus_handle_count` are also
optional: a producer that cannot prove a fact must emit `None`, not a guessed false or zero. This
keeps older imported JSON distinguishable from a live negative observation. An inactive window may
retain a logical `FocusHandle`, but `from_window` reports no `focused_window_id` until that window
again owns keyboard focus. Downstream JSON consumers must accept null for every unavailable fact.
