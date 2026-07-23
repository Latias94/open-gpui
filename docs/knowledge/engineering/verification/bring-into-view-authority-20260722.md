---
type: Verification Evidence
title: Window-owned bring-into-view authority
status: verified
timestamp: 2026-07-22
git_branch: refactor/ui-framework-authority-convergence
verified_by:
  - cargo fmt --all -- --check
  - cargo nextest run --locked -p open-gpui bring_into_view --no-fail-fast
  - cargo nextest run --locked -p open-gpui --test bring_into_view_surface --no-fail-fast
  - cargo nextest run --locked -p open-gpui -E "test(autoscroll_commits_only_when_its_transformed_source_survives_paint) | test(deferred_guard_capture_does_not_reborrow_list_state_during_item_prepaint) | test(setting_follow_tail_cancels_a_deferred_list_reveal)" --no-fail-fast
  - cargo nextest run --locked -p open-gpui-ui-components -E "test(~virtualized_list_)" --no-fail-fast
  - cargo nextest run --locked -p open-gpui-ui-components -E "test(~tree_)" --no-fail-fast
  - cargo check --locked -p open-gpui-ui-components
  - cargo nextest run --locked -p open-gpui-ui-components --no-fail-fast
  - cargo nextest run --locked -p open-gpui-ui-foundation-gallery --no-fail-fast
  - cargo run --locked -p xtask -- scan-ui-contract
  - cargo run --locked -p xtask -- scan-public-api --check
  - cargo run --locked -p xtask -- scan-doc-links
  - cargo run --locked -p xtask -- verify-release-docs
  - cargo run --locked -p xtask -- scan-theme-drift
  - cargo run --locked -p xtask -- scan-theme-schema
  - git diff --check
---

# Verification

- Added an opaque, same-window `RevealTargetHandle` whose final successful frame records checked
  geometry and committed inner-to-outer scroll ancestry.
- Application, winning-focus, and AccessKit requests share one request sequence, overlap
  arbitration, transform conversion, pixel-grid convergence, and terminal-outcome authority.
- Instant and Motion-backed requests use the same physical-axis alignment and cancellation model;
  effective reduced motion completes without creating a second scroll path.
- Explicit portals reset ancestry. Virtual collections materialize stable logical identity before
  handing final physical alignment to GPUI.

# Tests Added Or Tightened

- Core tests cover nested vertical and mixed-axis scrollports, every alignment, margins, oversized
  targets, already-visible and saturated containers, no progress, wrong-window use, cached and
  deferred geometry, portals, suppression, non-uniform transforms, overlapping and disjoint chains,
  user override, unmount, close, focus arbitration, AccessKit, animation, and reduced motion.
  Deferred-guard coverage verifies capture before a target binds, later same-chain submission,
  outer-ancestor direct-scroll rejection, scroll-axis capability changes, and preservation of a
  direct scroll on an unrequested axis. `ScrollChainFence` coverage also verifies that a fenced
  focus claim can settle without replaying an interrupted implicit reveal. `ListState` exposes its
  direct-input revision without reborrowing layout state during child prepaint; explicitly enabling
  tail following and an accepted local autoscroll both advance the vertical revision, so either
  operation rejects a previously captured deferred reveal.
- VirtualizedList tests cover fixed and measured materialization, unavailable data retry, duplicate
  identity rejection, filtering, reorder between phases, stale completion, ABA replacement, and
  direct-scroll interruption before and after the next-frame physical submission. Geometry retry
  reopens only a completed physical request; every cancellation terminates the stale operation.
  Tree re-resolves a current unique logical focus target after reorder and does not materialize a
  focus claim that loses the same window turn or to a later ordinary prepaint commit. Its terminal
  focus-stable phase rejects a competing focus claim, and a rejected static handoff retries only
  when no newer claim has replaced it. Command, Listbox, and Table consumers use the same physical
  authority after their own domain-specific materialization.
- The Gallery scenario proves application, keyboard-focus, AccessKit, animation, wheel
  cancellation, and virtual materialization through one transformed nested flow whose target ends
  inside the virtual, inner, and outer committed viewports.

# Public Contract

- Retain one target with `Window::new_reveal_target` and bind it on every rendered frame with
  `RevealTargetExt::track_reveal_target`.
- Request final physical alignment with `Window::bring_into_view`; use
  `bring_into_view_with_completion` only when the exact terminal outcome matters.
- Every successfully published accessibility node exposes `ScrollIntoView` as a geometry action.
  Disabled nodes may retain it without gaining Click or Focus capability; stale, suppressed, and
  unpublished nodes cannot route it.
- Use explicit horizontal and vertical policies. `BringIntoViewOptions::vertical` preserves the
  horizontal offset.
- Keep collection keys and row identities in the collection adapter. Materialize first, then reveal
  the bound physical target.
- Use direct `ScrollHandle` operations only for intentional low-level scrolling; they cancel any
  affected in-flight reveal.
- `Window::request_autoscroll` remains a local direct-scroll protocol for an accepting container,
  not a nested bring-into-view request. An accepted List autoscroll likewise cancels older reveal
  work on that List.
- A custom deferred adapter captures `DeferredBringIntoViewGuard` from prepaint inside the
  intended final scroll ancestry as soon as logical materialization completes, then submits it
  through the guarded window method after the target binds. The guard rejects direct interruption,
  a missing target, or a changed nested ancestry or scroll-axis capability without entering reveal
  authority; it must not be replaced with a new baseline after user input.
- Retain `ScrollChainFence` for a virtual materialization or cross-frame focus handoff. Submit a
  focus handoff through `Window::focus_with_completion_and_scroll_fence` so focus arbitration
  remains ordinary while an interrupted fence suppresses only automatic physical reveal.
- `Window::record_prepaint_focus_stable_commit` is a terminal prepaint checkpoint for a callback
  that must observe all normal commit mutations. It rejects focus and blur mutations from that
  callback and is used by virtual Tree materialization rather than as a general scheduling escape
  hatch.

# References

- Architecture decision: `docs/adr/0025-open-gpui-bring-into-view-authority.md`
- Component contract: `docs/ui/component-contract.md`
- Migration guide: `docs/ui/migration-v0.3.md`
- Core implementation: `crates/gpui/src/window/bring_into_view.rs`,
  `crates/gpui/src/elements/reveal_target.rs`
- Virtual collection integration: `crates/ui_components/src/virtualized_list/runtime.rs`
- Gallery integration: `examples/ui-foundation-gallery/src/shell/bring_into_view.rs`
