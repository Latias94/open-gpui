# Open GPUI Component Contract

Official Open GPUI components use an adapter-first, productized GPUI shape. A component may render
with GPUI today, but its behavior and semantic state should stay renderer-neutral enough to test
without rewriting the public API. ADR 0008 treats the current UI crates as the active product
boundary; future headless extraction is historical boundary evidence, not the current roadmap or
the next implied refactor.

## Contract Tables

The component contract tables are the product authority for component metadata used by local tests.
Cargo remains the distribution authority for official implementations, and crate source remains the primary inspection surface for humans and AI agents.

Open GPUI does not ship a generated component registry manifest, scaffold recipe manifest, or registry JSON/schema artifact.
The removed hybrid registry layer duplicated typed source facts without proving enough value over direct source inspection and focused contract scans.

Use `crates/ui_components/src/component_contract/` to keep public component ownership, source homes, docs tokens, gallery status, and default export intent aligned.
Use `cargo run -p xtask -- scan-ui-contract` and focused `cargo nextest` gates to catch drift.

## Resolved State

Every component should expose a resolved state or descriptor type. The state type is the primary
unit for tests and documentation.

Resolved state should contain:

- semantic input state such as disabled, selected, checked, indeterminate, open, invalid, read-only,
  and required;
- shared action metadata such as label, renderer-neutral icon intent, resolved icon facts,
  shortcut, disabled reason, tooltip, and accessibility description when the component presents an
  app command or action;
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
- concrete color values produced from a render-time
  `open_gpui_ui_components::ThemeContext`, usually via `ThemeResolver::current(cx)`.

The adapter should read from the resolved state rather than duplicating semantic decisions in the
render body.

## Ecosystem Adapter Helpers

`open_gpui_ui_components` may expose adapter helpers that project headless ecosystem snapshots into
existing component state. These helpers are public because applications need stable bridge code,
but they are not standalone official components and should not move cache, form, or async task
ownership into the component crate.

The form adapter surface is `FormProjection`, `FormFieldConfig`, `FormFieldProjection`,
`form_text_value`, `form_number_value`, `form_checkbox_value`, and `form_select_value`.
`FormProjection` resolves form-level busy state and submit eligibility. The field projection
consumes `open_gpui_form::FieldSnapshot` and `open_gpui_form::FormStatus`, then resolves existing
`FieldState`, `TextInputState`, `TextareaState`, `NumberInputState`, and `CheckboxState` inputs.
Validation activity is busy but remains editable; submission is a separate disabling policy. The
Field, TextInput, Textarea, NumberInput, and Checkbox builders must preserve projected busy state
when they rematerialize their resolved state. The
owning form store, ticket generations, derived lifecycle, submission eligibility, and
redaction-aware `FormSnapshot` stay in `open-gpui-form`.

The resource adapter surface is `ResourceAdapterLabels`, `ResourceCollectionProjection`,
`ResourceMutationProjection`, and `resource_query_key_label`. It consumes
`open_gpui_resource::ResourceSnapshot` and `open_gpui_resource::MutationSnapshot`, then resolves
existing feedback, command, table/tree children-load, and virtualized-list status inputs. Fetchers,
retry timers, cancellation, mutations, cache invalidation, pagination, and redacted
`ResourceSnapshot` values stay in `open-gpui-resource`.

The Components gallery shows `FormProjection`, `FormFieldProjection`,
`ResourceCollectionProjection`, and `ResourceMutationProjection` as adapter-only rows in the
`ecosystem-adapters` section. That keeps
adoption visible while preserving the component catalog distinction between official rendered
components, renderer-neutral state contracts, adapter helpers, and internal anatomy.

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

GPUI placement adapters remain responsible for `deferred` and `anchored` rendering, hitboxes, and
AccessKit relationship wiring. `WindowOverlayRuntime` is the sole per-window authority for live
registration order, parentage, input subscriptions, controlled close intent, modal barriers, focus
handles, focus claims, and restoration. `open_gpui_ui_components::gpui_adapter` also provides the
narrow placement mapping layer: deferred priority, snap-to-window margin, GPUI anchor mapping, and
placement resolution. `open_gpui_ui_core::overlay` owns the
renderer-neutral policy resolvers such as `resolve_escape_key` and `resolve_outside_press`; the
window runtime consumes them for production arbitration rather than rebuilding stack ownership in
each component.
`open_gpui_ui_core::overlay::resolve_overlay_placement` is the shared anchored-placement solver for
explicit neutral placement inputs. It returns `OverlayPlacementResolution` with fit and trace
metadata, so point-anchored and render-plan overlays that provide anchor bounds, content size, and
safe bounds use one flip/shift policy. `open_gpui_ui_components::overlay` owns the GPUI host
mapping: `GpuiOverlayPlacement`, `gpui_relative_overlay_layer`, `gpui_positioned_overlay_layer`,
and `gpui_full_window_overlay_layer` convert neutral placement and layer policy into concrete
`anchored` / `deferred` / full-window elements. Trigger-anchored components that do not yet own
measured trigger/content bounds still delegate final live positioning to GPUI's anchored layer; the
neutral solver remains the testable policy boundary rather than a measured overlay runtime.

Interactive overlay adapters register stable logical layers and focus targets with
`WindowOverlayRuntime`. Resolved state declares `InitialFocusIntent` and `FocusRestoreIntent`; the
runtime realizes those intents from current rendered descendants and rejects stale handles.
Dialog, Sheet, and Popover callers bind an explicit target with
`focus_target(FocusTargetRegistration::new(id, &handle))`. The declared ID is local to that layer:
the runtime creates its canonical window identity, rebinds availability on rerender, and removes
registrations omitted by a later render. Callers and component families must not prefix IDs with a
layer or maintain a parallel live-target registry.
Component-local Escape, outside-press, initial-focus, and restoration handlers are not an extension
point.

`TooltipState` is the descriptive overlay component contract. It records content kind,
disabled/open state, hover/focus/manual open intent, placement preference, delay policy, resolved
metrics, token intents, and tooltip layer state. Hover/focus timing and anchored/deferred rendering
remain component adapter responsibilities, while every visible Tooltip registers a passive window
layer. Tooltip never claims or restores focus and does not participate in outside-press ownership.
Rich hover cards and action-bearing tooltip content should not reuse the descriptive tooltip
contract as-is.

`PopoverState` is the interactive non-modal overlay contract. It records controlled versus
uncontrolled open mode, default-open state, trigger expanded/selected intent, placement preference,
outside-press policy, initial focus intent, focus restore intent, resolved metrics, token intents,
and non-modal dismissible layer state. The GPUI adapter owns concrete trigger/content elements and
`deferred`/`anchored` rendering; its runtime binding owns outside/Escape arbitration, live focus
handles, parentage, and conditional restoration. Popover defaults to the non-modal overlay
initial-focus policy (`InitialFocusIntent::None`); callers must opt in when content should receive
focus and register any explicit target handle through `Popover::focus_target`. Nested official
overlay descendants inherit Popover parentage through the ambient runtime surface. Modal Popover
variants remain follow-up work.

`DialogState` is the modal overlay contract. It records controlled versus uncontrolled open
mode, default-open state, title and description metadata, Escape policy, outside-press policy,
initial focus intent, focus restore intent, resolved metrics, token intents, and modal layer state.
The GPUI adapter owns the concrete dialog surface and deferred rendering. Its window runtime binding
owns the barrier, controlled close intent, keyboard/outside arbitration, nested modal focus loop,
live focus handles, and restoration. Explicit caller targets use `Dialog::focus_target`. Alert
dialogs build on the same modal lifecycle rather than installing a second focus or dismissal path.

`AlertDialogState` is the action-critical modal derivative. It records required title and
description text, cancel and primary action metadata, destructive intent, action disabled state,
initial focus preference, Escape policy, outside-press policy, focus restore intent, token intents,
and modal layer state. Alert dialogs default to consuming outside press without dismissing so the
underlay stays inert and critical decisions require an explicit action. The primary destructive
action is represented as metadata, while the cancel action remains the default initial focus target.
The GPUI adapter owns concrete button and deferred surface rendering. The window runtime owns
keyboard/outside arbitration, modal focus, controlled close lifecycle, and restoration; action
callbacks enter the component owner without bypassing that lifecycle.

`SheetState` is the edge-attached overlay contract. It records controlled versus uncontrolled open
mode, default-open state, attached side, modal versus non-modal mode, close affordance visibility,
title and optional description metadata, Escape policy, outside-press policy, initial focus intent,
focus restore intent, resolved metrics, token intents, and layer state. Modal sheets block underlay
input and default to dismissing while consuming outside press. Non-modal sheets use the same
surface anatomy while mapping to the non-modal dismissible layer kind and defaulting to
dismiss-and-pass-through outside behavior without installing a blocking barrier. The GPUI adapter
owns edge positioning, concrete close controls, callbacks, and deferred rendering. The window
runtime owns the modal barrier when applicable, keyboard/outside arbitration, live focus handles,
and restoration. The built-in close affordance and caller declarations from `Sheet::focus_target`
share the same runtime-owned target set.

`HoverCardState` is the interactive hover/focus overlay contract. It records controlled versus
uncontrolled open mode, default-open state, hover/focus/manual open intent, placement preference,
open and close delay policy, resolved metrics, token intents, and non-modal layer state. Hover cards
are not descriptive tooltips: their surfaces may contain interactive content, but they never claim
or restore focus. Outside participation is fixed to transparent with
`OutsidePressPolicy::Ignore`, so an outside press continues to the underlay without closing the
card. The keyed component runtime owns delay and pointer/focus epochs; `WindowOverlayRuntime` owns
Escape dismissal and the visible layer lifecycle.

`MenuState` and `ContextMenuState` are the first menu overlay contracts. `MenuState` records
controlled versus uncontrolled open mode, action, checkbox, radio, separator, and submenu items,
caller-owned checked state, disabled item state and disabled reason, resolved action icon facts,
shortcut, tooltip, accessibility description, stable item paths, visible submenu rows, roving
focus, pure typeahead targets, keyboard submenu open/close targets, activation payloads with item
kind/path/checked-at-activation, local scrollability, Escape policy, outside-press policy,
placement preference, resolved metrics, token intents, and menu layer state. `ContextMenuState`
reuses the same item, submenu, typeahead, scrollability, action metadata, and roving focus model
while adding a point anchor and renderer-neutral placement input sized from the visible menu
surface. Keyboard and pointer activation both invoke item-level selection handlers before
component-level selection handlers. Hover-open submenu affordance is now implemented for menu
items, and submenu hover timers / close timing remain component policy in the GPUI adapter runtime.
`MenuSubmenuSurface` and
`MenuSafeHoverCorridor` provide the renderer-neutral placement and pointer-transition contract for
floating submenu panels, while the GPUI adapter renders those panels as deferred anchored layers
and keeps the branch content scrollable. Every root/submenu branch registers explicit parentage with
the window runtime, which owns topmost dismissal, branch focus targets, and LIFO restoration.
Menubars, application menu integration, global command dispatch, and native OS menu bridging remain
follow-up work.

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
intents, and the listbox content role. `SelectState::resolve` takes a `SelectStateRequest` so
callers group overlay policy, selection inputs, descriptors, and theme tokens explicitly. The GPUI
`Select` adapter owns trigger/content rendering, keyed runtime open/selected/active state,
callbacks, and deferred anchored rendering. Its window runtime binding owns outside/Escape
arbitration, the popup layer, trigger/surface focus handles, controlled refusal, and restoration.

`choice.rs` owns the shared stable-value seam for the choice family. It projects flat item
identity, normalizes query text, filters disabled or missing selected values, deduplicates
multi-select chips, resolves active descendants by active value, selected value, then first enabled
item, and provides the typeahead hook used by listbox-backed surfaces. `Listbox`, `Select`,
`Combobox`, and `Command` should agree on those base rules. Command ranking and match scoring are
an extension layered over the shared choice model; they must not become the base semantics for
`Select` or `Combobox`.

`ComboboxState` composes an editable text input, non-modal dismissible popup, scroll viewport
metadata, and nested `ListboxState`. It records controlled versus uncontrolled open mode,
default-open state, required/disabled metadata, query text, selected value and label, active option
value, filtered and total option counts, empty-state label, placement preference, outside-press
policy, initial focus intent, focus restoration intent, resolved metrics, token intents, and
editable-combobox/listbox roles. Filtering controls only the visible list: the selected value is
resolved from the unfiltered descriptors and is not cleared just because the current query hides
that option. `ComboboxState::resolve` takes a `ComboboxStateRequest` so query, selection, filtering
inputs, overlay policy, and theme tokens stay grouped at the module interface. The GPUI adapter
owns the `TextInputController`, keyed runtime query/open/selection state, callbacks, outside-press
and Escape policy inputs, deferred anchored rendering, and scroll handles. Its window runtime
binding owns popup arbitration and preserves the editor focus/active-descendant authority.

`CommandState` composes a search text input, ranked grouped command results, optional dialog
wrapper, loading metadata, selected chips, a virtualized result window, and nested `ListboxState`.
It records controlled versus uncontrolled open and query modes, default-open/default-query seed
state, single-select or multi-select behavior, selected and active command values, query text,
filtered and total command counts, standalone/grouped command anatomy, shortcut labels, disabled
command state and disabled reason, resolved action icon facts, tooltip, accessibility description,
deterministic match source/score metadata, app-owned index revision/mode metadata, empty-state
label, provider/shortcut status items, keyboard navigation behavior, Escape policy, focus
restoration intent, resolved metrics, token intents, non-modal inline overlay state, and modal
dialog overlay state when dialog presentation is enabled. `CommandNavigationBehavior` makes
the palette-specific keyboard layer explicit: Home/End target the first/last focusable command,
Up/Down loop by default but can be bounded with `loop_navigation(false)`, and Alt+Up/Alt+Down move
between rendered command groups.
`CommandState::resolve` takes a `CommandStateRequest` so query ownership, selection inputs,
`CommandStateDataSource`, overlay policy, and theme tokens stay grouped at the module interface
instead of leaking parallel resolver signatures.
`CommandStateDataSource` makes local descriptors and indexed snapshots mutually exclusive at the
public contract boundary.
`CommandIndexSnapshot` lets applications pass indexed, pre-ranked, or pre-filtered descriptor
snapshots with loading metadata, while keeping command discovery, global registries, keybinding
resolution, dispatch, enablement policy, and async task ownership outside `ui_components`. The GPUI
adapter owns the `TextInputController`, keyed runtime query/open/selection state, callbacks,
deferred dialog rendering, and scroll handles; its window runtime binding owns the dialog layer,
outside/Escape arbitration, modal focus handles, controlled refusal, and restoration. Inline mode
does not register an overlay. The renderer-neutral state owns ranking, selection projection,
snapshot metadata, and the virtualized result render plan.
The reusable command ecosystem boundary is documented in
[`docs/ui/command-ecosystem.md`](command-ecosystem.md): `open_gpui` owns action/keymap execution,
`open_gpui_command` owns command metadata, deterministic registry snapshots, scoped registration,
availability projection, neutral menu trees, usage history, and GPUI command-id dispatch adapters.
`open_gpui_ui_components` owns rendering only. `open_gpui_command::CommandDescriptor` is the shared
app-command metadata contract for component projection. It carries id, label, renderer-neutral
icon descriptor, group, keywords, shortcut, disabled state, optional disabled reason, tooltip,
accessibility description, caller-owned `when` metadata, and app-owned menu path without storing
callbacks, keybinding resolution, icon asset loading, or a global runtime singleton.
`open_gpui_ui_components::ActionDescriptor` and `ResolvedActionState` are the UI-side projection
bridge. Applications resolve icon descriptors into `ResolvedActionIcon` values and diagnostics;
components render those resolved facts and return selection or activation intent.
`CommandItem::from_command_descriptor`, `CommandIndexSnapshot::command_descriptor`,
`MenuItem::from_command_descriptor`, `ToolbarItem::from_resolved_action`,
`SidebarItem::from_resolved_action`, `Button::from_resolved_action`,
`IconButton::from_resolved_action`, and `ContextMenu`'s shared menu state consume the one-item
presentation fields so Command, Menu, ContextMenu, Toolbar, Sidebar, Button, and IconButton can
present the same metadata while applications remain the execution authority. `CommandMenuTree` is
the command-crate hierarchy projection for callers that want submenu trees from `menu_path`.

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
metrics, token intents, and `Role::Image`. `AvatarGroupState` tracks visible and hidden counts for
the overlap family, while `AvatarGroupCountState` resolves the overflow bubble. Async image loading
status, retry policy, cache state, and fallback delay timers remain caller-owned.

## Focus Rings

Interactive component state should expose `FocusRing` metadata instead of rendering focus by
changing border width. `FocusRing` keeps the focus color as a `ColorIntent`, records the paint
width as neutral `UiPx`, and documents that it does not change layout.

The GPUI adapter should apply the ring inside `focus_visible` using
`open_gpui_ui_components::gpui_adapter::focus_ring_shadow_with_theme` and the render-time
`ThemeContext`. This paints an outer box shadow, so keyboard focus visibility does not resize or
move the focused component. The helper is available only through
`open_gpui_ui_components::gpui_adapter` because its `BoxShadow` return type is renderer-specific.

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
`VirtualizedList::default_active_key`, `VirtualizedList::default_selected_key`,
`VirtualizedList::default_selected_keys`,
`Combobox::default_query`, `Command::default_query`, `Menu::default_focused_value`, and
`ContextMenu::default_focused_value`. Direct names such as `Sidebar::selected`,
`Listbox::selected`, `Select::selected`, `Combobox::selected`, and `Command::selected` remain
reserved for caller-owned render-frame inputs. `Switch::on_change`, `Toggle::on_change`, and
`TextInput::on_change` are scalar value-change callbacks. Bootstrap callback exceptions such as
`Button::on_click`, `AlertDialog::on_action`, `AlertDialog::on_cancel`, and
`Table::on_sort_requested` must stay explicit in the API inventory because they represent command
activation, modal action outcomes, or table sort requests rather than scalar value changes. Sheet
close requests are not an exception: `Sheet::on_open_change` receives the runtime-issued
`OverlayOpenIntent` for its close affordance, Escape, outside press, and programmatic requests.

Keep crate-root exports explicit. The crate-root default export surface lives in
`crates/ui_components/src/public_api/default.rs`; the smaller common application import surface
lives in `crates/ui_components/src/public_api/common.rs` and is re-exported by
`open_gpui_ui_components::prelude`. Do not use wildcard public re-exports in component crates
except for those curated public-api hops. GPUI-specific helpers that remain public for concrete
applications must be reachable through `open_gpui_ui_components::gpui_adapter`; current examples
include `TextInputController`, `init_text_input`, `focus_ring_shadow_with_theme`,
`UiA11yElementExt`, accessibility mapping helpers, geometry conversion helpers,
`VirtualizedListGpuiExt`, and GPUI overlay scheduling
helpers. The crate root is reserved for official components and component-facing state/readout
types; the prelude is reserved for common application imports. Advanced command registry/runtime
types live in
`open_gpui_command`; table-core, virtualizer, and grid-window contracts live in
`open_gpui_ui_core`; advanced theme registry/runtime and JSON loader APIs live under
`open_gpui_ui_components::theme`. Component examples may consume those owner-crate APIs
explicitly, but `open_gpui_ui_components` should not re-export them as broad default-surface
conveniences. This rule narrows the default and common surfaces; it does not ban narrow
component-module re-exports of component-facing neutral dependencies, and it does not expand the
small prelude-only convenience allowlist in `prelude.rs` without an explicit public-surface test
update.

The current foundation refactor makes these names shipped high-value component families:
`Accordion`, `Collapsible`, `Slider`, `NumberInput`, `ToggleGroup`, `Link`, `Breadcrumb`, `Tag`,
and `ToastStack`. They deliberately choose one canonical API per family: ToggleGroup instead of a
parallel ButtonGroup, Tag instead of a separate Chip, ToastStack instead of a separate Notification
surface, and NumberInput instead of a separate Stepper. Aliases may remain as narrow type aliases
only when they preserve semantic vocabulary without creating a second component contract.

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

`open_gpui_ui_components::component_contract` owns the current product contract table;
its source home is the `crates/ui_components/src/component_contract/` module family.
`component_contract/mod.rs` is only the facade; `component_contract/types.rs` owns row types,
`component_contract/rows.rs` is the row facade, `component_contract/rows/catalog.rs` owns canonical
contract rows, `component_contract/projections.rs` owns the primary `component_contract_entry`
lookup plus derived default-surface, gallery, official-component, overlay, and recipe row queries,
`component_contract/source_mapping.rs` owns source-owner projections,
`component_contract/surfaces.rs` owns adjacent public-surface rows, and
`component_contract/api_inventory.rs` owns public API inventory and method baselines. That split
keeps marker lists derived from the canonical rows instead of preserving a second fact source.
That contract table classifies official components, official recipes, renderer-neutral state contracts,
GPUI adapter helpers, public anatomy, diagnostics, and removed compatibility targets. It also
records API inventory rows, source homes, docs tokens, gallery status, and default-export intent.
`examples/ui-foundation-gallery::pages::components::catalog::COMPONENT_CATALOG` is a gallery view
model over that table. The Components page re-exports that catalog through
`examples/ui-foundation-gallery::pages::components::COMPONENT_CATALOG` so tests and rendering keep
their stable consumer path, but official status and family grouping are derived from the component
crate contract rows. Entries with contract status `official` satisfy the checklist above. Entries with
contract status `adapter-only` are public GPUI helper surfaces such as `TextInputController`, not
standalone components. Entries with contract status `internal-anatomy` are public parts of a
component family, such as `ToolbarItem`, `SidebarItem`, and `ListboxOption`, and should not be
promoted to standalone components without a new resolved-state contract. Entries with contract
status `state-contract` are public renderer-neutral contracts with gallery readouts and signal
coverage, but they are not themselves rendered GPUI components. They may sit beside an official
adapter, as `TreeState` does for `Tree`. They must use `state_contract_selector`, not the official
`sample_selector`, and they must not satisfy the official rendered-component gate by accident.
Entries with contract status `deferred` are planned components that must not be treated as shipped
API until they satisfy the checklist and gain a contract row.

Public surface ownership uses a stricter source-facing vocabulary than the visible gallery status.
`official component` names are rendered GPUI components with inventory rows and sample selectors.
`renderer-neutral state contract` names are public state/resolution models that can be consumed
without GPUI runtime types. `gpui adapter helper` names are public only through
`open_gpui_ui_components::gpui_adapter` or narrowly scoped adapter modules. `TableBehaviorSnapshot`
is the public Table behavior readout: it exposes user-observable state without leaking render-plan
geometry, virtualizer internals, or adapter row layout. Crate-private render plans still explain
resolved geometry and row/window projection for the adapter, but they are not component facades or
the default application state API. `internal implementation detail` names are public anatomy owned by one official family.
`deprecated removal target` names are shallow compatibility surfaces scheduled for deletion or
already deleted. The removed targets are the former pass-through
`ui_components::primitives` modules for active-descendant, collection metadata, controllable state,
and overlay policy aliases. Import those neutral contracts from `open_gpui_ui_core`; remaining
`ui_components::primitives` modules must own GPUI adapter behavior or component-family anatomy.

Breaking migration notes for the 0.3 UI deepening pass:

- Replace old table render-plan probes with
  `Table::behavior_snapshot(scroll_offset, viewport_extent)`. Application tests, gallery readouts,
  and integration probes should consume `TableBehaviorSnapshot`, `TableRowBehaviorSnapshot`,
  `TableColumnBehaviorSnapshot`, `TableColumnRegionSnapshot`, `TableHeaderSummarySnapshot`,
  `TableTreeSummarySnapshot`, and `TableVisibleRowsSnapshot`.
- Remove imports of table render-plan internals from application code, including
  the internal `TableRenderPlan`, `TableColumnRenderPlan`, `TableCellRenderPlan`,
  `TableCenterColumnWindowPlan`, `TableColumnRegionRenderPlan`, header render-plan types,
  `TablePinnedLayoutPlan`, and `TableRowRenderPlan`. Those structures are private to the table
  adapter implementation; algorithm coverage belongs in `crates/ui_components/src/table` module
  tests.
- Replace removed primitive pass-through imports under `open_gpui_ui_components::primitives` with
  their renderer-neutral owners in `open_gpui_ui_core` or with the official component/adapter API
  that owns the GPUI runtime behavior.
- Official overlay adapters register stable layers with `WindowOverlayRuntime`; owner commit,
  controlled intent, closing presence, callback dispatch, and focus arbitration pass through that
  binding. The former `OverlayOpenRuntimeRequest`, close-tail helpers, and shallow
  `OverlayLayerHost` were deleted without compatibility aliases. Application code should keep using
  component builders and renderer-neutral overlay policy types.
- `Select`, `Combobox`, and dialog `Command` register their popup/dialog layer with the window
  runtime. Selection, Escape dismissal, and outside-press dismissal restore focus only according to
  the registered runtime condition. Tests that previously assumed focus remained inside an
  unmounted popup should instead assert focus on the trigger/input row or opt out through the
  component's supported focus policy.
- Keep reference repositories as references only. This pass does not add dependencies on
  `repo-ref/fret` or `repo-ref/gpui-component`, and it does not preserve compatibility shims around
  APIs that were only exposing old implementation structure.

The foundation component families above are official rendered components. Their resolved states
must stay aligned with the same ownership vocabulary as the older components:

- `AccordionState` and `CollapsibleState` own disclosure semantics, stable item values, disabled
  rows, and controlled/default open state without storing callbacks or GPUI handles.
- `SliderState` and `NumberInputState` own clamped numeric value, min/max/step metadata, disabled
  and read-only or invalid state, and keyboard/step payload shape while keeping formatting and app
  persistence caller-owned.
- `ToggleGroupState` owns single or multiple selection over stable item values, disabled-item
  skipping, optional selection-required policy, and roving-focus targets.
- `LinkState` and `BreadcrumbState` own accessible navigation text, disabled/current state, stable
  activation payloads, and renderer-neutral roles.
- `TagState` owns display variant, removable metadata, disabled remove affordance, and badge-family
  color/metric vocabulary.
- `ToastStackState` owns stack ordering, visible overflow, timeout pruning, dismiss reasons, and
  action metadata; timers and notification delivery remain application-owned.

## Theme Resolution

Component state should expose `ColorIntent` values rather than concrete GPUI colors. A color intent
keeps the semantic `TokenKey`, `ColorState`, and fallback RGB visible for tests, documentation, and
future adapter work.

The GPUI adapter should resolve intents through `ThemeResolver::current(cx)` immediately before
calling style APIs such as `bg`, `border_color`, and `text_color`. The returned `ThemeContext`
owns the render-time snapshot, so adapters and their helper functions should pass it explicitly or
pre-resolve concrete colors before storing style/event closures. `ThemeResolver::resolve` remains
a legacy default-light compatibility path for tests or compatibility shims only; production render
paths should not call it directly. Code with an immutable snapshot can call
`ThemeResolver::resolve_with(intent, snapshot)` so `(TokenKey, ColorState)` lookups come from that
snapshot before falling back to the intent RGB.

`ThemeSnapshot` is an immutable table view with a `ThemeMode`, `revision`, and color entries. The
revision is the cache invalidation hook for future app-level theme providers. Components should not
read global theme state directly; keep the resolved component state renderer-neutral and pass theme
snapshots at the adapter edge.

`ThemeColor` entries pair semantic tokens with `ColorState` values.

`ThemeRegistry` is the app-level owner for built-in and user-loaded theme snapshots. The registry
preloads light, dark, and high-contrast entries, validates `ThemeDefinition` identity fields,
replaces entries by stable id, and stores owned color tables behind `ThemeRegistryEntry`.
`ThemeRegistrationDiagnostics`
records which built-in mode supplied fallback colors and how many entries were filled from that
fallback. Missing optional token/state colors are completed from a built-in snapshot; missing id,
label, mode, or revision fail validation with `ThemeValidationError`. `ThemeRuntime` is the GPUI
app-global owner for the active theme id plus registry, and render code consumes cloned
`ThemeContext` values from that runtime. Consumers can still choose an entry, take its immutable
`ThemeSnapshot`, and pass that snapshot to `ThemeResolver::resolve_with` when they need an explicit
non-runtime lookup.

Portable theme files are JSON and versioned by `THEME_JSON_SCHEMA_VERSION`. The public loader
surface is the explicit theme-owner API under `open_gpui_ui_components::theme`:
`theme_json_schema`, `theme_definition_from_json_str`, `theme_definition_from_json_file`,
`register_theme_json_str`, and `register_theme_json_file`.
The reviewable schema artifact for version 1 lives at
`docs/schemas/open-gpui-theme-v1.schema.json`; regenerate it with
`cargo run -p open-gpui-ui-components --example export_theme_schema --quiet` when
`theme_json_schema()` changes, then run `cargo run -p xtask -- scan-theme-schema`.
Those facades validate schema version, identity fields, `ThemeMode`, duplicate token/state pairs,
supported semantic token names, supported `ColorState` names, and six-digit RGB values before a
definition reaches `ThemeRegistry::register`. Loader failures are structured as `ThemeLoadError`
with `ThemeFileField` for missing top-level or per-color fields, so applications can show messages
without parsing error strings. The schema covers the current resolver vocabulary: semantic surface,
text, accent, destructive, overlay, modal overlay, and focus-ring tokens plus default, hover,
selected, disabled, read-only, invalid, required, placeholder, message, focus-visible, overlay,
and modal-overlay states. A pressed state is intentionally absent until the resolver grows a real
`ColorState` for it.

Schema vocabulary audit target:

- Top-level fields: `schema_version`, `id`, `label`, `mode`, `revision`, `fallback_mode`, `colors`.
- Color entry fields: `token`, `state`, `rgb`.
- Modes: `light`, `dark`, `high-contrast`.
- Tokens: `semantic.surface`, `semantic.surface_muted`, `semantic.border`, `semantic.text`,
  `semantic.text_muted`, `semantic.accent`, `semantic.accent_foreground`, `semantic.focus_ring`,
  `semantic.destructive`, `semantic.destructive_foreground`, `semantic.overlay`,
  `semantic.modal_overlay`.
- States: `default`, `hover`, `selected`, `disabled`, `read-only`, `invalid`, `required`,
  `placeholder`, `message`, `focus-visible`, `overlay`, `modal-overlay`.

Theme module ownership is intentionally split: `theme/snapshot.rs` owns immutable snapshot data,
`theme/registry.rs` owns explicit registration and fallback diagnostics, `theme/resolver.rs` owns
intent-to-color resolution, `theme/schema.rs` owns the JSON schema and loader facade,
`theme/palette.rs` owns the built-in token tables, and `theme/recipes.rs` owns component color
recipes. Component files should call
`ThemeResolver::*_colors` but should not add local `impl ThemeResolver` blocks. The
`scan-theme-drift` xtask gate checks recipe catalog coverage and built-in palette token/state
shape, so missing recipe or token additions fail before visual drift reaches gallery tests.
Table composition recipes follow the same rule: `TableToolbarState` exposes `TableToolbarColors`
from the shared theme recipe rather than hand-assembling toolbar text intents in the table module.

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
foundation size, disabled state, action/toggle/separator items, resolved action icon facts,
shortcut, disabled reason, tooltip, accessibility description, pressed toggle metadata, focused
item, tab stop, shared button metrics, and focus-ring/color intents. Separators are visual only and
must not participate in roving focus or activation.

The GPUI `Toolbar` adapter owns focus handles, keyboard/click dispatch, and concrete item rendering.
It should expose `Role::Toolbar`, `aria_orientation`, explicit item labels, button roles for action
and toggle items, and toggled metadata for pressed toggle items. It should reuse the shared
roving-focus helpers so arrow keys, Home, and End skip disabled items and separators consistently
with Tabs, RadioGroup, and Menu.

Toolbar v1 is a primitive command surface, not an application command registry. Automatic overflow
menus, command enablement policies, persisted customization, and icon asset resolution remain
application responsibilities; toolbar items render resolved action facts supplied by the app.

## Sidebar Contract

`SidebarState` describes renderer-neutral shell navigation: side, variant, size, collapse mode,
effective collapsed state, accessible label, sections, flattened navigation items, disabled state,
resolved action icon facts, shortcut, disabled reason, tooltip, accessibility description,
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
Long Sidebar navigation uses the shared local scroll primitive so wheel input stays inside the
sidebar viewport instead of leaking to the outer Components page.

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
Content-fit width growth is also renderer-neutral: `TableColumn::with_content_fit` marks a column
as adapter-measured, `TableBehaviorSnapshot` exposes the resulting column widths as behavior metadata, and the
GPUI table adapter keeps header/body alignment stable while visible content changes. The Components
gallery surfaces this behavior through the focused `content-fit-release` sample.
`Table::row_measure_mode(TableRowMeasureMode::Measured)` is the sibling body-row height recipe:
the adapter measures rendered row heights, feeds them back into the row virtualizer cache, and
keeps wrapped body content from overlapping the following row. `Fixed` remains the default
constant-height contract.
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

Virtualized adapters share `open_gpui_ui_core::grid_viewport::RowWindow` and `RowWindowItem` around
`VirtualizerResolvedState`. That renderer-neutral seam maps the virtualizer's rendered
measurements into component row payloads, stable render keys, visible-row counts, overscan budget,
and scroll geometry without owning a concrete `ScrollHandle`. `VirtualizedList`, `Tree`, and the
`Table` center-row body use this shared projection for their visible/overscan windows, while
duplicate-key disambiguation, tree hierarchy, activation payloads, and pinned Table row bands
remain component-specific contracts.

The GPUI `Table` adapter resolves table state and virtualizer ranges before rendering. The adapter
owns the element tree, concrete scroll viewport, wheel containment, sticky header overlay,
body drawing, sortable header activation callbacks, row focus handles, source-tree disclosure
affordances for loaded, unloaded, loading, and failed branches, controlled row activation /
expansion-request payloads, callback-backed column resize handles, and AccessKit mapping. Table
accessibility metadata includes table, row, column-header, and cell roles, row and
column position metadata, sort metadata for sortable headers, grouped-row and source-tree depth /
parent metadata, selected state, and branch `aria-expanded` state keyed by stable row id. The
adapter keeps row activation independent from selection and expansion; callers decide whether a
click, double-click, Enter, Space, Left, or Right payload changes app-owned `TableState`.
`Table::state()` returns the renderer-neutral `TableState` input as the default application-facing
readout. `Table::behavior_snapshot(scroll_offset, viewport_extent)` is the explicit public behavior
surface for tests, gallery readouts, and adapter debugging; it is not a layout API. The snapshot
exposes row counts, visible and overscan row summaries, column-region counts and widths, header
summaries, row/cell behavior, selection, sorting, filtering, pagination, faceting, and accessibility
roles without exposing render-plan structures.
Table module ownership is deliberately split by responsibility rather than by product feature:
`open-gpui-ui-core::table` keeps renderer-neutral identity, rows, columns, headers, filtering,
faceting, aggregation, sizing, selection, and row-model resolution behind `table/mod.rs` re-exports.
The GPUI adapter keeps `crates/ui_components/src/table/mod.rs` as the public `Table` facade and
builder, while `resolve.rs`, `runtime.rs`, `render_plan.rs`, `layout.rs`, `virtualization.rs`,
`content_fit.rs`, `header.rs`, `body.rs`, `cell.rs`, `editors.rs`, `resize.rs`, `interaction.rs`,
filter recipes, `column_visibility.rs`, `toolbar.rs`, and `metrics.rs` own the concrete maintenance
surfaces. This ownership note is about review locality only; it does not add a second public Table
contract or promise behavior beyond the exported `TableState`, `TableBehaviorSnapshot`, `Table`,
filter recipes, callback payloads, and stable gallery/debug-selector proofs.
`TableBehaviorSnapshot` keeps the public readout in `table/behavior/mod.rs`, while row counts and
visible windows live in `table/behavior/counts.rs`, column regions and column snapshots in
`table/behavior/columns.rs`, nested header summaries in `table/behavior/header.rs`, tree summaries
in `table/behavior/tree.rs`, and rendered row/cell snapshots in `table/behavior/rows.rs`. This
mirrors the Command split pattern without reopening the Table renderer.
Crate-private region render plans expose summed widths to the adapter, and header/body cells read
the same resolved column widths. For pinned tables, a crate-private center-column window virtualizes
the shared horizontal center lane from adapter-owned horizontal scroll input. The adapter keeps
left/right pinned lanes fully mounted while mounting only the rendered center-column window, so the
center can scroll without moving pinned columns or the outer page. The header band is rendered as an
absolute overlay at the top of the table root, and the body receives matching top padding so vertical
scroll does not move the header. `TableBehaviorSnapshot` exposes the public behavior counterpart:
current filtering, sorting, pagination, and faceting ownership modes, pagination row/page totals,
column-region summaries, visible row windows, and per-column facet metadata so gallery readouts and
consumers can distinguish local row-model transforms from app-owned server snapshots. Facet metadata covers deterministic unique value/count
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
`TableColumnOrderChange` is the sibling official column-order recipe: it emits controlled
before/after placement payloads through `Table::on_column_order_change`, applies only the caller-
owned column-order slice, and keeps sorting, filtering, pinning, sizing, and row-model state
untouched. The Components gallery uses `release-rollup` as the proof sample for that controlled
reorder path.
Nested header groups are resolved as renderer-neutral row families rather than data columns.
`TableBehaviorSnapshot` exposes nested-header behavior as a header-row count and visible group-header
summary, while crate-private render plans keep the left, center, and right header-row layout details
internal. Pinned regions split group families when visibility or pinning crosses region boundaries,
while group headers continue to stay leaf-column-driven for sort, resize, visibility, and selection
behavior. Flat tables still resolve to a single header row.
Value cell editing is the official inline-edit recipe over table column metadata:
`TableCellEditor::Text`, `TableCellEditor::MultilineText { rows }`,
`TableCellEditor::Checkbox`, and `TableCellEditor::Select` opt columns into editable leaf cells,
while synthetic group rows and missing source cells stay display-only. The GPUI adapter renders
controlled `TextInput`, `Textarea`, `Checkbox`, or fixed-option `Select` paths for those editors,
then emits the same `TableCellEditChange` through `Table::on_cell_edit_change`; applications keep
row data app-owned and feed back a changed `TableState`. The helper `TableCellEditChange::apply_to`
updates the matching stable source row id while preserving unrelated row-model inputs such as
sorting, filters, pagination, selection, pinning, expansion, faceting, and sizing. Dynamic
row-height measurement, validation, dirty-state tracking, commit/cancel workflows, clipboard range
editing, and server persistence remain application-owned or follow-up work.
The fixed-option select path uses the same leaf-cell contract as text and checkbox editing: it is
adapter-owned, keeps row activation suppressed when the editor consumes the click, and preserves
the stable `(row_id, column_id)` edit payload shape.
For row-pinned tables, `TableBehaviorSnapshot` exposes top, center, and bottom row counts plus
rendered rows with neutral `TableRowRegion` metadata, while the vertical virtualizer consumes only
the center region. The GPUI adapter renders top and bottom row bands outside the center body
`ScrollArea`, keeps `table:{id}:body:{top|center|bottom}` debug selectors stable, and reuses the
normal row renderer so focus, activation, expansion, pinned-column lanes, and accessibility row
indexes keep the same payload shape across pinned and center rows. Combined two-axis viewport
details remain adapter-internal; public tests assert the resulting row/column behavior through
`TableBehaviorSnapshot` and gallery runtime probes instead of inspecting the render plan.

An official Table entry must satisfy the normal component completion gate: `Table` and
`TableBehaviorSnapshot` export at the crate root and common prelude; component-owned table controls
such as `TableGlobalFilter` and `TableToolbar` export at the crate root and owner modules, while
`TableState`, `TableRow`, `TableColumn`, row-model, virtualizer, and resize math import from
`open_gpui_ui_core`. The entry also needs matching `SIGNALS` entries, a `COMPONENT_CATALOG`
official entry, at least one `gallery:component-table-sample:{id}` rendered selector, state tests
for row identity, grouping, source-tree expansion, row interaction payloads, and virtualizer
behavior, and gallery runtime tests for nested scroll containment, faceted-filter row updates,
predicate-filter row updates, single-line, multiline, and checkbox value-cell updates, and nested
header gallery proof.
Dataset-wide exact autosizing, data-source fetch/cache orchestration, global faceting, dynamic
editor row measurement, and deeper two-axis grid virtualization beyond the pinned center-column
window remain follow-up capabilities.

## Splitter Constraints

`SplitterState` describes renderer-neutral resize constraints: stable group id, orientation, panel
fractions, per-panel min/max bounds, collapsible/collapsed metadata, handle adjacency, disabled
state, and handle metrics. The state owns the constraint solver for normalizing fractions and
clamping handle deltas; tests should exercise those rules without a GPUI window.

`SplitterLayoutScene` is the shared resolved-geometry contract for split views. It turns a
`SplitterState` plus bounds and metrics into panel rectangles and handle rectangles that callers can
feed to renderers, overlays, hit maps, accessibility descriptors, or motion plans. `SplitterHitMap`
consumes those resolved handle rectangles and owns handle/junction precedence; component and
docking adapters should not carry their own handle hit solvers once the scene is available.
The core split module also owns renderer-neutral transition diff descriptors for comparing two
resolved scenes and sampling them into projection clips. A sampled split transition keeps panel
content at final semantic bounds while `MotionProjectionClip` describes the visible viewport for
entering, leaving, moving, resizing, collapsing, and expanding panels. The GPUI `Splitter` adapter
now consumes those samples for programmatic identity, count, collapse, and expand changes through an
absolute transition overlay while the semantic flex layout snaps to the final state. Plain
element-backed panels remain one-shot GPUI elements; `SplitterPanel::view` is the retained-content
path for panels that must render real leaving content after the caller removes them.

The GPUI `Splitter` adapter renders resolved panel fractions and resize handles from that state and
wires pointer dragging through keyed runtime state. Drag move events use the root splitter bounds to
translate pixels into fraction deltas, then feed those deltas through `SplitterState::resized_by`.
For programmatic changes that keep the same ordered panel ids, the adapter animates from the current
runtime fractions to the new resolved fractions with committed layout motion; pointer dragging stays
immediate and cancels any in-flight programmatic transition. For structural programmatic changes,
the adapter samples `SplitterLayoutTransition` through the same committed-layout model and renders
final-size panel content through overlay clips. `Splitter::motion_preference` controls both
programmatic committed-layout paths, and reduced motion must complete at the final state without
scheduling a transition.
The public motion contract for these paths is `MotionScalarTrack` / `MotionScalarController`,
`MotionFrameDemand`, `MotionModel`, `MotionPreset`, motion policy validation, and
`MotionProjectionClip`; the lower-level scalar value state remains private inside `ui_core`.
Dragging a collapsible panel past its restore threshold clears its collapsed state and resumes
normal min/max resizing; dragging below that threshold keeps the collapsed fraction stable.
The adapter may use GPUI layout primitives, cursor styles, drag callbacks, and `Entity` runtime
state, but it should not invent sizing rules in the render body. Keyboard splitter resizing,
controlled resize callbacks, application-level layout persistence, RTL behavior, and nested
splitter arbitration should build on `SplitterState::resized_by` instead of duplicating
min/max/collapse logic in adapter code.

Docking consumes the same split primitives through its presentation scene. `DockGraph` continues to
own docking semantics and graph mutation validation, while `DockPresentationScene` resolves split
panes and splitters through core splitter scenes. Divider and corner hit maps consume those
resolved splitter rectangles through `SplitterHitMap`, then commit graph changes through
transactional resize actions. The remaining docking-local split layout helper exists only to derive
graph child pane bounds; old handle-center and handle-hit geometry belongs in the core hit-map path.
Docking-local corner affordance states should only represent states the runtime actually produces;
clamp and rejected-resize behavior remains transaction/test evidence until a user-visible state is
wired to the interaction model.

Renderer-neutral accessibility also follows this boundary. `open_gpui_ui_core::Role::Splitter`,
orientation, selected state, disabled state, and action descriptors are the stable vocabulary;
the component crate maps that vocabulary to GPUI roles and ARIA-style element state through
`open_gpui_ui_components::gpui_adapter`. Docking's accessibility scene should describe panes,
tabs, tab bars, splitters, drop targets, drag sources, and overlay state from the same presentation
scene rather than from render-local rectangles.

Component accessibility assertions use `ComponentA11yContract` rather than a live platform
accessibility backend. The contract records `A11yLabelSource`, `A11yDescriptionSource`, selected,
checked, expanded, disabled, `A11yValueMetadata`, `A11yValueKind`, `A11yStateEvidence`,
orientation, and supported `AccessibleAction` values for a component or component part. Validation returns
`A11yContractViolation` with `A11yContractError` when a role that requires an accessible name,
value metadata, or action omits that fact. Focused tests in `crates/ui_components/tests/a11y.rs`
cover representative primitives, form controls, icon-only controls, overlays, listbox choices,
Table, Tree, VirtualizedList, and Splitter handles. GPUI adapter mapping tests remain separate, so
the contract does not claim full platform screen-reader coverage.

## Gallery Conformance Surface

`examples/ui-foundation-gallery` is the durable conformance surface for official UI components. It
consumes `open_gpui_ui_components::component_contract` for shipped status, family grouping, and
public-contract evidence, then adds gallery-owned sample selectors, focused-section ids, runtime
probes, and rendered dogfood. It should expose stable sample ids, real resolved state, and a short
gate list that names the regression-prone behaviors each slice must keep covered.

The Components page should keep the contract-backed component catalog visible and distinguish
shipped components from adapter-only helpers, internal anatomy, state contracts, and deferred
entries. Its root module is a small facade: catalog view-model metadata lives in
`components/catalog.rs`; the visible conformance gate list lives in `components/conformance.rs`;
`components/samples.rs`,
`components/runtime.rs`, and `components/render.rs` are private parent facades over explicit
family-owned modules. Sample descriptors and static sample data live under
`components/samples/`; Tree, Table, and VirtualizedList runtime probes live under
`components/runtime/`; page orchestration, section dispatch, readouts, focus controls, and shared
card helpers live under `components/render/`. `components.rs` must not expose `runtime` or
`samples` as public modules, and it must not use wildcard facade exports such as
`pub use runtime::*` or `pub use samples::*`; only stable sample accessors, conformance metadata,
and runtime probe names are re-exported explicitly. The page has two supported inspection modes:
the full all-components conformance page, and a focused component-family view entered from
official catalog cards. Focused
mode may hide unrelated sections, but it must keep the section directory available, expose an
explicit `All components` control, reset the page viewport when the family changes, and keep nested
sample scrolling local to the sample viewport. Directory chips remain anchor jumps inside the
current page mode; they must not implicitly change the focused family. The page should also keep
these gates visible:

- crate-root, common, and prelude exports stay explicit;
- contract-row default-export intent stays aligned through
  `root_and_prelude_exports_match_contract_default_surface_intent`;
- contract-row source ownership stays aligned through
  `command_component_source_mapping_tracks_split_owners`;
- gallery catalog status and family grouping stay contract-owned through
  `components_catalog_consumes_component_contract_rows`;
- adapter-only helper exports stay grouped under `open_gpui_ui_components::gpui_adapter`;
- every official catalog entry keeps matching component/state signals and a rendered sample
  selector;
- every official overlay entry keeps matching catalog metadata, component/state signals, rendered
  sample selectors, and visible catalog cards on the Overlay page;
- every official component, state readout, and overlay sample has a `StoryContract` that declares
  the selectors and runtime probes tests may use for open, dismiss, select, edit, scroll, focus,
  activate, and public-payload assertions;
- `component_story_contract_for(name)` and `component_story_contracts_for_focus(mode)` are the
  gallery-side authority for focused-section ids, sample selector pairs, state readout selector
  pairs, and focusable catalog traversal;
- official Listbox, Select, Combobox, and Command stories expose state readout selector pairs so
  search/choice payloads can be asserted without inspecting renderer internals;
- gallery samples continue to show real resolved state for each shipped component;
- all-components and focused component-family modes preserve the catalog, section directory, page
  scroll reset, and nested scroll containment contracts;
- the gallery navigation rail and page viewport stay independently scrollable on compact windows;
- ScrollArea redraws preserve the default keyed runtime handle;
- Table and virtualizer samples keep long table scrolling inside the table viewport;
- Splitter runtime fractions continue to share one constraint solver;
- Tabs keep overflow and roving-focus behavior visible in the page;
- icon-only affordances and labels keep their accessible metadata explicit;
- final `TreeUpdate` and real AccessKit action tests own evidence for migrated semantic producers;
  `COMPONENT_A11Y_EVIDENCE` and gallery `COMPONENT_A11Y_CLAIMS` are transitional bindings only for
  producer families that have not yet moved to the unified projection;
- `cargo run -p xtask -- scan-ui-contract` keeps contract rows, default exports, docs tokens,
  conformance evidence, a11y claims, and the theme schema artifact aligned before gallery smoke
  tests are needed.

Large or behavior-heavy sections must use the same lazy or virtualized rendering primitives that
the component library exposes to applications. The Components page mounts sections through a
`ListState`-backed page list and keeps heavyweight families such as Tabs, Table, VirtualizedList,
Signals, and the foundation component samples behind stable section ids. A focused catalog entry
must render the target family and its state readouts without forcing unrelated heavy sections to
mount, while the all-components page still remains a complete conformance surface.

## Headless Readiness Checkpoint

ADR 0008 makes current-crate productization the active roadmap. The boundary rules below remain
useful hygiene for tests and future adapter work, but they are not a directive to create
`open-gpui-ui-headless` in the current branch.

The 2026-07-01 productization follow-up keeps both standalone headless extraction and broad
remaining-1k-line component splitting out of scope. Split a large component file only when a
specific contract, runtime, accessibility, or theme ownership problem requires it; file size alone
is not the roadmap driver.

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
- `TextInputController`, externally supplied `ScrollHandle`, `focus_ring_shadow_with_theme`, and
  GPUI overlay scheduling helpers are adapter-only public surfaces. They are intentionally grouped under
  `open_gpui_ui_components::gpui_adapter`; a future headless crate needs smaller neutral models or
  an explicit rule that these capabilities remain framework-specific.

## Current Known Gaps

The current deep-module productization slice is
`docs/plans/2026-07-02-003-refactor-ui-framework-deep-modules-plan.md`: runtime theme context,
typed a11y evidence, removed registry history, shared overlay placement, shared row-window
projection, gallery story contracts, and app-command descriptor projection are now the active
architecture boundary. Broad remaining-1k-line component splitting and `open-gpui-ui-headless`
extraction are not part of that slice. The runtime theme table now covers semantic component
colors for light, dark, high-contrast, registry-loaded snapshots, and JSON-loaded
`ThemeDefinition` values; UIs should load or construct definitions, register them, install or
select a `ThemeRuntime`, and let component adapters resolve colors from `ThemeResolver::current(cx)`.
Single-line editable text input now uses GPUI's `EntityInputHandler`/
`ElementInputHandler` path through `TextInputController`. Applications can either supply an
adapter-owned controller directly or use the standard controlled shape
`TextInput::value(...).on_change(...)`; the latter creates a keyed adapter controller internally,
emits sanitized single-line values, and expects callers to feed the accepted value back through
`value` on the next render. `TextInputDisplayMode` now distinguishes plain display from password
display: password mode masks one glyph per stored grapheme for render, caret, selection, hit
testing, and IME geometry while keeping the stored value and `on_change` payload unchanged.
`Textarea` is a separate controlled multiline form editor rather than a `TextInput` mode. It
preserves `\n` values and callback payloads, exposes renderer-neutral `TextareaState` rows,
min-height, placeholder, required, invalid, read-only, disabled, metrics, colors, and role
metadata, and keeps GPUI focus handles, input handlers, scroll handles, and callbacks inside the
adapter. Field composition can wrap either `TextInput` or `Textarea` without owning editor values.
`FormControlState` is the shared renderer-neutral control metadata for `Field`, `TextInput`,
`Textarea`, and `NumberInput`; it owns size, disabled, read-only, invalid, required,
controller-driven, editability, activation, and tab-stop facts so form controls do not duplicate
those rules. The old `open_gpui_ui_components::primitives::FieldState` helper is removed because it
was both shallower than the real field contract and name-confusable with `FieldState`.
Password reveal toggles, credential-manager affordances, textarea auto-grow/drag-resize, undo/redo,
completion, validation engines, rich text, and code-editor behavior remain out of scope. `Field`
still stays separate from the editing controller and remains composition-only.
`focus_ring_shadow_with_theme` is GPUI-adapter code and should stay out of a future headless crate
if `FocusRing` is extracted.
ADR 0008 keeps current-crate productization as the active roadmap. ADR 0006 keeps the strict
boundary checkpoint as future extraction evidence, not active work, and ADR 0007 records the
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
`TextInputController`, externally supplied `ScrollHandle`, `focus_ring_shadow_with_theme`, the
adapter geometry conversion helpers, and GPUI overlay scheduling
helpers. These public adapter APIs are now grouped under `open_gpui_ui_components::gpui_adapter`.
Shared roving-focus helpers now live behind the private `roving_focus` implementation module;
explicit low-level consumers use `open_gpui_ui_components::primitives::roving_focus_group`, while
`Tabs` preserves compatibility re-exports.
The choice family now also has a shared internal seam in `open_gpui_ui_components::choice` for flat
stable-value projection, enabled-item selected/active fallback, typeahead matching, multi-select
dedupe, and normalized query handling across `Listbox`, `Command`, `Combobox`, and `Select`.
`Select` and `Combobox` now use split module owners for model, style, render-plan, and runtime
responsibilities; existing popup rendering continues to call the overlay adapter boundary without
moving overlay ownership into this choice/search seam.
`open_gpui_ui_core` now owns `UiPx`, `UiPoint`, `UiSize`, `UiRect`, and `UiEdges`, and
`ContextMenuState` stores a neutral point anchor plus renderer-neutral `OverlayPlacementInput`.
`open_gpui_ui_core::overlay::resolve_overlay_placement` resolves side/alignment/fit/safe-bounds
behavior for explicit neutral inputs before the GPUI adapter maps it into `GpuiOverlayPlacement`.
GPUI layer mounting and any final live trigger/content measurement remain inside the
adapter/render boundary. Overlay stack Escape, outside-press, placement policy, and focus-restore
ordering now have window-free tests in `open_gpui_ui_core`.
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
display-only and exposes no role in resolved state. `Button` and `IconButton` can be constructed
from `ResolvedActionState` so primary actions, icon-only controls, toolbar entries, menu rows, and
navigation items share label, resolved icon, disabled reason, tooltip, and accessibility metadata.
`IconButton` reuses Button visual variants and focus-ring color intents, but requires an explicit
accessible label because the visible icon glyph is not a reliable accessible name. `Tooltip` is
descriptive-only and currently maps its surface to `Role::Label` until the public GPUI/AccessKit
role wrapper exposes a tooltip role; trigger association and timed hover/focus execution stay in
the adapter layer, while visible-layer ownership belongs to `WindowOverlayRuntime`. `Popover`
covers non-modal dismissible surfaces with default-open and controlled-open state; nested overlay
parentage, topmost dismissal, and focus restoration use the shared window runtime.
`HoverCard` covers interactive hover/focus/manual non-modal surfaces with delayed open/close,
transparent outside-press participation, and no focus authority; safe pointer corridors, arrows,
and text-selection leases remain deferred.
`ScrollArea` covers viewport overflow, axis metadata, scrollbar width metrics, and explicit
reset-on-key-change semantics. It intentionally does not yet expose custom scrollbar anatomy,
nested scroll arbitration, or Radix-style hover/auto scrollbar visibility.
`Tabs` keeps the roving-focus contract in resolved state, and the GPUI adapter routes vertical
tablists through the shared `ScrollArea` primitive so the rail owns its own viewport instead of
relying on ad hoc overflow handling.
`Table` covers stable row ids, row-model ordering, grouping, expansion, built-in group-row
aggregate cells, source-tree branches with manual expansion and child-load metadata, pinned
left/center/right column regions, runtime column visibility overrides, locked column hideability,
manual filtering/sorting/pagination modes with pagination totals, committed column sizing state,
clamped width resolution with region totals/offsets, row pinning with top/center/bottom regions,
sortable header action payloads, crate-root/prelude
exports, table/cell roles, and a vertically virtualized GPUI recipe whose body scroll stays inside
the table viewport.
For pinned samples, the adapter renders fixed left/right lanes plus a shared horizontal center lane
backed by crate-private center-column windowing, so off-window center headers and cells are unmounted while
spacer geometry preserves the full scrollable width. It also ships GPUI resize handles with
controlled commit callbacks and on-end/on-change resize mode support.
For row-pinned samples, top and bottom row bands render outside the center vertical scroll area,
and the center virtualizer counts only center rows.
`VirtualizedList`, `Tree`, and the Table center row body share
`open_gpui_ui_core::grid_viewport::RowWindow` for stable keys, measurements, visible counts,
overscan counts, and scroll offsets. The projection intentionally stops at the row-window
boundary: Table row regions, Tree selection/focus metadata, and VirtualizedList
activation/selection payloads stay owned by their components.
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
measurements but not captured scroll offsets. Sticky headers, dataset-wide exact autosizing, data-source
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
selection/focus/toggle actions. `TreeChildrenLoadState` lets callers mark branch children as
loaded, unloaded, loading, or failed without making `Tree` own asynchronous fetch work.
`TreeItemDescriptor`, `TreeItemState`, and `TreeToggle` expose loaded-child counts and child-load
metadata so applications can start fetches or retries from toggle payloads. Loading branches render
as branches but do not emit repeat toggle requests while the caller reports loading.
`TreeState::typeahead_target` provides renderer-neutral prefix matching over the current visible,
focusable row list; the GPUI adapter owns the printable-key buffer and reset timing, then moves
focus without selecting the matched row. Typeahead intentionally does not search collapsed,
unloaded, or virtualized descendants.
The private `roving_focus` implementation module now owns the shared vertical, paged, and
typeahead target helpers used by `Listbox`, `Tabs`, `RadioGroup`, `Menu`, `Sidebar`, `Toolbar`,
`Tree`, and `VirtualizedList`; public low-level consumers use
`primitives::roving_focus_group`, and the component-specific adapters keep only their own branch
and activation rules.
`VirtualizedList` is now an official rendered component. Its adapter keeps the render plan
crate-private, exposes `VirtualizedListBehaviorSnapshot` for diagnostics, owns a keyed GPUI
runtime plus persistent `ScrollHandle`, and keeps row rendering inside its viewport.
`VirtualizedListState` remains the renderer-neutral keyboard/navigation contract:
active/selected keys with index diagnostics, page navigation, activation payloads, viewport item
count, row metrics, overscan, typeahead target resolution, replacement-style multi-select range
selection, and semantic scroll strategy labels. The GPUI adapter owns the printable-key typeahead
buffer, anchor-key lifecycle, and reveal side effects: typeahead moves the active row without
selecting it, while Shift-range interaction replaces selected keys with the current selectable
anchor-to-target range. `VirtualizedListBehaviorSnapshot::sticky_section` returns an optional
`VirtualizedListStickySectionSnapshot` for the section row that owns the first visible selectable
row. `VirtualizedListBehaviorSnapshot::sticky_overlay` returns an optional
`VirtualizedListStickyOverlaySnapshot` for the presentation-only sticky header layer: the overlay is
positioned against the viewport, while the underlying section row remains the semantic owner. It
does not add a second interactive section row or change focus order, hit testing, selection, or
accessibility roles. `VirtualizedListRowMeasureMode`
keeps fixed rows as the default hot path and exposes measured rows as an explicit opt-in;
`VirtualizedList::virtualizer_snapshot` seeds measured heights by stable render key, removed keys
are dropped from emitted snapshots, and missing measurements fall back to the estimated row height
with estimated reveal targets. `VirtualizedListItemDescriptor` is the typed row descriptor: item
rows can carry primary text, secondary text, text value, disabled reason, leading/trailing metadata,
badge, and status; section, separator, loading, empty, and error rows are non-selectable and expose
their row kind through behavior snapshots. `VirtualizedListGpuiExt::render_row` is the custom
content escape hatch exposed through `open_gpui_ui_components::gpui_adapter`: it receives
`VirtualizedListRowRenderContext`, but the outer row keeps virtual layout, measured-height
feedback, role/ARIA metadata, focus, hit testing, selection, and activation ownership. The same
adapter trait exposes `scroll_handle` for host-owned GPUI viewport handles. The active-descendant
indicator uses `open_gpui_motion` as paint-only chrome keyed by
the active row; `VirtualizedList::motion_preference` controls reduced-motion behavior, and the
motion sample must not change row layout, scroll offsets, selection state, focus order, hit
testing, or accessibility roles. Row enter/exit animation, public presence, keyframes,
repeat/reverse/speed controls, shared-layout orchestration, and MotionValue subscriptions remain
deferred. Rendered range calculation remains owned by `open_gpui_ui_core::VirtualizerState`.
`TreeBehaviorSnapshot` and `CommandBehaviorSnapshot` follow the same public boundary: behavior
probes are stable, renderer assembly plans are internal.

Collection component selection is intentionally narrow:

- Use `VirtualizedList` when the application already has flat row descriptors and needs local
  virtualized rendering, key-based active/selection state, optional measured row heights, or a
  constrained row content renderer.
- Use `Listbox` when the surface is a finite option picker with grouped choices, typeahead, and
  listbox semantics but no large virtualized window.
- Use `Command` when query ownership, ranked command results, provider/index snapshots, shortcuts,
  loading/status metadata, or dialog/inline command presentation are part of the workflow.
- Use `Table` when rows need columns, sorting/filtering/grouping, expansion, pinning, editing,
  column sizing, or row-region behavior.
- Use `Tree` when hierarchy, expansion, lazy children, tree roles, and branch/leaf state are the
  primary model.
- Use low-level `open_gpui_ui_core::VirtualizerState` only when building a new adapter or domain
  component that owns its own row semantics, focus, accessibility, and callbacks.
`command/mod.rs` is the reference split facade: descriptor, model, style, render-plan, and runtime
owners stay in sibling modules, while `open_gpui_command::CommandDescriptor` is the cross-surface
app-command descriptor consumed by Command, Menu, and ContextMenu projections. `Menu`,
`ContextMenu`, and `Tree` follow that shape: `menu/mod.rs` keeps the builder/render facade while
`menu/descriptor.rs`, `menu/model.rs`, `menu/render_plan.rs`, `menu/runtime.rs`, and
`menu/style.rs` own the public model, submenu placement contract, timing, and metrics;
`context_menu/mod.rs` keeps the point-anchor facade while `context_menu/model.rs` owns the
renderer-neutral context-menu state; `tree/mod.rs` keeps the render facade while
`tree/descriptor.rs`, `tree/model.rs`, `tree/runtime.rs`, `tree/style.rs`, `tree/movement.rs`, and
`tree/render_plan.rs` own descriptor data, state, adapter runtime, metrics, drag/drop movement,
and virtualized behavior snapshots.
`menu/runtime.rs` owns submenu hover timing, branch switching, trigger-bound caches, and local
submenu scroll handles for `Menu` and `ContextMenu`, keeping render assembly thin while preserving
safe hover and local scroll ownership.
After these family splits, the shared UI framework contract work is enforced through
`cargo run -p xtask -- scan-ui-contract`: `component_contract` module ownership, a11y contract
claims, gallery conformance evidence, and theme schema/loading all have an audit entry point. The
remaining large component files should stay intact unless one of those product contracts exposes a
concrete ownership problem.
`Splitter` covers panel fraction normalization, min/max constraints, collapsed-panel metadata,
stable handle anatomy, and local pointer dragging through keyed runtime state. Keyboard resizing,
controlled resize callbacks, persisted layouts, RTL behavior, and nested splitter arbitration
remain follow-up work.
