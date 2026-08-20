# ADR 0032: Dockspace Scenario Testkit Boundary

**Status**: Accepted
**Date**: 2026-08-20

## Context

Docking verification already covers graph mutations, rendered host interaction, viewport routing,
native captured drag, close/recovery, and hot-path benchmarks. The coverage was broad but
fragmented: complex workflows such as nested split docking, multi-viewport rehome, committed
destination loss, preview stability, and cancel rollback were described separately in hand-written
tests, native manifests, and benchmark fixtures.

Dear ImGui and ImGui Test Engine are useful reference corpora for docking behavior because they
test user-facing docking actions across dockspaces, tabs, splits, undock, viewport focus, viewport
close, and owner-change cases. They are not implementation dependencies for Open GPUI. Copying
their immediate-mode context, dock node internals, pointer ownership, or settings model would
replace Open GPUI's retained `DockGraph`, `DockSurface`, viewport runtime, and exact presentation
authorities.

The component-library direction also favors clear headless boundaries, but docking currently has
only one production adapter: GPUI. A public `open-gpui-docking-headless` crate would freeze an
interface before a real non-GPUI consumer exists.

## Decision

`open-gpui-docking` owns an internal dockspace scenario testkit. Its topology builders, operation
resolver, and headless runner remain crate-private and are available only to tests or the existing
`test-support` path. A doc-hidden `native_scenario_support` module may expose a feature-gated,
data-only projection to workspace-owned native tests. This projection contains fixture ids, layouts,
semantic targets, production drop-guide points, oracle labels, and headless checkpoints; it cannot
mutate docking state or expose controller/runtime internals. The testkit is a scenario vocabulary
and adapter harness, not a second docking engine.

The testkit may define:

- topology seeds for tabs, nested splits, floating spaces, empty spaces, and viewport spaces;
- user-facing operation steps such as tab move, edge dock, center dock, tear off, rehome, close,
  cancel, and restore;
- typed oracle labels for graph/layout, presentation geometry, focus/selection, route/native
  lifetime, interaction feel, and performance layers;
- fixture rows that map Open GPUI scenarios to ImGui docking behavior categories;
- benchmark builders that create stable scenario labels and measurement-layer rows.

The testkit must execute production authorities. Pure model cases run through `DockGraph`,
`DockWorkspace`, and workspace transaction paths. Rendered cases run through GPUI host and viewport
runtime adapters. Native cases project scenario metadata into the checked-in native manifest,
execute the complete fixture step sequence in order, and must keep exact observation domains such
as capture, wnd-proc, point-stack, presentation, activation, geometry, and lifetime. Native target
points come from the same production drop-guide geometry used by the renderer; fixture checkpoints
come from production headless transactions. Benchmarks measure named scenario operations and emit
Open-GPUI/ImGui metrics only when the rows share the same declared operation family and measurement
layer.

`DockSurface` remains the common application facade. Root and prelude exports must not expose the
testkit, raw workspace/controller/runtime types, or viewport handles. Explicit `model`, `runtime`,
and `advanced` tiers remain the escape hatches for low-level integration and diagnostics. A public
headless docking crate is deferred until a production adapter outside GPUI needs it.

## Consequences

- Complex docking behavior can be described once and reused by headless model tests, GPUI host
  tests, native manifest rows, and benchmark fixtures.
- ImGui parity becomes concrete scenario metadata and benchmark shape rather than copied docking
  internals.
- Performance results can distinguish resolver time, viewport-route lookup, dockable-target preview,
  scene churn, allocation count, and native input-to-submit latency.
- Public API guards must fail if scenario vocabulary or low-level docking types leak through the
  common root or prelude.
- Scenario fixtures must reject duplicate panel labels, unknown operations, and adapter/oracle
  combinations that cannot produce typed evidence.
- Native multiview correctness remains opt-in owning-platform evidence; screenshot or ad hoc edge
  percentages are not sufficient correctness proof.

## Rejected Alternatives

### Publish a headless docking crate now

**Rejected** because GPUI is the only production adapter today. Publishing a crate would freeze
premature test vocabulary as public API and make later component-library or native-adapter work
harder to reshape.

### Keep adding hand-written tests

**Rejected** because it preserves coverage fragmentation. Hand-written tests still have value for
edge cases, but shared workflows need stable scenario ids, typed oracles, and benchmark labels.

### Depend on ImGui Test Engine

**Rejected** because ImGui Test Engine is a reference corpus, not a compatible runtime. Open GPUI
must preserve retained graph transactions, exact viewport generations, and renderer-submitted
presentation gates.

### Let benchmarks stand in for behavior tests

**Rejected** because docking feel depends on correctness gates such as preview stability, focus
restoration, route generation locking, cancel rollback, and native lifetime convergence. Benchmarks
measure costs after those semantics are already proven.

## Success Criteria

| Criterion | Evidence |
|---|---|
| Complex topology scenario vocabulary exists without public leakage | `dockspace_testkit` tests and `public_surface_tests` |
| Headless execution uses production workspace/transaction paths | `workspace_move`, `workspace_drop`, `workspace_action`, and `dockspace_testkit` gates |
| Fixture corpus maps Open GPUI rows to ImGui categories | `dockspace_scenario_fixture_tests` and benchmark JSON labels |
| GPUI/native layers prove typed scenario oracles | host viewport tests, schema-6 native manifest validation, and the three-HWND fixture runner |
| Scenario benchmarks emit stable comparable rows | `tab_drag_latency` bench and `tools/benchmark_docking_drag.py --json` |
| Public headless extraction remains deferred | no root/prelude testkit exports and README boundary note |
