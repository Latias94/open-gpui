# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking Changes and Migration Notes

- `open-gpui-docking` now keeps diagnostics and transition internals out of the crate root and `prelude`; import `open_gpui_docking::advanced::...` for `DockTransitionPlan`, `DockTransitionExecutionState`, `DockViewportRuntimeStatus`, runtime status record types, `DockVisualAffordanceDebugSummary`, `DockVisualAffordanceDebugLayer`, and `DockViewportTearOffCancelReason`.
- `open-gpui-motion` now requires `MotionFrameHost::reset(MotionFrameHostResetReason::...)` so adapters document why they start a new local motion epoch; replace bare `reset()` calls with the matching retarget, cancel, finish, prune-terminal, or motion-identity reason.
- `open-gpui-docking` adds the required `DockHostOptions::motion_preference` field for host-owned reduced-motion policy; struct literals must set it explicitly or use `DockHostOptions::default()`.
- `open-gpui-ui-components` no longer exports `virtualized_list_navigation_target` or `virtualized_list_scroll_target` from the default component API. Use the key-first `VirtualizedListState::navigation_target`, `scroll_target_for_key`, or `scroll_target_for_key_with_snapshot` methods instead.
- `VirtualizedList` behavior snapshots now count only unique, enabled item rows in listbox option positions and set sizes. Disabled, duplicate-key, structural, and status rows are still rendered, but no longer participate in roving focus or option-set metadata.

### Changed

- `VirtualizedList` internals are split into descriptor, model, render-plan, runtime, render, style, and motion modules while keeping the public facade key-first.
- `VirtualizedList` now has explicit async/infinite status rows for initial loading, prepend loading, append loading, exhausted, empty, error, and retry states; keyed measured reveal after prepends; presentation-only sticky section overlay metadata and rendering; and theme-backed `VirtualizedListColors`.
- Web verification now includes a stable browser smoke for app readiness, canvas initialization, focus/input delivery, single-window shell interaction, and explicit unsupported platform-viewport capability on web.

## [0.2.0] - 2026-07-07

Open GPUI v0.2.0 is the first broad foundation release for the fork. It publishes the component, docking, canvas, command, motion, web, and platform crates to crates.io under the Open GPUI package names.

### Highlights

- Component library foundations are now available through `open-gpui-ui-core` and `open-gpui-ui-components`, including theme tokens, accessibility helpers, overlays, choice/menu/select/combobox primitives, table/tree foundations, and conformance coverage.
- `VirtualizedList` is now a real component-library primitive instead of a text-label list: it supports stable keys, typed rows, custom row rendering, selection modes, sections, sticky metadata, typeahead, measured rows, and motion-backed active-item chrome.
- `open-gpui-command` adds command-center building blocks for providers, keymap preflight, shortcut inspection, conflict diagnostics, palette history, and provider refresh flows.
- `open-gpui-docking` adds a GPUI-native docking foundation with tab stacks, split layouts, drag/drop targets, floating panels, multi-viewport routing, lifecycle helpers, debug status, and a native example.
- `open-gpui-canvas` adds a reusable infinite-canvas foundation with JSON Canvas import/export, journaling, tool sessions, runtime caches, persistence hooks, and native examples.
- `open-gpui-motion` adds renderer-neutral motion primitives for deterministic timeline/spring sampling, progress runs, keyed sequences, policy validation, frame demand, geometry, and layout projection.
- Web and wasm support is substantially more usable: default fonts are bundled, canvas sizing is initialized correctly, callbacks avoid borrow panics, and the browser `hello_web` path is runnable.
- Dependency baselines were refreshed across the workspace, including `windows` 0.62, `wgpu` 29, `reqwest` 0.13, `wasm-bindgen` 0.2.126, and related platform bindings.

### Fixes

- Fixed Windows dispatcher and DirectWrite integration after the `windows` 0.62 upgrade.
- Fixed Linux Wayland clipboard/headless behavior and X11/headless regressions.
- Fixed native text rendering in examples, streamed `reqwest` body reads, SVG renderer regressions, list scrolling behavior, scheduler leak checks, and Darwin process-tree cleanup.
- Fixed focus restore behavior for Select, Combobox, and dialog Command overlays after selection, Escape dismissal, and outside-press dismissal.
- Fixed package archives so publishable crates include Apache-2.0 license text and NOTICE attribution files.

### Security

- Resolved the cargo audit findings tracked during the dependency upgrade sweep.

### Breaking Changes and Migration Notes

- Public-facing defaults now use Open GPUI naming. `ZED_ALLOW_ROOT` becomes `OPEN_GPUI_ALLOW_ROOT`, `get_shell_safe_zed_path` becomes `get_shell_safe_app_path`, and `get_zed_cli_path` becomes `get_open_gpui_cli_path`.
- Platform-specific identifiers for keyrings, pasteboard metadata, Windows credential targets, and Windows window classes now use Open GPUI names.
- `open-gpui-http-client` no longer exposes Zed Cloud/API/LLM URL helpers such as `build_zed_api_url`, `build_zed_cloud_url`, and `build_zed_llm_url`.
- New keymap JSON should use `open_gpui::NoAction` and `open_gpui::Unbind`. The old `zed::...` action names remain accepted as migration aliases.
- `VirtualizedList` was rebuilt before v0.2.0. Index-only activation and text-label-only rows are replaced by stable keys, typed descriptors, explicit selection/activation events, behavior snapshots, sticky section snapshots, row measurement modes, and the `render_row` content boundary.

## [0.1.0] - 2026-06-09

### Added

- Root-level fork attribution and licensing notes, plus per-crate `NOTICE` files that preserve upstream copyright notices.
- A publish-check workflow that validates leaf crate packaging first and package contents for the rest of the workspace.

### Fixed

- Fork dependencies now resolve from crates.io via `open-gpui-scap` and `open-gpui-font-kit`, and publishable Open GPUI crates no longer inherit the workspace root's `publish = false` guard.

### Changed

- Public package names and Rust import paths are standardized around the `open-gpui` / `open_gpui::...` branding.
- Workspace metadata is aligned to the fork author and unified version line for the first release.
