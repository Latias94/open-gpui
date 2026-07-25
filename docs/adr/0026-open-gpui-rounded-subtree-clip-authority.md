# ADR 0026: Open GPUI Rounded Subtree Clip Authority

**Status**: Accepted
**Date**: 2026-07-22

## Context

GPUI previously represented descendant clipping as one rectangle-only `ContentMask`. The same
approximation was copied into scene primitives, hitboxes, deferred draw records, cached views,
portal snapshots, and renderer shader inputs. It cannot represent a nested rounded rectangle: an
intersection AABB is useful for culling but is not the visible or interactive region. It also made
numeric projection failure fail open in some paths, allowing a subtree to paint or receive input
outside its declared clip.

The renderer contract must support WGPU, DirectX, and Metal without making one backend the
semantic authority. A fixed shallow stack, a primitive-local copied stack, an arbitrary path API,
or a stencil-only implementation would either silently change semantics, make cache replay
ambiguous, or make exact hit testing depend on a renderer implementation.

## Decision

GPUI has one checked rounded-rectangle subtree clip authority. The public input is an immutable
`SubtreeClip` with a rectangle or rounded rectangle in zero-origin, child-local logical
coordinates relative to the child's post-layout border box. The wrapper provides an own-border-box
shorthand. Checked constructors reject non-finite values, negative dimensions, and negative
elliptical radii. A zero-width or zero-height clip is empty. Before projection, radii are normalized
against the declared local box with the CSS corner-overlap scale factor, preserving asymmetric
ellipses.

At scope entry, the window resolves the declaration once through the current U12 subtree transform
into a window-space `ResolvedClip`. A `ClipStackSnapshot` is an immutable ordered collection of
resolved clips plus its conservative intersection AABB. Descendant transforms do not reproject an
ancestor snapshot. Exact containment evaluates every rectangle and corner ellipse in stack order;
the AABB is permitted only for culling, accessibility bounds, and explicitly conservative public
snapshots.

The old `ContentMask` type and raw window-space mask injection are deleted. Style overflow, Div,
List, uniform list, text, image, surface, and Canvas enter the same checked authority. Canvas
prepaint resolves a declaration against its own post-layout border box into an opaque token bound
to the window, frame, geometry validity, and inherited snapshot; paint may only re-enter that token.
It cannot submit a raw window-space shape or stack. Dock viewport routing retains an opaque
committed hit-test snapshot and requires active presentation, valid geometry, host bounds, and exact
clip containment; an AABB alone is never a routing proof.

Style overflow uses an internal adapter rather than a second public shape. With both axes clipped,
it resolves the padding-box ellipse by subtracting the adjoining border widths independently from
each x/y radius and then applying the same CSS normalization as public clips. With only one axis
clipped, it appends a rectangular strip bounded only on that axis and leaves the other axis to the
inherited stack; a one-axis clip does not pretend that a two-axis rounded corner is meaningful.
Border color does not change clip geometry.

The previous transform-only validity tracking is generalized to `SubtreeGeometryValidity`. Its
error domain distinguishes transform, clip, and device conversion failure. A failed geometry scope
rolls back or suppresses every
`FrameOutput`, journal entry, and retained publication for the affected subtree, including scene
primitives, hitboxes and pointer capture, listeners, tab stops, focus and IME records, tooltips,
reveal targets and bring-into-view facts, deferred work, cache data, debug geometry, portal data,
and accessibility. No failure path may execute the child with its parent clip or with no clip.

Hitboxes and frame hit testing carry the same snapshot and call the same exact point-containment
implementation. Pointer capture retains the existing post-acquisition behavior; clipping only
controls initial target discovery. Ordinary deferred work captures the current snapshot. The named
window-space portal starts a fresh root-viewport snapshot deliberately. Cached-view keys compare
the complete snapshot, including normalized radii and order. Scene replay and `Scene::finish`
import and remap referenced clip ranges, so no primitive can retain an index into a previous
frame's clip arena.

Ordinary deferred semantic roots retain their captured AccessKit parent, including a semantic clip
owner. A window-space portal resets both clip ancestry and AccessKit parentage to the window root:
`clips_children` is a parent-wide AccessKit fact, so retaining a portal below a clipped parent would
incorrectly make assistive technology clip an element that deliberately escaped the visual clip.
Deferred snapshots also retain their source root sibling anchor. Nested deferred and portal work
extend that anchor rather than deriving a position from deferred replay or paint priority.

The scene owns a frame-local deduplicated, dynamically sized flattened clip arena. The canonical
`#[repr(C)] GpuClipShape` is 48 bytes with 4-byte alignment: scaled window-space `f32` bounds at
offsets 0/4/8/12 (`origin.x`, `origin.y`, `size.width`, `size.height`), x radii at offsets
16/20/24/28, and y radii at offsets 32/36/40/44. Both radius groups use TL/TR/BR/BL order. The
canonical `#[repr(C)] ClipEnvelope` is 24 bytes with 4-byte alignment: conservative bounds at
offsets 0/4/8/12 followed by `u32 first_clip` at 16 and `u32 clip_count` at 20. Arena indices count
`GpuClipShape` elements, not bytes. Each primitive carries that envelope. There is no semantic
fixed-depth limit. An unrepresentable range or conversion is a failed geometry scope, not a
truncated stack.

WGPU, DirectX, and Metal bind one read-only clip-shape buffer shared by their primitive pipelines.
Shaders may use the envelope AABB for early rejection, but fragment coverage loops over the exact
clip range. Path rasterization applies the exact range in its intermediate pass before any later
copy. Each renderer owns compile-time layout/offset checks and conversion tests for the shared ABI.
Native paint surfaces consume the same exact clip. Their target-gated API is absent on backends
that cannot provide the surface payload, so an unsupported combination cannot enter the Scene;
no renderer reports success after painting an AABB approximation.

Accessibility remains intentionally conservative, but semantic exclusion is distinct from visual
and pointer clipping. Public `SubtreeClip` declarations and non-scrolling overflow axes
(`Overflow::Hidden` and `Overflow::Clip`) exclude fully clipped descendants from AccessKit. The
root viewport and `Overflow::Scroll` still constrain paint and initial pointer targeting exactly,
but do not remove off-viewport descendants from the semantic tree: those nodes must remain
available to AccessKit `ScrollIntoView`.

A nested scroll viewport releases an earlier semantic clip on an axis only when that viewport has
an interior reachable through every preceding semantic clip. An offscreen scroll viewport cannot
bypass a hidden ancestor merely because its own descendants could be revealed locally. List and
uniform-list adapters mark only their computed scroll axes as revealable, including the optional
horizontal axis of an unconstrained uniform list.

For a semantically published non-empty node, a shared CPU query returns a conservative AABB after
semantic clipping. The built-in fallback `Click` separately requires an exact interior witness in
the complete visual/pointer stack. Thus an offscreen scroll target may be published and expose
`ScrollIntoView` without receiving a usable fallback click location. Numerically uncertain or
boundary-only non-empty cases fail closed for the relevant query. Zero-area semantic nodes may
remain only when their anchor point is inside the semantic stack; they receive no pointer witness
and no built-in fallback `Click`. Published bounds use the conservative AABB because AccessKit
cannot encode the curves, while any fallback `Click` acts at the witness rather than the AABB
center. Clip owners expose `clips_children` only where the active AccessKit surface supports it.
Documentation states that AccessKit bounds are not evidence that every enclosed point is
pointer-interactive.

Arbitrary paths, fill rules, stencil/tessellation choices, group opacity, blend modes, and raw clip
stack injection are outside this authority. They require a separate retained rendering feature with
its own paint, hit, cache, and accessibility semantics.

## Consequences

- Nested rounded clips have one exact paint and initial-input meaning on every supported backend.
- Rectangle-only paths remain efficient through zero radii, while their behavior is no longer a
  separate authority.
- The scene ABI grows by a clip envelope per primitive plus a shared frame buffer, instead of
  duplicating an entire stack per primitive.
- Cached and deferred rendering rebuilds whenever any exact clip fact changes, even when the
  conservative AABB is unchanged.
- Ordinary deferred semantics retain their logical parent, while window-space portal semantics
  deliberately begin at the window root so a clipped ancestor cannot suppress them.
- Deferred semantic and portal roots preserve source sibling order independently of their replay
  round or paint priority.
- Portal anchor consumers retain a conservative public clip AABB while internal paint and routing
  retain exact geometry.
- Existing direct `ContentMask` users become explicit migrations, allowing the obsolete public
  rectangle API to disappear rather than fossilizing a second contract.

## Verification

Pure GPUI tests cover checked construction, radius normalization, asymmetric ellipses, empty
clips, nested ordering, U12 composition, and boundary points around every corner. Runtime tests
cover layout invariance, hover/click/wheel/drag/drop containment, capture after acquisition,
scrolling, U13 suppression, deferred work, cache replay, portal reset, debug output, and late
conversion failure.

Scene tests verify that every primitive family preserves a complete snapshot and that conservative
culling never determines final coverage. WGPU, DirectX, and Metal each compile ABI layout checks
and execute conversion tests; capable native runners add nested asymmetric pixel samples. Canvas
and Dock regression tests prove their migrated paths use exact containment. Accessibility tests
cover semantic-clipped removal, scroll-viewport semantic retention and `ScrollIntoView`,
conservative partial bounds, stable identity, and fallback actions only inside the visual/pointer
region.

## Related Documents

- [UI framework authority convergence plan](../plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md)
- [Interactive subtree transform authority](0021-open-gpui-interactive-subtree-transform-authority.md)
- [Subtree presentation authority](0022-open-gpui-subtree-presentation-authority.md)
- [Typed committed portal anchor authority](0024-open-gpui-typed-committed-portal-anchor-authority.md)
- [Open GPUI v0.3 UI migration guide](../ui/migration-v0.3.md)
