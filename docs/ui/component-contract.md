# Open GPUI Component Contract

Official Open GPUI components use an adapter-first, productized GPUI shape. A component may render
with GPUI today, but its behavior and semantic state should stay renderer-neutral enough to test
without rewriting the public API. ADR 0008 treats the current UI crates as the active product
boundary; future headless extraction is historical boundary evidence, not the current roadmap or
the next implied refactor.

## Contract Tables

`crates/ui_components/src/component_contract/` is the product authority for official component id,
contract revision, family, and required scenario ids. It deliberately does not own Rust export
paths, source files, Gallery selectors, DevTools projections, package names, test targets, or test
functions. Those facts live with the modules and artifacts that execute them.

Public modules generate typed export facts and positive facade compile witnesses from the same
declaration as each `pub use`. Diagnostic declarations also generate per-export compile-fail
doctests for the root, common, and prelude facades. The Components and Overlay galleries own their
selectors and runtime probes. Each native integration test target owns a sibling
`*.scenarios.toml` artifact that binds product requirements to exact nextest coordinates. Gallery
stories and DevTools semantic identities independently receive the same immutable
`ComponentContractMetadata` from canonical product rows. `cargo run -p xtask -- scan-ui-contract`
joins these owners and rejects drift without becoming a second registry.

The following table is a human-readable projection of the narrow product rows. The scanner checks
it exactly against the typed source rows.

<!-- BEGIN COMPONENT CONTRACT PROJECTION -->
| Contract ID | Revision | Family |
| --- | ---: | --- |
| `Accordion` | 1 | `disclosure` |
| `Button` | 1 | `action` |
| `Badge` | 1 | `display` |
| `Collapsible` | 1 | `disclosure` |
| `Link` | 1 | `navigation` |
| `Breadcrumb` | 1 | `navigation` |
| `Tag` | 1 | `display` |
| `ToastStack` | 1 | `feedback` |
| `IconButton` | 1 | `action` |
| `Slider` | 1 | `form` |
| `NumberInput` | 1 | `form` |
| `Switch` | 1 | `form` |
| `Checkbox` | 1 | `form` |
| `RadioGroup` | 1 | `choice` |
| `Toggle` | 1 | `action` |
| `ToggleGroup` | 1 | `action` |
| `Toolbar` | 1 | `shell` |
| `Sidebar` | 1 | `shell` |
| `Tree` | 1 | `hierarchy` |
| `Listbox` | 1 | `choice` |
| `Select` | 1 | `choice` |
| `Combobox` | 1 | `choice-search` |
| `Command` | 1 | `choice-search` |
| `Label` | 1 | `form` |
| `TextInput` | 1 | `form` |
| `Textarea` | 1 | `form` |
| `Field` | 1 | `form` |
| `Tabs` | 1 | `navigation` |
| `ScrollArea` | 1 | `layout` |
| `Splitter` | 1 | `layout` |
| `Table` | 1 | `data` |
| `VirtualizedList` | 1 | `data` |
| `StatusCue` | 1 | `feedback` |
| `EmptyState` | 1 | `feedback` |
| `Separator` | 1 | `layout` |
| `Kbd` | 1 | `display` |
| `Progress` | 1 | `status` |
| `Skeleton` | 1 | `status` |
| `Avatar` | 1 | `identity` |
| `AvatarGroup` | 1 | `identity` |
| `Tooltip` | 1 | `overlay` |
| `HoverCard` | 1 | `overlay` |
| `Popover` | 1 | `overlay` |
| `Dialog` | 1 | `overlay` |
| `AlertDialog` | 1 | `overlay` |
| `Sheet` | 1 | `overlay` |
| `Menu` | 1 | `overlay` |
| `ContextMenu` | 1 | `overlay` |
<!-- END COMPONENT CONTRACT PROJECTION -->

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
  `open_gpui_ui_components::ThemeContext`, usually via
  `ThemeResolver::current(window, cx)`.

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

The resource adapter surface is `ResourceAdapterLabels`, `ResourceAdapterNamespace`,
`ResourceCollectionProjection`, `ResourceMutationProjection`, and `resource_query_key_label`. Each
projection requires a caller-owned diagnostic-safe namespace; its command status identity is
derived from that namespace rather than a query key or mutation ID. It consumes
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

## Subtree Presentation Authority

Official components use `open_gpui::SubtreePresentation` when an owning subtree must retain layout
while becoming inert or hidden. They must not recreate ancestor paint, input, focus, IME, tooltip,
overlay, Inspector, or accessibility flags. The exact matrix is:

- `Visible`: layout, paint, input, focus/IME, and accessibility;
- `Inert`: layout and paint only;
- `Hidden`: layout only.

A suppressive ancestor wins over every descendant declaration. Component `disabled` remains a
resolved semantic fact and may be present in the final accessibility tree; subtree inertness is
absent from accessibility and makes semantic or programmatic activation unavailable. Decorative
leaf omission through `omit_accessibility_node` does not hide descendants and cannot substitute
for subtree presentation. Renderer-neutral producers represent that leaf operation with
`SemanticDescriptor::with_omit_accessibility_node`; the descriptor intentionally has no generic
`hidden` state because accessibility omission and subtree presentation are different contracts.

Official overlay adapters bind their local presentation to `WindowOverlayRuntime`. A suppressed
layer remains in its controlled lifecycle but releases modal input and focus authority; its open
overlay descendants inherit the effective suppression, while independent window-root layers do
not. Restoration to `Visible` resumes current lifecycle without claiming initial focus or replaying
the input that originally opened the surface. Tooltip, HoverCard, Popover, Dialog, AlertDialog,
Sheet, Menu, and ContextMenu must follow this same runtime path rather than installing family-local
presentation tails.

## Exact Subtree Clip Authority

`open_gpui::SubtreeClip` and `SubtreeClipExt` are the only public authority for clipping a complete
element subtree. A clip is declared in zero-origin child-local logical coordinates relative to the
child's post-layout border box. `SubtreeClip::own_border_box` creates a rectangle; checked rounded
constructors accept finite, non-negative elliptical corner radii. `clip_to_border_box` and
`with_subtree_clip` preserve layout, measurement, sibling flow, and scroll extent while constraining
paint, initial pointer hit testing, drag/drop acquisition, focus/IME eligibility, Inspector picks,
deferred/cache replay, and accessibility publication.

Nested clips are an exact intersection stack. Conservative window-space AABBs may cull primitives
or describe portal/accessibility geometry, but they are never final rounded hit coverage. The
committed `HitTestSnapshot` used by consumers such as Dock retains the exact stack and frame
eligibility, so an AABB corner outside a rounded clip is not a valid target. Pointer capture follows
the existing capture policy after a valid acquisition; clipping does not add a second capture rule.

Style overflow feeds the same visual/pointer stack: one-axis overflow is a rectangular strip that
inherits the unclipped axis, and two-axis overflow derives its padding-box elliptical radii before
shared normalization. Its semantic-exclusion axes are narrower: `Overflow::Hidden` and
`Overflow::Clip` exclude fully clipped AccessKit descendants, while `Overflow::Scroll` retains
off-viewport semantics so `ScrollIntoView` can reveal them. Ordinary deferred and cached descendants
inherit the current stack; a named window-space portal deliberately resets clip ancestry. Invalid
transform, clip, or device conversion suppresses the affected subtree transaction across every
channel rather than substituting an AABB or identity geometry. A native surface that cannot consume
the resolved clip is rejected or isolated.

Ordinary deferred semantic roots retain their captured AccessKit parent. A window-space portal
begins its semantic subtree at the window root as well as resetting its clip: an AccessKit
`clips_children` declaration applies to every child of its owner, so preserving that parentage would
incorrectly reapply the escaped visual clip.
Captured deferred semantics also retain a source root sibling anchor. Nested deferred and portal
replay extends that anchor, so accessibility order remains independent of deferred paint priority.

AccessKit has no rounded-region representation. Public `SubtreeClip` and non-scrolling overflow
exclude fully clipped nodes, but the root viewport and `Overflow::Scroll` do not: an offscreen
scroll target remains semantic and can receive `ScrollIntoView`. A semantically published non-empty
node exposes a conservative AABB; built-in fallback Click dispatch separately requires a CPU-proven
interior witness in the full visual/pointer stack. A zero-area semantic node may remain only when
its anchor point is inside the semantic stack; it receives no pointer witness or fallback Click.
Components must not infer rounded hit coverage from published bounds. Arbitrary path clips, fill
rules, and renderer-specific clip payloads are intentionally not public API.

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

GPUI placement adapters remain responsible for deferred/window-portal rendering, hitboxes, and
AccessKit relationship wiring. `WindowOverlayRuntime` is the sole per-window authority for live
registration order, parentage, input subscriptions, controlled close intent, modal barriers, focus
handles, focus claims, and restoration. `open_gpui_ui_components::gpui_adapter` provides only the
narrow placement mapping layer: deferred priority, snap-to-window margin, GPUI anchor mapping, and
placement resolution. `open_gpui_ui_core::overlay` owns renderer-neutral policy resolvers such as
`resolve_escape_key`, `resolve_outside_press`, and `resolve_overlay_placement`; the window runtime
consumes them for production arbitration rather than rebuilding stack ownership in each component.
`open_gpui_ui_core::overlay::resolve_overlay_placement` is the shared placement solver for explicit
neutral inputs. It returns `OverlayPlacementResolution` with fit and trace metadata, so portal,
point-anchored, and render-plan overlays that provide anchor bounds, content size, and safe bounds
use one flip/shift policy. `OverlayAnchorInput` remains a pure renderer-neutral snapshot; it is not
a live target reference.

GPUI owns live trigger geometry through `PortalAnchorHandle`. A handle belongs to one window, binds
exactly one target per completed frame through `PortalAnchorExt::track_portal_anchor`, and may feed
multiple followers. `PortalAnchorSnapshot` exposes only window identity, frame generation, opaque
`ElementGeometry`, effective `SubtreePresentation`, and the target's effective clip AABB. During a
draw, followers read only the current candidate; outside a draw they read only the last completed
frame. Hidden, absent, unmounted, rolled-back, or numerically invalid targets resolve as unlinked;
there is no last-known geometry fallback. Inert remains a linked GPUI fact.
Presentation and transform wrappers on the tracked target root are order-independent relative to
`track_portal_anchor`; builder order cannot bypass Hidden unlinking or publish untransformed bounds.
When the tracked element is a cached `AnyView`, its cache layout and rendered root layout are one
semantic anchor root. GPUI recaptures that root instead of replaying an inner cache journal, while
ordinary descendant scopes remain outside the target facts.

`portal_anchor_follower` resolves after ordinary prepaint and emits an explicit window-space portal.
Views that resolve a portal anchor, including custom deferred work, are recorded as cross-view
dependencies and are rebuilt on the next frame instead of replaying a stale cached deferred journal.
Ordinary deferred content continues to inherit transform and clip. A window portal consumes the
already projected target geometry and deliberately escapes the target clip while retaining theme
and presentation inheritance.
Every overlay inside region captures `Window::hit_test_snapshot` during prepaint. Runtime
outside-press arbitration uses its exact active clip stack rather than raw or clipped-away layout
bounds; its conservative displayed bounds are only an early empty-region check. An
unrepresentable transform publishes no region.

GPUI separately owns physical reveal through `RevealTargetHandle`. A completed frame binds one
target to its final `ElementGeometry` and committed inner-to-outer scroll ancestry. Application
requests, the winning focus claim, and AccessKit `ScrollIntoView` enter the same
`Window::bring_into_view` authority with explicit physical axes, overlap arbitration,
transform-correct local deltas, and typed terminal outcomes. Portals begin a new rendered ancestry;
component adapters must not infer a source chain through an anchor or retained rectangle.
Every successfully published semantic node exposes AccessKit `ScrollIntoView` as a geometry action,
including disabled nodes that cannot receive activation or focus actions; stale, suppressed, and
unpublished nodes expose no route.
An adapter that bridges materialization and a later physical request captures
`Window::capture_deferred_bring_into_view_guard` from prepaint inside the intended final scroll
ancestry as soon as logical materialization completes, then submits it with
`Window::try_bring_into_view_with_guard_and_completion` after the target binds. The opaque guard
validates the target, the complete committed ancestry, and relevant direct-scroll revision
atomically; a failed guard submits nothing. `ScrollHandle::direct_scroll_revision` remains a per-handle
low-level interruption token, not a substitute for a nested reveal chain or a second reveal
authority.

UI Components accepts only `SubtreePresentation::Visible` portal snapshots for interactive overlay
followers. An ineligible or missing target forces the layer noninteractive and dispatches
`DismissReason::AnchorUnlinked`: uncontrolled owners commit closed state, while controlled owners
hide immediately and receive one typed open-change intent. If a controlled owner keeps requesting
Open, the pending intent remains without being redispatched; committing Hidden clears it. A later
eligible target can establish a new opening generation only after the owner requests Open again.

Popover, Select, Combobox, HoverCard, Menu roots, and Menu submenu rows bind runtime-owned portal
anchors. Standalone `Tooltip` accepts a caller-owned handle through `Tooltip::portal_anchor` and
reports unlink through `Tooltip::on_open_change`; bind that same handle to the target every rendered
frame. An initial closed, anchorless Tooltip establishes no registration, but once a retained
Tooltip ID has bound a handle, later renders reuse that exact capability and reject replacement.
GPUI-native delayed tooltips intentionally retain pointer-point anchoring. ContextMenu retains an
explicit window point, and Dialog, AlertDialog, Sheet, and other viewport surfaces use a named
window portal; neither path is converted into a trigger handle or transformed again by an ancestor.

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
resolved-state type, rendered sample selector, Gallery-local group, coverage summary, and Story
probes.
`overlay_sample_selector_pairs()` is the focused selector contract for rendered overlay samples.
Default-open overlay samples may expose default-open metadata, but the gallery must keep them
visually non-blocking at page load so modal barriers and floating layers do not prevent scrolling
or navigation.

`ListboxState` is the renderer-neutral collection choice contract. It records grouped and
standalone option descriptors, separator rows, disabled option state, selected value, active
descendant value, tab-stop value, APG-style Up/Down/Home/End navigation, Enter/Space activation
payloads, typeahead target resolution, resolved metrics, token intents, and listbox/listbox-option
roles. It does not own popup state, adapter selection storage, scroll handles, focus handles,
callbacks, or GPUI element ids. The GPUI adapter treats `selected(Option<String>)` as a
caller-owned render-frame value and `default_selected` as an uncontrolled runtime seed.
`default_active` seeds adapter-owned active state once; navigation and typeahead then resolve from
the event-time runtime value. Pointer, unmodified Enter/Space key-up, AccessKit Click, and
`activation_handle(value, handle)` requests enter one selection transaction. An option handler
replaces the Listbox public fallback, while an embedding Select or Combobox owner transaction still
commits selection, input, and overlay state before that one chosen callback is delivered. Duplicate
selectable values fail closed; structural rows do not participate in domain-value uniqueness.

`SelectState` composes a trigger, non-modal dismissible overlay, scroll viewport metadata, and a
nested `ListboxState`. It records controlled versus uncontrolled open mode, default-open state,
placeholder and selected trigger label, selected and active option values, placement preference,
outside-press policy, initial focus intent, focus restoration intent, resolved metrics, token
intents, and the listbox content role. `SelectState::resolve` takes a `SelectStateRequest` so
callers group overlay policy, selection inputs, descriptors, and theme tokens explicitly. The GPUI
`Select` adapter owns trigger/content rendering, keyed runtime open/selected/active state,
callbacks, and deferred anchored rendering. `selected(Option<String>)` is caller-owned and
`default_selected` seeds adapter-owned selection; `default_active` seeds popup active state. Its
window runtime binding owns outside/Escape arbitration, the popup layer, trigger/surface focus
handles, controlled refusal, and restoration.

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
`Combobox::selected(Option<String>)` is caller-owned and `default_selected` seeds adapter-owned
selection. `default_active` seeds the keyed active value; subsequent editor navigation is resolved
from runtime state. A controlled selection intent cannot change runtime selection or editor text
until the owner supplies a later selected prop.

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
`Command::selected(Option<String>)` and `selected_values(...)` are caller-owned;
`default_selected`, `default_selected_values`, and `default_active` seed their adapter-owned
counterparts.
Multi-select change payloads toggle the raw caller/runtime selection set, preserving values that
are currently missing, disabled, or filtered out; only chips and semantic selection projection
filter that set against the current command collection.
Command value uniqueness is resolved against the full unfiltered collection. A duplicate value
remains ambiguous and disabled even when ranking or query filtering leaves only one occurrence
visible; surface-disabled and ambiguous rows expose neither pointer activation nor an AccessKit
Click action. Uncontrolled single and multi-selection transactions notify their keyed runtime so
inline selected state and chips do not wait for an unrelated redraw.
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
Seed-shaped runtime builders must stay explicit in their owning Rust APIs and executable
state/runtime tests. Current examples include
`Listbox::default_selected`, `Select::default_selected`, `Combobox::default_selected`,
`Command::default_selected`, `Command::default_selected_values`, `Tabs::default_selected`,
`RadioGroup::default_selected`, `Toolbar::default_focused`,
`Sidebar::default_focused`, `Tree::default_selected`, `Tree::default_focused`,
`VirtualizedList::default_active_key`, `VirtualizedList::default_selected_key`,
`VirtualizedList::default_selected_keys`,
`Combobox::default_query`, `Command::default_query`, `Menu::default_focused_value`, and
`ContextMenu::default_focused_value`. Direct names such as `Sidebar::selected`,
`Listbox::selected`, `Select::selected`, `Combobox::selected`, and `Command::selected` remain
reserved for caller-owned render-frame inputs. `Switch::on_change`, `Toggle::on_change`, and
`TextInput::on_change` are scalar value-change callbacks. Bootstrap callback exceptions such as
`Button::on_click`, `AlertDialog::on_action`, `AlertDialog::on_cancel`, and
`Table::on_sort_requested` must stay explicit in their owning APIs and callback tests because they
represent command activation, modal action outcomes, or table sort requests rather than scalar
value changes. Sheet
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
- crate-root, `common`, and prelude exports are declared through the typed public-export macro and
  covered by compile tests;
- metrics, sizes, colors, focus rings, and accessibility metadata use foundation vocabulary;
- callbacks, focus handles, scroll handles, image loading, deferred rendering, and subscriptions
  stay in the GPUI adapter layer;
- the Components gallery exposes real samples, stable sample ids, and resolved-state metadata;
- every official catalog entry has matching `SIGNALS` entries for its component type and resolved
  state type, plus at least one rendered `gallery:component-*-sample:{id}` selector;
- every official overlay family has a matching `OVERLAY_CATALOG` row with canonical component
  metadata, component/state `SIGNALS`, a Gallery-local behavior group, and at least one rendered
  `gallery:overlay-*-sample:{id}` selector;
- required native scenarios are declared next to their owning integration targets in
  `*.scenarios.toml`, while focused runtime tests cover behavior that state tests cannot prove;
- `docs/verification.md` names any manual or automated gate added by the component.

`open_gpui_ui_components::component_contract` owns only the narrow product table and typed joins.
`component_contract/types.rs` owns `ComponentContractMetadata`, `ComponentContractEntry`, and
`PublicApiExport`; `component_contract/rows/catalog.rs` owns the 48 canonical id/revision/family
rows and required scenario ids; `component_contract/projections.rs` provides lookup plus typed
export projections. Public API modules, Gallery catalogs, DevTools snapshots, native scenario
artifacts, and docs remain independent owners whose facts are joined by `scan-ui-contract`.
That table contains only official rendered component contracts. It does not classify recipes,
renderer-neutral state contracts, GPUI adapter helpers, public anatomy, diagnostics, or removed
compatibility targets, and it does not record method names, source homes, docs tokens, Gallery
status, or export intent.
`examples/ui-foundation-gallery::pages::components::catalog::COMPONENT_CATALOG` consumes canonical
metadata for its official rows but owns its presentation status, sample selectors, state readouts,
coverage summaries, and Story probes locally. Gallery-only `adapter-only`, `internal-anatomy`, and
`state-contract` rows therefore do not become product rows by appearing beside official samples. A
future rendered component joins the product table only after it satisfies the completion checklist
and owns native scenario coverage.

Public surface ownership uses a stricter source-facing vocabulary than the visible gallery status.
`official component` names are rendered GPUI components with canonical product rows and Gallery
sample selectors.
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
  and `TableRowRenderPlan`. Those structures are private to the table adapter implementation;
  algorithm coverage belongs in `crates/ui_components/src/table` module tests.
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

The GPUI adapter resolves intents through `ThemeResolver::current(window, cx)` immediately before
calling style APIs such as `bg`, `border_color`, and `text_color`. The resolver applies one
precedence order: nearest `ThemeScope`, then window selection or override, then app selection, then
the built-in light fallback. The returned `ThemeContext` owns the immutable render-time snapshot,
so adapters and their helper functions pass it explicitly or pre-resolve concrete colors before
storing style or event closures. Code with an explicit immutable snapshot can call
`ThemeResolver::resolve_with(intent, snapshot)`. The former app-only resolver signature and the
default-light `ThemeResolver::resolve` compatibility path are deleted.

`ThemeScope::new` requires a stable `ElementId`, an owned `ThemeContext`, and one child subtree.
Its window-local stack is restored by RAII after normal returns and unwinding. A changed scope
context invalidates cached child-view journals even when the child entity itself did not notify.
The implementation stays theme-specific: no independent non-theme immutable-stack consumer met
the adoption gate for a public generic GPUI inherited-context API.
The ownership and prototype evidence are recorded in
[Theme scope resolution and deferred capture](../knowledge/engineering/decisions/theme-scope-resolution.md).

Official overlay bindings capture the effective context when a lifecycle generation enters Open.
Trigger styling continues to use the current context, while every surface color and deferred child
uses that generation's opening context until Hidden; close and reopen starts a new capture.
Delayed native tooltip builders are another detached render boundary. Button, IconButton, Menu,
Sidebar, and Toolbar capture automatically. A caller attaching a UI Components tooltip builder
directly through GPUI interactivity must wrap it with `Tooltip::scoped(context, builder)` so both
builder execution and the returned view retain the same opening context.

`ThemeSnapshot` is the complete immutable Theme v1 value. It owns `ThemeMode`, source revision
metadata, the complete `ThemeColor` table, and one `ThemeDesignScales` value. `ThemeContext` wraps
that snapshot with a runtime-owned effective revision. Source revision describes the file or
definition that supplied the value; it is never used as the render cache authority. Effective
revision changes monotonically when effective content or selection authority changes and cannot be
supplied by callers. Clones and detached opening-generation captures preserve it.

`ThemeDesignScales` contains typed `ThemeTypographyScale`, `ThemeSpacingScale`,
`ThemeRadiusScale`, and `ThemeElevationScale` values plus `Density` and `MotionPreference`.
`ThemeElevationLayer` is renderer-neutral and stores logical offsets, blur, spread, and opacity.
Every public design token has two production recipe consumers: Button and TextInput share control
typography, spacing, radius, and density resolution; official overlay surfaces and Tooltip share
elevation; Splitter and VirtualizedList share the strict motion-policy merge. Explicit component
`Size` wins over the theme density default. Reduced motion is a safety floor, so either the theme
or component may request reduction and an explicit animated request cannot relax a reduced theme.
Structural component dimensions and motion execution remain outside the theme value.

Public `Button::state()` and `TextInput::state()` calls have no Window from which to resolve a
scope, so their metrics use the built-in Theme v1 baseline for deterministic authored-state tests
and Gallery metadata. Production rendering resolves the effective `ThemeContext` first, injects
the resulting recipe metrics into that same state shape, and renders only from those state metrics;
it does not maintain a parallel render-only metric authority.

`ThemeColor` entries pair semantic tokens with `ColorState` values. Components do not read
registry globals directly; renderer-neutral authored state remains separate from render-time theme
resolution, and the GPUI adapter consumes an owned `ThemeContext` at the render edge.

`ThemeRegistry` owns built-in and user-loaded definitions. Private app theme state owns the
installed registry plus app fallback selection; `Window::use_window_state` owns each window's
selection, explicit override, scope stack, and app-change observer. Public mutation APIs are
`install_theme_registry`, `register_theme`, `set_app_theme`, `set_app_theme_mode`, `set_window_theme`,
`override_window_theme`, and `clear_window_theme`. Unknown IDs are rejected before mutation, and
equal selections do not update window state or refresh unaffected windows. A registry replacement
that omits a window-selected id retains that window's last-known immutable context until the id is
registered again or the caller explicitly clears the selection. The registry preloads
light, dark, and high-contrast entries, validates `ThemeDefinition` identity fields, replaces
entries by stable id, and stores complete owned contexts behind `ThemeRegistryEntry`. A definition
must provide id, label, mode, source revision, design scales, and every supported token/state color
exactly once. Missing, duplicate, or unsupported color entries fail validation before mutation;
there is no built-in color fill, registration diagnostic, or partial production fallback. An
identical or metadata-only replacement preserves the existing effective revision. Changed content
allocates a new one, and selecting a different id allocates a new one even when its payload is
identical because the authority changed. Consumers can resolve a registered definition to an owned
`ThemeContext`, borrow its immutable `ThemeSnapshot`, and pass that snapshot to
`ThemeResolver::resolve_with` for an explicit non-runtime color lookup. Programmatic definition
failures are reported as `ThemeValidationError` before the registry is mutated.

Portable theme files are JSON and versioned by `THEME_JSON_SCHEMA_VERSION`. The public loader
surface is the explicit theme-owner API under `open_gpui_ui_components::theme`:
`theme_json_schema`, `theme_definition_from_json_str`, `theme_definition_from_json_file`,
`register_theme_json_str`, `register_theme_json_file`, and `theme_json_string`.
The reviewable schema artifact for version 1 lives at
`docs/schemas/open-gpui-theme-v1.schema.json`; regenerate it with
`cargo run -p open-gpui-ui-components --example export_theme_schema --quiet` when
`theme_json_schema()` changes, then run `cargo run -p xtask -- scan-theme-schema`.
Those facades strictly validate schema version, identity fields, complete nested design scales,
`ThemeMode`, `Density`, `MotionPreference`, exactly two valid elevation layers, duplicate and
complete token/state coverage, supported names, and six-digit RGB values before a definition
reaches `ThemeRegistry::register`. Loader failures are structured as `ThemeLoadError` with
`ThemeFileField` for missing top-level, nested design, size-scale, elevation, or color fields, so
applications can show messages without parsing error strings. The generated JSON Schema defines
the portable wire shape; loader and registry validation additionally enforce semantic token/state
completeness and cross-entry uniqueness that JSON Schema cannot express directly. Enum literals
and RGB strings are canonical: surrounding whitespace and `0x` prefixes are rejected rather than
normalized. The old color-only document,
`fallback_mode`, and partial-palette parsing are unsupported; this is an in-place breaking
replacement under schema version 1, not a parallel v2 or compatibility loader. A pressed state is
intentionally absent until the resolver grows a real `ColorState` for it.

Schema vocabulary audit target:

- Top-level fields: `schema_version`, `id`, `label`, `mode`, `revision`, `colors`, `design`.
- Color entry fields: `token`, `state`, `rgb`.
- Design fields: `typography`, `spacing`, `radius`, `elevation`, `density`, `motion_policy`.
- Typography fields: `control_text`, `control_line_height`.
- Spacing fields: `control_inline`, `control_block`.
- Radius fields: `control`.
- Size-scale fields: `xsmall`, `small`, `medium`, `large`.
- Elevation fields: `overlay`.
- Elevation-layer fields: `offset_x`, `offset_y`, `blur_radius`, `spread_radius`,
  `opacity_percent`.
- Modes: `light`, `dark`, `high-contrast`.
- Densities: `compact`, `comfortable`, `spacious`.
- Motion policies: `animated`, `reduced`.
- Tokens: `semantic.surface`, `semantic.surface_muted`, `semantic.border`, `semantic.text`,
  `semantic.text_muted`, `semantic.accent`, `semantic.accent_foreground`, `semantic.focus_ring`,
  `semantic.destructive`, `semantic.destructive_foreground`, `semantic.overlay`,
  `semantic.modal_overlay`.
- States: `default`, `hover`, `selected`, `disabled`, `read-only`, `invalid`, `required`,
  `placeholder`, `message`, `focus-visible`, `overlay`, `modal-overlay`.

Theme module ownership is intentionally split: `theme/snapshot.rs` owns complete immutable values,
`theme/registry.rs` owns atomic validation and registration, `theme/runtime.rs` owns effective
revision allocation and app/window authority, `theme/resolver.rs` owns intent-to-color resolution,
`theme/schema.rs` owns the strict JSON schema and loader facade, `theme/palette.rs` owns complete
built-ins, and `theme/recipes/` owns color and design recipes. Component files call cataloged
`ThemeResolver` recipes but do not add local `impl ThemeResolver` blocks. The `scan-theme-drift`
xtask gate checks recipe catalog coverage, built-in palette shape, complete built-in design
payloads, and the two-production-recipe rule for every public design token. The
`scan-theme-schema` gate checks generated artifact equality and exact documented vocabulary, so a
schema, recipe, or token change cannot silently drift from its consumers or documentation.
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
must not participate in roving focus or activation. Item values are stable identities and must be
unique within one Toolbar; duplicate values remain visible but fail closed as disabled,
non-activatable items so focus, element, and programmatic-handle identity cannot alias.

The GPUI `Toolbar` adapter owns focus handles, semantic activation binding, and concrete item
rendering. Action items accept Enter and Space on key-up; toggle items accept Space only. Pointer,
keyboard, AccessKit, and programmatic activation share one transaction and report the caller-owned
pressed value from before activation. An item-level handler overrides the toolbar-level fallback,
so one activation invokes one domain callback.
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
keeps selection app-owned; activating an item produces a `SidebarActivation` payload plus the
normalized `Activation` input but does not own routing or persistent preferences. Selected and
focused projection is resolved by the shared `ChoiceCollection` single-optional vertical policy.
Stable item values must be unique across every section. Duplicate values remain visible but fail
closed as disabled, non-focusable, and non-addressable items.

Icon collapse keeps navigation items visible and focusable while hiding visible text; item labels
remain explicit accessibility labels. Offcanvas collapse removes items from roving focus by making
them invisible and non-focusable. `SidebarCollapseMode::None` ignores collapsed input and keeps the
expanded width. Disabled items are skipped by the shared vertical roving-focus helper and cannot
produce activation payloads.

The GPUI `Sidebar` adapter owns focus handles, concrete rendering, scroll handles through
`ScrollArea`, and AccessKit mapping. Pointer, unmodified Enter/Space key-up, AccessKit Click, and
`ActivationHandle` requests converge through one `ActivationBinding`; Arrow/Home/End remain focus
navigation only. An item-level `SidebarItem::on_activate` handler replaces the Sidebar fallback so
one intent invokes exactly one domain callback. Disabled, duplicate, whole-Sidebar-disabled, and
offcanvas items reject every activation source through the same gate. It should expose
`Role::Navigation` on the container, `Role::Section` for groups, explicit item labels, selected and
disabled metadata, and set-position metadata only for focusable items. Structured occurrence
identities keep duplicate section and item nodes disjoint without colliding with legal values. A
redraw transfers physical focus to the resolved fallback only when a previously focused Sidebar
handle became invalid; external window focus is preserved.
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

`TableState` describes renderer-neutral table behavior: caller-owned business `TableRowId` values,
exact `TableRowIdentity` values, nested source rows, row lookup, row-model stage vocabulary,
selection keyed by exact `TableSourceRowIdentity`, column visibility and ordering, pinned column regions, row
pinning, sorting, filtering, grouping, built-in aggregation, expansion, column groups, nested
headers, and pagination. The official table contract resolves the full pipeline core -> filtered
-> grouped -> sorted -> expanded -> paginated -> final without changing an exact row identity. Row
pinning derives the top, center, and bottom presentation partitions from resolved models according
to its policy; it is not another row-model stage.

`TableRowId` is not an exact source-instance identity. Source-backed exact identities use
`TableSourceRowIdentity` with `TableSourceInstanceIdentity::Unique` when the business id is unique,
`Explicit(TableRowInstanceId)` when the caller supplies a stable instance id, or
`Occurrence(TableRowOccurrenceIdentity)` as a source-snapshot-local fallback for duplicates.
Synthetic group rows use their separate typed group identity namespace. An identity built with
`TableSourceRowIdentity::unique` resolves as `TableSourceRowLookup::Ambiguous` when duplicate
business ids exist. `TableState::source_row_identity_at(row_id, occurrence)` produces an exact
occurrence identity for the current snapshot, and `TableState::source_row_lookup` distinguishes
`Found`, `Missing`, `Ambiguous`, and `StaleSnapshot`.

An occurrence identity remains valid through cloned state and every row-model transform, but
`TableState::with_rows` creates a new source snapshot, including when callers reorder equivalent
rows. Retained occurrence identities are then stale rather than silently retargeted. Callers that
need identity to survive source replacement or reorder must assign `TableRow::with_instance_id`
and address the row through an explicit source-instance identity. `TableRowModel::row` accepts only
`TableRowIdentity` and performs exact lookup, including lookup-only rows retained from an earlier
stage; `rows()` remains the materialized order for that stage. Business-id lookup is deliberately
separate: `source_rows(&TableRowId)` returns every matching source instance and
`unique_source_row(&TableRowId)` returns a row only when exactly one match exists.

`TableState::with_selected_rows` and `selected_rows` use exact `TableSourceRowIdentity` values.
Duplicate business ids therefore do not alias selected state, descendant propagation, or
`TableRowSelectionChange::current_selection`. The callback projects caller-owned explicit roots in
source-model order rather than promoting derived selected descendants into state. Under descendant
propagation, canceling a parent removes its explicit subtree, while canceling an inherited child
removes the explicit ancestor that covers it so the committed payload cannot immediately reselect
the child. Selection remains source-row-only; there is no implicit business-id bulk target or
speculative selection scope. A future bulk operation must use an explicitly named target, following
`TableRowPinTarget::AllSourceRows` rather than weakening the exact state contract.

Source tree rows remain distinct from synthetic group rows: `TableRow` may own child rows,
resolved source rows expose depth, exact parent identity, branch/leaf state, descendant counts, and
expansion metadata through `TableTreeRow`, and collapsed source descendants stay addressable by
exact `TableRowIdentity` lookup rather than by `TableRowId`. `TableRow` can also be marked expandable
before children are loaded, and `TableRowChildrenLoadState` carries caller-owned idle, loading, or
failed child-load metadata into resolved tree rows. `TableExpansionMode::Client` keeps the normal
client-pruned source tree behavior, while `TableExpansionMode::Manual` preserves the
caller-supplied source snapshot for ungrouped tree rows so applications can own server/manual
expansion, child fetches, cancellation, and cache policy. `TableStageMode` lets filtering and
sorting stay client-owned or become manual independently, and `TablePagination` supports the same
manual ownership mode with server-known `row_count` / `page_count` metadata. Manual row-model
stages preserve the caller-supplied snapshot while still keeping exact identity lookup, selection,
grouping, and expansion intact; the table cache key includes those ownership modes and
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
`TableRowPinning` is caller-owned state with ordered top and bottom `TableRowPinTarget` values.
`Exact(TableRowIdentity)` addresses one authoritative source instance or typed group row;
`AllSourceRows(TableRowId)` is the explicitly named bulk operation for every currently resolved
source instance with one business row id. There is no implicit string or business-id conversion to
exact pin state. Caller target order controls each pinned region, bulk matches retain current model
order, and logical identity is deduplicated after target expansion with top taking precedence over
bottom. The default `TableRowPinningPolicy::KeepPinnedRows` resolves targets from the expanded
pre-pagination row model so a pinned row can remain visible while the current page changes;
`PageOnly` limits targets to the current paginated model. Unknown identities, filtered-out rows,
and collapsed descendants are ignored. `TableResolvedState` exposes `TableRowRegions` plus top,
center, and bottom row accessors; the final visual model is top + center + bottom while exact row
lookup and source-instance identity remain stable across pinning changes.

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
body drawing, sortable header activation callbacks, row focus handles, a stable Table-root focus
proxy, and source-tree disclosure affordances for loaded, unloaded, loading, and failed branches,
controlled row activation /
expansion-request payloads, callback-backed column resize handles, and AccessKit mapping. Table
accessibility metadata includes table, row, column-header, and cell roles, row and
column position metadata, sort metadata for sortable headers, grouped-row and source-tree depth /
parent metadata, selected state, and branch `aria-expanded` state keyed by exact logical row
identity. The adapter keeps row activation independent from selection and expansion; callers decide
whether a click, double-click, Enter, Space, Left, or Right payload changes app-owned `TableState`.
Expansion requests never write an adapter-owned override: pointer and keyboard paths keep focus and
emit the next intent, while the rendered branch remains on caller state until a later commit.

Logical Table focus is reconciled against the complete final row model, while only rows in the
rendered virtual window own physical row focus handles. When a logically focused row leaves
overscan, the same claim moves to the stable Table-root proxy: the final AccessKit tree focuses the
Table node, publishes no stale row node or missing-row actions, and real Up, Down, Home, End,
Enter, and Space input continues against exact final-model identities. A remounted row receives
physical focus only while the proxy still owns that claim; focus moved elsewhere is never stolen
back. If the exact identity leaves the complete final model, logical focus falls back to the first
remaining final-model row, or clears when that model is empty. Physical focus and
`TreeUpdate.focus` migrate or clear only while the Table or its proxy still owns focus.

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
`TableState::column_order()` may contain a partial caller-owned order. Its
`normalized_column_order()` projection emits each known explicit id once, then appends every
unlisted source column in source order; unknown and duplicate ids are ignored. Visibility and
pinning are later independent projections, so neither may remove a column from the canonical
source order. `TableColumnOrderChange` is the sibling official column-order recipe: it emits
controlled before/after placement payloads through `Table::on_column_order_change` and its
`apply_to` helper first normalizes the complete source-column order before moving either a listed
or previously unlisted column. The resulting `TableState` stores that full order while sorting,
filtering, visibility, pinning, sizing, and row-model state remain untouched. The Components
gallery uses `release-rollup` as the proof sample for the controlled reorder path; focused core and
component tests characterize partial-order normalization under visibility and pinning.
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
row data app-owned and feed back a changed `TableState`. The edit target is the exact
`(TableRowIdentity, TableColumnId)` pair returned by `identity()` and `column_id()`;
`source_row_id()` is only a business-id readout and is not target authority. Public construction
uses `TableCellEditRequest::new` with one exact `TableSourceRowIdentity`, so a synthetic group row
cannot become an editable source target. `TableCellEditChange` has no public identity-only
constructor: it is emitted from a real resolved row and its `TableRowAction` therefore never
fabricates `model_index`, `source_index`, selection, hierarchy, or modifier metadata.

`TableCellEditRequest::apply_to` and callback-owned `TableCellEditChange::apply_to` return the next state together with
`TableCellEditApplyOutcome::{Updated, RowNotFound, AmbiguousRowId, StaleRowIdentity, CellNotFound}`.
A unique-assumption target against duplicate business ids returns `AmbiguousRowId`; an occurrence
target retained across `with_rows` returns `StaleRowIdentity`. Both are inspectable no-ops that
preserve app data and the current Table cache identity. An exact unique, explicit-instance, or
current-snapshot occurrence target updates only its intended source row while preserving unrelated
row-model inputs such as sorting, filters, pagination, selection, pinning, expansion, faceting,
and sizing. Dynamic row-height measurement, validation, dirty-state tracking, commit/cancel
workflows, clipboard range editing, and server persistence remain application-owned or follow-up
work.
The fixed-option select path uses the same leaf-cell contract as text and checkbox editing: it is
adapter-owned, keeps row activation suppressed when the editor consumes the click, and preserves
the exact `(TableRowIdentity, TableColumnId)` edit payload shape.
For row-pinned tables, `TableBehaviorSnapshot` exposes top, center, and bottom row counts plus
rendered rows with neutral `TableRowRegion` metadata, while the vertical virtualizer consumes only
the center region. The GPUI adapter renders top and bottom row bands outside the center body
scroll surface, keeps `table:{id}:body:{top|center|bottom}` debug selectors stable, and reuses the
normal row renderer so focus, activation, expansion, pinned-column lanes, and accessibility row
indexes keep the same payload shape across pinned and center rows. Combined two-axis viewport
details remain adapter-internal; public tests assert the resulting row/column behavior through
`TableBehaviorSnapshot` and gallery runtime probes instead of inspecting the render plan.

An official Table entry must satisfy the normal component completion gate: `Table` and the real
restoration inputs `TableVirtualizerSnapshot` / `TableVirtualizerSnapshotItem` export at the crate
root, `common` module, and prelude. Behavior readouts such as `TableBehaviorSnapshot` are diagnostic APIs
under `open_gpui_ui_components::table`; component-owned controls such as `TableGlobalFilter` and
`TableToolbar` export at the crate root and owner module. `TableState`, `TableRow`, `TableColumn`,
row-model, virtualizer, and resize math import from `open_gpui_ui_core`, while the production
runtime cache invalidation key remains `open_gpui_ui_core::table::TableStateCacheKey`. The entry
also needs matching
`SIGNALS` entries, a `COMPONENT_CATALOG`
official entry, at least one `gallery:component-table-sample:{id}` rendered selector, state tests
for row identity, grouping, source-tree expansion, row interaction payloads, and virtualizer
behavior, and gallery runtime tests for nested scroll containment, faceted-filter row updates,
predicate-filter row updates, single-line, multiline, and checkbox value-cell updates, and nested
header gallery proof. Table identity gates additionally cover explicit duplicate instances through
every row-model stage and source reorder, occurrence invalidation, ambiguous and stale edit no-ops,
partial column-order normalization, duplicate NodeId and measurement separation, virtual focus
proxy keyboard behavior, no-steal remounting, and first-row or empty-model focus fallback.
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
wires pointer dragging through keyed runtime state. `SplitterMetrics::panel_axis_extent` is the
single placement-aware conversion from root bounds to the panel axis: `BetweenPanels` reserves its
handle hit regions before pixels become fraction deltas, while `OverlayBoundary` keeps the full root
axis. Both `SplitterLayoutScene` and the adapter use that conversion before feeding deltas through
`SplitterState::resized_by`.
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

Resolved `TextInputState` and `TextareaState` values derive an owned
`TextControlSemanticProjection` on demand. That projection owns the policy-filtered value,
placeholder, form-control flags, actions, and optional text selection consumed by GPUI and the
final AccessKit tree; it is never stored as a second component state authority.

The durable authority decision is recorded in
[Semantic accessibility and final-tree authority](../knowledge/engineering/decisions/semantic-accessibility-final-tree-authority.md).

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

`examples/ui-foundation-gallery` is the durable rendered conformance surface for official UI
components. Official Components and Overlay entries receive canonical
`ComponentContractMetadata`, then add Gallery-owned status, presentation grouping, sample
selectors, focused-section ids, runtime probes, and rendered dogfood. Adapter-only helpers,
internal anatomy, and renderer-neutral state-contract rows remain Gallery-local and carry no
official component metadata.

The Components page should keep the contract-backed component catalog visible and distinguish
shipped components from adapter-only helpers, internal anatomy, state contracts, and deferred
entries. Its root module is a small facade: catalog view-model metadata lives in
`components/catalog.rs`; `components/samples.rs`,
`components/runtime.rs`, and `components/render.rs` are private parent facades over explicit
family-owned modules. Sample descriptors and static sample data live under
`components/samples/`; Tree, Table, and VirtualizedList runtime probes live under
`components/runtime/`; page orchestration, section dispatch, readouts, focus controls, and shared
card helpers live under `components/render/`. `components.rs` must not expose `runtime` or
`samples` as public modules, and it must not use wildcard facade exports such as
`pub use runtime::*` or `pub use samples::*`; only stable catalog/story metadata, sample accessors,
and runtime probe names are re-exported explicitly. The page has two supported inspection modes:
the full all-components conformance page, and a focused component-family view entered from
official catalog cards. Focused
mode may hide unrelated sections, but it must keep the section directory available, expose an
explicit `All components` control, reset the page viewport when the family changes, and keep nested
sample scrolling local to the sample viewport. Directory chips remain anchor jumps inside the
current page mode; they must not implicitly change the focused family. The executable gates are:

- crate-root, common, prelude, and diagnostic exports stay explicit through typed declarations;
- `official_components_match_typed_public_exports` checks the product/export join;
- `gallery_contract_metadata_matches_component_rows` checks 40 Components plus 8 Overlay entries,
  their rendered `name`/`family`, Story metadata, and Gallery-local row isolation;
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
- exact native test ownership is declared by sibling `*.scenarios.toml` artifacts and executed by
  `scan-ui-contract`;
- all-components and focused component-family modes preserve the catalog, section directory, page
  scroll reset, and nested scroll containment contracts;
- the gallery navigation rail and page viewport stay independently scrollable on compact windows;
- ScrollArea redraws preserve the default keyed runtime handle;
- Table and virtualizer samples keep long table scrolling inside the table viewport;
- Splitter runtime fractions continue to share one constraint solver;
- Tabs keep overflow and roving-focus behavior visible in the page;
- icon-only affordances and labels keep their accessible metadata explicit;
- final `TreeUpdate` and real AccessKit action tests own evidence for semantic producers; static
  accessibility evidence rows, Gallery claims, and their consumers are not parallel authorities;
- `cargo run --locked -p xtask -- scan-ui-contract` joins narrow contract rows, typed export facts,
  the docs projection, Gallery canonical metadata, and test-owned scenario artifacts, then executes
  every registered exact test coordinate.

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
ephemeral semantic projection with final-tree/action evidence, removed registry history, shared
overlay placement, shared row-window projection, gallery story contracts, and app-command
descriptor projection are now the active
architecture boundary. Broad remaining-1k-line component splitting and `open-gpui-ui-headless`
extraction are not part of that slice. The runtime theme table now covers semantic component
colors for light, dark, high-contrast, registry-loaded snapshots, and JSON-loaded
`ThemeDefinition` values; UIs should load or construct definitions, install the app registry,
select app or window fallback authority explicitly, and let component adapters resolve colors from
`ThemeResolver::current(window, cx)`. Subtree overrides use `ThemeScope`, while official deferred
overlays and delayed tooltip builders freeze the opening context rather than rereading ambient app
state later.
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
`TextInputController`, `FieldControl`, `FieldControlSemantics`, externally supplied `ScrollHandle`,
`focus_ring_shadow_with_theme`, the adapter geometry conversion helpers, and GPUI overlay scheduling
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
the box, indicator, label, and focus ring. `Label` derives its visible-text semantics from
`LabelState`; `Field` exclusively owns control `labelled_by`, `described_by`, and error-message
relations from its rendered composition. `FieldState` no longer carries a logical control id that
can drift from that rendered composition. `Tabs` now keeps the roving-focus
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
`Table` covers exact typed row identities, row-model ordering, grouping, expansion, built-in
group-row aggregate cells, source-tree branches with manual expansion and child-load metadata,
pinned left/center/right column regions, runtime column visibility overrides, locked column hideability,
manual filtering/sorting/pagination modes with pagination totals, committed column sizing state,
clamped width resolution with region totals/offsets, row pinning with top/center/bottom regions,
sortable header action payloads, crate-root/common/prelude
exports, table/cell roles, and a vertically virtualized GPUI recipe whose body scroll stays inside
the table viewport.
For pinned samples, the adapter renders fixed left/right lanes plus a shared horizontal center lane
backed by crate-private center-column windowing, so off-window center headers and cells are unmounted while
spacer geometry preserves the full scrollable width. It also ships GPUI resize handles with
controlled commit callbacks and on-end/on-change resize mode support.
For row-pinned samples, top and bottom row bands render outside the center vertical scroll surface,
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
`StatusCue` and `EmptyState` are official feedback components. `StatusCue` exposes resolved
feedback intent, size, role, live priority, atomicity, busy state, metrics, and token intents,
while the GPUI adapter owns concrete styling and rendered debug selectors. It maps ordinary
intents to `Role::Status` and danger to `Role::Alert`; callers may opt a static sample out with
`LivePoliteness::Off`. `EmptyState` exposes its structural `Role::Section` plus feedback intent,
size, metrics, and token intents; it is not a live region, so a page that needs an announcement
uses `StatusCue` or an explicit window announcement. `Tree` is now an official rendered component
backed by `TreeState`.
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
focusable row list. The private collection typeahead session owns printable-input normalization,
the buffer, and the 700ms executor-clock deadline; the GPUI adapter supplies the event-time stable
focused value and moves focus without selecting the matched row. Typeahead intentionally does not
search collapsed, unloaded, or virtualized descendants.
Tree keeps business values as callback identity, but assigns a private collision-safe render identity
to every visible row for GPUI elements, debug selectors, and accessibility nodes. Ambiguous duplicate
business values are non-focusable; callers cannot use render identity as a value or persistence key.
For a virtual keyboard target, the adapter submits the stable `FocusHandle` claim at input time,
re-resolves the current unique focusable value on the next frame, and materializes only while that
claim remains the latest window focus revision. The physical materialization commit runs in
Window's focus-stable prepaint phase after normal commit callbacks; focus and blur mutations are
rejected within that terminal phase. Its retained `ScrollChainFence` spans first materialization
and guarded focus reveal, so direct ancestor scrolling or a chain/axis change cancels before it can
move the virtual viewport. A later focus request, removal, ambiguity, or disabled state has the
same result.
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
selection, and stable-key materialization results. The private collection typeahead session owns
the printable-input buffer and deadline. The GPUI adapter supplies the event-time stable active key
and owns only the logical materialization phase: typeahead moves the active row without selecting
it, while Shift-range interaction replaces selected keys with the current selectable
anchor-to-target range. `VirtualizedList::bring_key_into_view` materializes the stable logical row,
binds its physical `RevealTargetHandle`, and delegates final nested alignment to the window
authority; it does not compute a final container-relative reveal offset. When materialization and
physical submission span frames, the adapter captures `DeferredBringIntoViewGuard` from prepaint
inside the intended final scroll ancestry as soon as logical materialization completes, then
consumes that guard through the guarded window method after the target binds. The guard validates
the target, its complete committed scroll ancestry, and direct-scroll revisions for the requested
axes atomically, so wheel, scrollbar, keyboard, touch, or explicit direct scrolling on any affected
ancestor cancels the operation rather than becoming a new baseline. If geometry
changes after a physical request is in flight, a retry waits for its exact terminal outcome and is
allowed only after completion; every cancellation, including `Superseded`, `ScrollOverridden`,
`TargetUnlinked`, `AncestryChanged`, `TargetSuppressed`, `NoProgress`, and `WindowClosed`, ends
the stale operation. The guard's retained `ScrollChainFence` also compares the complete ordered
chain's available axes, so a reconfigured scrollport cannot turn an unobserved axis into a reveal
path.
`VirtualizedListBehaviorSnapshot::sticky_section` returns an optional
`VirtualizedListStickySectionSnapshot` for the section row that owns the first visible selectable
row. `VirtualizedListBehaviorSnapshot::sticky_overlay` returns an optional
`VirtualizedListStickyOverlaySnapshot` for the presentation-only sticky header layer: the overlay is
positioned against the viewport, while the underlying section row remains the semantic owner. It
does not add a second interactive section row or change focus order, hit testing, selection, or
accessibility roles. `VirtualizedListRowMeasureMode`
keeps fixed rows as the default hot path and exposes measured rows as an explicit opt-in;
`VirtualizedList::virtualizer_snapshot` seeds measured heights by stable render key, removed keys
are dropped from emitted snapshots, and missing measurements fall back to the estimated row height
for materialization only. `VirtualizedListMaterializationResult` rejects missing, duplicate,
disabled, structural, and status-row keys; its target index and estimated flag are mounting facts,
not final reveal geometry. Render keys are opaque adapter identities for one ordered descriptor
snapshot: unique business identity remains `VirtualizedListItemDescriptor::key()`, while duplicate
source keys receive a collision-safe occurrence encoding. Consumers may round-trip render keys for
measurement but must not parse, construct, or persist their representation across source reorder.
`VirtualizedListItemDescriptor` is the typed row descriptor: item
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
After these family splits, `cargo run -p xtask -- scan-ui-contract` joins component metadata,
Gallery ownership, public export facts, docs projection, and native scenario registrations.
`scan-theme-schema` and `scan-theme-drift` separately audit theme schema/loading and consumer
coverage. Semantic behavior is proven at the final `TreeUpdate` and real AccessKit action boundary.
The remaining large component files should stay intact unless one of those product contracts
exposes a concrete ownership problem.
`Splitter` covers panel fraction normalization, min/max constraints, collapsed-panel metadata,
stable handle anatomy, and local pointer dragging through keyed runtime state. Keyboard resizing,
controlled resize callbacks, persisted layouts, RTL behavior, and nested splitter arbitration
remain follow-up work.
