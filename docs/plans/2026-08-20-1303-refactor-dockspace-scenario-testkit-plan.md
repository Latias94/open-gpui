---
title: Dockspace Scenario Testkit and ImGui Parity Benchmarks - Plan
type: refactor
date: 2026-08-20
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-plan-bootstrap
execution: code
---

# Dockspace Scenario Testkit and ImGui Parity Benchmarks - Plan

## Goal Capsule

Objective: Open-GPUI docking has a scenario-driven verification system that proves complex dockspace and multiview behavior, and it produces comparable functional and performance evidence against mature ImGui docking behavior.

Means: Add a crate-private dockspace scenario testkit over the existing `DockGraph`, `DockWorkspace`, drop resolver, and viewport route seams; project the same scenarios into headless tests, GPUI host tests, native multiview tests, and Rust/ImGui benchmark fixtures. (KTD1, KTD2, KTD5, KTD6)

Authority hierarchy: The Product Contract owns behavior. The Key Technical Decisions own implementation mechanisms. `docs/adr/0002-docking-gpui-integration.md`, `docs/adr/0012-docking-runtime-capability-alignment.md`, `docs/adr/0030-open-gpui-dock-surface-window-session-authority.md`, and `crates/gpui_docking/src/public_surface_tests.rs` remain boundary authorities. ImGui Test Engine and ImGui docking tests are reference corpora, not implementation dependencies.

Stop conditions:

- Stop if the work requires `DockGraph` or the workspace model core to store GPUI views, `Entity`, focus handles, platform windows, or native handles.
- Stop if the only way to expose the testkit is through common root or prelude public exports.
- Stop if a native scenario can only be asserted through screen-position heuristics instead of typed observation domains.
- Stop if a benchmark compares two operations that do not share a declared scenario step and measurement layer.

Execution profile: Code implementation plan for a Rust crate. Native Windows scenarios remain opt-in ignored tests. Benchmark work must not replace functional verification.

Tail ownership: `ce-work`, `/goal`, or a human executor should implement units in dependency order and keep commits conventional.

## Product Contract

### Summary

Open-GPUI should graduate from scattered docking tests and hot-path microbenchmarks to a shared scenario system. The new system must describe complex dockspace operations once and reuse that description across model, host, native, and benchmark layers. It should make ImGui parity concrete without copying ImGui internals.

### Problem Frame

The current branch is already in a performance and correctness phase for tab drag latency, native route resolution, and multiview docking. Recent changes add zero-allocation native drag resolution, multiview benchmarks, and ImGui dockable target benchmarks.

The remaining gap is not one more fast path. Existing tests cover graph mutations, host rendering, viewport runtime behavior, native capture, close/recovery, DPI, and occlusion, but complex multiview workflows are still hand-written in separate tests. ImGui Test Engine provides the stronger model: tests express user-facing docking actions and assert behavior across complex topologies.

### Key Decisions

- KD1. ImGui is the parity reference for docking behavior and benchmark shape. (session-settled: user-directed - chosen over local heuristics: the user asked to avoid heuristic design and compare against mature ImGui docking and multiview behavior.) Governs R9, R10, R12.
- KD2. Functional coverage is as important as performance coverage. (session-settled: user-directed - chosen over perf-only benchmarking: the user asked to test complex docking and multiview features, not only latency.) Governs R4, R5, R6, R7, R8.

### Requirements

#### Dockspace Authority

- R1. `DockGraph` must remain the pure semantic layout authority and must not store GPUI view, focus, window, or platform-native state.
- R2. The plan must deepen the existing `DockWorkspace` and `DockController` ownership path before introducing any new public headless package.
- R3. The common root and prelude surface must stay `DockSurface`-first and must not export low-level model, runtime, viewport-handle, or testkit types.

#### Scenario Testing

- R4. The test system must express complex topologies with tabs, nested splits, floating spaces, empty spaces, and multiple viewport spaces.
- R5. Each reusable scenario must declare operations and typed oracles for graph/layout state, presentation geometry, focus or selection, viewport routing, native lifetime, interaction-feel signals such as preview stability and cancel rollback, and performance when those facts apply.
- R6. Headless scenarios must run without GPUI rendering or native windows when the behavior is pure model, transaction, drop-resolution, or route-planning logic.
- R7. GPUI host scenarios must drive real `TestAppContext` or `VisualTestContext` adapters when rendering, focus, preview geometry, debug bounds, or host interaction behavior is the contract.
- R8. Native scenarios must keep exact observation domains such as capture, wnd-proc, point-stack, presentation, activation, geometry, and lifetime; unsupported capabilities must fail closed.

#### ImGui Parity And Benchmarks

- R9. The scenario catalog must map Open-GPUI cases to ImGui docking behavior categories such as dock into parent, dock over dockspace, tab order, tab focus restore, undock, sizing, split payload, viewport focus, viewport close, and viewport owner change.
- R10. Performance benchmarks must distinguish resolver time, viewport-route lookup time, dockable-target preview time, scene churn cost, allocation count, and native input-to-submit latency.
- R11. Benchmark output must use stable scenario labels and must emit Open-GPUI and ImGui metrics in one machine-readable result when both sides are available.

#### Engineering Boundaries

- R12. The work must not copy ImGui internals or add ImGui Test Engine as an Open-GPUI test dependency.
- R13. The work must not introduce a public `open-gpui-docking-headless` crate until a real non-GPUI production adapter or consumer exists.
- R14. The scenario DSL or builder must not become a second docking engine; it may build inputs, drive adapters, and assert outcomes only.

### Success Criteria

- A complex topology such as `AAA, BBB | CCC + DDD` is described once and can drive at least headless model tests, GPUI host tests, and benchmark fixtures.
- At least one multiview scenario covers three or more viewport spaces with cross-window rehome behavior.
- At least one cross-window scenario records preview stability, cancel rollback, and input-to-preview timing as typed evidence rather than screenshot heuristics.
- Existing retained-scene and known-target multiview benchmarks keep zero allocations per move.
- The benchmark JSON reports current Open-GPUI metrics and comparable ImGui metrics with shared scenario labels.
- Public surface tests still prevent low-level docking model, runtime, and testkit types from leaking into common/prelude exports.

### Key Flows

- F1. A scenario author defines topology, operations, expected oracles, and adapter tags in the testkit.
- F2. The headless adapter builds a `DockGraph`/workspace seed, executes model or route steps, and checks graph and layout oracles.
- F3. The GPUI host adapter opens controller-backed hosts, maps scenario operations to visual interaction helpers, and checks presentation, focus, and preview facts.
- F4. The native adapter projects selected scenarios into the existing native interactive manifest and runner, preserving typed observations.
- F5. The benchmark adapter projects selected scenarios into Rust benchmark fixtures and the ImGui comparison fixture, then emits one labeled result set.

### Acceptance Examples

- AE1. Given `AAA, BBB | CCC + DDD`, when the scenario drags `AAA` to the right edge of `CCC`, the headless oracle reports one scoped split under the hit leaf and no duplicate panel item. Covers R4, R5, R6.
- AE2. Given three viewport spaces, when a source tab is dragged from the first viewport into a nested target in the third viewport and then docked back, the host oracle reports the expected preview target, selected tab, focus restoration, and final graph topology. Covers R5, R7, R9.
- AE3. Given a native captured drag across multiple HWNDs, when a committed destination disappears before release, the native oracle reports source-owned capture, route-lock failure closed, no stray window handle in the runtime, and recoverable layout state. Covers R5, R8.
- AE4. Given the complex topology benchmark, when the benchmark script runs with ImGui available, the JSON contains Open-GPUI resolver metrics, Open-GPUI multiview metrics, ImGui dockable-target metrics, allocation counts, and scenario labels for the same operation family. Covers R10, R11, R12.

### Scope Boundaries

In scope:

- Crate-private or feature-gated scenario testkit code inside `crates/gpui_docking`.
- Focused internal refactoring that separates workspace model authority from live GPUI panel registration.
- Scenario fixture data for complex dockspace and multiview operations.
- GPUI host, native interactive, and benchmark adapters that reuse scenario definitions.
- Documentation and ADR updates that record the internal testkit boundary.

Deferred to follow-up work:

- A public headless docking crate for third-party adapters.
- A full ImGui Test Engine clone with screenshot, video, and cinematic playback tooling.
- Browser or Wayland multiview parity beyond existing capability gates.
- User-facing component-library API redesign outside docking verification.

Outside this product's identity:

- Copying ImGui docking implementation details.
- Replacing GPUI's window, focus, or application authority with a docking-owned platform layer.
- Treating screenshot heuristics as native docking correctness evidence.

### Sources

- Current branch commits from `main..HEAD`, including native drag zero-allocation work, multiview drag benchmarks, and ImGui dockable target benchmarks.
- `crates/gpui_docking/src/lib.rs`, `workspace.rs`, `controller.rs`, `runtime.rs`, `model.rs`, `public_surface_tests.rs`.
- `crates/gpui_docking/src/benchmark_support.rs`, `crates/gpui_docking/benches/tab_drag_latency.rs`, `tools/benchmark_docking_drag.py`, `tools/benchmarks/imgui_docking_preview.cpp`.
- `examples/docking-native/src/native_interactive_tests.rs` and `examples/docking-native/tests/native_windows_interactive.native-scenarios.toml`.
- `repo-ref/imgui_test_engine/imgui_test_suite/imgui_tests_docking.cpp` and `repo-ref/imgui_test_engine/imgui_test_suite/imgui_tests_viewports.cpp`.
- `repo-ref/egui/tests/test_viewports/src/main.rs` and `repo-ref/fret/ecosystem/fret-ui-headless/README.md`.
- `docs/adr/0002-docking-gpui-integration.md`, `docs/adr/0012-docking-runtime-capability-alignment.md`, `docs/adr/0030-open-gpui-dock-surface-window-session-authority.md`.
- `docs/knowledge/engineering/verification/docking-nested-inner-edge-20260628.md`, `docs/knowledge/engineering/subagents/docking-presentation-prior-art-20260630.md`, `docs/knowledge/engineering/subagents/docking-runtime-capability-followup-20260630.md`.

## Planning Contract

### Key Technical Decisions

- KTD1. Add a crate-private `dockspace_testkit`, not a public headless crate. (session-settled: user-approved - chosen over a public `open-gpui-docking-headless` crate: the existing GPUI adapter is still the only real production adapter, so a public package would freeze a premature interface.) Bench-only construction helpers may flow through the existing `test-support`/`benchmark_support` path, but common root and prelude exports stay closed. Governs R2, R3, R6, R13.
- KTD2. Make the scenario catalog the shared interface across tests and benchmarks. (session-settled: user-approved - chosen over more hand-written tests: the current coverage is strong but too dispersed to compare complex workflows reliably.) Governs R4, R5, R9, R10, R11.
- KTD3. Start with a strongly typed scenario builder and fixture schema; defer a textual parser until the builder proves stable. (session-settled: user-approved - chosen over parser-first DSL design: a parser would risk becoming a second layout language before the scenario semantics settle.) Governs R4, R14.
- KTD4. Split workspace model authority from live GPUI panel registration behind an internal module boundary. Governs R1, R2, R6.
- KTD5. Use an adapter ladder: headless proves graph/workspace/route facts, GPUI host proves rendered interaction facts, native proves platform facts, and benchmarks prove measured costs. Governs R5, R6, R7, R8, R10.
- KTD6. Benchmark by named scenario operation and measurement layer, not by a single global docking number. Governs R9, R10, R11, R12.
- KTD7. Preserve `DockSurface` as the stable application facade and keep public-surface tests as a drift gate. Governs R3, R12, R13.

### High-Level Technical Design

#### Component Topology

```text
Dockspace Scenario Catalog
  -> Topology Builder
       -> DockGraph / DockLayout seed
  -> Operation Steps
       -> drag tab, dock edge, dock center, tear off, rehome, close, restore, resize
  -> Oracle Set
       -> graph/layout, presentation, focus, route, native lifetime, perf
  -> Adapters
       -> Headless Adapter
       -> GPUI Host Adapter
       -> Native Manifest Adapter
       -> Benchmark Adapter
            -> Rust benchmark fixture
            -> ImGui comparison fixture
```

#### Adapter Authority Ladder

```text
Headless
  proves pure graph, workspace transactions, policy rejection, drop target, route planning.

GPUI Host
  adds rendered bounds, preview geometry, focus, host subscriptions, transition-visible behavior.

Native
  adds HWND/native window identity, capture, activation, z-order, client geometry, process lifetime.

Benchmark
  adds cost measurements for named operations and records the layer being measured.
```

Each higher adapter may assert lower-layer facts, but it should not be the primary proof for facts that a lower adapter can prove deterministically.

#### Scenario Data Flow

```text
Scenario definition
  -> seed topology
  -> adapter-specific setup
  -> execute one operation step
  -> capture typed evidence
  -> apply oracles
  -> emit adapter result
```

The scenario definition is data and intent. It must not perform graph mutation itself.

#### Native Multiview State Flow

```text
Idle
  -> Captured(source generation)
  -> Routed(known target frame)
  -> Previewed(target facts accepted)
  -> Released(route generation locked)
  -> Committed
       or FailedClosed
       or RecoveredFromDestinationLoss
```

Native scenarios must keep the current branch's source-owned capture model and route generation locking. They must not fall back to polling or inferred screen ownership.

#### Measurement Layers

| Layer | Open-GPUI measurement | ImGui measurement | Primary use |
|---|---|---|---|
| Resolver | `DockHostDropScene::resolve_pointer_move` | nearest comparable preview setup | Hot-path regressions |
| Multiview route | known/unknown viewport target route lookup | known host or dockable target path | Cross-window drag cost |
| Dockable target | accepted preview/dockable target setup | `BeginDockableDragDropTarget` path | ImGui parity comparison |
| Interaction feel | preview stability, cancel rollback, and input-to-preview timing traces | comparable hover/accepted-target trace when the fixture can expose it | UX regression signal |
| Native latency | input-to-submit p50/p95/p99 | no direct equivalent unless added externally | User-perceived Windows path |

### Assumptions

- The testkit can live under `#[cfg(test)]` first, with shared construction helpers promoted behind the existing narrow `test-support` feature when benches need them.
- The `tab_drag_latency` bench already requires `test-support`, so benchmark-facing scenario helpers should reuse that feature rather than adding a new public package.
- Existing `graph_test_support.rs`, `host_test_support.rs`, `viewport_test_support.rs`, and `host_viewport_runtime_test_support.rs` can be wrapped before they are moved.
- The first useful scenario vocabulary is a Rust builder plus optional fixture schema, not a text parser.
- Native Windows tests remain opt-in because they open windows and inject input.

### Implementation Constraints

- Use `cargo nextest` for Rust tests and `cargo fmt` for formatting.
- Avoid broad parallel `cargo` jobs during implementation.
- Use repo-relative paths in docs and fixtures.
- Keep fixture schemas small and data-only.
- Keep scenario operations user-facing, not graph-internal.
- Add comments only where the testkit boundary would be hard to understand from type names.

### Sequencing

```text
U1 -> U2 -> U3 -> U4 -> U5 -> U6 -> U7
        \          \          /
         ---------->----------
```

U1 establishes vocabulary. U2 gives the vocabulary a deterministic headless executor. U3 creates reusable scenario data. U4 and U5 project the same scenarios into host and native layers. U6 makes the benchmark comparison scenario-based. U7 closes documentation and public API gates.

### Risks And Dependencies

- Risk: `DockWorkspaceModel` extraction could grow too large. Mitigation: keep `DockWorkspace` as the compatibility facade and move only graph, policy, revision, and transaction ownership first.
- Risk: The scenario builder could duplicate docking logic. Mitigation: builders only seed topology and dispatch operations to production code paths.
- Risk: Native tests can become flaky. Mitigation: keep typed observations, exact route generations, and opt-in ignored execution.
- Risk: ImGui comparison can become apples-to-oranges. Mitigation: every benchmark row declares a scenario operation and measurement layer.
- Risk: Public API drift can leak internals. Mitigation: extend `public_surface_tests.rs` before exposing any reusable helper outside test-only scope.

## Implementation Units

### U1. Create The Crate-Private Dockspace Testkit Vocabulary

Goal: Add the internal scenario vocabulary that can describe topology, operations, adapter tags, and oracles without performing docking logic.

Requirements: R1, R3, R4, R5, R13, R14; KTD1, KTD2, KTD3.

Files:

- `crates/gpui_docking/src/lib.rs`
- `crates/gpui_docking/src/dockspace_testkit/mod.rs`
- `crates/gpui_docking/src/dockspace_testkit/topology.rs`
- `crates/gpui_docking/src/dockspace_testkit/ops.rs`
- `crates/gpui_docking/src/dockspace_testkit/assertions.rs`
- `crates/gpui_docking/src/graph_test_support.rs`

Approach: Add `dockspace_testkit` behind `#[cfg(test)]` first. Define data types for named panels, tab stacks, split nodes, floating spaces, viewport spaces, operation steps, and oracle categories. Keep bench-facing builders separable so U6 can promote only the needed data construction pieces through the existing `test-support`/`benchmark_support` path. Re-export or wrap existing graph support helpers instead of moving all tests in the first patch.

Test Scenarios:

- Build `AAA, BBB | CCC + DDD` and assert that the generated seed has one horizontal split, one tab stack with `AAA`/`BBB`, one nested stack with `CCC`, and one floating or detached `DDD` space according to the scenario definition.
- Build a scenario with duplicate panel labels and assert that the testkit returns a descriptive setup error before creating a graph.
- Build a scenario with an empty central space and a floating space and assert that the layout roundtrip preserves both semantic roles.

Verification:

- `cargo nextest run -p open-gpui-docking dockspace_testkit --no-fail-fast`

### U2. Deepen Workspace Model Authority For Headless Scenario Execution

Goal: Give scenarios a deterministic headless executor over production workspace transactions without live GPUI panel registration.

Requirements: R1, R2, R5, R6, R14; KTD4, KTD5.

Files:

- `crates/gpui_docking/src/workspace.rs`
- `crates/gpui_docking/src/workspace_action.rs`
- `crates/gpui_docking/src/workspace_drop_transaction.rs`
- `crates/gpui_docking/src/workspace_move_transaction.rs`
- `crates/gpui_docking/src/controller.rs`
- `crates/gpui_docking/src/dockspace_testkit/headless.rs`
- `crates/gpui_docking/src/workspace_*tests.rs`

Approach: Introduce an internal workspace-model boundary that owns graph, policy, revision, commit identifiers, and transaction application. Keep `DockWorkspace` as the app-facing facade that also owns panel registry and host options. The headless scenario adapter should execute production transaction paths through this boundary or through the existing facade with dummy descriptors while the extraction is in progress.

Test Scenarios:

- Execute tab move, edge dock, center dock, float, and merge-back operations from a scenario and assert final graph topology plus item uniqueness.
- Apply a stale commit token or route generation and assert that the mutation is rejected without graph changes.
- Apply a policy-rejected dock class operation and assert that the scenario records a rejected outcome and unchanged layout.
- Close and reopen a registered panel descriptor without constructing a live `AnyView`.

Verification:

- `cargo nextest run -p open-gpui-docking workspace_move workspace_drop workspace_action dockspace_testkit --no-fail-fast`

### U3. Add A Reusable Dockspace Scenario Fixture Corpus

Goal: Store complex docking scenarios as reusable data that can feed headless tests, host tests, native manifest generation, and benchmarks.

Requirements: R4, R5, R9, R12, R14; KTD2, KTD3.

Files:

- `crates/gpui_docking/src/fixtures/dockspace_scenarios_v1.toml`
- `crates/gpui_docking/src/dockspace_testkit/fixtures.rs`
- `crates/gpui_docking/src/dockspace_scenario_fixture_tests.rs`
- `crates/gpui_docking/src/dock_op_fixture_tests.rs`

Approach: Add a small fixture schema for scenario id, topology, steps, expected oracle categories, adapter tags, and ImGui reference categories. Keep the existing `dock_op_sequences_v1.json` tests working; new fixture rows should cover larger topologies and cross-layer scenario labels.

Test Scenarios:

- Load a fixture for `tabs_split_float_merge` and assert that its generated graph matches the same result as the typed builder.
- Load a fixture for `three_viewport_rehome` and assert that it is tagged for headless, host, native, and benchmark adapters only where supported.
- Reject a fixture that names an unknown operation or references a panel not present in the topology.
- Map fixture rows to ImGui behavior categories without requiring ImGui source at test runtime.

Verification:

- `cargo nextest run -p open-gpui-docking dockspace_scenario_fixture_tests dock_op_fixture_tests --no-fail-fast`

### U4. Project Scenarios Into GPUI Host And Viewport Runtime Tests

Goal: Reuse scenario definitions to test rendered host behavior, preview geometry, focus, tab selection, and multiview route behavior.

Requirements: R5, R7, R9; KTD2, KTD5.

Files:

- `crates/gpui_docking/src/dockspace_testkit/gpui_host.rs`
- `crates/gpui_docking/src/dockspace_testkit/viewport_runtime.rs`
- `crates/gpui_docking/src/host_test_support.rs`
- `crates/gpui_docking/src/host_viewport_runtime_test_support.rs`
- `crates/gpui_docking/src/host_multiview_scenario_tests.rs`
- `crates/gpui_docking/src/host_viewport_model_tests.rs`
- `crates/gpui_docking/src/host_viewport_matrix_tests.rs`
- `crates/gpui_docking/src/host_interaction_tests.rs`

Approach: Add adapter helpers that open controller-backed hosts from scenario seeds, translate scenario operations into existing visual drag helpers, and collect presentation/debug/focus facts. Migrate the closest existing multiview model and matrix cases to consume the scenario vocabulary before adding new cases.

Test Scenarios:

- Tear off `AAA` from a main host, dock it into a nested target in a sibling viewport, dock it back, and assert preview target, selected tab, focus restoration, and final topology.
- Drag over a nested mixed-axis inner edge and assert that preview and delivery stay scoped to the hit leaf.
- Sweep the pointer across adjacent nested edges and assert that preview targets change only at declared region boundaries.
- Cancel a cross-window drag after preview and assert that focus, selection, and layout return to their pre-drag state.
- Route a known viewport target and a stale viewport target and assert that only the current route commits.

Verification:

- `cargo nextest run -p open-gpui-docking host_multiview_scenario_tests host_viewport_model_tests host_viewport_matrix_tests host_interaction_tests --no-fail-fast`

### U5. Extend Native Interactive Scenarios For Complex Multiview Docking

Goal: Prove complex docking operations through native Windows evidence without making native tests the primary model oracle.

Requirements: R5, R8, R9; KTD5.

Files:

- `examples/docking-native/src/native_interactive_tests.rs`
- `examples/docking-native/tests/native_windows_interactive.native-scenarios.toml`
- `xtask/src/native_windows_interactive.rs`
- `crates/gpui_docking/src/dockspace_testkit/native_manifest.rs`

Approach: Bump the native manifest schema to `5` when scenario topology id, step id, and oracle references are added. Migrate the checked-in rows and update `xtask` plus the non-ignored registry test to require the new schema. Do not maintain a dual schema parser unless a real external manifest consumer is found. Add scenario-driven rows for three or more HWNDs, cross-viewport rehome, committed destination loss, mixed DPI, and occlusion.

Test Scenarios:

- Run a three-HWND captured drag where the source window keeps capture, the target window previews a nested dock, and release commits through the locked route generation.
- Run a same-HWND provisional promotion in a complex topology and assert that the runtime records the expected terminal status.
- Run committed destination loss during cross-window drag and assert fail-closed delivery plus recoverable layout state.
- Run mixed-DPI plus opaque occlusion in a multiview scenario and assert that point-stack, client geometry, and presentation observations agree.

Verification:

- `cargo nextest run --locked -p xtask --lib native_manifest --no-fail-fast`
- `cargo nextest run -p open-gpui-docking-native --run-ignored only --test-threads 1 --no-fail-fast native_interactive_three_hwnd_complex_multiview`
- `cargo nextest run -p open-gpui-docking-native --run-ignored only --test-threads 1 --no-fail-fast native_interactive_committed_loss_recovery_complex`

### U6. Make Benchmarks Scenario-Based And ImGui-Comparable

Goal: Extend the benchmark system so complex Open-GPUI operations and ImGui docking paths are compared by named scenario and measurement layer.

Requirements: R9, R10, R11, R12; KTD2, KTD6.

Files:

- `crates/gpui_docking/src/benchmark_support.rs`
- `crates/gpui_docking/benches/tab_drag_latency.rs`
- `tools/benchmark_docking_drag.py`
- `tools/benchmarks/imgui_docking_preview.cpp`
- `crates/gpui_docking/src/dockspace_testkit/benchmarks.rs`

Approach: Capture the current same-machine benchmark JSON before changing the U6 benchmark code, then reuse scenario seeds to build retained scenes, multiview host-scene registries, unknown-route cases, and scene-churn cases. Move only the builder pieces required by benches behind `test-support` and keep runtime test adapters `#[cfg(test)]`. Extend the ImGui fixture with comparable complex dockable-target operations. Emit a JSON schema with scenario id, measurement layer, Open-GPUI metrics, ImGui metrics, allocation counts, and ratio fields where the operation is comparable.

Test Scenarios:

- Run the existing small retained-scene benchmark and assert that allocations per move remain zero.
- Run a complex nested-split scenario benchmark and assert that labels and allocation metrics are present.
- Run a multiview known-target benchmark and an unknown-route fallback benchmark and assert that both are reported separately.
- Run the Python script without native latency and assert that ImGui build/run failures are reported as missing comparison data, not as malformed Open-GPUI metrics.

Verification:

- `cargo bench -p open-gpui-docking --bench tab_drag_latency --features test-support`
- `python -m py_compile tools/benchmark_docking_drag.py`
- `python tools/benchmark_docking_drag.py --json`

### U7. Document The Boundary And Preserve Public Surface Guards

Goal: Record the internal testkit design and ensure the new seams do not drift into public API by accident.

Requirements: R1, R2, R3, R12, R13; KTD1, KTD7.

Files:

- `docs/adr/0032-dockspace-scenario-testkit.md`
- `docs/verification.md`
- `crates/gpui_docking/src/public_surface_tests.rs`
- `crates/gpui_docking/README.md`

Approach: Add an ADR or verification note that states the testkit is crate-private, `DockSurface` remains the app facade, and public headless extraction is deferred until a real adapter exists. Extend public-surface tests if any new module or helper risks export drift.

Test Scenarios:

- Assert that common root exports and prelude exports still omit `DockWorkspace`, `DockController`, `DockHost`, viewport runtime handles, and dockspace testkit types.
- Assert that advanced exports remain explicit and do not import the testkit.
- Review the ADR and README for contradictions with ADR 0002, ADR 0012, and ADR 0030.

Verification:

- `cargo nextest run -p open-gpui-docking public_surface_tests --no-fail-fast`
- `git diff --check`

## Verification Contract

### Required Gates

- `cargo fmt --all --check`
- `cargo nextest run -p open-gpui-docking dockspace_testkit --no-fail-fast`
- `cargo nextest run -p open-gpui-docking dockspace_scenario_fixture_tests dock_op_fixture_tests --no-fail-fast`
- `cargo nextest run -p open-gpui-docking workspace_move workspace_drop workspace_action host_multiview_scenario_tests host_viewport_model_tests host_viewport_matrix_tests host_interaction_tests public_surface_tests --no-fail-fast`
- `cargo bench -p open-gpui-docking --bench tab_drag_latency --features test-support`
- `python -m py_compile tools/benchmark_docking_drag.py`
- `python tools/benchmark_docking_drag.py --json`
- `cargo nextest run --locked -p xtask --lib native_manifest --no-fail-fast`
- `cargo run --locked -p xtask -- scan-ui-contract`
- `git diff --check`

### Native Opt-In Gates

Run these only on a Windows machine where native interactive tests are allowed to open windows and inject input:

- `cargo run --locked -p xtask -- native-windows-interactive`
- `cargo nextest run -p open-gpui-docking-native --run-ignored only --test-threads 1 --no-fail-fast native_interactive_three_hwnd_complex_multiview`
- `cargo nextest run -p open-gpui-docking-native --run-ignored only --test-threads 1 --no-fail-fast native_interactive_committed_loss_recovery_complex`

### Performance Exit Criteria

- Existing retained-scene and known-target multiview benchmark rows still report `0` allocations per move.
- Existing benchmark rows do not regress by more than 20% against a same-machine baseline captured before U6 changes.
- New complex benchmark rows emit stable scenario labels and allocation counts on every run.
- Ratio fields are emitted only when Open-GPUI and ImGui rows measure the same declared operation family and measurement layer.
- Native input-to-submit latency is reported separately from ImGui comparison metrics.

### Review Gates

- Public-surface tests must fail if any testkit, low-level runtime, or low-level model type leaks into common root or prelude exports.
- Scenario fixtures must fail fast on unknown operations, duplicate panel labels, and adapter tags that cannot support a required oracle category.
- Native manifest validation must fail if a scenario row names native observations without corresponding typed evidence.

## Definition of Done

- U1 is done when the testkit can seed complex topologies and reject invalid scenario data without public API exposure.
- U2 is done when headless scenario execution uses production workspace or transaction paths and can prove stale-token and policy-rejection behavior.
- U3 is done when a reusable fixture corpus contains complex topology and multiview rows with ImGui reference categories.
- U4 is done when at least one complex scenario runs through GPUI host/viewport adapters and asserts preview, focus, and final topology.
- U5 is done when native interactive rows cover three-HWND multiview behavior and committed-loss recovery through typed observations.
- U6 is done when benchmark JSON contains scenario-labeled Open-GPUI and ImGui comparison rows plus allocation counts.
- U7 is done when docs and public-surface tests make the internal boundary durable.
- The full plan is done when required gates pass, native opt-in gates are either run on Windows or explicitly reported as not run, abandoned experimental code is removed, and commits preserve the repo's conventional commit style.

## Appendix

### Current Branch Baseline

The branch is ahead of `origin/optimize/docking-tab-drag-latency` and contains docking performance and benchmark commits including:

- `perf(docking): make native drag resolution zero allocation`
- `perf(docking): add multiview drag benchmark`
- `test(docking): benchmark imgui dockable target path`
- `perf(docking): borrow drop scene during tab drag`
- `perf(docking): select drop candidates without allocation`

The recent benchmark result set reported:

| Metric | Value |
|---|---:|
| `open_gpui_ns_per_move` | `185.8` |
| `open_gpui_multiview_known_target_ns_per_move` | `471.291` |
| `open_gpui_allocations_per_move` | `0` |
| `open_gpui_multiview_known_target_allocations_per_move` | `0` |
| `imgui_ns_per_preview` | `30.9559` |
| `imgui_multiview_known_host_ns_per_preview` | `31.2171` |
| `imgui_dockable_target_ns_per_preview` | `1037.724` |
| `imgui_multiview_dockable_target_known_host_ns_per_preview` | `1074.824` |
| `resolver_to_imgui_dockable_target_ratio` | `0.179` |
| `multiview_known_target_to_imgui_dockable_target_known_host_ratio` | `0.438` |

### ImGui Test Engine Behavior Categories To Mirror

The first scenario corpus should mirror these categories from the local ImGui reference where Open-GPUI has corresponding behavior:

- Basic docking and dock into parent.
- Dock over dockspace.
- Tab order, tab focus restore, and hidden tab bar behavior where applicable.
- Undock simple and large node cases.
- Sizing and split payload behavior.
- Window appearing layout and preserve docking info.
- Viewport focus, platform close, and owner change.

### Existing Open-GPUI Areas To Consolidate

The plan should consolidate rather than replace these existing strengths:

- Pure graph and layout tests in `graph_*tests.rs`, `workspace_*tests.rs`, and `layout_tests.rs`.
- GPUI visual and host interaction tests in `host_render_tests.rs`, `host_interaction_tests.rs`, `host_presentation_scene_tests.rs`, and host viewport tests.
- Native Windows scenario rows in `native_windows_interactive.native-scenarios.toml`.
- Benchmark fixtures in `benchmark_support.rs`, `tab_drag_latency.rs`, and `benchmark_docking_drag.py`.
