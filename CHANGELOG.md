# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-07-06

### Added

- Added `open-gpui-ui-core` and `open-gpui-ui-components` as the first component-library
  foundation, including theme tokens, component contracts, accessibility helpers, overlay
  primitives, choice/menu/select/combobox surfaces, table and tree foundations, and gallery
  conformance coverage.
- Added `open-gpui-command` for command-center workflows, including command providers, keymap
  preflight, shortcut inspection and editing state, conflict diagnostics, palette query history,
  and provider refresh plumbing.
- Added `open-gpui-docking` as a GPUI-native docking foundation with tab stacks, split layouts,
  drag/drop targets, floating panels, multi-viewport routing, viewport lifecycle helpers, debug
  status reporting, and a native docking example.
- Added `open-gpui-canvas` as a reusable infinite-canvas foundation with JSON Canvas
  import/export, document mutation journaling, tool sessions, spatial/runtime caches, kind
  policies, persistence adapters, and native canvas examples.
- Added `open-gpui-motion` for renderer-neutral motion foundations, including deterministic
  timeline/spring sampling, scalar motion controllers, policy validation, frame-demand reporting,
  neutral geometry, and layout projection primitives.
- Added stable wasm surface checks for the web backend and a browser-runnable `hello_web` path.
- Added platform/system hooks such as system wake callbacks.

### Changed

- Public-facing framework defaults now use Open GPUI naming instead of Zed naming. Legacy
  `.ZedSans`, `.ZedMono`, `zed::NoAction`, and `zed::Unbind` names remain as compatibility aliases.
- Web builds now bundle usable default fonts and resolve the default UI font stack through Open GPUI
  virtual font names.
- The web backend now keeps wasm applications alive after startup, initializes canvas size before
  first render, and dispatches callbacks without holding mutable callback borrows across user code.
- Overlay, choice, command, table, tree, and gallery internals were tightened around explicit
  runtime requests and contract objects, reducing shallow compatibility surfaces before 1.0.
- Canvas APIs were tightened so mutation, paint, runtime cache, edge routing, and kind policy stay
  behind the editor/runtime boundaries.
- Dependency baselines were refreshed, including `windows`/`windows-core` 0.62, `wgpu` 29,
  `reqwest` 0.13, `wasm-bindgen` 0.2.126, and related macOS/Windows platform bindings.

### Fixed

- Fixed web/wasm rendering failures where the canvas stayed at `1x1`, bundled fonts were missing,
  or callback dispatch could panic with `RefCell already borrowed`.
- Fixed Windows dispatcher integration and DirectWrite type alignment after the `windows` 0.62
  upgrade.
- Fixed Linux Wayland clipboard/headless behavior and X11/headless platform regressions ported from
  upstream GPUI fixes.
- Fixed native text rendering in examples.
- Fixed streamed `reqwest` bodies so pending reads preserve the body state.
- Fixed SVG renderer regressions, list scrolling behavior, scheduler leak checks, and process-tree
  cleanup on Darwin.
- Fixed focus restore behavior for Select, Combobox, and dialog Command overlays on selection,
  Escape dismissal, and outside-press dismissal.

### Security

- Resolved the cargo audit findings tracked during the dependency upgrade sweep.

### Breaking Changes

- `ZED_ALLOW_ROOT` is replaced by `OPEN_GPUI_ALLOW_ROOT`.
- `get_shell_safe_zed_path` is replaced by `get_shell_safe_app_path`.
- `get_zed_cli_path` is replaced by `get_open_gpui_cli_path`.
- `open-gpui-http-client` no longer exposes Zed Cloud/API/LLM URL helpers such as
  `build_zed_api_url`, `build_zed_cloud_url`, and `build_zed_llm_url`.
- New keymap JSON should use `open_gpui::NoAction` and `open_gpui::Unbind`. The old `zed::...`
  action names are still accepted as deprecated aliases for migration.
- Platform-specific identifiers for keyrings, pasteboard metadata, Windows credential targets, and
  Windows window classes now use Open GPUI names.

## [0.1.0] - 2026-06-09

### Added

- Root-level fork attribution and licensing notes, plus per-crate `NOTICE` files that preserve
  upstream copyright notices.
- A publish-check workflow that validates leaf crate packaging first and package contents for the
  rest of the workspace.

### Fixed

- Fork dependencies now resolve from crates.io via `open-gpui-scap` and `open-gpui-font-kit`, and
  publishable Open GPUI crates no longer inherit the workspace root's `publish = false` guard.

### Changed

- Public package names and Rust import paths are standardized around the `open-gpui` /
  `open_gpui::...` branding.
- Workspace metadata is aligned to the fork author and unified version line for the first release.
