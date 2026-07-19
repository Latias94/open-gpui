# UI Interface Follow-on Research

**Date:** 2026-07-18
**Scope:** Public substrate candidates after U12 Interactive Subtree Transform and U13
Presentation State
**Evidence policy:** Repository source, checked-in reference source, W3C specifications,
AccessKit source, and first-party framework documentation only

## Executive Decision

Open GPUI should add a live-region and announcement authority immediately after U13. It is
the smallest remaining accessibility gap with direct product demand, complete AccessKit
support, and no renderer dependency.

Three other capabilities have enough evidence for implementation units, but should not be
folded into U12 or U13:

1. A typed, window-owned portal anchor backed by current-frame and committed geometry.
2. A window-owned bring-into-view authority used by focus, accessibility actions, and
   application reveal requests.
3. A rounded-rectangle subtree clip, after a short renderer/ABI design gate.

Locale and logical direction are ultimately required for a general-purpose UI framework, but
they need a dedicated cross-cutting design unit before implementation. Arbitrary path clipping,
true group opacity/compositing, and a multi-pointer gesture arena should remain research-only
until their missing substrate is designed.

| Candidate | Decision | Proposed plan placement | Primary reason |
| --- | --- | --- | --- |
| Live region and announcement | Implement | U14, immediately after U13 | AccessKit already carries and emits the required semantics; GPUI currently drops them |
| Typed portal anchor | Implement | U15 | U12 makes raw point/bounds snapshots an avoidable coordinate-space hazard |
| Unified bring-into-view/focus reveal | Implement | U16, after typed anchors | Current reveal code is container-specific and cannot traverse committed nested scroll ancestry |
| Rounded-rectangle subtree clip | Implement after a focused design gate | U17 | Existing shaders already evaluate rounded-rect SDFs, but the clip ABI and hit-test stack are rectangular |
| Locale and logical direction | Research, then implement as a dedicated epic | After the current convergence plan | It changes layout, text, overlays, navigation, icons, IME, and accessibility together |
| Arbitrary path subtree clip | Research only | Revisit after rounded clipping ships | Winding, antialiasing, cache, stencil/tessellation, and native-surface policy are unresolved |
| Group opacity/compositing | Research only; correct the current API description now | Separate renderer epic | Current opacity is per-primitive alpha multiplication, not isolated group compositing |
| Multi-pointer gesture arena | Research only | After a pointer-input substrate unit | Current public input and capture are mouse/button based and lack pointer identity |

This ordering is intentionally conservative about public syntax and aggressive about authority.
No candidate should ship as a visual-only helper or a public placeholder that a later unit must
reinterpret.

## Repository Baseline

The current repository establishes several useful boundaries:

- GPUI constructs one AccessKit `TreeUpdate` per active accessibility frame and relies on stable
  `GlobalElementId`-derived node IDs across frames. See the [accessibility architecture](../../crates/gpui/src/window/a11y.rs#L1)
  and [public accessibility guide](../../crates/gpui/src/_accessibility.rs#L52).
- `InteractivityAccessibility` already projects labels, values, `busy`, hidden state, relations,
  and actions, but has no live-region, language, or text-direction field. See
  [the current projection record](../../crates/gpui/src/elements/div/accessibility.rs#L33).
- `ui_core::SemanticDescriptor` is the renderer-neutral component projection authority, so a
  component-level live contract belongs there rather than in individual widgets. See
  [SemanticDescriptor](../../crates/ui_core/src/a11y.rs#L239) and the
  [GPUI adapter](../../crates/ui_components/src/a11y.rs#L494).
- U12 introduces opaque committed `ElementGeometry` and `MeasuredElementSnapshot` values, but a
  callback snapshot is not a stable target/follower link. See [ElementGeometry](../../crates/gpui/src/geometry.rs#L3918)
  and [measured_element](../../crates/gpui/src/elements/measured.rs#L12).
- `window_portal` deliberately resets inherited geometry, while its caller supplies a projected
  raw point. See [window_portal](../../crates/gpui/src/elements/deferred.rs#L17) and
  [defer_draw_in_window_space](../../crates/gpui/src/window.rs#L4704).
- `OverlayAnchorInput` is correctly renderer-neutral, but its upstream values are untyped
  point/layout/visual rectangles with no window or frame generation. See
  [OverlayAnchorInput](../../crates/ui_core/src/overlay.rs#L603).
- Reveal behavior is split among `ScrollAnchor`, `ScrollHandle`, list-specific methods, and
  component-private row geometry helpers. See [ScrollAnchor and ScrollHandle](../../crates/gpui/src/elements/div.rs#L4387),
  [ListHandle::scroll_to_reveal_item](../../crates/gpui/src/elements/list.rs#L626), and
  [component row reveal](../../crates/ui_components/src/scroll_surface.rs#L156).
- `ContentMask` explicitly supports only rectangles, and every renderer encodes a rectangle in
  each primitive. See [ContentMask](../../crates/gpui/src/window.rs#L1924) and the
  [WGPU clip implementation](../../crates/gpui_wgpu/src/shaders.wgsl#L194).
- `opacity` is documented as affecting an element and its children, but implementation multiplies
  a scalar into each descendant primitive color. See [the public style method](../../crates/gpui/src/styled.rs#L737)
  and [with_element_opacity](../../crates/gpui/src/window.rs#L4119).
- Platform input remains a mouse-event vocabulary. `TouchPhase` describes wheel and platform
  pinch phases, while capture is one window session keyed by `MouseButton`. See
  [PlatformInput](../../crates/gpui/src/interactive.rs#L680) and
  [PointerCapture](../../crates/gpui/src/window/pointer_session.rs#L31).

These facts make live regions immediately implementable, typed anchors and reveal natural U12
follow-ons, rounded clipping bounded but cross-backend, and the remaining candidates unsuitable
for opportunistic APIs.

## U14: Committed Live Regions and Announcements

### Why this should be implemented

WAI-ARIA defines `aria-live` as the ordering/interruptibility hint for changes, `aria-atomic` as
the whole-region versus changed-content hint, and `aria-busy` as the batching boundary for an
update that is not ready to announce. `polite` normally waits for a suitable opportunity, while
`assertive` may interrupt current output. See the normative definitions of
[`aria-live`](https://www.w3.org/TR/wai-aria-1.2/#aria-live),
[`aria-atomic`](https://www.w3.org/TR/wai-aria-1.2/#aria-atomic), and
[`aria-busy`](https://www.w3.org/TR/wai-aria-1.2/#aria-busy).

The [`status`](https://www.w3.org/TR/wai-aria-1.2/#status) role is implicitly polite and atomic;
the [`alert`](https://www.w3.org/TR/wai-aria-1.2/#alert) role is implicitly assertive and atomic.
Neither role requires moving focus to announce its content. This matches the framework's existing
focus-authority direction: announcements must never be implemented as synthetic focus changes.

AccessKit already provides `Live::{Off, Polite, Assertive}`, `live_atomic`, `busy`, `language`,
and `text_direction` in its node schema. See the checked-in AccessKit source for
[`Live` and `TextDirection`](https://github.com/AccessKit/accesskit/blob/e1f63acbb2c36e3cae741871300aeb121c9e6274/common/src/lib.rs#L439-L590)
and [the node properties](https://github.com/AccessKit/accesskit/blob/e1f63acbb2c36e3cae741871300aeb121c9e6274/common/src/lib.rs#L1800-L2141).
The platform adapters consume live-node diffs rather than requiring GPUI to call native speech
APIs directly:

- Windows emits `UIA_LiveRegionChangedEventId` when an included live node is added or its name or
  politeness changes. See the [Windows change handler](https://github.com/AccessKit/accesskit/blob/e1f63acbb2c36e3cae741871300aeb121c9e6274/platforms/windows/src/adapter.rs#L239-L303).
- macOS converts live node value changes into an accessibility announcement with medium or high
  priority. See the [macOS event generator](https://github.com/AccessKit/accesskit/blob/e1f63acbb2c36e3cae741871300aeb121c9e6274/platforms/macos/src/event.rs#L228-L305).
- AT-SPI maps live politeness and emits announcement events when the accessible name changes. See
  the [AT-SPI node adapter](https://github.com/AccessKit/accesskit/blob/e1f63acbb2c36e3cae741871300aeb121c9e6274/platforms/atspi-common/src/node.rs#L545-L558).

AccessKit's own Winit example implements an imperative announcement by adding a live `Role::Label`
node with a stable root relationship; it does not invoke a platform speech service. See
[`build_announcement` and its queue](https://github.com/AccessKit/accesskit/blob/e1f63acbb2c36e3cae741871300aeb121c9e6274/platforms/winit/examples/simple.rs#L78-L181).
The checked-in Fret reference independently uses renderer-neutral live flags and maps them into
AccessKit, including an exact mapping test. See [Fret semantics](../../repo-ref/fret/crates/fret-core/src/semantics.rs#L107)
and [its AccessKit mapping test](../../repo-ref/fret/crates/fret-a11y-accesskit/src/tests.rs#L947).

Flutter's direct announcement API is useful counter-evidence: its old global API is deprecated for
multi-window incompatibility, and its documentation recommends semantic-tree changes where the
platform can announce them naturally. See
[`SemanticsService.announce`](https://api.flutter.dev/flutter/semantics/SemanticsService/announce.html)
and [`SemanticsProperties.liveRegion`](https://api.flutter.dev/flutter/semantics/SemanticsProperties/liveRegion.html).

### Authority and public surface

U14 should have one authority with two entry paths:

1. Declarative live-region facts flow through `ui_core::SemanticDescriptor`, the existing
   `ui_components` adapter, GPUI element projection, and the final AccessKit tree.
2. A transient announcement request enters a window-owned queue, which synthesizes a semantic
   live node into that same final tree. It must not call a platform-specific announcement API.

Illustrative public vocabulary:

```rust
pub enum LivePoliteness {
    Off,
    Polite,
    Assertive,
}

SemanticDescriptor::with_live(LivePoliteness)
SemanticDescriptor::with_live_atomic(bool)

StatefulInteractiveElement::aria_live(accesskit::Live)
StatefulInteractiveElement::aria_live_atomic(bool)

window.announce(AccessibilityAnnouncement::polite(message), cx)
window.announce(AccessibilityAnnouncement::assertive(message), cx)
```

The exact names may follow repository conventions, but the ownership should not change:

- `ui_core` owns renderer-neutral `LivePoliteness`, `Role::Status`, `Role::Alert`, and descriptor
  fields. It must not depend on GPUI or AccessKit.
- `ui_components` owns descriptor-to-GPUI mapping and optional `StatusRegion`/`AlertRegion`
  components. Components do not own queues or timers.
- `open-gpui` owns the window queue, synthetic node IDs, frame integration, and final AccessKit
  projection.
- Native adapters remain AccessKit's responsibility.

`aria-relevant` should not be exposed in U14. The current AccessKit schema has no equivalent
property, so a GPUI API could not make a portable promise. U14 also must not promise that a user
heard a message; assistive technology owns its speech queue and may suppress or reorder output.

### Lifecycle contract

Declarative regions and transient requests need explicit, different lifetimes while sharing the
same tree authority:

- A declarative region exists only when its node is present in a successfully committed final
  accessibility tree. U12 transform rollback and U13 `Inert`/`Hidden` exclusion suppress it with
  the rest of the subtree.
- A transient request is window-scoped. It is accepted only while that window's accessibility
  generation is active and the window is not closing. Requests made while inactive are dropped,
  not replayed when accessibility later activates.
- Every accepted transient request receives a monotonically increasing per-window sequence used
  for semantic node identity. Repeating the same text must therefore still produce a meaningful
  node change.
- A transient node remains in at least one complete committed accessibility generation and is
  removed by a later committed generation. Removal must be scheduled without retaining message
  history indefinitely.
- Multiple requests in one turn preserve call order within a bounded queue. The implementation
  may coalesce only by an explicit public channel/key contract, never by equal message text.
- An assertive request changes politeness only. It does not cancel GPUI focus work, mutate domain
  state, or claim guaranteed interruption.
- Window close, accessibility deactivation, or replacement activation generation clears pending
  and retained transient nodes.
- Production diagnostics may record ID, sequence, politeness, accepted/dropped state, and window
  identity, but must not persist announcement text in DevTools history, export, or logs. The final
  tree test harness may inspect text for correctness.

For persistent live regions, initial non-empty content and `Hidden`/`Inert` to `Visible` re-entry
can be observed by AccessKit as node addition and may therefore announce. U14 should document and
test that fact rather than pretending browser-specific initial-content suppression is portable.
Applications that do not want an initial announcement should create an empty stable region first,
then update its value.

### U14 test matrix

| Layer | Required evidence |
| --- | --- |
| `ui_core` | `Status` and `Alert` roles; polite/assertive/off; atomic and busy descriptor values; no AccessKit dependency |
| GPUI projection | Exact final `TreeUpdate` fields for role, value/label, live, atomic, and busy |
| Stable region lifecycle | Value change, busy true/content changes/busy false, unmount, cache replay, deferred content, same stable node identity |
| Transient queue | Same-text repeat, multiple ordered messages, bounded overflow policy, one-generation retention, deterministic removal |
| Presentation/transform | U12 failed scope publishes no declarative node; U13 inert/hidden removes it; visible re-entry behavior is explicit |
| Focus | Status and alert changes do not move focus; transient announcements do not create tab stops or actions |
| Window ownership | Two windows isolate queues, node IDs, activation generations, close, and deactivation |
| Activation | Inactive requests are dropped and are not replayed after activation; stale activation generations cannot publish |
| Privacy | Unique message canary is absent from production diagnostics/history/export while present in the captured test `TreeUpdate` |
| Platform gate | Windows/macOS/Linux adapter versions compile; native smoke verifies event generation where the owning runner can observe it |

### U14 acceptance boundary

U14 is complete only when official status/error/toast-like first-party consumers use the semantic
descriptor or the window announcement queue, final-tree tests prove their behavior, and there is no
component-owned hidden label/timer used as a competing announcement mechanism.

## U15: Typed Committed Portal Anchors

### Why this should be implemented

U12 correctly distinguishes layout bounds from displayed window bounds and adds checked projection
helpers. A raw `Point<Pixels>` or `Bounds<Pixels>` passed between a trigger and a portal can still
lose its coordinate space, window, generation, presentation state, or validity. The current
`OverlayAnchorInput` should remain a pure placement value; the missing layer is a GPUI lifecycle
binding that produces it safely.

Flutter uses a shared `LayerLink`: a target updates the link during the same compositing frame, one
target may have multiple followers, the target must precede followers in paint order, and the
unlinked state is explicit. See
[`CompositedTransformTarget`](https://api.flutter.dev/flutter/widgets/CompositedTransformTarget-class.html)
and [`FollowerLayer`](https://api.flutter.dev/flutter/rendering/FollowerLayer-class.html).

### Authority and public surface

Prefer a narrow `PortalAnchorHandle` over a generic public DOM-like node reference. GPUI may share
private machinery with later reveal targets, but the public capability should say what it permits.

```rust
let anchor = window.new_portal_anchor();

div()
    .track_portal_anchor(&anchor)
    .child(trigger)

match window.resolve_portal_anchor(&anchor) {
    Ok(Some(snapshot)) => { /* convert snapshot.geometry() into OverlayAnchorInput */ }
    Ok(None) => { /* explicit unlinked policy */ }
    Err(error) => { /* wrong window or invalid use */ }
}
```

`PortalAnchorSnapshot` should contain the window ID, frame generation, opaque `ElementGeometry`,
effective presentation state, and effective clip AABB. It should not expose the resolved transform
matrix. `ui_components` converts the snapshot to `OverlayAnchorInput`; `ui_core` remains unaware of
GPUI handles and windows.

### Lifecycle contract

- A handle belongs to exactly one window and may bind to exactly one target per frame.
- Binding occurs during target prepaint. A follower later in the same frame can use the validated
  candidate; external reads see only the last committed snapshot.
- Multiple followers may read one target.
- If the target is not bound in a frame, the committed state becomes unlinked. Last-known geometry
  must not silently remain authoritative.
- The snapshot retains `Visible`/`Inert`/`Hidden` as data. Overlay adapters require `Visible`, while
  non-interactive visual consumers may define a different explicit policy.
- A failed U12 transform candidate never commits. Cache replay must reproject under the current
  ancestor transform before publishing.
- Crossing windows is an error. A native cross-window portal is a separate future capability.
- Followers explicitly choose `CloseWhenUnlinked`, `HideWhenUnlinked`, or another controlled
  policy. The geometry handle itself does not mutate overlay state.
- Ordinary deferred descendants inherit geometry and clipping. A named window-space portal resets
  coordinate and clip ancestry deliberately, then consumes the already projected anchor snapshot.

### U15 test matrix

Test same-frame target/follower ordering, duplicate binding, one target with multiple followers,
unmount/rebind, wrong window, U12 nested transforms and numeric failure, U13 transitions, scrolling,
deferred and cached target replay, explicit portal reset, clip AABB, controlled overlay close, and
absence of stale geometry after a failed frame.

## U16: Bring Into View as the Reveal Authority

### Why this should be implemented

Current Open GPUI reveal APIs resolve one container or one list model at a time. They do not have a
committed ancestor chain, cannot move through multiple nested scrollports, and leave focus,
AccessKit `ScrollIntoView`, and component keyboard navigation to call different tails.

Flutter's `Scrollable.ensureVisible` walks all enclosing scrollables and supports two-dimensional
containers. See the [official implementation](https://api.flutter.dev/flutter/widgets/Scrollable/ensureVisible.html).
CSSOM View similarly defines block/inline `start`, `center`, `end`, and `nearest` alignment against
scrolling boxes. See [`scrollIntoView`](https://www.w3.org/TR/cssom-view/#dom-element-scrollintoview).

### Authority and public surface

The primitive should be named for the operation, not its first consumer. Focus uses bring-into-view;
focus does not own scrolling.

```rust
pub struct BringIntoViewOptions {
    pub block: RevealAlignment,
    pub inline: RevealAlignment,
    pub margin: Edges<Pixels>,
    pub behavior: RevealBehavior,
}

window.request_bring_into_view(&target, options, cx)
```

A narrow `RevealTargetHandle` records committed geometry plus the ordered scroll-container ancestry
needed for this operation. It may share private binding infrastructure with `PortalAnchorHandle`;
neither should become a generic public node handle.

GPUI owns request generations, committed geometry conversion, cancellation, and inner-to-outer
application. `ui_components` owns virtual collection materialization and maps component alignment
to the GPUI request. `ui_core` may own renderer-neutral alignment enums but no window lifecycle.
Motion owns timing samples; theme only supplies reduced-motion policy.

### Lifecycle contract

- A request targets a stable handle and receives a generation. A newer request for the same target,
  target unmount, U13 suppression, window close, or direct user scroll cancels the older request.
- Processing starts from committed target and scrollport geometry, applies the innermost container,
  commits, then continues outward until visible or no progress is possible.
- Each delta is converted through the container's opaque geometry helpers, so non-uniform U12
  transforms do not turn window-space deltas into incorrect local scroll offsets.
- `Nearest` is the default. Both axes are explicit; a vertical-only helper must not accidentally
  move horizontal position.
- Focus requests reveal after the focus claim wins end-of-turn arbitration. A losing or stale focus
  claim does not scroll.
- AccessKit `ScrollIntoView` dispatches the same request.
- Virtual collections use two phases: resolve/materialize the logical item by stable identity, then
  bind and reveal its committed physical target. The substrate does not guess virtual indices.
- Animated behavior respects reduced-motion policy and has deterministic fake-clock tests. The
  instant path remains available and does not depend on Motion.
- An explicit portal boundary starts a new rendered scroll ancestry. Following an anchor back to a
  source scroll container requires an explicit application policy, not implicit tree guessing.

### U16 test matrix

Cover nested vertical scrollports, mixed horizontal/vertical scrollports, nearest/start/center/end,
oversized targets, margins, non-uniform transforms, cached/deferred targets, portal boundaries,
focus arbitration, AccessKit action dispatch, U13 suppression, user-scroll cancellation,
reduced-motion completion, virtual materialization, wrong-window targets, and deterministic no-
progress termination.

## U17: Rounded-Rectangle Subtree Clip

### Split rounded rectangles from arbitrary paths

The W3C clipping contract is the right semantic baseline: clipping affects an element and its
descendants, nested clips intersect, layout geometry is unchanged, and clipped-out regions do not
receive pointer events. See [CSS Masking, section 5](https://www.w3.org/TR/css-masking-1/#clipping-paths).

Open GPUI already has rounded-rectangle SDF code for quads and polychrome sprites in all three
renderers, while every primitive's content mask remains a rectangle. Flutter similarly gives
rounded rectangles a specialized clip and warns that arbitrary path clipping is more expensive.
See [`ClipPath`](https://api.flutter.dev/flutter/widgets/ClipPath-class.html).

That evidence supports a bounded rounded-rectangle implementation, not a public path placeholder.

### Authority and public surface

Replace the internal rectangle-only clip stack with one checked resolved authority:

```rust
pub enum SubtreeClip {
    Rect(Bounds<Pixels>),
    RoundedRect {
        bounds: Bounds<Pixels>,
        radii: Corners<Size<Pixels>>,
    },
}
```

The exact representation must normalize radii, reject non-finite/negative values, and preserve
elliptical radii under non-uniform U12 scale. Existing `ContentMask` and `overflow` paths should
become inputs to the same stack rather than parallel clip authorities.

GPUI owns the frame-local clip stack, hit-test containment, debug shape, deferred/cache journal,
and conservative accessibility projection. Renderers receive only validated resolved clip data.
`ui_core` and `ui_components` do not own renderer clip shapes.

### Lifecycle and channel contract

- Clip is post-layout and does not change Taffy measurement or sibling flow.
- Paint and initial hit testing use the same nested intersection. Pointer capture, once validly
  acquired, continues according to capture semantics even after the pointer leaves the clip.
- Rect/rounded intersections may remain a stack; they must not be replaced by an incorrect single
  bounding rectangle.
- Ordinary deferred and cached descendants inherit the clip. A named window portal deliberately
  resets it.
- U12 transforms apply before window-space clip evaluation. Invalid transform/clip composition
  fails closed for the complete subtree transaction.
- Accessibility cannot express rounded hit regions. Fully clipped nodes are omitted; partially
  clipped node bounds use a conservative AABB and the clip owner exposes AccessKit's
  `clips_children` fact. Documentation must state the platform limitation.
- Native surfaces require an explicit policy. A backend that cannot clip a native surface must
  reject or isolate that combination, not silently paint outside the clip.

### U17 test matrix

Cover normalized and asymmetric radii, nested rect/rounded intersections, points just inside and
outside every corner, non-uniform scale, scrollports, all scene primitive families, text and
surfaces, hover/click/drag/drop, capture, deferred/cache replay, portal reset, U13 presentation,
debug geometry, conservative AccessKit bounds, renderer ABI tests, and backend pixel smokes.

### Why arbitrary path clipping remains research-only

Before a path API exists, a separate design must settle fill rule/winding, path ownership and
mutation, tessellation versus stencil, antialiasing, transformed point containment, nested clip
complexity, cache keys, native surfaces, and memory/performance limits. `SubtreeClip` should remain
non-exhaustive internally or be broken intentionally later; it should not publish a `Path` variant
whose contract is unknown.

## Locale and Logical Direction: Research, Then Implement

This is not optional for a credible general-purpose framework, but it is too cross-cutting for an
incremental flag. Current GPUI exposes physical left/right edges, maps overlay `Start` to physical
left, and has no inherited locale or layout-direction state. AccessKit can carry BCP 47 language
and text direction, but accessibility metadata alone cannot repair layout or shaping.

Flutter models direction as an inherited subtree value; direction-sensitive values such as
`EdgeInsetsDirectional` resolve against it. See
[`Directionality`](https://api.flutter.dev/flutter/widgets/Directionality-class.html). Flutter's
`TextDirection` design also explains why low-level APIs should not silently default direction and
why physical and logical edge types remain distinct. See
[`TextDirection`](https://api.flutter.dev/flutter/dart-ui/TextDirection.html).

A dedicated design epic should define:

- `LocaleContext` and `LayoutDirection` as separate facts. Locale may supply a default direction;
  a subtree may override direction without lying about language.
- App/OS fallback, window override, and subtree override precedence, with window-owned observation
  and frame-scoped inherited resolution.
- A new logical edge/alignment vocabulary (`start`/`end`) alongside existing physical edges. Do
  not reinterpret existing `left`/`right` APIs.
- Direction-aware overlay placement, keyboard navigation, icon mirroring policy, text alignment,
  shaping/bidi, selection, IME, and AccessKit language/text-direction projection.
- Portal/deferred/cache capture rules and OS locale-change lifecycle.
- Explicit deferral of vertical writing modes unless text and layout backends can support them
  together.

The design gate must inventory every current `Start`/`End`, left/right keyboard action, physical
edge style, placement solver, and text shaping backend before selecting public types. Its test
matrix must include nested LTR/RTL overrides, mixed bidi text, numbers inside RTL text, logical
padding/margin/alignment, overlays, arrow navigation, selection/IME, BCP 47 validation, OS changes,
accessibility projection, transforms, portals, deferred work, and cache replay.

No `rtl: bool`, `locale: String`, or isolated `aria_text_direction` scope should be added as a
substitute for this design. Explicit node-level accessibility language/direction fields may be
added only as honestly named semantic metadata, not presented as layout-direction support.

## Group Opacity and Compositing: Research Only

CSS group opacity is a post-processing operation: the element and descendants are rendered into
one offscreen image, then that image is blended into the scene. Overlapping descendants therefore
do not accumulate alpha independently. See
[CSS Color 4 opacity](https://www.w3.org/TR/css-color-4/#transparency). Flutter describes the same
intermediate-buffer cost and notes that zero opacity does not disable descendant hit testing. See
[`Opacity`](https://api.flutter.dev/flutter/widgets/Opacity-class.html).

Open GPUI's current `opacity` multiplies alpha into each primitive. That is a useful non-isolating
alpha modulation operation, but it is not group opacity when descendants overlap. The immediate
correctness action is to rename or precisely document the existing API as an alpha multiplier and
add an overlap characterization test. Do not claim CSS/group semantics.

A future compositing design must decide offscreen target allocation, bounds inflation, isolation,
blend modes, nested groups, cache/damage invalidation, transformed/clipped groups, HDR/color space,
native surfaces, GPU memory budget, fallback/failure behavior, and backend parity. Its interaction
contract should remain simple: opacity does not change layout, hit testing, focus, IME, or
accessibility; U13 controls participation. `opacity == 0` is not an alias for `Hidden` or `Inert`.

Until those decisions and overlap pixel tests exist, a public `group_opacity`, blend mode, or
generic compositing layer would be premature.

## Multi-pointer Gesture Arena: Research Only

Pointer Events requires stable per-contact identity, device type, primary-pointer state,
per-pointer capture, cancellation, and capture release. See
[Pointer Events 3](https://www.w3.org/TR/pointerevents3/). Flutter's gesture arena arbitrates a
sequence by pointer ID; recognizers join, accept/reject, hold, release, and sweep the arena. See
[`GestureArenaManager`](https://api.flutter.dev/flutter/gestures/GestureArenaManager-class.html).

Open GPUI currently receives mouse down/up/move, platform-resolved pinch, and phased wheel input.
Its strong U12 capture work is still one mouse/button session, not a multi-contact substrate. The
checked-in egui reference demonstrates the missing minimum: device ID, stable touch ID, start/move/
end/cancel, active-contact storage, and derived multi-touch deltas. See
[egui touch events](https://github.com/emilk/egui/blob/68b74530b7848cef6bff4efc5fc9906bfbd1e8ca/crates/egui/src/data/input/event.rs#L120-L146)
and [touch-state tracking](https://github.com/emilk/egui/blob/68b74530b7848cef6bff4efc5fc9906bfbd1e8ca/crates/egui/src/input_state/touch_state.rs#L70-L170).

The prerequisite implementation unit is a unified pointer substrate, not an arena API:

- stable `PointerId` and `PointerDeviceKind` for mouse, touch, and pen;
- down/move/up/cancel with pressure, buttons, primary state, and coalescing policy;
- per-pointer capture and deterministic lost-capture events;
- native backend parity and a deterministic multi-contact test injector;
- compatibility mapping for existing mouse listeners and drag/drop;
- interaction with nested scroll, transformed coordinates, U13 suppression, window lifecycle, and
  platform pinch/rotate events.

Only after that substrate ships should a recognizer/arena design choose ownership, teams,
accept/reject timing, cancellation, capture transfer, nested scroll-versus-drag arbitration,
long-press thresholds, and reduced-motion-independent timing. Publishing recognizers before pointer
identity would freeze the wrong event model.

## Proposed Plan Shape

```text
U12 Interactive Subtree Transform
  |
U13 Visible / Inert / Hidden
  |
U14 Committed Live Regions and Announcements
  |
U15 Typed Committed Portal Anchors
  |
U16 Bring Into View Authority
  |
U17 Rounded-Rectangle Subtree Clip

Separate design epics:
  locale + logical direction -> implementation only after inventory/design gate
  group compositing         -> implementation only after renderer architecture gate
  pointer substrate         -> gesture arena research, then implementation decision
  arbitrary path clipping   -> revisit after rounded clipping evidence
```

U14 is the only candidate that should be added to the current convergence completion path without
another design checkpoint. U15 and U16 are high-confidence follow-ons because they deepen U12's
geometry authority. U17 is an implementation candidate with an explicit renderer/ABI checkpoint.
The separate epics should not receive public placeholder types or count toward the current plan's
Definition of Done.

## Cross-cutting Review Gates

| Concern | Required owner | Required invariant |
| --- | --- | --- |
| Frame commit | `open-gpui::Window` | No candidate publishes half-frame or failed-scope state |
| Renderer-neutral semantics | `ui_core` | No GPUI, AccessKit, renderer, or window dependency |
| Component adaptation | `ui_components` | Components project state; they do not own window queues or geometry runtimes |
| Window isolation | GPUI window-owned state | Handles, announcements, reveal requests, and generations cannot cross windows silently |
| Presentation | U13 authority | `Hidden`/`Inert` participation is inherited and cannot be bypassed by a helper |
| Transform | U12 authority | All geometry uses checked opaque conversion; no raw matrix or identity fallback |
| Deferred/cache/portal | frame journal and named portal boundary | Inheritance versus reset is explicit and tested for every candidate |
| Accessibility | final AccessKit tree | Roles/properties/removal/actions are asserted after repair and delivery |
| Privacy | DevTools/redaction authority | Free text is not persisted merely because a runtime needs it transiently |
| Native parity | owning backend CI | Unsupported behavior is rejected or explicitly limited, never silently approximated as success |

## Final Recommendation

Add U14 now. Add U15 and U16 as concrete follow-on implementation units, and add U17 only with its
renderer/ABI gate written into acceptance criteria. Record locale/logical direction as a required
future design epic. Keep arbitrary path clipping, group compositing, and gesture arena APIs in
research until their substrate exists.

The common rule is more important than any individual API: a public subtree capability must own
every observable channel and a complete lifecycle. If Open GPUI cannot yet state that authority,
failure behavior, and test matrix precisely, it should publish research rather than syntax.
