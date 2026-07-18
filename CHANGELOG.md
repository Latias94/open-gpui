# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking Changes and Migration Notes

- `open-gpui-docking` is now facade-first: normal apps should build through `DockSurface`, low-level model/controller work should import from `open_gpui_docking::model`, direct viewport integration should import from `open_gpui_docking::runtime`, and diagnostics should import from `open_gpui_docking::advanced`; affected symbols include `DockTransitionPlan`, `DockTransitionExecutionState`, `DockViewportRuntimeStatus`, `DockVisualAffordanceDebugSummary`, `DockVisualAffordanceDebugLayer`, `DockViewportTearOffCancelReason`, `{DockController, DockControllerBuilder}`, `DockSurface::from_controller`, `DockHostOptions`, `DockPanelDescriptor`, and `DockPanelDescriptor: Eq`.
- `open-gpui-motion` now presents a duration-first animation facade at the crate root and keeps adapter/runtime internals under `open_gpui_motion::advanced`; migrate low-level imports such as `MotionExecutionPlan`, convert `MotionProgressExecution::start` lifecycle code to `MotionTransition::progress_run` plus elapsed-duration sampling, treat `MotionFrameReason` as diagnostic vocabulary, and call `MotionFrameHost::reset` with an explicit reset reason.
- `open-gpui-canvas` now has explicit API tiers: common document/editor/store/view APIs remain at the root, GPUI paint/view helpers move to `open_gpui_canvas::adapter`, persistence contracts move to `open_gpui_canvas::persistence`, and low-level graph/geometry/routing/index APIs move to `open_gpui_canvas::advanced`; affected root imports include `CanvasPaintFrame`, `CanvasPaintModel`, `CanvasPersistenceStore`, `CanvasJsonPersistenceCodec`, `CanvasGraphIndex`, `CanvasGeometryFacts`, `SpatialIndex`, `CanvasDefaultEdgeRouter`, and `CanvasEdgeDirection`.
- `open-gpui-ui-components` now separates default component APIs from GPUI adapters, recipes, anatomy helpers, and advanced foundation contracts; use key-first `VirtualizedListState` APIs instead of `virtualized_list_navigation_target` or `virtualized_list_scroll_target`, import GPUI hooks such as `VirtualizedList::render_row` through `VirtualizedListGpuiExt`, import `UiA11yElementExt` from `gpui_adapter`, move `{ToolbarItem, SidebarItem, ListboxOption}` to their owner modules, replace primitive `FieldState` with `FormControlState`, and update `VirtualizedListBehaviorSnapshot` assertions to count enabled unique options only.
- `open-gpui-ui-components` moves `TableBehaviorSnapshot` and its companion diagnostic snapshots out of the crate root/common prelude into `open_gpui_ui_components::table`; `Table` and `TableVirtualizerSnapshot` remain common imports.
- `open-gpui-ui-core` moves the production runtime cache invalidation key `TableStateCacheKey` from the crate root and `open_gpui_ui_core::table::prelude` to `open_gpui_ui_core::table` without a compatibility alias.
- `open-gpui-ui-core` removes `TABLE_ROW_MODEL_PIPELINE`, `TABLE_ROW_MODEL_V0_PIPELINE`, and `TableRowModelStage::implemented_in_v0`; use executable `TableResolvedState` stage models instead of version-label metadata.
- `open-gpui-ui-components` removes central inventory/evidence/status APIs such as `ComponentApiInventoryEntry`; use narrow `ComponentContractEntry` / `ComponentContractMetadata`, typed public export facts, Gallery stories, and test-owned scenario artifacts instead.
- `open-gpui-ui-core` now keeps the root prelude foundation-only; table, split, grid viewport, and virtualizer contracts should be imported from their module-local preludes rather than relying on `open_gpui_ui_core::prelude::*`.
- `open-gpui` scroll handlers now return typed intent and committed viewport facts; update `InteractiveElement::on_scroll_wheel`, `InteractiveElement::capture_scroll_wheel`, `Interactivity::on_scroll_wheel`, and `Interactivity::capture_scroll_wheel` callbacks to return `ScrollWheelIntent`, and update exhaustive matches on `ScrollViewportChangeSource`.
- `open-gpui-devtools` now sanitizes exported diagnostics and adapter payloads by default; replace `SnapshotDiagnostic { probe_id, message }` with `SnapshotDiagnostic::new` or `SnapshotDiagnostic::collection_failed`, avoid raw private values in `ProbeId::new`, handle `SnapshotKind::as_label` returning `Cow`, update snapshot assertions for sanitized `SnapshotNode::new` output, import adapter helpers from `open_gpui_devtools::adapters::*`, and rename command helpers such as `command_keybinding_projection_envelope` to the new `..._snapshot_...` forms.
- `open-gpui-devtools` event selection is now identity-first: replace `DevtoolsInspectorState::select_event(sequence)` and `selected_event_sequence()` with `select_event_identity(&row.event_identity)` and `selected_event_identity()`, and treat `DevtoolsEventIdentity::as_key()` as a sanitized stable selector/diff key rather than the old colon-joined raw-ish display string.
- `open-gpui-devtools` resolved semantic payloads now pair `contract_id` with numeric `contract_revision` and canonical `family`; update strict JSON consumers and checked-in fixtures.

### Changed

- `VirtualizedList` internals are split into descriptor, model, render-plan, runtime, render, style, and motion modules while keeping the public facade key-first.
- `VirtualizedList` now has explicit async/infinite status rows for initial loading, prepend loading, append loading, exhausted, empty, error, and retry states; keyed measured reveal after prepends; presentation-only sticky section overlay metadata and rendering; and theme-backed `VirtualizedListColors`.
- `VirtualizedListDataSource` now lets component-library users project domain records, section rows, and async status rows into renderer-neutral list descriptor storage before rendering.
- `FormControlState` now centralizes renderer-neutral size, disabled, read-only, invalid, required, controller-driven, editability, activation, and tab-stop metadata across `Field`, `TextInput`, `Textarea`, and `NumberInput`.
- Core scroll handling now exposes typed `ScrollWheelIntent`, committed `ScrollViewportSnapshot` facts, and `TestInputDispatchSnapshot` probes so product code and tests can assert final viewport/input outcomes instead of scraping render plans.
- `open-gpui-docking` now supports product panel placement through `DockPanelPlacement`, descriptor default placement, last-known reopen placement, and explicit close/reopen outcome facts.
- `open-gpui-ui-components` now lets `VirtualizedList` hosts own the scroll handle, reveal stable keys through `VirtualizedListState::scroll_target_for_key*`, and keep nested actions contained without taking row ownership away from the list.
- Command and component actions now carry typed icon metadata through `CommandIconDescriptor`, `ActionDescriptor`, `ResolvedActionState`, and `ActionIconDiagnostic` across toolbar, menu, context menu, command, sidebar, button, and icon-button surfaces.
- Web verification now includes a stable browser smoke for app readiness, canvas initialization, focus/input delivery, single-window shell interaction, and explicit unsupported platform-viewport capability on web.
- Release verification now checks changelog release notes, user-facing README versions, public crate README coverage, breaking-change inventory coverage, and public documentation links before publishing crates or GitHub Release notes.
- Open GPUI now declares Rust 1.92 as the workspace MSRV and verifies MSRV drift, duplicate dependency versions, and cargo-audit results through a dedicated dependency-health gate.
- User entry points now include a minimal single-window docking example and refreshed crate READMEs for component, motion, docking, web, platform, and verification workflows.
- `open-gpui-devtools` split its GPUI feature implementation into runtime DTO/capture, inspector controller, and render modules while preserving the `open_gpui_devtools::gpui` facade and root re-exports.
- `open-gpui-devtools` now exposes `DevtoolsWorkbench`, a renderer-neutral app-owned wrapper around local sessions, bounded history, refresh status, inspector state, and sanitized diff readouts.
- `open-gpui-devtools` now exposes `DevtoolsReport` and `open-gpui-devtools-report/v1` for headless diagnostics, and `xtask devtools` can render, diagnose, diff, and stream DevTools artifacts as JSON, Markdown, or JSONL.
- `open-gpui-devtools` now exposes schema-versioned artifact records and writer sinks for app-owned capture/session/report producers; `xtask devtools` can query, assert, and follow artifacts through stable selectors and bounded wait semantics.
- Gallery and docking-native now publish deterministic sanitized DevTools fixture artifacts, and DevTools reports include first-pass docking, layout, motion, command, form, and resource findings for headless UI debugging.

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
