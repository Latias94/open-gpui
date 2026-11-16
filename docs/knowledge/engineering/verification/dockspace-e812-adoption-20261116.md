---
type: Verification Evidence
title: Dockspace e812 Candidate Verification and Persistence Blocker
status: blocked
timestamp: 2026-11-16T16:39:00+08:00
git_commit: 55f4a3cb1db74b425342f2d8478239516d49fc30
---

# Dockspace e812 Candidate Verification and Persistence Blocker

## Fixed sources

This follow-up starts at Open-GPUI `55f4a3cb1db74b425342f2d8478239516d49fc30`
and prepares an upgrade from dockspace `dcd65e983220e17662b1f02929fcf84348cd8eb3`
to `e8120864e06de445e1b588b07d6b322bb7125e41`. The candidate workspace and both
independent consumer lockfiles retain that exact Git source. No other dependency
is upgraded. The migration remains unpublished because of the persistence
regression below; the committed production pin stays at `dcd65e983220`.
The preceding [integration repair record](dockspace-gpui-followups-20261116.md)
retains its original source pin and failed opaque physical receipt.

The tested integration staging tree is
`f0d8236292496674cb55ed344a2d1da020fb88c3`. Its independent patch is retained as
`%TEMP%/open-gpui-dockspace-e812-candidate-20261116.patch`. It includes the native
adoption, measurement, repaint and precision changes below, with their regression
tests. It is a candidate tree, not a published commit.

## P1 adoption blocker: valid v1 documents no longer restore

The independent public consumer generates a document using the previous pinned
revision, without GPUI, native windows or renderer state. Its controls produce:

```text
old_writer_self_restore=passed
new_reader_old_document=Err("DockspacePersistenceError { source: Restore(BindingHashMismatch) }")
current_writer_self_restore=passed
```

The locked reproduction deliberately exits with code 101 when the new reader
rejects the old valid document. Both aliases use exactly pinned serializer
dependencies: serde `1.0.228`, serde_json `1.0.150` and thiserror `2.0.18`, matching
the Open-GPUI workspace. The old and current core revisions are fixed above.

The [original v1 fixture](fixtures/dockspace-dcd65e9-v1.json) is the old writer's
unchanged 638-byte output. SHA-256:
`bc5eaee46c544b7ef4526ff94a516c000958d3ee850f640082b497be2474c682`.
It has one tab, no split weights and no hidden items, isolating the new empty
`hidden_items` field from floating-point representation changes.
The [current v1 control](fixtures/dockspace-e812086-v1.json) is also retained
unchanged: 664 bytes, SHA-256
`ee6e70023ebd10fd162fbc410c43003b160b1bd872ba81fa18c0c969923e3073`.
It restores at `e8120864`; a migration must account for both already-emitted
v1 payload shapes rather than fixing one by invalidating the other.

Both writer revisions emit document version `1`, with hash domain
`dockspace-document/v1\0`. The current `WorkspaceSnapshot` deserializer defaults
missing `hidden_items`, but serializing that value appends `"hidden_items":[]`.
`binding_hash` serializes the typed workspace again before checking integrity,
so the original hash no longer matches. Ordinary current-version roundtrips do
not detect this regression.

Copyable upstream issue:

```text
P1: Preserve restoration of previously valid v1 documents across schema evolution.

Writer: dcd65e983220e17662b1f02929fcf84348cd8eb3
Reader: e8120864e06de445e1b588b07d6b322bb7125e41
Observed: the old writer restores its own bytes; the new reader rejects the same
one-tab document with Restore(BindingHashMismatch). The current writer restores
its own bytes. Both payloads still declare document version 1.

WorkspaceSnapshot now serializes hidden_items, including an empty default, while
the binding hash still serializes the whole typed workspace under the v1 domain.
Missing-field serde defaults therefore do not preserve old document integrity.

Add version-aware decoding/migration or compatible canonical hashing. Verify the
old document's original hash before conversion, retain identity/key associations,
and keep tampered or mixed-generation payloads rejected. Include both already-
emitted v1 payload shapes, old-writer/new-reader fixtures, current roundtrips and
corruption checks. Adapters must not
recompute an old document's binding hash merely to make restoration succeed.
```

The complete public-consumer source, manifest, lockfile and generated old/current
documents are under `%TEMP%/open-gpui-dockspace-compat-e812-20261116-1626/`.
The final log is `%TEMP%/open-gpui-dockspace-e812-document-compat-locked-20261116.log`.
Reading the original fixture through the new public API is sufficient to repeat
the failing side:

```rust
use dockspace::{policy::DockPolicy, runtime::DockspaceSession};

let bytes = include_bytes!("dockspace-dcd65e9-v1.json");
match DockspaceSession::from_document_json(bytes, DockPolicy::default(), |_, _| true) {
    Ok(_) => {}
    Err(error) => panic!("a previously valid v1 document must restore: {error:?}"),
}
```

## Disposition of the core follow-ups

| Previous request | Upstream implementation | Adoption boundary |
| --- | --- | --- |
| P1: Native placement over an opaque Foreign window | `ec29336` adds edge-local `with_native_tear_off`, `NativeDragTearOff` and create-effect correlation. Foreign hover still blocks covered docking targets. | GPUI must supply a fresh work-area capability on the actual edge, retain the original provisional drag key and adopt only a matching committed create. |
| P2: Interaction-only repaint reporting | `591c937` includes old/new source, ghost, guide and preview visuals in `repaint_surfaces()` while excluding equivalent provenance-only changes. | GPUI uses the public report and removes its global pointer-repaint fallback. |
| P2: Permanently rejected ingress termination | The core guide now distinguishes retryable candidates, waiting for actual terminals and failed generations. `7e35fc1` also improves the egui native adapter's lifecycle servicing. | The specification is available. The egui implementation does not implement GPUI teardown. GPUI retains the original rejected FIFO and affine owners and never reports an unconsumable generation as successfully drained. |
| P3: Cross-adapter conformance | Public host-consumer regressions cover Foreign tear-off, transient repaint and rejected release/effect-result custody. `ADAPTER_TESTING.md` assigns backend obligations. | Public-contract coverage improved. The shared scenario catalog does not replace GPUI input, renderer or physical qualification. |

## GPUI migration

The retained element uses `prepare_surface_with` and propagates fallible
measurement answers directly. Core owns output admission, retained gesture and
preview geometry, and whether a surface can be remeasured in an encoding pass.
The explicit encoding boundary remains before reading the presentation roster.
Native Bootstrap readiness retains its separate deferred-packet wake path.

Legacy v2/v3 imports keep their original serialized scalar representation and
convert weights to `f64` at the layout compilation boundary. Derived ratios stay
in double precision. The persistent wire format is not replaced.

The crate reexports core, so consumers must also migrate typed split weights to
`f64` and measurement resolvers to `Result` answers. Code constructing a
`NativeWindowRequest` literal must supply `drag_tear_off`; programmatic requests
use `None`. A custom factory still owns its original creation path.

Captured move/release and stationary desktop continuations keep their real
Foreign/OutsideAll/Unknown receiver classifications. A matching current display
observation supplies independent tear-off authority; Unknown hover, stale display
facts and framework blockers do not acquire that authority. Provisional resource
adoption uses the exact core drag key in addition to the reserved surface.
Native creation, final placement, first-live presentation, original View transfer
and capture/retirement barriers keep their original owners.

The rootless-background receiver regression now checks that framework clearance
does not invent a semantic hover target. The presented hit facts still exist,
but delivery, drag delivery and hover remain empty away from an explicit guide.
The new managed cancellation regression requires actual target feedback before
cancellation, checks that both old surfaces reencode, retains an unrelated
window's exact packet and serial, and verifies idle settlement does not loop.

## Remaining boundaries

Failed-generation disposal and automatic reconstruction are not added by this
dependency upgrade. A permanently rejected accepted prefix can prevent semantic
quiescence. The application must retain the complete old generation until every
backend, callback, output and asynchronous owner actually terminates, then create
a fresh session from committed logical state and application pane data. Closing
windows, inspecting an error or detaching a handler is insufficient.

Normal renderer output backpressure currently reaches the retained element's
error/retry path. Supporting a semantic-only commit while retaining the old
packet's exact artifact and input authority is a separate adapter improvement;
the new core preparation API alone does not provide that GPUI publication path.

This session uses one remotely controlled Windows display. Mixed-DPI placement,
display disconnect/reconnect, real cross-window IME composition and other
backends require their own qualification. No result below implies those cases.

## Verification

Cargo uses the existing target directory with one build job and serial test
execution. Physical cases retain the original manifest selectors and receipt
protocol.

| Check | Result | Original run identity |
| --- | --- | --- |
| Default shared docking | 148/148 passed | `1f0dc9ad-2873-4ed3-9f7a-dd19471db338` |
| All docking features, including legacy compatibility | 1612/1612 passed | `3bf38de7-3b0c-4637-a06d-c4aa45516feb` |
| Example, editor, multiviewport and DevTools consumers | All-target check passed with the legacy compatibility feature | `%TEMP%/open-gpui-dockspace-e812-consumers-20261116.log` |
| Formatting, all-feature/all-target docking check, public API and documentation links | Passed | `%TEMP%/open-gpui-dockspace-e812-final-checks-20261116.log` |
| Original opaque physical selector | Passed with verified behavior receipt | `ba281fb9-c910-46c9-a8cc-01f00f1263e8` |
| Previous-writer/current-reader document compatibility | Failed: `Restore(BindingHashMismatch)`; both same-version controls passed | `%TEMP%/open-gpui-dockspace-e812-document-compat-locked-20261116.log` |

The original default run found fixture capability/publication setup errors and
the changed rootless hover expectation. Those failures were repaired without
relaxing the same-window, drag-key, first-live or unrelated-window repaint
assertions. The final default run above exercises the corrected preparation.

The first complete selected physical cohort is retained under
`%TEMP%/open-gpui-physical-acceptance-20261116-101601/focused-160301/`.
It passes thirteen rows, including opaque, and fails source-capture before its
docking journey: the pointer canary observes a different cursor position from
the requested point. Its original failure receipt and successful cleanup census
remain recorded. This cohort is not relabeled as fourteen passes. A subsequent
independent run must retain its own receipts.

## Final independent single-display physical cohort

The final cohort executes the same candidate independently under
`%TEMP%/open-gpui-physical-acceptance-20261116-101601/focused-163512/`.
All fourteen original behavior receipts match, and all fourteen rows pass.
No receipt from the earlier canary failure or standalone opaque run is substituted.
Mixed-DPI remains excluded, so this is not the complete fifteen-row manifest gate.
The persistence blocker remains independent of this successful physical cohort.

| Manifest row | Result | Original nextest run ID |
| --- | --- | --- |
| `native.u27.source-capture` | Passed | `5ef4e18b-184f-425b-8ded-48e5e287ccb7` |
| `native.u29.complex-dockspace-workflow` | Passed | `efa022f8-7b43-446b-8d6e-2425a12944a5` |
| `native.u27.opaque-occlusion` | Passed | `5b7e5765-08b9-4f93-b005-6f65981b6977` |
| `native.u27.opaque-hit-transparent-prefix` | Passed | `6635b9dd-344f-4821-a653-cbd1c335dbc7` |
| `native.u27.surface-shutdown` | Passed | `b3d38b88-5957-4762-8697-0412b2109fd3` |
| `native.u29.provisional-same-hwnd-promotion` | Passed | `db3678c2-dbed-4e35-bde0-643f2ed85f28` |
| `native.u29.live-route-and-release-lock` | Passed | `bd3f07c1-2d36-42d9-a0a0-aad4432211fa` |
| `native.u29.committed-loss-recovery` | Passed | `b1202fba-0155-4af2-8391-e29e960cfca4` |
| `native.u28.activation-terminal` | Passed | `19cfaa3a-b29f-4b41-913b-b4649f0f1a9a` |
| `native.u28.provisional-activation-z-order` | Passed | `a0bcf963-45d8-4b03-93fe-d4701d7c0439` |
| `native.u28.client-geometry-reconciliation` | Passed | `f6d55651-03bd-4e45-a672-369ba0a566ed` |
| `native.u28.event-driven-geometry-wake` | Passed | `b3bf5064-ac92-406b-87ae-4d4de9d09a76` |
| `native.u28.process-convergence` | Passed | `1d567e3b-1d53-4499-8d78-4a176ba8cdac` |
| `native.u28.no-input-pass-through` | Passed | `7244025b-8bd6-4d99-9942-2a3eb074813d` |

Original worker nonces for the primary risk groups are:

| Worker | Nonce |
| --- | --- |
| opaque | `97444-primary-18e8799addbf5e58` |
| process convergence / last window closed | `102364-last_window_closed-18e879a989d04618` |
| process convergence / App shutdown | `102364-app_shutdown-18e879a9f8cb0be8` |
| committed loss recovery | `76840-primary-18e879a23a47b100` |

Each reports successful capture release and zero unterminated native generations.
The original worker stdout/stderr, Job and HWND census, commands, token receipts
and run times remain in the cohort directory and its `results.json`.
