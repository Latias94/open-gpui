# ADR 0021: Open GPUI Interactive Subtree Transform Authority

**Status**: Accepted
**Date**: 2026-07-18

## Context

Open GPUI previously exposed an SVG-only `Transformation` and renderer matrix types. The SVG API
changed raster output without changing hit testing, descendants, IME geometry, accessibility, or
debugging. Reusing that shape for an arbitrary subtree would create a visual transform beside the
framework's input and semantic geometry authorities.

Interactive scaling also crosses more boundaries than paint. A correct subtree mapping must agree
across scene primitives, rectangular clips, hitboxes, routed-event coordinates, pointer capture,
scrolling, drag/drop, text input, AccessKit, deferred work, cached frame journals, tooltips, and
Inspector output. A numeric failure on only one channel cannot safely fall back to identity or
publish a partial subtree.

## Decision

`open-gpui` owns one public, layout-neutral `SubtreeTransform` declaration. It accepts only:

- finite, strictly positive normal axis scale whose reciprocal is representable;
- finite translation in logical pixels;
- a post-layout `SubtreeTransformOrigin` expressed as a finite size-relative anchor plus a finite
  logical-pixel offset.

The public declaration is opaque. `SubtreeTransformExt::with_subtree_transform` is the application
entry point. Layout, measurement, flex/grid placement, scroll extent, and sibling flow remain
unchanged. GPUI resolves the origin after layout and privately normalizes every declaration to one
invertible `scale * source + offset` mapping. A child mapping is composed before its parent.

Rotation, skew, perspective, reflection, zero or negative scale, arbitrary matrices, and 3D are
not accepted. Supporting those forms would first require non-rectangular clipping and hit testing,
text/native-surface policy, and complete platform/accessibility semantics.

## Public Geometry And Input

`ElementGeometry` is the immutable read interface for committed post-layout geometry. It exposes
untransformed layout bounds, displayed window bounds, zero-origin local bounds, and checked
local/layout/window point and vector conversions. `Hitbox::geometry()` and
`MeasuredElementSnapshot::geometry()` return the same value, so custom elements and measurement
consumers do not reconstruct transform arithmetic.
Custom prepaint channels that already own layout bounds use `Window::try_element_geometry`; it
returns the same opaque value and invalidates the active transform scope on projection failure.

Raw platform events remain window-space and are available through `TargetedEvent::window_event()`.
High-level listeners use explicit target-local helpers. Click, pointer, wheel, drag-start,
drag-move, and drop adapters bind the hitbox from the frame that routed the event. Pixel wheel
deltas are inverse-projected; line deltas retain their semantic unit. Pointer capture keeps one
logical owner while each committed frame rebinds that owner to current geometry.

`measured_element` publishes only immutable snapshots after a valid frame commit. It never calls a
listener from a failed transform transaction, and cached journal replay publishes geometry for the
current ancestor mapping.

## Transaction And Frame Ordering

Prepaint and paint enter the same scoped mapping and validity token. Any composition, projection,
inverse, or renderer conversion failure invalidates the complete owning scope. GPUI filters scene,
hitboxes, listeners, capture bindings, focus/IME, deferred entries, cache records, debug output,
and accessibility nodes through that token and emits one structured diagnostic per failed scope.

Tooltip source bindings may read a still-active hitbox from the frame currently being built after
root prepaint; outside that phase they read only the last committed hitbox. This keeps an already
visible tooltip synchronized when a transform changes without allowing a late failed scope to
publish geometry. Inspector state and measured snapshots update after commit; Inspector schedules
one follow-up frame only when committed geometry changed.

Cross-window consumers use the same validity boundary. A prepaint producer may build a candidate
snapshot, but only a valid paint commits it; an invalid transformed frame runs an explicit discard
path that retracts the previous snapshot. Retained publications allocate one stable
`PrepaintPublicationId`; `record_prepaint_window_transaction` also runs the previous frame's discard
when that ID is absent from the completed next frame. Subtree removal, an enclosing prepaint
rollback, and an ancestor transform that skips the producer therefore cannot retain stale external
state. Docking uses this transaction for viewport scene candidates, so a failed or absent host
cannot leave an old drop route or presentation scene observable.

Dock divider routing also follows committed visual surfaces rather than one flat rectangle list.
Root and each floating container build junctions independently; the last rendered floating
container owns its complete bounds through a blocking pointer boundary, so its ordinary content as
well as its dividers occludes every lower surface. A root splitter and a floating splitter therefore
cannot synthesize a cross-surface corner. Raw Dock splitter and composite-floating drags use the
stable Dock host capture owner. Standard GPUI payload drags use the source element's stable owner,
acquired by GPUI only after crossing the drag threshold; `on_drag` therefore requires a stable
element ID. A frame-scoped cancel listener published by the rendered `DockHost` clears raw drag
state and the matching payload session, preview, anchor, and captured native route on deactivation,
capture revocation, presentation suppression, or window removal. GPUI dispatches terminal
cancellation from the old committed frame before replacing its listener journal, so the true owner
observes cancellation once without leaving a window-global observer after the host subtree
disappears. Cross-window payload routing is separately bound to the source capture owner's exact
drag generation and ingress sequence. Its immutable physical callback frame selects a current
committed host scene without target-window raw input; a terminal fact detaches that exact route
before effects, and missing or stale geometry fails closed instead of consulting a poll or prior
preview. A single-tabs floating drag publishes its Dock payload only after floating policy accepts
the transient session; policy or geometry rejection retracts the GPUI drag and capture without
leaving a second partial authority.

Ordinary deferred descendants capture the current mapping. `window_portal` is an explicit
window-coordinate boundary: its anchor is projected before the content mapping resets. The reset
does not bypass theme or presentation inheritance. Cached view replay is revalidated against the
current transform fingerprint and replays every observable channel together.

## Renderer And Motion Boundaries

Renderer crates receive an opaque `PrimitiveTransform` ABI projection. Its fields and checked
constructors are not application-writable; backend accessors exist only because native renderer
crates cross the Rust crate boundary. Each primitive batch carries the same mapping and validates
it before backend use.

Device-pixel snapping is a CPU-owned post-projection policy. GPUI keeps primitive shading geometry
in unsnapped device-local coordinates, projects its raster envelope through the checked subtree
mapping, snaps or covers the displayed edges, and derives the renderer projection that maps the
original local envelope onto those edges. Paths retain their unsnapped checked projection rather
than rounding individual vertices. Border and underline widths are rounded in the displayed axis
and mapped back into local shading units. Renderer shaders do not implement a second rounding rule.

`open-gpui-motion` remains renderer- and GPUI-neutral. `MotionProjection` returns a fallible
`MotionProjectionTransformSample`; a consumer that depends on both crates converts that sample to
`SubtreeTransform`. Motion never emits a GPUI type and never substitutes identity for invalid
geometry. Exact final and reduced-motion samples return the identity endpoint even for large valid
source ratios.

## Consequences

- The SVG-only `Transformation`, `TransformationMatrix`, and `with_transformation` APIs are
  deleted without aliases. SVG rotation has no replacement in this restricted contract.
- Applications use `SubtreeTransform` for interactive axis-aligned presentation and
  `ElementGeometry`, `Window::try_element_geometry`, or `TargetedEvent` for coordinate conversion.
- Backends, Motion, SVG, Canvas, Gallery, and components cannot introduce a second public subtree
  transform stack.
- Rounded/path subtree clips, group opacity/compositing, and general affine transforms require
  separate capability decisions and complete cross-channel tests.

## Verification

The contract is covered by pure numeric/property tests; every-primitive scene and backend ABI
tests; runtime hit, click, wheel, drag/drop, pointer-capture, IME, AccessKit, deferred, cache,
tooltip, measurement, and debug tests; public-surface/source guards; Motion endpoint tests; and the
Gallery Presentation flow with nested non-uniform transforms and real controls. Docking is a real
consumer: its local policy scenes remain in absolute layout coordinates while platform routing and
tear-off geometry remain in displayed window coordinates derived from committed
`ElementGeometry`; floating-surface z-order and terminal pointer cancellation remain aligned with
the same committed interaction frame. Tests include overlapping root/floating dividers, a top
floating content surface over a lower floating title bar, policy and inverse-geometry rejection,
host-subtree removal for floating and tab-item payloads, and terminal cancellation with multiple
independent Dock hosts in one window.

## Related Documents

- [UI framework authority convergence plan](../plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md)
- [Open GPUI v0.3 UI migration guide](../ui/migration-v0.3.md)
- [Verification guide](../verification.md)
- [ADR 0018: Open GPUI Motion Crate Boundary](0018-open-gpui-motion-crate-boundary.md)
