---
title: Motion Component Docking v0.2.0 Convergence - Plan
type: refactor
date: 2026-07-07
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: explicit-user-confirmation
execution: code
---

# Motion Component Docking v0.2.0 Convergence - Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Finish the pre-v0.2.0 convergence work for `open-gpui-motion`, `open-gpui-ui-components`, and `open-gpui-docking`: restore a normal checkout verification baseline, make motion frame ownership consistent, deepen `VirtualizedList` as a component-library primitive, and make crate documentation discoverable. |
| Authority | Current `main` at `4df8947`, `docs/adr/0018-open-gpui-motion-crate-boundary.md`, `docs/plans/2026-07-06-002-refactor-open-gpui-motion-system-plan.md`, `docs/plans/2026-07-06-003-refactor-virtualized-list-motion-v020-plan.md`, `docs/plans/2026-07-05-002-refactor-web-docking-viewport-capability-gates-plan.md`, `docs/verification.md`, and focused read-only repo research. |
| Execution profile | Fearless pre-1.0 refactor. Breaking APIs, deleting misleading compatibility paths, and removing optional examples from the default workspace are allowed when they reduce future migration cost. |
| User confirmation | The user explicitly approved implementing the whole plan with `$compound-engineering:ce-work`, allowing subagents, commits, merges to local `main`, and pushes to remote `main`. No extra scope interview is required. |
| Product boundary | Motion remains renderer-neutral and emits samples, policy reports, geometry/projection facts, and frame demand. Components and docking own GPUI adapters, semantic state, scroll/focus/hit testing, platform capabilities, and concrete frame requests. |
| Stop conditions | Stop and re-plan if the implementation requires a global animation scheduler, public DOM/React/WAAPI compatibility, public stable presence/enter-exit APIs, motion-owned selection/focus/layout semantics, making web multi-viewport optimistic by default, or keeping a default workspace member that depends on missing sibling repositories. |
| Tail ownership | Implementation owns code changes, focused tests, docs, progress notes, logical commits, merge to local `main`, and remote push when the focused gates pass or when the user explicitly asks to land a partial slice. |

---

## Product Contract

### Summary

The previous two plans already extracted the motion crate and rebuilt `VirtualizedList` from a label-only demo into a key-based list component with row descriptors, custom row renderers, measured rows, and active-indicator motion. This plan is the convergence pass before v0.2.0. It does not reopen the entire architecture. It prioritizes the pieces that still block users and maintainers:

- a clean Cargo graph in a normal clone;
- one adapter-facing motion frame protocol instead of per-component boolean request logic;
- the minimum `VirtualizedList` interaction depth expected from a reusable component before v0.2.0;
- crate-level README/discovery for motion, docking, and components;
- verification and docs that match the current capability-gated web/docking reality.

### Problem Frame

Open GPUI is now close to having a coherent component, motion, and docking story, but several seams still feel like implementation leftovers rather than public framework design:

| Current fact | Risk | Plan response |
|---|---|---|
| `examples/canvas-jellyflow` is a workspace member with sibling path dependencies outside this checkout. | `cargo metadata`, `cargo check -p ...`, and release/CI gates fail before loading the package graph. | Remove it from the default workspace or otherwise make it optional so a normal checkout can verify. |
| `MotionFrameHost` exists and `VirtualizedList` consumes it, but `Splitter` returns `bool` and docking transition executors call `Window::request_animation_frame()` through `sample(window)`. | Frame scheduling decisions drift across adapters, and the core/adapter split is harder to teach. | Migrate first-party motion consumers to a shared frame-demand adapter pattern. |
| `VirtualizedList` already supports descriptors, status rows, custom rendering, measured rows, key reveal, multi-select data, and active-indicator motion. | Repeating the old refactor would waste time, but missing interaction affordances still make it weaker than a component-library list. | Add typeahead, range selection, sticky section summary, and stronger measured-scroll anchoring without letting motion own semantics. |
| Web docking multi-viewport capability gates already exist. | Future work may accidentally expose optimistic multi-window behavior on unsupported web/Wayland surfaces. | Audit and document capability names, fail-closed behavior, and focused tests rather than adding another gate shape. |
| `crates/motion` has a README, but `ui_components` and `gpui_docking` do not have crate READMEs. | Users cannot quickly learn which crate owns motion, docking, and component APIs for v0.2.0. | Add concise crate READMEs and cross-link the component contract and verification docs. |

### Requirements

- R1. A normal checkout must load Cargo metadata and run focused package checks without needing sibling `../crates/jellyflow` repositories.
- R2. `MotionFrameHost` or an equivalent `MotionFrameDemand` adapter decision must be the first-party pattern for component/docking motion frame requests.
- R3. Motion core must not import GPUI windows, UI component state, docking graph state, platform backends, DOM, CSS, or WAAPI.
- R4. `Splitter`, `VirtualizedList`, and docking transition rendering must translate motion frame demand into GPUI frame requests at adapter boundaries only.
- R5. `VirtualizedList` must support reusable component-library interactions beyond text labels: key-first typeahead, multi-select range operations, structural rows, custom row rendering, measured rows, and deterministic reveal snapshots.
- R6. `VirtualizedList` motion remains paint-only chrome for active-descendant indication. It must not change selection, focus order, hit testing, a11y roles, scroll offsets, or row layout authority.
- R7. Docking platform viewport windows remain explicitly gated by both app policy and backend capability; unsupported web/Wayland paths must fail closed.
- R8. Motion, components, and docking crates must have discoverable README-level documentation with current capabilities, explicit non-goals, and verification pointers.
- R9. Public docs and `docs/verification.md` must match the final API shape and verification commands.
- R10. Focused tests must cover each new behavior, and broad verification must be attempted after the Cargo graph blocker is fixed.

### Acceptance Examples

- AE1. Given a fresh checkout without sibling Jellyflow repositories, `cargo metadata --no-deps --format-version 1` succeeds.
- AE2. Given Splitter programmatic layout animation, the runtime samples motion, exposes frame demand through the shared adapter pattern, and pointer drag remains immediate.
- AE3. Given docking pane or visual-affordance transitions, sampling no longer embeds a direct window request inside the lower-level executor; render/presentation adapter code owns the GPUI frame request.
- AE4. Given a `VirtualizedList` with explicit `text_value` fields, typing a prefix focuses the next matching enabled item by key and does not select it.
- AE5. Given a multi-select `VirtualizedList`, Shift-range selection selects the stable key range between the anchor and the active item, skipping non-selectable structural rows.
- AE6. Given section rows and scroll movement, the behavior snapshot exposes the current sticky section summary without making section rows selectable.
- AE7. Given measured rows with changed heights, key-based reveal uses exact measurements when available, estimated geometry when not, and avoids stale removed-key measurements.
- AE8. Given reduced motion, active-indicator and layout motion publish final presentation state without continuing frame demand.
- AE9. Given web or Wayland docking, platform-window tear-off/multi-viewport remains unavailable unless both policy and backend capability are present.
- AE10. Given a user opens crate docs or READMEs, they can tell when to use `open-gpui-motion`, `open-gpui-ui-components`, and `open-gpui-docking`, and which animation/docking features are intentionally deferred.

### Scope Boundaries

#### In Scope

- Workspace manifest cleanup for optional examples that depend on missing sibling repositories.
- First-party frame-demand adapter convergence in motion consumers.
- `VirtualizedList` typeahead, range selection, sticky section snapshot, measured-scroll anchoring, examples, and tests.
- Docking transition executor cleanup around frame request ownership.
- Web/docking capability-gate audit, docs, and targeted regression tests if gaps are found.
- README/docs for `crates/motion`, `crates/ui_components`, `crates/gpui_docking`, `docs/ui/component-contract.md`, and `docs/verification.md`.
- Focused nextest/check/doc-test gates and a final broad verification attempt.

#### Deferred

- Public stable `MotionValue` subscription graph.
- Public stable presence/enter-exit API.
- Keyframes, repeat/reverse/speed controls, variants, stagger orchestration, and full shared-layout projection.
- Native compositor, browser WAAPI, CSS transition, or worklet backends.
- Full table/tree/command rewrites on top of `VirtualizedList`.
- Browser multi-window support beyond the existing explicit capability model.

#### Outside Product Identity

- Treating Open GPUI motion as a DOM compatibility layer.
- Letting motion mutate semantic state.
- Using row index as public identity for virtualized list activation/selection.
- Hiding unsupported platform behavior behind optimistic API names.
- Making CI require private or locally adjacent repositories.
- Re-adding `canvas-jellyflow` to the default workspace before its sibling dependencies are vendored, published, or moved into this repo.

---

## Planning Contract

### Key Technical Decisions

- KTD1. Fix verification first. The missing `jellyflow` sibling dependency blocks ordinary Cargo graph loading, so it is the first P0 slice.
- KTD2. `MotionFrameHost` is the adapter vocabulary. Consumers may still expose domain-specific samples, but frame request decisions should be derived from `MotionFrameDemand` through a shared host/update object.
- KTD3. Lower-level executors should not require `Window` just to sample motion. Render/presentation adapters call GPUI frame APIs after inspecting frame demand.
- KTD4. `VirtualizedList` is already descriptor-based; do not redo that work. Add missing interaction depth and preserve the layered contract: state resolves keys, virtualizer resolves ranges, adapter owns scroll/focus/render, motion paints chrome.
- KTD5. Sticky section support starts as behavior/render-plan metadata for v0.2.0. A rendered sticky overlay is a P2 follow-up unless this plan first proves viewport positioning, a11y, custom renderer, scroll, and hit-test semantics.
- KTD6. Range selection is key-based and selectable-row-only. Disabled rows, sections, separators, loading, empty, and error rows are skipped.
- KTD7. Web docking multi-viewport remains capability-gated, not feature-name-gated alone. Feature flags can expose compile-time code, but runtime behavior still requires typed backend facts.
- KTD8. Documentation is part of the API surface for v0.2.0. READMEs must state supported behavior and non-goals plainly.
- KTD9. Commit boundaries follow risk boundaries: workspace baseline, motion adapter convergence, VirtualizedList interactions, docs/verification.
- KTD10. v0.2.0 release gates are narrower than the full improvement backlog. Cargo graph health, motion frame ownership, crate docs, default runnable demos, and the key VirtualizedList interactions block v0.2.0; sticky overlay rendering and extra capability audit cleanup must not delay those gates unless they reveal a regression.

### Assumptions

- `main` is synced with `origin/main` at the start of this plan.
- The user accepts breaking changes and deletion of misleading code because v0.2.0 is pre-stable.
- `docs/solutions/` and root `CONCEPTS.md` do not exist in this checkout; relevant durable memory lives in `docs/knowledge/engineering/`, ADRs, and previous plans.
- External web research is not required for this convergence slice because local ADRs, current code, and existing reference-repo research already constrain the design.
- If a user or another agent edits unrelated files during implementation, those changes must remain untouched and unstaged unless explicitly incorporated.

### Current State Snapshot

- `open-gpui-motion` exists with timelines, springs, policy, neutral geometry/projection, `MotionFrameDemand`, and `MotionFrameHost`.
- `VirtualizedList` already has key-based descriptors, section/separator/loading/empty/error rows, custom row renderer, measured row mode, key reveal targets, multi-select data, and active-indicator motion using `MotionFrameHost`.
- `Splitter` imports `open_gpui_motion` but returns a local `bool` from runtime sync methods and requests frames directly in render code from that bool.
- `DockTransitionExecutor::sample(window)` samples motion and directly calls `window.request_animation_frame()` when the sample needs a frame.
- Docking platform viewport windows are already guarded by `DockPolicy::allow_platform_viewports` and `PlatformViewportCapabilities::platform_viewport_windows`; web and Wayland fail closed.
- `cargo metadata --no-deps --format-version 1` fails because `examples/canvas-jellyflow` depends on missing sibling path crates.

---

## Priority Order

| Priority | Work | Reason |
|---|---|---|
| P0 | Restore Cargo graph and focused verification baseline. | No architectural refactor is trustworthy while Cargo cannot load metadata in a normal checkout. |
| P0 | Motion frame adapter convergence for Splitter and docking. | This is the last visible mismatch in the core/adapter split and affects all future motion consumers. |
| P0 | Crate READMEs, verification docs, and at least one normal-checkout runnable demo path. | v0.2.0 users need discovery, accurate non-goals, and a working first impression. |
| P1 | VirtualizedList typeahead, range selection, a11y contract, and measured reveal anchoring. | These are the minimum interactions that make the list credible as a component-library primitive. |
| P2 | Sticky section metadata, optional sticky overlay proof, and web/docking capability audit cleanup. | Existing gates and section rows already work; this is refinement and regression insurance. |

---

## Implementation Units

### Preflight. Branch And Working-Tree Setup

- **Goal:** Keep the large refactor isolated from `main` until verification.
- **Requirements:** R10.
- **Files:** Git state only.
- **Approach:** Confirm `git status` and `main`/`origin/main` start from `4df8947` or a newer synchronized commit. Create and switch to `refactor/motion-component-docking-v020-convergence` before U0. Keep user or unrelated changes unstaged unless they are explicitly incorporated by this plan. If remote `main` advances, merge or rebase deliberately before the final local-main merge.

### U0. Restore Normal Checkout Cargo Graph

- **Goal:** Make workspace metadata and focused package checks work without local sibling dependencies.
- **Requirements:** R1, R10.
- **Files:** `Cargo.toml`, `examples/canvas-jellyflow/Cargo.toml`, `README.md`, `docs/verification.md`, optional `examples/canvas-jellyflow/README.md`.
- **Approach:** Remove `examples/canvas-jellyflow` from the default workspace member list and add it to `workspace.exclude`, or convert it into a nested standalone workspace. If the example source is kept, make its manifest self-contained rather than relying on `*.workspace = true`, and document that it requires adjacent `jellyflow` and `jellyflow-open-gpui` checkouts until those crates are vendored, published, or moved into this repo.
- **Breaking decision:** A default workspace member that cannot load in a normal clone is worse than an optional example. Remove it from the default workspace now.
- **Default experience:** A normal checkout must still have at least one runnable non-sibling demo path, such as `examples/smoke-native`, `examples/docking-native`, `examples/ui-foundation-gallery`, or `examples/canvas-notes`. Root docs should point users to those first and label `canvas-jellyflow` as an optional external showcase.
- **Tests:** `cargo metadata --no-deps --format-version 1`; `cargo check -p open-gpui-motion --tests --locked`; `cargo check -p open-gpui-ui-components --tests --locked` if platform dependencies allow.

### U1. Converge Motion Frame Adapter Ownership

- **Goal:** Make `MotionFrameHost` and `MotionFrameDemand` the shared adapter protocol for first-party motion consumers.
- **Requirements:** R2, R3, R4, R8.
- **Files:** `crates/motion/src/frame_host.rs`, `crates/motion/src/controller.rs`, `crates/motion/tests/public_contracts.rs`, `crates/ui_components/src/splitter.rs`, `crates/gpui_docking/src/transition_executor.rs`, `crates/gpui_docking/src/presentation_commands.rs`, `crates/gpui_docking/src/render.rs`, relevant docking/splitter tests.
- **Approach:** Keep motion core host-neutral. In Splitter, replace boolean frame return values with `MotionFrameDemand` or `MotionFrameHostUpdate` so render code requests a frame from an adapter decision. In docking, remove `Window` from low-level transition sampling, carry frame demand on `DockTransitionSample`, and request frames in presentation/render adapter code. Preserve terminal sample behavior and retargeting.
- **Epoch/reset contract:** `MotionFrameHost` state must be reset when a consumer changes motion run epoch, cancels, reaches terminal prune, or starts a new elapsed-time sampling run. Consumers that already own elapsed sampling should use `observe(MotionFrameDemand)` rather than `sample_elapsed` or `sample_since`.
- **Test scenarios:**
  - `MotionFrameHost` aggregates idle and active demand deterministically.
  - Splitter programmatic animation requests frames while in flight and idles at terminal state.
  - Splitter drag sync cancels programmatic motion and does not schedule an animation frame through motion.
  - Retarget, cancel, and terminal prune reset or bypass host elapsed state so the next run cannot sample against stale elapsed time.
  - Docking animated start/midpoint samples carry demand; terminal and reduced-motion samples do not.
  - Docking transition retargeting still starts from sampled visual geometry.
- **Verification:** `cargo nextest run -p open-gpui-motion --no-fail-fast`; focused Splitter and docking transition tests.

### U2. Add VirtualizedList Typeahead

- **Goal:** Add key-first typeahead over `VirtualizedListStateItem::text_value`.
- **Requirements:** R5, R6.
- **Files:** `crates/ui_components/src/virtualized_list/mod.rs`, `crates/ui_components/src/roving_focus.rs`, `crates/ui_components/tests/layout.rs`, gallery samples.
- **Approach:** Reuse the existing roving/typeahead helper where possible. Add state-level target resolution for the next enabled selectable row from the current active key. Adapter keydown handling updates active key and pending reveal, but does not select or activate.
- **Test scenarios:**
  - Typeahead skips disabled and structural rows.
  - Typeahead starts after current active key and wraps deterministically.
  - Explicit `text_value` wins over visible label.
  - No match leaves active key, selected keys, and reveal state unchanged.
  - Empty, loading, and error-only lists ignore typeahead.
  - Runtime test proves focus/active key changes without selection.

### U3. Add VirtualizedList Range Selection

- **Goal:** Make multi-select lists usable for real component-library workflows.
- **Requirements:** R5, R6.
- **Files:** `crates/ui_components/src/virtualized_list/mod.rs`, `crates/ui_components/tests/layout.rs`, gallery samples, `docs/ui/component-contract.md`.
- **Approach:** Track a stable `selection_anchor_key` in adapter runtime and use replacement-style range selection. Plain click and plain Space toggle the target row and set the anchor to that key. Plain non-shift navigation, paging, Home/End, and typeahead move the active key and set the anchor to the new active key. Shift+Arrow moves active and replaces `selected_keys` with all selectable keys between the anchor and active key. Shift+Space applies the same anchor-to-active selectable range. Shift+Click replaces `selected_keys` with the anchor-to-clicked selectable range. If the anchor key is missing, filtered out, disabled, or structural, fall back to the active or clicked key before resolving the range. Enter activates without mutating selection. Selection changes emit changed key and the full selected-key set.
- **A11y contract:** Active descendant/focus metadata follows typeahead and range keyboard movement. Every range-selected option exposes selected state. Multi-select capability is exposed where the component model supports it. Structural, disabled, empty, loading, and error rows remain non-focusable and non-selectable.
- **Test scenarios:**
  - Range selection is stable after reorder because it resolves keys against current item order.
  - Non-selectable rows are skipped.
  - Single-select mode ignores range behavior.
  - Anchor fallback is deterministic when the anchor key is removed, filtered, disabled, or structural.
  - Shift+Arrow, Shift+Space, and Shift+Click all use the same replacement-style range.
  - Enter still activates without mutating multi-selection.

### U4. Add Sticky Section Metadata

- **Goal:** Expose the current section heading for virtualized lists with structural rows without blocking v0.2.0 on overlay rendering.
- **Requirements:** R5, R6, R8.
- **Files:** `crates/ui_components/src/virtualized_list/mod.rs`, gallery samples, `docs/ui/component-contract.md`.
- **Approach:** Extend render-plan/behavior snapshot with current sticky section derived from the first visible selectable row and preceding section row. Keep section rows non-selectable and avoid changing option counts. A rendered sticky overlay is deferred to P2 unless this slice proves it is non-focusable, non-interactive, not announced as a duplicate section row, and compatible with custom row renderers, scroll containment, hit testing, and overscan.
- **Test scenarios:**
  - Snapshot reports no sticky section when no section rows exist.
  - Snapshot reports the preceding section for visible item rows.
  - Snapshot reports no sticky section when there is no visible selectable row, no preceding section, or the current row set is loading/empty/error only.
  - Sticky metadata does not change active/selected indices or a11y roles.
  - Gallery sample exposes grouped/sticky behavior.

### U5. Strengthen Measured Scroll Anchoring

- **Goal:** Make measured-row reveal and measurement invalidation reliable enough for v0.2.0 docs.
- **Requirements:** R5, R6, R10.
- **Files:** `crates/ui_components/src/virtualized_list/mod.rs`, `crates/ui_core` virtualizer tests if needed, `crates/ui_components/tests/layout.rs`.
- **Approach:** Audit `row_measurements` keying, removed-key pruning, and `scroll_target_for_key_with_snapshot`. Add tests for changed heights, removed keys, estimated fallback, and active-row reveal after PageDown.
- **Test scenarios:**
  - Exact measurements are used when present.
  - Removed keys do not keep stale heights.
  - Estimated fallback is labeled as estimated.
  - Active row remains visible after keyboard paging and measurement changes.

### U6. Add Crate READMEs And v0.2.0 Discovery Docs

- **Goal:** Make the motion/component/docking crates understandable to users.
- **Requirements:** R8, R9.
- **Files:** `crates/motion/README.md`, `crates/ui_components/README.md`, `crates/gpui_docking/README.md`, `README.md`, `CHANGELOG.md`, `docs/ui/component-contract.md`, `docs/verification.md`.
- **Approach:** Keep READMEs concise and accurate. Motion README states the core/adapter split and supported primitives. UI components README explains concrete GPUI components and highlights `VirtualizedList`, Splitter, overlays, and component contract docs. Docking README explains graph/workspace/host/viewport capability gates. Docs should point normal-checkout users to runnable demos before optional external showcases. Update changelog only if it remains appropriate for v0.2.0 release notes.
- **Acceptance:** A new user can choose the right crate and run a default demo without reading source modules.

### U7. Audit Docking Web/Multi-Viewport Capability Gates

- **Goal:** Keep web and unsupported backends fail-closed for platform viewport windows.
- **Requirements:** R7, R9, R10.
- **Files:** `crates/gpui_docking/src/*viewport*`, `crates/gpui_web`, `crates/gpui_platform`, `docs/verification.md`, `docs/knowledge/engineering/current-state.md`.
- **Approach:** Confirm public docs and tests consistently describe the two gates: `DockPolicy::allow_platform_viewports` and `PlatformViewportCapabilities::platform_viewport_windows`. Add only missing tests or docs. Do not invent another feature gate if typed runtime capability already covers the behavior.
- **Priority note:** This is P2 unless the audit finds an actual regression. U6 docs can ship first using the existing capability-gate facts; U7 only appends corrections if it discovers drift.
- **Test scenarios:**
  - Unsupported backend route preview records an unsupported status.
  - Open/tear-off no-ops with diagnostics when capability is absent.
  - Supported test backend still works when both gates are true.
  - Stable wasm package checks still compile.

### U8. Final Verification, Review, Commit, Merge, Push

- **Goal:** Land the work cleanly and keep `main` current.
- **Requirements:** R10.
- **Files:** All changed files plus progress/verification notes if added.
- **Approach:** Run focused gates after each slice. Before landing, inspect `git diff`, run formatting, run targeted nextest/check commands, and attempt `cargo run -p xtask -- verify` if local platform dependencies allow. Use subagent/code-review for diff review before the final commit when useful. Commit with Conventional Commit messages, merge to local `main`, and push `origin/main` after verification.

---

## Verification Contract

### Required Focused Gates

```bash
cargo metadata --no-deps --format-version 1
cargo fmt --all --check
cargo check -p open-gpui-motion --tests --locked
cargo nextest run -p open-gpui-motion --no-fail-fast
cargo nextest run -p open-gpui-ui-components virtualized_list --no-fail-fast
cargo nextest run -p open-gpui-ui-components tree_typeahead_targets_visible_focusable_items_from_current_focus tree_runtime_typeahead_focuses_visible_matching_row --no-fail-fast
cargo nextest run -p open-gpui-docking host_transition --no-fail-fast
cargo nextest run -p open-gpui-docking host_viewport_platform_capability --no-fail-fast
```

### Platform/CI Gates To Attempt

```bash
cargo check -p open-gpui-web --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-platform --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-wgpu --target wasm32-unknown-unknown --locked -j 1
cargo run -p xtask -- verify
```

If a local command fails for an environmental reason, record the exact blocker and preserve the focused passing gates.

---

## Landing Strategy

- Commit 1: workspace/Cargo baseline and optional example docs.
- Commit 2: motion frame adapter convergence for Splitter and docking.
- Commit 3: VirtualizedList typeahead/range/a11y and measured reveal hardening.
- Commit 4: crate READMEs, verification docs, and v0.2.0 changelog adjustments.
- Optional commit 5: sticky section metadata or capability-audit cleanup if it does not delay the release gates.
- Merge local feature branch back into `main` after focused gates pass.
- Push `origin/main` after merge verification, unless remote has advanced and needs a normal pull/rebase/merge review.

---

## Definition Of Done

- Cargo graph loads in a normal checkout.
- Motion frame request ownership is consistent across first-party motion consumers.
- `VirtualizedList` has key-first typeahead, range selection, sticky section metadata, and measured anchoring tests.
- Docking web/multi-viewport docs and tests match fail-closed capability behavior.
- Motion, components, and docking crate docs are discoverable.
- Focused tests pass or any remaining failures are clearly categorized as pre-existing/environmental.
- Work is committed, merged to local `main`, and pushed to `origin/main` when verified.

---

## Planner Self-Review

- This plan intentionally does not reopen the completed motion extraction or VirtualizedList descriptor rewrite.
- The first implementation slice fixes the current Cargo graph blocker before larger refactors.
- Motion scope stays below public presence/keyframe/value-graph stabilization.
- The plan includes breaking/deletion decisions where preserving compatibility would keep a bad default checkout or misleading API shape.
- The user already selected goal execution, so the next step is `$compound-engineering:ce-work` against this plan rather than another approval menu.
