---
title: UI Motion Runtime Foundation - Plan
type: refactor
date: 2026-07-02
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-plan-bootstrap
execution: code
depends_on:
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
  - docs/adr/0012-docking-runtime-capability-alignment.md
  - docs/plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md
---

# UI Motion Runtime Foundation - Plan

## Goal Capsule

Build a small renderer-neutral motion runtime foundation in `ui_core`, then migrate the two current layout motion consumers, `ui_components::Splitter` and `gpui_docking`, onto that shared primitive.

The goal is capability alignment, not pixel-level parity with any reference implementation. The runtime should make layout motion deterministic, retargetable, reduced-motion-aware, and testable while keeping domain-specific scene structures in their owning crates.

This plan intentionally accepts breaking internal APIs. The project is not treating the current docking and splitter motion internals as stable public contracts.

## Product Contract

### Users

- Framework authors building reusable GPUI UI components.
- Docking users evaluating split, dock, tab, preview, zoom, and focus behavior in native examples.
- Future component authors who need motion without copying bespoke timeline logic.

### User-facing outcomes

- Programmatic layout changes animate consistently across splitter and docking.
- Pointer-driven dragging remains direct and does not lag behind the cursor.
- Mid-animation retargets start from the currently sampled visual state instead of jumping or drifting.
- Reduced motion completes to the correct semantic state without spatial animation.
- Docking tab previews stay anchored to their semantic target instead of following stale pointer samples.

### Non-goals

- Do not build a broad public animation framework for every GPUI element.
- Do not introduce a native compositor or CoreAnimation backend in this phase.
- Do not move docking graph, tab, viewport, or drop-zone semantics into `ui_core`.
- Do not replace GPUI frame scheduling. Adapters continue to request frames from the window/runtime they already own.
- Do not target pixel-perfect ImGui, BonSplit, or SuperSplit visuals.

## Planning Contract

### Existing evidence

- `crates/ui_core/src/motion.rs` already owns renderer-neutral vocabulary: `MotionPreference`, `MotionDuration`, `MotionEasing`, and `MotionSpec`.
- `crates/ui_core/src/split.rs` already owns renderer-neutral split scene and transition vocabulary: `SplitterLayoutScene`, `SplitterLayoutTransition`, panel transition kinds, and handle transition kinds.
- `crates/ui_components/src/splitter.rs` has local runtime mechanics for fractions, transition start time, sampling, completion, and interpolation.
- `crates/gpui_docking/src/transition_executor.rs` has a second local runtime with similar concerns: active plan, motion spec, start time, last sample, test start time, sample progression, completion, and retargeting.
- ADR 0011 placed renderer-neutral split/motion vocabulary in `ui_core`, GPUI adapter scheduling in `ui_components`, and docking semantics in `gpui_docking`.
- ADR 0012 placed docking presentation geometry authority in `DockPresentationScene` and docking transition sampling in `DockTransitionExecutor`.

### Design constraints

- `ui_core` may define value-shape-agnostic motion runtime contracts, timeline sampling, retarget metadata, stable identity helpers, and deterministic test-clock hooks.
- `ui_core` must not depend on GPUI windows, elements, rendering layers, cursor state, or docking-specific data structures.
- `ui_components` and `gpui_docking` own frame scheduling because only adapters know when to call `request_animation_frame`.
- Adapters own domain interpolation policies for panels, dividers, drop zones, panes, tabs, focus views, and zoomed layouts.
- Runtime APIs should be deep modules: small call surface, explicit invariants, and enough internal structure to prevent each component from reimplementing time/progress/retarget behavior.

### Required capabilities

- Stable identity based retargeting: when a transition is interrupted, matching items start from their current sampled values.
- Deterministic sampling: tests can sample progress at exact instants without sleeping or wall-clock flake.
- Reduced motion semantics: reduced motion produces final semantic state immediately while preserving completion behavior.
- Completion semantics: callers can distinguish active, completed, cancelled, and immediate transitions.
- Enter/leave policy hooks: adapters can decide how new or removed items appear without encoding those policies in `ui_core`.
- Pointer-drag bypass: pointer-driven layout updates can stay immediate while programmatic changes use the runtime.
- No stale sample reuse: pointer-coupled preview layers and semantic target previews cannot be retargeted from unrelated previous samples.

## Implementation Units

### Unit 1: Add the shared motion runtime primitive

Create the shared runtime in `ui_core` as either a new module, `crates/ui_core/src/motion_runtime.rs`, or a focused extension to `crates/ui_core/src/motion.rs`. Re-export it through the existing `ui_core` module/prelude pattern if the crate already exposes similar primitives there.

The primitive should cover:

- Motion timeline state derived from `MotionSpec`.
- Progress sampling with deterministic `Instant` input.
- Completion and cancellation state.
- Retarget construction from the current sampled state.
- Stable identity matching helpers.
- Reduced-motion and immediate-transition behavior.

Expected files:

- `crates/ui_core/src/motion.rs`
- `crates/ui_core/src/motion_runtime.rs`
- `crates/ui_core/src/lib.rs`
- `crates/ui_core/src/prelude.rs`

Implementation guidance:

- Keep generic interpolation outside the runtime core unless the implementation can express it without leaking component semantics.
- Prefer explicit structs and enums over callback-heavy APIs.
- Make reduced motion a first-class branch, not a duration hack.
- Name APIs around runtime behavior, not visual style. For example, prefer timeline/sample/retarget wording over spring/animation naming unless the implementation actually provides those models.

Verification:

- Unit tests for progress at start, midpoint, end, and after end.
- Unit tests for immediate and reduced-motion completion.
- Unit tests for retargeting from sampled state by stable identity.
- Unit tests for missing identity behavior.
- Unit tests that use deterministic timestamps only.

### Unit 2: Migrate `ui_components::Splitter` runtime

Replace the splitter's local transition timing and progress logic with the shared `ui_core` primitive. Keep splitter-specific fraction interpolation and drag policy in `crates/ui_components/src/splitter.rs`.

Expected files:

- `crates/ui_components/src/splitter.rs`
- `crates/ui_components/src/lib.rs` only if exports need adjustment.
- `crates/ui_components/src/prelude.rs` only if exports need adjustment.

Behavior to preserve:

- Direct pointer drag writes the live splitter fractions immediately.
- Programmatic insert/remove/collapse/expand/resize transitions can animate.
- A second programmatic change during an active transition retargets from current sampled fractions.
- Reduced motion completes to the requested final fractions immediately.

Cleanup:

- Remove duplicated splitter-only timeline state if the shared primitive covers it.
- Keep splitter-only geometry and panel fraction policy local.
- Delete unused helpers after migration instead of keeping compatibility shims.

Verification:

- Splitter tests cover direct drag, programmatic transition, retarget while active, reduced motion, and completion.
- Component API inventory tests still pass if the crate has one for this surface.

### Unit 3: Migrate docking transition execution onto the shared runtime

Refactor `DockTransitionExecutor` so it delegates timeline, progress, completion, and retarget mechanics to the shared primitive while keeping docking-specific plan, sample, scene, pane, divider, focus, zoom, tab, and overlay semantics local to `gpui_docking`.

Expected files:

- `crates/gpui_docking/src/transition_executor.rs`
- `crates/gpui_docking/src/transition_geometry.rs`
- `crates/gpui_docking/src/overlay_scene.rs`
- `crates/gpui_docking/src/render.rs`
- `crates/gpui_docking/src/host_transition_tests.rs`
- `crates/gpui_docking/src/host_render_tests.rs`
- `crates/gpui_docking/src/host_interaction_tests.rs`

Behavior to preserve:

- Drop-zone and tab preview overlays stay semantically anchored during hover and retarget.
- Existing docking target selection behavior remains unchanged.
- Active transitions complete to the final `DockPresentationScene`.
- Retargeting starts from the current sampled pane/divider/overlay state when identities match.
- Missing identities use docking-owned enter/leave policies.
- Reduced motion completes without leaving stale overlay or transition state.

Cleanup:

- Remove duplicate start-time/progress/completion fields if the shared primitive owns them.
- Keep docking-specific sample structs if they describe docking semantics.
- Delete stale overlay retarget branches that only existed to patch around missing runtime identity rules.

Verification:

- Docking transition tests cover normal completion, retarget while active, reduced motion, missing identity enter/leave behavior, and stale preview regression.
- Docking render tests cover sampled layer rendering and pointer-coupled preview pinning.
- Docking interaction tests cover edge drop zones, center tab preview, cross-window docking, and subwindow-to-main-window retarget cases already found by manual testing.

### Unit 4: Add a small motion proof surface

Add or extend a lightweight example/test surface that can exercise splitter and docking motion states without relying only on manual native-window testing.

Expected locations:

- Existing UI component gallery or example crate, if available.
- Existing docking native example or runtime status panel, if that is the only practical proof surface.
- Unit tests first if no maintained gallery exists.

Requirements:

- Show or assert the active motion state, sampled progress, and reduced-motion mode.
- Include a simple programmatic splitter change.
- Include a docking transition path that retargets while active.
- Avoid adding a marketing demo or visually heavy showcase.

Verification:

- The proof surface builds in normal workspace checks.
- Any new manual example steps are documented in a repo-local verification note.

### Unit 5: Update architecture docs and remove obsolete code

Update docs only where the implementation changes the boundary described by ADR 0011 or ADR 0012.

Expected files:

- `docs/adr/0015-ui-motion-runtime-foundation.md` if a new architectural decision is needed.
- `docs/verification.md` or an existing verification note if manual example steps are added.
- This plan file can be linked from follow-up status notes.

Documentation requirements:

- State that `ui_core` owns renderer-neutral motion runtime primitives.
- State that adapters own frame scheduling and semantic interpolation.
- State that docking remains descriptor-first and scene-authoritative.
- Record any deleted compatibility APIs so future agents do not reintroduce them.

## Verification Contract

Run focused tests first, then the affected package suites:

```sh
cargo nextest run -p open-gpui-ui-core motion split --no-fail-fast
cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
cargo nextest run -p open-gpui-docking --no-fail-fast
cargo fmt --all -- --check
git diff --check
```

If a proof example is changed, also run the relevant build/check command for that example package. For the docking native example, prefer:

```sh
cargo check -p open-gpui-docking-native
```

Manual verification should cover:

- Drag a tab to center: tab preview remains anchored and does not drift.
- Drag a tab to a pane edge: edge affordance remains visible and dropping docks into the expected side.
- Drag from a subwindow back into the main window: retargeting starts from the current visual state.
- Programmatically change a splitter: it animates without affecting direct pointer drag.
- Enable reduced motion, if available: transitions complete semantically without spatial movement.

## Definition of Done

- `ui_core` contains the shared motion runtime primitive and tests for its invariants.
- `ui_components::Splitter` no longer owns duplicate timeline/progress/completion mechanics.
- `gpui_docking::DockTransitionExecutor` uses the shared primitive for timing and retarget scaffolding.
- Docking-specific semantics remain in `gpui_docking`; splitter-specific fraction policy remains in `ui_components`.
- Stale sample reuse is covered by regression tests.
- The affected package tests pass with `cargo nextest`.
- Formatting and diff whitespace checks pass.
- Any boundary changes are captured in an ADR or verification note.

## Follow-up Candidates

- Add a compositor-backed runtime adapter only after the renderer-neutral primitive is stable.
- Add zoom/unzoom motion after docking and splitter share the same motion runtime semantics.
- Add focus-view motion after zoom/unzoom has proven the layer identity model.
- Investigate accessibility announcements for programmatic layout movement as a separate plan.
- Revisit external references such as BonSplit, ImGui docking, and the SuperSplit design notes after the primitive proves it can express current project behavior cleanly.
