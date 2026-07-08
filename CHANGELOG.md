# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking Changes and Migration Notes

- `open-gpui-docking` now keeps diagnostics, transition internals, raw graph/action/workspace types, host construction, and direct viewport runtime outcomes out of the crate root and `prelude`; use `DockSurface` for normal apps, `open_gpui_docking::model::...` for graph/layout tooling, `open_gpui_docking::runtime::...` for direct viewport runtime integration, and `open_gpui_docking::advanced::...` for diagnostics such as `DockTransitionPlan`, `DockTransitionExecutionState`, `DockViewportRuntimeStatus`, runtime status record types, `DockVisualAffordanceDebugSummary`, `DockVisualAffordanceDebugLayer`, and `DockViewportTearOffCancelReason`.
- `open-gpui-motion` moved low-level scalar/runtime/spec/model imports such as `MotionExecutionPlan` under `open_gpui_motion::advanced`; ordinary component motion should use the root `MotionTransition`, `MotionScalarRun`, `MotionProgressRun`, `MotionFrameDriver`, and sequence facades.
- `open-gpui-motion` progress runs are duration-first at the root; replace common `MotionProgressExecution::start(... Instant)` usage with `MotionTransition::progress_run(started_at: Duration)` and sample through elapsed `Duration` or `MotionClockSample` values.
- `open-gpui-motion` marks `MotionFrameReason` as non-exhaustive; prefer `MotionFrameDemand::needs_frame()` for scheduling decisions and keep reason matching diagnostic-only with a wildcard arm.
- `open-gpui-motion` now requires `MotionFrameHost::reset(MotionFrameHostResetReason::...)` so adapters document why they start a new local motion epoch; replace bare `reset()` calls with the matching retarget, cancel, finish, prune-terminal, or motion-identity reason.
- `open-gpui-docking` adds the required `DockHostOptions::motion_preference` field for host-owned reduced-motion policy; struct literals must set it explicitly or use `DockHostOptions::default()`.
- `open-gpui-docking` no longer treats `DockPanelDescriptor: Eq` as a public contract now that descriptors carry default and last-known product placement facts; use `PartialEq` or field-level comparisons instead.
- `open-gpui` `InteractiveElement::on_scroll_wheel`, `InteractiveElement::capture_scroll_wheel`, `Interactivity::on_scroll_wheel`, and `Interactivity::capture_scroll_wheel` callbacks now return `ScrollWheelIntent`; return `allow_default()` or `handled()` and use raw wheel variants only for low-level adapter-owned behavior.
- `open-gpui` `ScrollViewportChangeSource` now distinguishes initial layout, resize, content-size changes, wheel/default scrolling, scrollbar, keyboard, touch, and named programmatic requests; update exhaustive matches and read `ScrollHandle::committed_viewport_snapshot` for final facts.
- `open-gpui-ui-components` no longer exports `virtualized_list_navigation_target` or `virtualized_list_scroll_target` from the default component API. Use the key-first `VirtualizedListState::navigation_target`, `scroll_target_for_key`, or `scroll_target_for_key_with_snapshot` methods instead.
- `VirtualizedListBehaviorSnapshot` now counts only unique, enabled item rows in listbox option positions and set sizes. Disabled, duplicate-key, structural, and status rows are still rendered, but no longer participate in roving focus or option-set metadata.
- `open-gpui-ui-components` no longer exports `{ToolbarItem, SidebarItem, ListboxOption}` from the crate root/default surface. Import them from `open_gpui_ui_components::toolbar::ToolbarItem`, `open_gpui_ui_components::sidebar::SidebarItem`, and `open_gpui_ui_components::listbox::ListboxOption`.
- `VirtualizedList::render_row` and `VirtualizedList::scroll_handle` moved to the `open_gpui_ui_components::gpui_adapter::VirtualizedListGpuiExt` extension trait; import the trait before calling GPUI-only hooks.

### Changed

- `VirtualizedList` internals are split into descriptor, model, render-plan, runtime, render, style, and motion modules while keeping the public facade key-first.
- `VirtualizedList` now has explicit async/infinite status rows for initial loading, prepend loading, append loading, exhausted, empty, error, and retry states; keyed measured reveal after prepends; presentation-only sticky section overlay metadata and rendering; and theme-backed `VirtualizedListColors`.
- Core scroll handling now exposes typed `ScrollWheelIntent`, committed `ScrollViewportSnapshot` facts, and `TestInputDispatchSnapshot` probes so product code and tests can assert final viewport/input outcomes instead of scraping render plans.
- `open-gpui-docking` now supports product panel placement through `DockPanelPlacement`, descriptor default placement, last-known reopen placement, and explicit close/reopen outcome facts.
- `open-gpui-ui-components` now lets `VirtualizedList` hosts own the scroll handle, reveal stable keys through `VirtualizedListState::scroll_target_for_key*`, and keep nested actions contained without taking row ownership away from the list.
- Command and component actions now carry typed icon metadata through `CommandIconDescriptor`, `ActionDescriptor`, `ResolvedActionState`, and `ActionIconDiagnostic` across toolbar, menu, context menu, command, sidebar, button, and icon-button surfaces.
- Web verification now includes a stable browser smoke for app readiness, canvas initialization, focus/input delivery, single-window shell interaction, and explicit unsupported platform-viewport capability on web.
- Release verification now checks changelog release notes, user-facing README versions, public crate README coverage, breaking-change inventory coverage, and public documentation links before publishing crates or GitHub Release notes.
- Open GPUI now declares Rust 1.92 as the workspace MSRV and verifies MSRV drift, duplicate dependency versions, and cargo-audit results through a dedicated dependency-health gate.
- User entry points now include a minimal single-window docking example and refreshed crate READMEs for component, motion, docking, web, platform, and verification workflows.

### Security

- Updated `crossbeam-epoch` to `0.9.20` to resolve `RUSTSEC-2026-0204`.

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
