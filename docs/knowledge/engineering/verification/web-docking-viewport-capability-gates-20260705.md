---
type: "Verification Evidence"
title: "Web docking viewport capability gates"
description: "Verification evidence for stable wasm CI gates and docking platform viewport window capability fail-closed behavior."
timestamp: 2026-07-05T21:52:13+08:00
tags: ["docking", "wasm", "ci", "platform-capabilities", "verification"]
status: "verified"
git_branch: "refactor/web-docking-capability-gates"
related_plan:
  - "docs/plans/2026-07-05-002-refactor-web-docking-viewport-capability-gates-plan.md"
---

# Summary

Stable wasm checks are now encoded in `.github/workflows/verify.yml` for the Linux matrix:
`open-gpui-web`, `open-gpui-platform`, and `open-gpui-wgpu` compile on
`wasm32-unknown-unknown` with `--locked`.

Docking platform viewport windows now require both an application policy opt-in and a backend
capability fact. `DockPolicy::allow_platform_viewports` remains the workspace policy gate.
`PlatformViewportCapabilities::platform_viewport_windows` is the backend fact and defaults to
false. macOS, Windows, X11, and TestPlatform opt in. Web and Wayland stay fail-closed for
platform-window tear-off/multi-viewport behavior, while single-window docking remains available.

# Verification

- `cargo check -p open-gpui-docking --tests --locked`: passed.
- `cargo nextest run -p open-gpui-docking viewport_runtime_handle_drop_route_fails_closed_when_platform_viewport_windows_unsupported viewport_runtime_open_viewport_fails_closed_when_platform_viewport_windows_unsupported viewport_runtime_tear_off_fails_closed_when_platform_viewport_windows_unsupported --no-fail-fast`: passed 3/3.
- `cargo nextest run -p open-gpui-docking host_viewport_route_tests host_viewport_platform_capability_tests host_viewport_placement_tests --no-fail-fast`: passed 79/79.
- `cargo nextest run -p open-gpui-docking host_viewport_lifecycle --no-fail-fast`: passed 53/53.
- `cargo check -p open-gpui-docking-native --tests --locked`: passed.
- `cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast`: passed 1/1.
- `cargo check -p open-gpui-web --target wasm32-unknown-unknown --locked -j 1`: passed.
- `cargo check -p open-gpui-platform --target wasm32-unknown-unknown --locked -j 1`: passed.
- `cargo check -p open-gpui-wgpu --target wasm32-unknown-unknown --locked -j 1`: passed.
- `cargo fmt --all --check`: passed.
- `git diff --check`: passed.
- `python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering`: passed with existing rollup-size and absolute-path warnings.

# Behavior Notes

- Route preview now records `PlatformViewportWindowsUnsupported` instead of pretending an
  outside-registered-viewport release can tear off on unsupported backends.
- `DockViewportRuntimeHandle::open_viewport` and the test-only tear-off open path return
  `std::io::ErrorKind::Unsupported` before creating a GPUI window when backend capability is false.
- TestPlatform defaults to supported so existing native-style multi-window tests retain their
  contract; unsupported backend tests opt out explicitly through `TestAppContext`.
- `hello_web` nightly/shared-memory verification remains optional and is not part of the stable CI
  gate.
