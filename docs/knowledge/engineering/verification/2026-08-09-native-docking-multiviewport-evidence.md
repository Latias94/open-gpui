---
type: Verification Evidence
title: Windows native Dock capture, provisional presentation, and surface shutdown
status: pending
timestamp: 2026-08-09
git_branch: refactor/ui-framework-authority-convergence
git_commit: 7c144212f88689727ec65bf3e7b608a037bf6f5d
verified_by:
  - cargo nextest run -p open-gpui-windows --locked --no-fail-fast --test-threads 1
  - cargo nextest run -p open-gpui-docking --locked --no-fail-fast --test-threads 1
  - cargo nextest run -p open-gpui-docking-native --locked --no-fail-fast --test-threads 1
  - cargo nextest run -p open-gpui-windows --locked --run-ignored only -E 'test(/native_interactive_runner_sentinel_proves_system_pointer_delivery_and_capture/)' --test-threads 1 --no-fail-fast
  - cargo nextest run -p open-gpui-docking-native --locked --run-ignored only --no-fail-fast --test-threads 1 -E 'test(/native_interactive_(two_hwnd_captured_drag_routes_preview_and_drop|provisional_is_presented_before_release_and_promotes_same_hwnd|anchor_close_releases_capture_and_retires_dependent_hwnds)/)'
  - cargo run --locked -p xtask -- scan-public-api --check
  - cargo run --locked -p xtask -- scan-ui-contract
  - cargo run --locked -p xtask -- scan-doc-links
  - cargo run --locked -p xtask -- scan-theme-drift
  - cargo run --locked -p xtask -- scan-theme-schema
  - cargo fmt --all -- --check
  - git diff --check
---

# Scope

This evidence records the real Windows slice added for U27/U28 and the live provisional path
owned by U29. It is intentionally separate from the older deterministic DockSurface evidence;
that historical record remains unchanged and is not used as proof of native behavior.

# Runner contract

The scenarios require `OPEN_GPUI_NATIVE_INTERACTIVE=1` and the dedicated
`open-gpui-windows-native-interactive-ephemeral` Windows runner. The runner must provide an
interactive desktop, compatible integrity/UIPI, a functioning renderer, and serialized cursor
ownership. A missing or incapable runner is an infrastructure failure, not a skipped product
assertion.

# Scenarios

`native_interactive_two_hwnd_captured_drag_routes_preview_and_drop` injects an unaddressed
`SendInput` drag into two distinct rendered HWNDs. It proves that the source keeps native capture,
the source framework observer receives the captured move/up while the target observer receives no
mouse event, a non-empty target preview is rendered before release, and exactly one newer durable
graph revision moves the payload from the source space to the target space. The Windows sentinel,
not this scenario, is the typed proof for canary-tagged WndProc receipt.

`native_interactive_provisional_is_presented_before_release_and_promotes_same_hwnd` holds the
physical button down after moving to a runner-selected point outside the two host rectangles. It
requires one additional runtime-opening, visible HWND with a submitted non-empty frame containing
the payload before release. The provisional window is checked for disabled pointer input,
activation, and click focus; durable graph revision and viewport registration remain unchanged.
After the locked release, the same HWND becomes the committed viewport, the gate opens, the payload
moves exactly once, and one durable surface revision is published. A framework observer installed
when the exact window first appears proves it did not receive replayed Move/Down/Up input.

`native_interactive_anchor_close_releases_capture_and_retires_dependent_hwnds` begins a real
captured drag, posts `WM_CLOSE` to the primary anchor, and waits for the typed surface session to
reach `Closed`, the runtime and App window registry to empty, capture and active drag to clear, and
the source and anchor HWNDs to be destroyed with no pending or failed terminal ticket. This proves
final convergence rather than exact native-destroy ordering. The pointer guard restores the
physical button and cursor on success, timeout, or panic; a separate process watchdog prevents an
AsyncApp deadlock from consuming the workflow's full timeout.

# Results

- `open-gpui-windows`: 98 passed, 1 skipped; nextest run id
  `1c1d0057-4137-40aa-a8cb-28255b221517`.
- `open-gpui-docking`: 1,326 passed; nextest run id
  `b3554471-c243-4f9e-8631-2600f4dbc64a`.
- `open-gpui-docking-native`: 26 passed, 4 ignored; nextest run id
  `dbfc6876-2af8-490b-91c5-2d291bbf4dd2`.
- Windows interactive sentinel: 1 passed; nextest run id
  `d9559907-3400-4ce9-ad38-f9464f55bbb7`.
- Real interactive Dock slice: 3 passed; nextest run id
  `60014c75-641a-496b-99be-95f9bab54a97`.
- Public API, federated UI contract, documentation-link, theme-drift, and theme-schema scanners
  passed.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed (PowerShell/Git may report the repository's existing CRLF conversion
  warning; no whitespace error was reported).

# Limits

This snapshot does not claim the complete U28 matrix or a remote GitHub workflow run. Mixed-DPI
divergence, foreign opaque-window occlusion, two-surface isolation, all renderer/device fault
permutations, subprocess lifetime coverage, exact `WM_NCDESTROY` ordering, and every
presentation-shutdown ordering remain separate deterministic or native gates. The local evidence
is complete for the three named vertical slices; the overall U28 release gate remains pending until
the owning-platform workflow and the remaining matrix are green.

# References

- Plan: `docs/plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md` (U27-U29)
- Workflow: `.github/workflows/native-windows-interactive.yml`
- Interactive scenarios: `examples/docking-native/src/native_interactive_tests.rs`
- Older lifecycle evidence: `docs/knowledge/engineering/verification/dock-surface-window-session-authority-20260728.md`
