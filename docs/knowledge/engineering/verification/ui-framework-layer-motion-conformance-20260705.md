---
type: "Verification Evidence"
title: "UI framework layer motion conformance verification"
description: "Verification evidence for the overlay host, motion execution, choice navigation, component contract, and public API cleanup refactor."
timestamp: 2026-07-05T00:00:00Z
tags: ["ui-core", "ui-components", "gallery", "overlay", "motion", "choice", "public-surface"]
status: "verified"
related_plan: "docs/plans/2026-07-05-001-refactor-ui-framework-layer-motion-conformance-plan.md"
git_branch: "refactor/ui-framework-non-overlay-depth"
---

# Verification

The 2026-07-05 layer/motion/conformance plan was executed as logical commits on
`refactor/ui-framework-non-overlay-depth`.

# Result

Verified with focused compile, nextest, cargo-test fallback, and contract-scan gates.
The shipped boundary now has a private GPUI overlay host, shared motion execution sampling,
narrowed motion projection public API, Menu/Tree navigation through choice behavior, deeper
component-contract gallery projections, and a narrower default public API that no longer exposes
the low-level `roving_focus` module.

# Evidence

- `cargo check -p open-gpui-ui-core --tests`: passed.
- `cargo check -p open-gpui-ui-components --tests`: passed.
- `cargo check -p open-gpui-docking --tests`: passed.
- `cargo check -p open-gpui-ui-foundation-gallery --tests`: passed.
- `cargo nextest run -p open-gpui-ui-core -E 'test(/overlay|motion/)' --no-fail-fast`: passed 63/63.
- `cargo nextest run -p open-gpui-ui-components -E 'test(/overlay|choice|navigation|public_surface/)' --no-fail-fast`:
  passed 95/95 on the follow-up rerun.
- `cargo nextest run -p open-gpui-docking -E 'test(/transition|host_transition/)' --no-fail-fast`: passed 17/17.
- `cargo nextest run -p open-gpui-ui-foundation-gallery -E 'test(/overlay|component/)' --no-fail-fast`: passed 93/93.
- `target/debug/deps/overlay-40f6bb1cc2593e1b --list --format terse`: passed and listed 43 tests.
- `target/debug/deps/overlay-40f6bb1cc2593e1b --nocapture --test-threads=1`: passed 43/43.
- `cargo test -p open-gpui-ui-components --test overlay -- --nocapture`: passed 43/43.
- `cargo test -p open-gpui-ui-components --test choice -- --nocapture`: passed 53/53.
- `cargo test -p open-gpui-ui-components --test navigation -- --nocapture`: passed 19/19.
- `cargo test -p open-gpui-ui-components --test public_surface -- --nocapture`: passed 40/40.
- `cargo run -p xtask -- scan-ui-contract`: passed.
- `cargo fmt --all --check`: passed before the final memory write.
- `git diff --check`: passed before the final memory write.
- Follow-up dependency-upgrade verification in
  `docs/knowledge/engineering/verification/dependency-upgrade-verification-20260705.md`: passed
  scheduler, ui-components, foundation-gallery, xtask, scan, format, and diff gates after the
  dependency upgrade.

# Environment Notes

- `cargo nextest run -p open-gpui-ui-components -E 'test(/overlay|choice|navigation|public_surface/)' --no-fail-fast`
  was initially interrupted after `choice` and `overlay` test binaries appeared to remain in
  `--list --format terse` for several minutes.
- Follow-up diagnosis after the dependency upgrade traced the repeated list-stage stalls to macOS
  dyld/Gatekeeper validation rather than Rust test code. Sampled test binaries were stopped at
  `_dyld_start` before test harness entry, and rebuilding the affected binaries restored normal
  nextest execution.
- The interruption is therefore treated as a local macOS test-binary startup condition, not a failed
  assertion or remaining code defect.

# Commits In Scope

- `c7d39c9` - `refactor(ui-components): introduce overlay layer host facade`
- `2ab0596` - `refactor(ui-components): route overlay dialogs and popups through host`
- `44da5ef` - `refactor(ui-components): host hover and tooltip overlays`
- `0c0bcf2` - `refactor(ui-components): route menu and command overlays through host`
- `4cc195f` - `refactor(ui-core): centralize motion execution sampling`
- `3267933` - `refactor(ui-core): narrow motion projection surface`
- `8e5893d` - `refactor(ui-core): restrict motion execution samples`
- `3af2f75` - `refactor(ui-components): route menu navigation through choice`
- `718ee02` - `refactor(ui-components): route tree navigation through choice`
- `61abcb3` - `refactor(ui-components): deepen component contract projections`
- `a92e625` - `refactor(ui-components): narrow roving focus public surface`
- `50f7cfc` - `build(deps): upgrade workspace dependencies`
- `05f63de` - `test(verification): restore gates after dependency upgrade`

# Citations

- `docs/plans/2026-07-05-001-refactor-ui-framework-layer-motion-conformance-plan.md`
- `crates/ui_components/src/overlay/host.rs`
- `crates/ui_core/src/motion_controller.rs`
- `crates/ui_core/src/motion_projection.rs`
- `crates/ui_components/src/choice.rs`
- `crates/ui_components/src/component_contract/projections.rs`
- `crates/ui_components/tests/public_surface/exports.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery/component_catalog_contracts.rs`
