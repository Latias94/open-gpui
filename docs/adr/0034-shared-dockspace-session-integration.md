# ADR 0034: Shared Dockspace Session Integration

**Status**: Accepted
**Date**: 2026-11-13

## Context

The application needs independent document Views, editor controls and native docking without
maintaining unrelated docking engines for GPUI and egui. The renderer-neutral `dockspace` crate
already owns a complete semantic session, including topology, interaction, presentation correlation,
close decisions and native establishment. It does not expose a private graph that can be inserted
under the existing GPUI controller.

The [framework evolution plan](../plans/2026-11-13-1145-refactor-editor-framework-evolution-plan.md)
initially treated replacement as a candidate decision. The user subsequently authorized implementation,
breaking changes and retirement of displaced code after its replacement is qualified.
The fixed source, compiler proof and executed qualification belong in the
[implementation evidence](../knowledge/engineering/verification/editor-framework-evolution-20261113.md).

## Decision

Use one complete `DockspaceSession` as the semantic authority of a migrated workspace. The GPUI
adapter owns retained entities, physical window bindings, event-time framework hit facts, exact
frame output correlation and native transport. It must not mirror the session into `DockGraph`,
run the incumbent controller beside it, or use the old interaction reducer as a recovery writer.

Keep the application facade useful but allow its pre-1.0 API to change. Each pane key identifies
one View occurrence. Several Views may share an application document, but each has a distinct
entity and local state. Moving a View retains that entity; duplicating it calls an application
factory. Document, history and final engine rendering remain application-owned.

The shared library's weighted N-ary canonical layout replaces the binary-kernel choice in ADR 0033.
The adapter uses the core's public revision-bound actions and transaction protocol. It does not
reimplement binary geometry, stable structural identities or a separate local-patch engine.
Selection, close, floating roots and persistence use the shared session's identities and contracts.

## Preserved physical contracts

ADRs 0029, 0030 and 0031 continue to govern platform and renderer ownership:

- Aggregate every logical region in one real Window under one original exact frame ticket.
- Commit the affine semantic update at GPUI candidate acceptance; do not keep that lease while a
  renderer defers submission. Other Windows must be able to progress independently.
- Report Presented only from the matching original submission's terminal observation. Refusal,
  withdrawal and terminal windows retire their own outputs without fabricating submission.
- Treat native close intent, logical removal, renderer/capture quiescence and actual native
  destruction as distinct events. A forced group drain freezes admission and retires dependents
  before the anchor. Core-owned Child bindings reach permanent quiescence before application
  Roots retire, including non-anchor Roots that may own recovery. Stale callbacks cannot acquire
  a new provider/window lifetime.
- Use the managed native producer for native workspaces. The local host-pointer producer is a
  separate configuration and cannot be enrolled alongside it.
- Preserve desktop physical coordinates and target-display DPI. Native frame bounds and client
  bounds are independent observations. Unsupported conversion, hit testing or capture remains
  explicit; an adapter cannot infer native capabilities from ordinary logical mouse events.
  The platform declares native desktop-position and button-state support independently of
  point-stack support. The managed bridge maps these declarations to the core's desktop-routing
  capabilities; an individual unavailable observation still carries `Unknown`.
- Index established native resources with the core's `NativeWindows`, retaining a bounded
  pre-open callback slot and original failed-open rollback tickets until a real GPUI ID exists.
  Physical Root continuation moves resources once while every predecessor placement retires
  separately. Deferred submission results keep their original binding and callback custody.
- Admit pending tear-off surfaces only through their exact native placement and the core's
  first-live presentation roster. Their absence from the published layout does not prove removal.
  A no-paint commit preserves current scene authority; explicit unavailable contributions revoke
  it. Native Dock Back also requires the original source outer geometry and current recovery scene.
- Removing a physical mount while retaining its logical surface commits a true per-surface
  unavailable contribution before permanent native-binding quiescence. Mount loss and renderer
  terminals use the existing native FIFO presentation barriers; rejected prefixes remain ordered.
  A current replacement owner suppresses stale loss, and ordinary prepaint rejection does not
  by itself revoke the previous displayed scene.

## Native preview ownership

The default managed factory may create a GPUI provisional window for the core's
`DockspacePreviewVisual::Native`. The reserved target surface correlates that
visual with a later create effect; it is not permission to register another core
root or encode a target output before release. The private
`adapter/host_native/provisional.rs` owner retains the visual shell, exact GPUI
provisional session and physical retirement authority.

An accepted source packet freezes its original physical pointer/display route.
The accepted native FIFO delivers release packets with their original source and
drag generation; only the matching provisional owner retains one for adoption.
Adoption installs
the create effect's callbacks on the existing Window and HWND, settles the final
target-DPI placement, and shares `semantics_ticket.frame_submission()` with the
ordinary docking presentation pipeline. Candidate withdrawal may reinstall that
same authority. Interaction requires both its renderer submission and the core's
exact first-live `Admitted` report.

An unadopted shell remains a dependent of the native anchor. Cancellation, route
loss, failed preparation and shutdown retain the original GPUI shutdown ticket
until renderer quiescence and native terminal; a failed Window borrow retains a
close intent for retry. `App` remains the physical retirement owner.

An application-supplied `set_native_window_factory` retains complete creation
ownership and opts out of the default factory's pre-release shell. The core's
Native visual may still provide a source-hosted placement cue, without creating
another native window or granting adoption authority. Installing a
custom factory retires an outstanding default shell on the next native pump.
The shared dispatch retains the factory's original callbacks during construction and registers
their exact installed WindowId before staged surface painting, including reentrant first render.
The original core outer intent remains distinct from converted client bounds, and a native-caption
anchor preserves the original pointer outside the client without changing its Z-order proof point.
Supporting application-defined provisional creation requires an explicit factory
contract; the adapter must not silently replace or bypass the application factory.

## Persistence migration

The versioned adapter envelope embeds the core's public document and durable external pane-key
history. Runtime entities, factories, focus handles, native handles and frame proofs are excluded.
Restoring a document validates every key, including closed historical keys, before any factory
runs or a live workspace changes. A factory is required only for an open occurrence.

Legacy v2/v3 layouts are accepted through a one-way migrator with an explicit
`LegacyTopologyMigration::RelativeWeights` choice. Order, tab selection, floating bounds and
representable relative shares are retained. Legacy node/group/floating identities and exact size
references remain in a provenance receipt, not a second live graph. Binary rounding, absolute
`SizeRef`, central sibling-size preservation and weighted zero-share cases are not silently
declared equivalent. Unsupported values fail atomically or return a named host-policy difference.
Saved logical window positions remain hints until the current platform establishes physical facts.

## Cutover and verification

The November 14 default cutover promotes the shared implementation to the crate
root and prelude. Ordinary examples, DevTools and the independently locked
consumer use this entry point. The incumbent engine is isolated under `legacy`
behind an explicit `legacy-docking` feature; enabling it never changes the root
types and no migrated workspace calls it as fallback. Its physical qualification
scenarios remain available until their declared replacements run. This is a
production entry-point cutover, not completed removal or universal native parity.
See the [cutover record](../knowledge/engineering/verification/dockspace-default-cutover-20261114.md).

The later [published dependency adoption](../knowledge/engineering/verification/dockspace-remote-adoption-20261114.md)
migrates the browser probe to the shared session and removes unused canonical
migration/detached payload experiments. The Windows dogfood still owns ten
explicit physical-input journeys; its legacy runtime remains until those
shared-session replacements execute. The public logical checkpoint is a
read-only reconstruction input, not durable document persistence or permission
to reuse an undrained native host.

This decision selects the architecture; it is not evidence that every backend or consumer has
passed native qualification. Implement and verify dependency compatibility, real pane content,
multi-surface aggregation, deferred-sibling progress, native create/first-live, close recovery and
persistence before deleting their incumbent production owners. Migrate readers and writers for a
workspace together. Once the declared replacement path passes, remove displaced graph/controller,
interaction and temporary comparison code; keep only the durable format conversion and required
GPUI physical lifecycle owners.

The shared core at the selected revision declares Rust 1.95. `open-gpui-docking` therefore declares
that floor explicitly; the core GPUI and component crates retain Rust 1.93. A diagnostic build with
`--ignore-rust-version` cannot replace the dependency's supported manifest contract.

CPU test-renderer results establish semantic and correlation behavior. Real platform windows,
physical capture, mixed-DPI placement, OS IME and renderer retirement require their owning runners.
Unexecuted platform scope stays visible in the evidence record and is not converted into a passing
result by documentation closure.
