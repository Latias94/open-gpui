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
  before the anchor. Stale callbacks cannot acquire a new provider/window lifetime.
- Use the managed native producer for native workspaces. The local host-pointer producer is a
  separate configuration and cannot be enrolled alongside it.
- Preserve desktop physical coordinates and target-display DPI. Native frame bounds and client
  bounds are independent observations. Unsupported conversion, hit testing or capture remains
  explicit; an adapter cannot infer native capabilities from ordinary logical mouse events.
- Index established native resources with the core's `NativeWindows`, retaining a bounded
  pre-open callback slot and original failed-open rollback tickets until a real GPUI ID exists.
  Physical Root continuation moves resources once while every predecessor placement retires
  separately. Deferred submission results keep their original binding and callback custody.
- Admit pending tear-off surfaces only through their exact native placement and the core's
  first-live presentation roster. Their absence from the published layout does not prove removal.
  A no-paint commit preserves current scene authority; explicit unavailable contributions revoke
  it. Native Dock Back also requires the original source outer geometry and current recovery scene.

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
