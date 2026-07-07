---
type: Verification Evidence
title: Runtime canvas docking depth refactor verification
status: passed
timestamp: 2026-07-07T12:09:14Z
git_branch: main
related_plan: docs/plans/2026-07-07-003-refactor-runtime-canvas-docking-depth-plan.md
tags:
  - verification
  - ce-work
  - canvas
  - docking
  - wasm
---

# Verification Matrix

Focused gates passed:

- `cargo nextest run -p open-gpui-canvas document tool gpui public_surface_tests --no-fail-fast`: 211 passed.
- `cargo nextest run -p open-gpui-docking drop_target viewport_drop_route viewport_drop_delivery public_surface_tests --no-fail-fast`: 119 passed.
- `cargo nextest run -p open-gpui-ui-components public_surface --no-fail-fast`: 40 passed.
- `cargo nextest run -p open-gpui-ui-components --no-fail-fast`: 474 passed.

Workspace and release gates passed:

- `cargo fmt --all --check`.
- `cargo check --workspace --locked`.
- `cargo run -p xtask -- verify`.
- `git diff --check`.

Stable wasm and browser smoke gates passed:

- `cargo check -p open-gpui-web --target wasm32-unknown-unknown --tests --locked -j 1`.
- `cargo check -p open-gpui-web --target wasm32-unknown-unknown --locked -j 1`.
- `cargo check -p open-gpui-platform --target wasm32-unknown-unknown --locked -j 1`.
- `cargo check -p open-gpui-wgpu --target wasm32-unknown-unknown --locked -j 1`.
- `cargo run -p xtask -- web-smoke`.

# Notes

The first final `xtask verify` run exposed a stale `ScrollArea` public API inventory baseline for `on_scroll_viewport_changed`; the inventory and callback vocabulary were updated, then `open-gpui-ui-components` and the full `xtask verify` gate passed. The first wasm web tests check exposed a wasm-only unused import warning for `MouseButton`; the import was made non-wasm-only, then all stable wasm gates passed warning-clean.

# Citations

- Progress: `docs/knowledge/engineering/progress/2026-07-07-runtime-canvas-docking-depth-refactor-final.md`
- Plan: `docs/plans/2026-07-07-003-refactor-runtime-canvas-docking-depth-plan.md`
