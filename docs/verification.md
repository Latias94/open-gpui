# Verification

Run the local Open GPUI gate with:

```sh
cargo run --locked -p xtask -- verify
```

The gate runs:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --locked` (including workspace examples such as `open-gpui-docking-minimal`)
- `cargo check -p open-gpui-smoke-native --locked`
- `cargo nextest run -p open-gpui-motion --locked`
- `cargo test -p open-gpui-motion --doc --locked`
- `cargo nextest run -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools --locked`
- `cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked`
- `cargo test -p open-gpui-devtools --all-features --doc --locked`
- package-wide locked nextest gates for `open-gpui`, `open-gpui-ui-core`,
  `open-gpui-ui-components`, and `open-gpui-ui-foundation-gallery`
- locked doctests for `open-gpui-ui-core` and `open-gpui-ui-components`
- `cargo run -p xtask -- verify-release-docs`
- `cargo run -p xtask -- scan-doc-links`
- `cargo run -p xtask -- dependency-health`
- `cargo run -p xtask -- scan-theme-drift`
- `cargo run -p xtask -- scan-theme-schema`
- `cargo run -p xtask -- scan-import-boundary`
- `cargo run -p xtask -- scan-public-api --check`
- `cargo run -p xtask -- scan-ui-contract`

On Windows, `xtask verify` sets `CARGO_BUILD_JOBS=1` only on the two DevTools all-features child
processes. It does not mutate the parent shell environment.

The U11 authority-convergence focused gate is:

```powershell
cargo nextest run --locked -p xtask --lib verify_plan_covers_u11_targets_features_and_process_scoped_environment --no-fail-fast
$env:CARGO_BUILD_JOBS = '1'
cargo nextest run --locked -p open-gpui-devtools --all-features --test framework_adapters --test resolved_semantic_redaction --test table_redaction --no-fail-fast
cargo test --locked -p open-gpui-devtools --all-features --doc
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery u11_gallery_convergence_smoke_composes_real_authorities_in_one_window --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery devtools_gallery --no-fail-fast
cargo run --locked -p xtask -- scan-theme-schema
cargo run --locked -p xtask -- scan-doc-links
```

`table_canaries_never_cross_any_devtools_artifact_boundary` is the Table privacy gate. It injects
unique Table id/label, column id/label, business row id, explicit instance id, grouped value, cell
value, identity diagnostic, and debug-selector forms, then proves none crosses live capture,
history, diff, Inspector detail/copy, session export, headless capture/export/report artifacts,
JSON/Markdown report, or the checked-in fixture. `table_opaque_ids_are_stable_only_inside_their_own_session`
proves that adapter-assigned ordinals are stable for one session without becoming reversible or
globally stable. The resolved-semantic target separately keeps form values/errors, accessible
text, labels, and input values out of the same downstream surfaces. The Gallery Inspector smoke
places a unique value on the real GPUI clipboard route, proves navigation neither ingests nor
replaces it, and then verifies the copied Inspector JSON remains redacted.
`devtools_gallery_headless_fixtures_match_producer_output` applies the shared canary absence check
to the generated Gallery capture, session, and report fixtures.

`framework_focus_snapshot_reads_the_rendered_window_authority` proves DevTools obtains focused
element presence and opaque claim/frame revisions from the real rendered window. Scope and handle
counts remain `None` when no complete producer registry can prove them; unknown counts must never
be serialized as guessed zeroes.

`u11_gallery_convergence_smoke_composes_real_authorities_in_one_window` is intentionally one real
window flow, not a catalog or hand-built snapshot assertion. It composes nested Popover -> Menu ->
Dialog topmost dismissal and LIFO focus restoration, sibling scoped themes with a deferred
surface, a real async-validating `FormStore`, final AccessKit value/busy/relations/action facts,
exactly-once Sidebar semantic activation, and fake-clock Tree typeahead reset. The focused command
is followed by the complete package gates in `xtask verify`; per-domain tests remain the owning
diagnostics when this convergence sentinel fails.

The full `open-gpui` nextest target is the unified gate for pointer-session, focus, input-dispatch,
and window-lifecycle regressions. Focused filters documented below provide faster implementation
feedback but do not replace that package-wide gate in `xtask verify` or CI.

The GitHub Actions `Verify` workflow also runs stable wasm surface checks on the Linux matrix:

```sh
cargo check -p open-gpui-web --target wasm32-unknown-unknown --tests --locked -j 1
cargo check -p open-gpui-web --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-platform --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-wgpu --target wasm32-unknown-unknown --locked -j 1
```

These are stable, single-threaded wasm gates. The Linux matrix also runs the stable browser smoke:

```sh
cargo run -p xtask -- web-smoke
```

`xtask web-smoke` builds `crates/gpui_web/examples/smoke_web` with Trunk, serves the generated files with cross-origin isolation headers, opens a headless Chrome/Chromium/Edge browser, and verifies app readiness, DOM/canvas initialization, focus/input delivery, a single-window shell interaction, the explicit unsupported platform-viewport capability on web, and a `DockSurface` viewport readiness/open probe that returns typed `backend_unsupported` without creating a window or runtime registration. The smoke intentionally avoids the nightly shared-memory example so CI proves the default stable browser path.

Nightly shared-memory/atomics checks for `hello_web` remain optional verification, not CI requirements.

Release and public-documentation gates are split into two focused commands:

```sh
cargo run -p xtask -- verify-release-docs
cargo run -p xtask -- scan-doc-links
```

Before publishing a prepared release tag, generate the GitHub Release notes from the target version section:

```sh
cargo run -p xtask -- verify-release-docs --version <version> --notes-output target/release/release-notes.md
```

`verify-release-docs` checks the target changelog section, rejects manually wrapped changelog bullets and paragraphs, validates user-facing README dependency versions, and requires crate-local README metadata for public entry crates. Daily verification checks `docs/release/breaking-changes.md` against `CHANGELOG.md` `[Unreleased]`; release-note generation checks the same inventory against the selected version section because that is the text published to GitHub Releases. `scan-doc-links` checks strict user-facing relative links in root docs, release docs, verification docs, the ADR index, every engineering decision record, and public crate READMEs. Historical plans and progress logs remain outside the strict link gate until they are archived or indexed.

For the 2026-07 DevTools live runtime workbench slice, the focused local gates are:

```powershell
cargo fmt -p open-gpui-devtools -p open-gpui-ui-foundation-gallery -p open-gpui-docking-native
cargo check -p open-gpui-devtools --no-default-features --tests --locked
cargo check -p open-gpui-devtools --features gpui --tests --locked
cargo check -p open-gpui-devtools --features docking --tests --locked
$env:CARGO_BUILD_JOBS = '1'; cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --all-features --test diff_contracts --test session_contracts --test inspector_contracts --no-fail-fast --locked
cargo check -p open-gpui-ui-foundation-gallery --tests --locked
cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked
cargo check -p open-gpui-docking-native --tests --locked
cargo nextest run -p open-gpui-docking-native --no-fail-fast --locked runtime_status_panel
rg "fn (theme_snapshot|form_snapshot|resource_snapshot|docking_snapshot)" examples/ui-foundation-gallery/src/pages/devtools.rs
$sequencePattern = 'select_event\(0\)|select_event\(sequence|devtools-inspector:event:\{sequence\}'
rg -n $sequencePattern crates examples docs/knowledge/engineering
$debugIdentityPattern = 'format!\(".*DevtoolsEventIdentity|Debug.*DevtoolsEventIdentity'
rg -n $debugIdentityPattern crates examples docs/knowledge/engineering
cargo run -p xtask -- scan-doc-links
cargo run -p xtask -- scan-public-api --check
git diff --check
```

The `rg` static-builder and sequence/debug-selector guards should return no matches. DevTools
session replay is local/offline only: imports validate schema, protocol, bounded history, JSON
size, and event counts before recomputing diffs, and they do not introduce remote transport or
mutation APIs. Event selection is identity-first through `DevtoolsEventIdentity`; append sequence is
display metadata only. GPUI runtime metadata is intentionally narrow: app/window/focus/input/frame
and scroll counters and geometry are allowed, while raw user input, clipboard payloads, editable
text values, unredacted window titles, and accessibility labels remain outside the capture
contract. Docking runtime facts must come from public `DockViewportRuntimeStatus` records; missing
private facts are not inferred.

For the 2026-07 DevTools headless artifact pipeline, the focused local gates are:

```powershell
cargo fmt -p open-gpui-devtools -p open-gpui-ui-foundation-gallery -p open-gpui-docking-native -p xtask --check
cargo check -p open-gpui-devtools --tests --locked
cargo check -p open-gpui-devtools --all-features --tests --locked
cargo nextest run -p open-gpui-devtools --all-features --test artifact_contracts --test report_contracts --test docking_runtime_contracts --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked
cargo check -p xtask --locked
cargo nextest run -p xtask --test devtools_cli_contracts --no-fail-fast --locked
cargo run -p xtask -- devtools --help
cargo check -p open-gpui-ui-foundation-gallery --all-targets --locked
cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked
cargo check -p open-gpui-docking-native --all-targets --locked
cargo nextest run -p open-gpui-docking-native devtools --no-fail-fast --locked
cargo run -p xtask -- scan-public-api --check
cargo run -p xtask -- scan-doc-links
cargo run -p xtask -- verify-release-docs
git diff --check
```

The checked-in artifacts under `crates/devtools/tests/fixtures/` are the fixture owner for CLI
smoke: they cover report, diagnose, diff, stream, query, assert, follow, and bounded wait behavior
without launching Gallery or docking-native. `scan-public-api` includes the `open-gpui-devtools`
root export allowlist so artifact writer types remain intentional and report-rule internals remain
private. The headless pipeline remains local/offline only: no CDP bridge, remote transport, runtime
mutation API, screenshot baseline store, or persistent trace database is part of this gate.

For the 2026-07 scroll viewport and wheel-input intent slice, the core contract is that tracked
scroll surfaces emit committed post-layout viewport facts, typed wheel intent controls default
scrolling and propagation, and focus-on-wheel is opt-in rather than implicit. The focused local
gates are:

Runtime tests should collect final viewport facts through `ScrollViewportChangedEvent` or
`ScrollHandle::committed_viewport_snapshot`, assert simulated-input side effects through
`TestInputDispatchSnapshot` and `VisualTestContext::last_dispatch_event_result`, and assert focus
ownership through `VisualTestContext::debug_selector_is_focused` or
`VisualTestContext::focused_debug_selector`. These probes expose committed correctness facts only;
component render plans, transient dispatch flags, and broader P2 performance telemetry stay private.

```sh
cargo test -p open-gpui scroll_handle_committed_viewport_events --locked
cargo test -p open-gpui scroll_handle_programmatic_reveal_uses_named_source --locked
cargo test -p open-gpui scroll_lifecycle_capture --locked
cargo test -p open-gpui scroll_wheel_intent --locked
cargo test -p open-gpui test_input_dispatch_snapshot --locked
cargo test -p open-gpui plain_scroll_wheel_preserves_focus_without_opt_in --locked
cargo test -p open-gpui scroll_wheel_focus_intent_moves_focus_deterministically --locked
cargo test -p open-gpui test_child_wheel_handler_prevents_parent_list_scroll --locked
cargo test -p open-gpui-ui-components table --locked
```

Dependency health is enforced through:

```sh
cargo run -p xtask -- dependency-health
```

The command checks that every workspace package declares the workspace MSRV, that the declared MSRV is at least the maximum `rust-version` in the resolved dependency graph, that duplicate registry crate versions are explicitly allowlisted, and that `cargo audit --json` reports no unignored vulnerabilities. The current MSRV is Rust 1.92 because the Linux platform dependency chain reaches `oo7 0.6.0`; `wgpu 30`, `naga 30`, `resvg 0.46`, and `usvg 0.46` currently require Rust 1.87. The dependency health workflow runs this command on Linux, and the release workflow requires a successful `dependency-health.yml` run for the release commit before publishing.

For the 2026-07 runtime UI hardening slice, the Web dispatcher exposes a typed
`WebDispatcherMode` through `WebDispatcher::mode()` and `WebPlatform::dispatcher_mode()`. Stable
web builds report `SingleThreaded { reason: BuiltWithoutMultithreadedFeature }`; multithreaded
shared-memory mode remains feature-, browser-capability-, and worker-startup gated. If workers
cannot be started after capability checks pass, the dispatcher reports
`SingleThreaded { reason: WorkerStartupFailed }` instead of panicking. The focused local gates for
this slice were:

```sh
cargo fmt --all
cargo fmt --all --check
cargo check -p open-gpui-web --target wasm32-unknown-unknown --tests --locked -j 1
cargo check -p open-gpui-web --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-platform --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-wgpu --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-windows --all-features --locked
cargo check --workspace --locked
cargo run -p xtask -- scan-ui-contract
cargo run -p xtask -- scan-import-boundary
git diff --check
(cd crates/gpui_web/examples/hello_web && cargo check --target wasm32-unknown-unknown -j 1)
```

The `hello_web` command uses the example's nightly/shared-memory wasm toolchain configuration and
is expected to emit Rust's `-Ctarget-feature=+atomics` warning.

The follow-up runtime/docking hardening pass keeps the same public capability posture and removes
several panic-oriented internal paths. Windows device-lost recovery errors are now logged and
retried instead of panicking, renderer refresh failures re-mark device invalidation, and optional
custom clipboard metadata/image formats are skipped when Windows format registration is
unavailable. The focused local gates that completed for this pass were:

```sh
CARGO_TARGET_DIR=/tmp/open-gpui-u1-check cargo check -p open-gpui-windows --all-features --locked
cargo test -p xtask public_api_snapshot --locked
cargo run -p xtask -- scan-public-api --check
cargo check -p open-gpui-web --target wasm32-unknown-unknown --tests --locked -j 1
cargo check -p open-gpui-docking --tests --locked
cargo fmt --all --check
git diff --check
```

Local `open-gpui` package checks, docking nextest runs, and repeated `xtask` scans can stall in
build-script or test-runner startup on the macOS workstation after prior cargo work has completed.
When that happens, stop the hung local command and use CI as the owner for full-workspace
confirmation rather than treating the incomplete local run as a pass.

The Windows platform backend no longer uses `unimplemented!()` for `hide_other_apps` or
`unhide_other_apps`; unsupported hide-other-apps behavior is a debug diagnostic/no-op on Windows.
The macOS host check proves the crate configuration still compiles locally, while the Windows CI
`open-gpui-windows --all-features` gate remains the final owner for Windows API coverage.

For the 2026-07 Zed GPUI upstream-sync utility slice, the focused U7 verification on the Windows
host was:

```powershell
cargo nextest run -p open-gpui-util
cargo check -p open-gpui-util --locked
cargo check -p open-gpui-util --target x86_64-apple-darwin --locked
cargo run -p xtask -- scan-import-boundary
git diff --check
```

The Windows Job Object process-tree tests ran on the host. The Darwin custom-`PATH` command tests
compiled for `x86_64-apple-darwin`; runtime execution still requires a macOS runner.

For the 2026-07 Zed GPUI upstream-sync SVG renderer slice, the focused U8 verification on the
Windows host was:

```powershell
cargo nextest run -p open-gpui -E "test(text_with_split_glyph_clusters_in_mixed_fonts_does_not_panic) or test(svg_renderer)"
cargo check -p open-gpui --locked
cargo run -p xtask -- renderer-smoke
cargo run -p xtask -- scan-import-boundary
cargo audit
git diff --check
```

The follow-up dependency remediation updated the actionable advisories: `quinn-proto` now resolves
to `0.11.16`, `anyhow` to `1.0.103`, `memmap2` to `0.9.11`, `async-tar` to `0.6.1` with the Tokio
runtime, `futures-lite` to `2.6.1`, `stacksafe` to `1.0.2`, `reqwest` to `0.13.4`, and `crossbeam-epoch` to `0.9.20`. `cargo audit`
now exits successfully. `.cargo/audit.toml` temporarily ignores the two `quick-xml 0.39.4`
advisories because the currently published `wayland-scanner 0.31.10` and `zbus_xml 5.1.1` releases
still pin `quick-xml = "0.39"`, and both reach this workspace through proc-macro/code-generation
paths rather than runtime XML parsing. Remove those ignores once upstream releases accept
`quick-xml >= 0.41`.

The remaining warning-class advisories are not denied by the local audit gate: `paste 1.0.15` comes
from `image`'s AVIF codec chain, and `ttf-parser 0.25.1` comes through the font/SVG stack. Removing
the first would drop AVIF image support; replacing the second requires a renderer/font-stack
migration. The updated SVG stack crates reviewed in this slice (`resvg 0.46.0`, `usvg 0.46.0`,
`imagesize 0.14.0`, `kurbo 0.13.1`, `polycool 0.4.0`, `roxmltree 0.21.1`, `svgtypes 0.16.1`) are
crates.io packages with MIT and/or Apache-2.0 licenses.

For focused `open-gpui-canvas` work, run:

```sh
cargo fmt -p open-gpui-canvas
cargo check -p open-gpui-canvas --benches
cargo nextest run -p open-gpui-canvas
cargo check -p open-gpui-smoke-native
```

When changing Canvas runtime query or cache ownership, also run:

```sh
cargo nextest run -p open-gpui-canvas spatial_cache runtime_query runtime --no-fail-fast
```

When changing Canvas document, tool, GPUI adapter, or root facade internals, also run:

```sh
cargo nextest run -p open-gpui-canvas document tool gpui public_surface_tests --no-fail-fast
```

The canvas crate also has a large-canvas Criterion baseline:

```sh
cargo bench -p open-gpui-canvas --bench large_canvas
```

Use the benchmark to compare spatial-index, visible-query, and paint-frame culling changes. It is
not part of the default CI gate because benchmark timing is runner-dependent.

For focused `open-gpui-ui-core`, `open-gpui-ui-components`, or UI foundation gallery work, run:

```sh
cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-ui-core
cargo check -p open-gpui-ui-components
cargo check -p open-gpui-ui-foundation-gallery
cargo nextest run -p open-gpui-ui-core
cargo nextest run -p open-gpui-ui-components
cargo nextest run -p open-gpui-ui-foundation-gallery
```

For focused `open-gpui-form`, `open-gpui-resource`, or `open-gpui-devtools` ecosystem work, run:

```sh
cargo fmt -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools --tests --locked
cargo check -p open-gpui-devtools --features command --tests --locked
cargo check -p open-gpui-devtools --features motion --tests --locked
cargo check -p open-gpui-devtools --features gpui --tests --locked
cargo check -p open-gpui-devtools --all-features --tests --locked
cargo check -p open-gpui-devtools --features form,resource --tests --locked
cargo check -p open-gpui-devtools --features gpui,motion,docking --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features form --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features resource --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features motion --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features docking --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features gpui --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features ui-components --tests --locked
cargo check -p open-gpui-ui-components --tests --locked
cargo check -p open-gpui-ui-foundation-gallery --tests --locked
cargo nextest run -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --test inspector_contracts --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features command --test command_adapters --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features motion timeline --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features gpui layout --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features form,resource form_resource_adapters --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features gpui,motion,docking framework_adapters --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-components form_adapter resource_adapter --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-foundation-gallery form resource devtools --no-fail-fast --locked
```

`open-gpui-form` and `open-gpui-resource` are renderer-neutral crates; their tests must not require
a live GPUI window. `open-gpui-resource` remains protocol-agnostic, so resource tests should drive
query and mutation generations around fake app-owned fetch results instead of introducing HTTP
policy. Devtools is read-only: tests should assert snapshot collection, filtering, diagnostics,
selection, target/domain/event capture, selected-detail JSON export, and redaction summaries
without mutating app state. On Windows, if `open-gpui-devtools --all-features` nextest hits
`link.exe` LNK1102 while linking test binaries, rerun the same gate with `CARGO_BUILD_JOBS=1`; do
not treat the first linker out-of-memory as a code pass.

The Components gallery has an `ecosystem-adapters` section for `FormFieldProjection`,
`ResourceCollectionProjection`, and `ResourceMutationProjection`. DevTools consumes those same
redacted form/resource snapshots through feature-gated first-party adapters, and framework facts
come from the `ui-components`, `gpui`, `motion`, and `docking` adapter modules when public
read-only snapshots exist. The DevTools gallery page is a separate registry-backed and
capture-backed dogfood page:

```sh
cargo run -p open-gpui-ui-foundation-gallery -- --page devtools
```

For the current `VirtualizedList` and `open-gpui-motion` foundation, run the focused gates below
before relying on the full workspace gate:

```sh
cargo metadata --no-deps --format-version 1
cargo test -p open-gpui-ui-core --locked virtualizer
cargo test -p open-gpui-ui-components --locked --lib virtualized_list
cargo test -p open-gpui-ui-components --locked --test public_surface
cargo test -p open-gpui-ui-components --locked --test layout virtualized
cargo check -p open-gpui-ui-foundation-gallery --tests --locked
cargo test -p open-gpui-ui-foundation-gallery --locked --test foundation_gallery component_catalog_contracts
cargo test -p open-gpui-ui-foundation-gallery --locked --test foundation_gallery component_smoke_tree_virtualized
cargo check -p open-gpui-motion --tests --locked
cargo nextest run -p open-gpui-motion --no-fail-fast --status-level fail --locked
cargo test -p open-gpui-motion --doc --locked
```

These gates cover key-based virtualized state, typed and custom-rendered rows, measured-row
snapshots, typeahead, replacement-style range selection, sticky-section snapshot metadata,
active-indicator motion demand/reduced-motion/offscreen behavior, typed public export ownership,
and gallery scroll/keyboard containment.

For current crate discovery, the normal-checkout user entry points are:

```sh
cargo run -p open-gpui-ui-foundation-gallery
cargo run -p open-gpui-docking-minimal
cargo run -p open-gpui-docking-native
cargo run -p open-gpui-canvas-notes
```

The minimal docking example is the common API entry point. The native docking example is the
dogfood surface for viewport runtime diagnostics and multi-window capability gates. The component,
motion, docking, web, and platform crate guides live at `crates/ui_components/README.md`,
`crates/motion/README.md`, `crates/gpui_docking/README.md`, `crates/gpui_web/README.md`, and
`crates/gpui_platform/README.md`. Public package metadata should point at crate-local README files.

The gallery package includes Components-page runtime smoke coverage for regressions that state-only
tests can miss: short-viewport page scrolling and navigation reset, navigation rail scrolling,
Select popup outside dismissal, nested ScrollArea wheel scrolling, vertical Tabs rail scrolling,
horizontal plus vertical Splitter pointer dragging, Table column resize dragging, and long Sidebar
internal navigation scrolling. Run the gallery package tests before relying on manual dogfood for
those paths.
Overlay gallery smoke coverage now also includes menu submenu hover-open and sibling branch
switching on the rich-items sample, so submenu branch visibility, local hover retention, and
old-branch dismissal are verified through the real gallery shell instead of only through
component-state tests.
Component-state coverage also includes `MenuSubmenuSurface` and `MenuSafeHoverCorridor`, which
prove renderer-neutral submenu placement inputs and safe-hover transition bounds for the floating
submenu panels.
The Components-page ScrollArea regressions also cover release-queue wheel isolation so scroll
gestures on the sample card chrome do not leak to the page shell.
Because the Components page now carries more depth samples, the longer-section smokes also rely on
catalog directory jumps and page-scroll handle alignment instead of only raw page wheel motion;
that keeps the focused inspection paths stable even as the page grows.
The Components page has two inspection modes: the full all-components conformance page, and a
catalog-driven focused component-family view. Directory chips remain pure anchor jumps. Focused
mode is entered from catalog cards and restored through the explicit `All components` control. The
focused-view proof includes a catalog-driven matrix that opens every focusable official or
state-contract catalog entry, plus focused runtime smokes for scroll reset and nested scroll
containment:

```powershell
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focuses_every_focusable_catalog_entry
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focuses_catalog_family_and_restores_all_mode
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_table_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_mode_resets_page_on_family_change
```

For focused docking split, preview, motion, zoom, divider, and accessibility primitive work, keep
the gates aligned to the shared primitive boundary:

```sh
cargo fmt --all -- --check
cargo nextest run -p open-gpui-motion --no-fail-fast
cargo check -p open-gpui-ui-core --tests
cargo nextest run -p open-gpui-ui-components splitter --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
cargo check -p open-gpui-docking-native
git diff --check
```

For docking render-authority convergence work, prove deterministic geometry through
`DockPresentationScene` parity rather than screenshot or pixel-level styling parity:

```sh
cargo fmt --all -- --check
cargo nextest run -p open-gpui-docking host_render_tests host_render_geometry_parity_tests host_presentation_scene_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_viewport_preview_tests host_viewport_preview_visual_tests host_viewport_route_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_interaction_tests --no-fail-fast
cargo check -p open-gpui-docking
cargo check -p open-gpui-docking-native
git diff --check
```

This gate locks root, leaf, tab-bar, empty-space, floating-title/content, split child, splitter,
zoom, divider hit map, and accessibility rectangles to the same deterministic scene/layout
authority. The remaining render-measured probe is intentionally named
`render_tab_label_drop_scene_fact_probe` and may only publish tab-label facts whose final bounds
depend on GPUI text shaping, intrinsic title layout, or close-button layout.

Use narrower checks while iterating:

```sh
cargo nextest run -p open-gpui-ui-core split --no-fail-fast
cargo nextest run -p open-gpui-ui-components splitter gpui_adapter_maps_splitter_role --no-fail-fast
cargo nextest run -p open-gpui-docking geometry host_accessibility_tests host_divider_hit_map_tests workspace_resize_policy_tests graph_split_tests interaction --no-fail-fast
cargo nextest run -p open-gpui-docking spatial_navigation_tests host_zoom_focus_tests::host_focus_neighbor_command_uses_spatial_navigation host_accessibility_tests::accessibility_splitter_actions_resize_through_transaction_path host_accessibility_tests::accessibility_vertical_splitter_actions_target_vertical_axis host_divider_hit_map_tests host_interaction_tests::horizontal_splitter_drag_updates_width_fractions host_interaction_tests::vertical_splitter_drag_updates_height_fractions host_interaction_tests::splitter_drag_clamps_to_minimum_pane_size host_interaction_tests::corner_splitter_drag_updates_both_axes_through_rendered_events interaction::tests::corner_splitter_drag_produces_two_axis_resize_request interaction::tests::corner_splitter_drag_clamps_one_axis_without_corrupting_other_axis render::tests::divider_affordance_states_have_distinct_feedback_colors --no-fail-fast
cargo nextest run -p open-gpui-docking transition_plan_from_route_affordance_describes_source_marker source_hover_over_known_viewport_renders_target_drop_preview routed_preview_replacement_clears_old_target_overlay_without_stale_payload escape_clears_routed_marker_target_overlay_and_active_drag viewport_runtime_begin_payload_drag_clears_previous_routed_preview viewport_runtime_revalidates_routed_preview_release_against_current_policy --no-fail-fast
cargo nextest run -p open-gpui-docking transition_pane_clip_mounts_real_pane_content host_unzoom_command_retargets_from_active_zoom_sample dragging_tab_to_other_stack_center_moves_panel transition_plan_from_overlay_scene_uses_current_bounds_for_matching_layers transition_plan_keeps_preview_layers_at_current_target_bounds overlay_replacement_keeps_preview_layers_at_current_target_bounds --no-fail-fast
```

These checks prove capability alignment instead of pixel parity: tab insertion previews remain tab
previews, nested edge targets stay scoped to the pane that owns the guide, cross-window route
markers stay separate from target previews, zoom/focus produce deterministic descriptors, divider
and corner hits derive from the shared split hit map, and accessibility descriptors expose roles,
bounds, orientation, selected state, disabled state, and actions.

The shared motion runtime checks additionally prove that `open_gpui_motion` owns deterministic
timeline sampling, spring sampling, scalar values, model-neutral scalar samples, frame-demand
reasons, explicit model/preset resolution, layout projection data, motion policy validation,
terminal state, reduced-motion completion, stable-identity retarget matching, and renderer-neutral
projection clips. The public motion surface is the controller/model/policy/projection layer;
`MotionValue` remains a private implementation detail behind `MotionScalarTrack`, and
`open_gpui_ui_core` must not re-export motion contracts.
`SplitterLayoutTransition::sample` now exposes final-content bounds plus visible
clip bounds for insert, remove, resize, collapse, and expand transition descriptors. The GPUI
Splitter adapter consumes those samples for programmatic identity/count/collapse/expand changes via
an overlay path; leaving panel content is retained when callers use view-backed
`SplitterPanel::view` panels, while one-shot element-backed panels keep their existing render
limitations.
`ui_components::Splitter` uses the scalar controller and explicit committed-layout model for
programmatic fraction changes while keeping pointer drags immediate and policy-tested. Docking uses
the same scalar motion model for transition progress, keeps explicit custom timeline specs intact,
renders move/resize panes through renderer-neutral projection visual bounds plus final-size
clip/occlusion layers, and keeps pane, divider, visual-affordance, zoom, focus, tab, route, and
viewport semantics local. Transition pane clips mount real final-size pane content behind an
occlusion mask rather than generic placeholder rectangles, visual-affordance preview geometry stays
pinned to the current semantic target, adapter-owned transition executors still request GPUI
frames, and interrupted zoom/unzoom starts from the current sampled geometry. The native runtime
panel exposes this as
`motion proof: shared-runtime+run-state+scalar-value+scalar-sample+explicit-models+policy-gates+layout-projection+projection-clips+sampled-progress+retargeted-identity+reduced-motion-final-state+high-frequency-bypass`.
The remaining render-measured drop-scene probe is intentionally limited to tab-label facts whose
bounds depend on text shaping; presentation-scene facts own root, pane, tab bar, empty, and
floating-title targets.

Cross-window preview cleanup is part of the same semantic contract. A routed hover may leave a
source-window route marker and a target-window preview at the same time, but those are distinct
overlay layers: source route markers become `RouteMarker` transition descriptors, target previews
own payload tab previews, and replacing the route target must clear the old target overlay before a
release can commit. Escape cancellation during a real GPUI docking drag clears the active drag,
source marker, target preview, and runtime session together without making the docking host steal
ordinary panel focus when no drag is active.

Shared split primitive coverage now owns generic fraction normalization, one-fill-child share
resolution, and pixel-delta adjacent resize helpers in `open_gpui_ui_core`. Docking consumes those
helpers for graph normalization, render flex shares, presentation-scene split layout, and splitter
drag resize transactions. Docking-local geometry should remain limited to docking-specific
drop-guide boxes, central-region target policy adapters, and GPUI `Bounds<Pixels>` conversion.
Docking-private spatial navigation now resolves nearest pane focus targets from the current
presentation scene using direction filtering, perpendicular overlap priority, and distance
tie-breaking. The direction enum is a docking command input, while the rectangle-neighbor resolver
remains private to docking because it depends on docking pane semantics and rendered presentation
facts.

Current docking accessibility output maps supported descriptor data into GPUI element state:
stable IDs, roles, labels, selected/disabled state, orientation, numeric splitter values, tab
focus/select actions, and splitter increment/decrement actions. Docking keeps hint strings and
drop affordance descriptors in its renderer-neutral scene, but GPUI currently has no generic
element API for an accessibility hint/description field or a platform drop action callback. Active
drop, drag-source, and rejected-target affordance nodes are therefore exposed as labeled group
descriptors without inventing unsupported platform actions, and focused tests assert that those
nodes disappear when the visual affordance scene is empty.

Docking visual affordance runtime work should use `DockVisualAffordanceScene` as the visual
feedback authority for drop guides, tab insertion, route markers, divider/corner affordances,
focus rings, zoom egress, accessibility, visual-affordance motion identity, and native diagnostics.
Target previews, route markers, accessibility descriptors, transition plans, and runtime
diagnostics now consume visual affordance descriptors directly; no `DockOverlayScene` semantic
adapter remains. The native docking runtime panel reads runtime-owned visual affordance status and
shows one compact affordance line per viewport with layer count, active layer, scope/state, target
node, zone, payload index, frame generation, and visual-affordance motion state.

Focused visual affordance runtime gates:

```sh
cargo fmt --all -- --check
cargo nextest run -p open-gpui-docking host_viewport_preview_tests host_viewport_preview_visual_tests host_viewport_route_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_render_tests host_transition_tests host_render_geometry_parity_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_accessibility_tests host_divider_hit_map_tests host_debug --no-fail-fast
cargo nextest run -p open-gpui-docking host_interaction_tests host_outside_release host_viewport_drop --no-fail-fast
cargo check -p open-gpui-docking
cargo check -p open-gpui-docking-native
git diff --check
```

Native dogfood command:

```sh
RUST_LOG=info,open_gpui_docking=debug,open_gpui=info RUST_BACKTRACE=1 cargo run -p open-gpui-docking-native --bin open-gpui-docking-native 2>&1 | tee /tmp/open-gpui-docking-native.log
```

Table gallery gates now follow the same split: `open-gpui-ui-core` tests prove row-model,
manual row-model stages, manual expansion, child-load metadata, virtualizer, column sizing,
column-window, row pinning, and resize-math contracts without rendering, including grouped row ids,
expansion lookup behavior, expandable unloaded branches, built-in group-row aggregate cells,
pinned-column region splitting, center-column virtual windows, top/center/bottom row regions,
keep-pinned versus page-only policies, manual filtering/sorting/pagination cache keys, pagination
row/page totals, per-column facet metadata, manual facet payload cache keys, and on-end/on-change
resize deltas. The same core gate proves exact source/group identity through every row-model stage,
snapshot-scoped occurrence invalidation, explicit duplicate instances across source reorder, and
canonical completion of partial column order. `open-gpui-ui-components` tests prove adapter exports,
state metadata, manual row-model render-plan metadata, faceting render-plan metadata, row-pinning
render-plan metadata,
expansion payload metadata, resize callback wiring, center-window header/body mounting, fixed
row-pinned bands, exact edit outcomes, duplicate NodeId and measurement separation, virtual focus
proxy behavior, and scroll ownership; gallery smokes prove long table scroll input stays inside
the table viewport, `release-resize` column dragging updates the controlled sample without moving
the outer Components page, wide center lanes scroll independently from fixed left/right pinned
lanes, `row-pinning` keeps top/bottom row bands fixed while the center body scrolls, `server-paged`
renders an app-owned page snapshot with total counts plus caller-provided facet summaries,
`filter-board` exposes client-derived status counts and score ranges, and `server-tree` renders
app-owned manual child loading. The focused proofs are:

`components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample` is the focused
sticky-pinned Table proof: it enters the Table family view, scrolls the `release-rollup` center
lane horizontally, and asserts left/right pinned lanes plus the outer Components page stay fixed.

`components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample` is the focused
center-column virtualization proof: it enters the Table family view, scrolls the `release-matrix`
center lane horizontally, verifies far center metric cells are unmounted before scrolling and
mounted after scrolling, and asserts left/right pinned lanes plus the outer Components page stay
fixed.

`components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample` is the focused row-pinning
proof: it enters the Table family view, aligns the `row-pinning` sample to an interactive center
cell, wheels inside the center body, and asserts the sample, top pinned band, bottom pinned band,
and left-pinned cells stay fixed while the center row window changes.

`components_gallery_smoke_table_server_tree_loads_children_from_expansion_request` is the focused
manual-expansion proof: it enters the Table family view, starts with `server-api` absent from the
app-supplied source snapshot, clicks the unloaded `server-workspace` disclosure, verifies the
expansion payload carries zero loaded children plus idle child-load state, and then confirms the
new child row renders after the gallery runtime supplies the loaded snapshot.

`components_gallery_smoke_faceted_filter_updates_table_rows` is the focused faceted-filter proof:
it enters the Table family view, opens the `filter-board` status popover, verifies wheel input on
the popup content stays local, selects the exact `Done` token, checks the controlled change payload
and filtered row counts, then toggles the token off and confirms the original row window returns.

`components_gallery_smoke_range_filter_updates_table_rows` is the focused numeric range proof:
it enters the Table family view, opens the `filter-board` score popover, verifies wheel input on
the popup content stays local, types a minimum score, checks the controlled
`TableRangeFilterChange` payload and filtered row counts against the same `TableState` contract,
and confirms a lower-score row leaves the rendered window.

`components_gallery_smoke_predicate_filter_updates_table_rows` is the focused predicate-filter
proof: it enters the Table family view, types into the `filter-board` name predicate control,
checks the controlled `TablePredicateFilterChange` payload and sample-owned predicate override,
verifies filtered/final row counts against the resolved `TableState`, and confirms the rendered
row window changes without moving the outer Components page.

`components_gallery_smoke_editable_table_cell_updates_sample_rows` is the focused text-cell editing
proof: it enters the Table family view, targets the `editable-release` sample, edits a rendered
`name` cell through the nested `TextInput`, verifies `TableCellEditChange` targets the exact
`(TableRowIdentity, TableColumnId)` pair, confirms the gallery applies the change to its app-owned
`TableState`, and proves a read-only `status` cell does not mount an editor.

`components_gallery_smoke_checkbox_table_cell_updates_sample_rows` is the focused checkbox editing
proof: it enters the Table family view, targets the `toggle-release` sample, toggles a rendered
`enabled` cell through the nested `Checkbox`, verifies `TableCellEditChange` targets the exact
`(TableRowIdentity, TableColumnId)` pair, confirms the gallery applies the bool change to its
app-owned `TableState`, and proves the checkbox cell does not mount a text editor.

`components_gallery_smoke_multiline_table_cell_updates_sample_rows` is the focused multiline
cell-editing proof: it enters the Table family view, targets the `multiline-release` sample, edits
a rendered `notes` cell through the nested `Textarea`, verifies the same exact identity pair and
newline-preserving `TableCellEditChange` payload, confirms the gallery applies the change to its
app-owned `TableState`, and proves non-multiline/read-only cells do not mount the wrong editor.

`components_gallery_smoke_content_fit_table_cell_edit_widens_name_column` is the focused
content-fit proof: it enters the Table family view, targets the `content-fit-release` sample,
edits the visible `name` cell, verifies the sample keeps the fixed `score` lane anchored, and
proves the adapter-measured `name` column widens while header and body stay aligned.

`table_runtime_measured_row_height_reflows_after_paint` is the focused measured-row proof: it
renders a measured `Table` with wrapped body content, verifies the first row grows beyond the
fallback row height, and confirms the second row is laid out below the expanded row after the
measurement cache settles.

`components_gallery_smoke_select_table_cell_updates_sample_rows` is the focused select-edit
proof: it enters the Table family view, targets the `select-release` sample, opens a fixed-option
`Select` editor, picks `blocked`, verifies `TableCellEditChange` targets the exact
`(TableRowIdentity, TableColumnId)` pair, confirms the gallery applies the text change to its
app-owned `TableState`, and proves the select cell does not activate or select the row.

`open-gpui-ui-components` table tests also cover the select editor adapter path directly:
`table_behavior_snapshot_exposes_editable_leaf_cell_kinds_for_leaf_cells_only`,
`table_runtime_select_cell_edit_emits_change_without_row_interaction`, and the other table cell
edit gates prove the fixed-option `Select` editor stays a leaf-cell recipe rather than a new row
interaction path.
The focused U5 identity gates are
`occurrence_identity_is_scoped_to_the_source_snapshot`,
`explicit_duplicate_identity_survives_every_row_model_stage`,
`column_order_normalizes_duplicate_unknown_and_partial_ids`,
`partial_column_order_reorder_normalizes_source_order_before_visibility_and_pinning`,
`ambiguous_business_id_edit_is_an_inspectable_no_op`,
`stale_occurrence_edit_cannot_retarget_a_reordered_duplicate`,
`duplicate_measurements_follow_explicit_identity_across_source_reorder`,
`duplicate_source_rows_publish_distinct_stable_accessibility_nodes`,
`table_virtual_focus_proxy_preserves_keyboard_claim_without_stealing_focus`, and
`table_focus_falls_back_only_when_identity_leaves_the_final_model`. Together they prove that
occurrence identity cannot retarget a newer snapshot, explicit instances survive reorder, partial
order retains unlisted source columns, duplicate edit/measurement/node paths remain disjoint, and
offscreen logical focus stays keyboard-actionable without reclaiming focus moved outside Table.
The Table modules are now verified by ownership layer. `open-gpui-ui-core` owns renderer-neutral
row-model, column, header, filtering, faceting, aggregation, sizing, selection, and virtualizer
contracts. `open-gpui-ui-components` owns the `Table` facade, behavior snapshots, crate-private
render-plan resolution, keyed runtime, header/body/cell/editor/resize element assembly, callback
payloads, and typed public export declarations.
`open-gpui-ui-foundation-gallery` owns the end-to-end samples and scroll containment proofs. For a
Table-only change, prefer the focused commands below before the full `xtask` gate; keep the public
surface and Gallery metadata commands when moving public paths so owner drift is detected early.

```powershell
cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-ui-core --tests
cargo check -p open-gpui-ui-components --tests
cargo nextest run -p open-gpui-ui-core table
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo test -p open-gpui-ui-core -p open-gpui-ui-components --doc --locked
cargo nextest run -p open-gpui-ui-foundation-gallery table
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata gallery_contract_metadata_matches_component_rows components_gallery_smoke_focused_table_scroll_stays_inside_sample components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_faceted_filter_updates_table_rows components_gallery_smoke_range_filter_updates_table_rows components_gallery_smoke_predicate_filter_updates_table_rows components_gallery_smoke_column_visibility_updates_release_matrix components_gallery_smoke_resizable_table_resize_updates_sample components_gallery_smoke_grouped_table_column_reorder_updates_sample
cargo nextest run -p open-gpui-ui-core numeric_range_filters_match_finite_number_cells_inclusively numeric_range_filters_normalize_open_and_reversed_bounds categorical_filters_match_exact_tokens_and_multiple_values
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test text_input --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata gallery_contract_metadata_matches_component_rows components_gallery_smoke_focuses_catalog_family_and_restores_all_mode components_gallery_smoke_editable_table_cell_updates_sample_rows components_gallery_smoke_checkbox_table_cell_updates_sample_rows components_gallery_smoke_select_table_cell_updates_sample_rows components_gallery_smoke_multiline_table_cell_updates_sample_rows
```

Key sentinels inside those binaries include
`row_model_pipeline_executes_each_stage_before_final_pinning_partition`,
`official_components_match_typed_public_exports`,
`common_extended_diagnostic_and_adapter_paths_compile`,
`gallery_contract_metadata_matches_component_rows`,
`table_range_filter_state_resolves_bounds_and_popover_contract`,
`table_range_filter_change_updates_filters_and_resets_pagination`,
`table_behavior_snapshot_exposes_faceting_metadata`,
`table_behavior_snapshot_exposes_editable_leaf_cell_kinds_for_leaf_cells_only`, and
`controlled_text_input_on_change_accepts_input_without_supplied_controller`.

`VirtualizedList` follows the same split at component scale: `open-gpui-ui-components` tests prove
render-plan rows, scroll-target math, PageDown reveal, and Enter/Space activation payloads, while
the gallery metadata and smoke tests prove the official catalog entry, 10k-item rendered sample,
and inner scroll containment inside the overflowing Components page. The focused proof is:

```powershell
cargo nextest run -p open-gpui-ui-components virtualized_list
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_card_wheel_does_not_leak_to_page
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_keyboard_reveals_and_activates
```

The components package includes runtime smoke coverage for Switch, TextInput, Textarea, RadioGroup,
Listbox, Select, Combobox, Command, Tabs, and Toolbar keyboard navigation. The focused Switch test
renders a controlled switch, clicks its real root selector, verifies `on_change` receives the next
checked value, and confirms disabled switches do not emit changes. The focused TextInput tests
render a standalone controller-backed input, click its real root, accept simulated platform text,
sanitize single-line input, verify the controller caret ends at the inserted text, and assert
password display mode masks one glyph per grapheme while preserving the stored value. The focused
Textarea checks prove newline-preserving controlled payloads in component tests and inner viewport
wheel containment in the Components gallery. The focused
RadioGroup test renders real radio
items, rejects disabled clicks, skips disabled items with arrow navigation, verifies default
selection seeding, click and arrow-selection payloads, and confirms Space on an already selected radio does not emit a duplicate
selection change. The focused Listbox test renders real standalone, separator, and grouped options,
rejects disabled clicks, keeps arrow navigation selection-free, skips disabled/separator rows, and
verifies Enter and Space dispatch both option-level and listbox-level selection callbacks. The
focused Select test opens the real trigger, rejects disabled popup option clicks, verifies click and
keyboard selection payloads, closes after selection, and confirms popup Listbox arrow navigation
skips disabled rows. The focused Combobox tests click the controller-backed text input, type a
query, open the filtered popup by trigger and keyboard paths, verify filtered Listbox options, and
select filtered options with ordered select/open callbacks. The focused Command tests cover
renderer-neutral ranking, controlled and default query ownership, stable-value selection across
descriptor reorder, multi-select selected chips, virtualized result render plans, app-owned index
snapshots, core `CommandDescriptor` projection into Command/Menu/ContextMenu surfaces, inline and
dialog command filtering, keyboard activation, shortcut payloads, non-dialog content persistence,
and dialog Escape/outside press dismissal. Command ownership is split across
`command/descriptor.rs`, `command/model.rs`, `command/style.rs`, `command/render_plan.rs`, and
`command/runtime.rs`, while `command/mod.rs` remains the public builder facade. Menu,
ContextMenu, Tree, and Table behavior snapshots now follow the same source-owner discipline:
`menu/` owns descriptor/model/render-plan/runtime/style plus the facade, `context_menu/` owns the
point-anchor facade and neutral state, `tree/` owns descriptor/model/movement/render-plan/runtime
boundaries, and `table/behavior/` owns counts, columns, header, rows, and tree summary snapshots.
The focused gallery Command smoke renders ranked, multi-select, virtualized, and indexed/loading samples in focused family mode,
verifies selected chips, stable selected values, and snapshot metadata are inspectable, and
confirms wheel input on the virtualized sample does not move the surrounding card.
The Components-page command contracts also cover the `registry-dispatch` sample for
`CommandCenter` shortcut/dispatch projection plus empty shortcut diagnostics, and the
`provider-search` sample for
`CommandProviderSource` refresh into a rendered `CommandIndexSnapshot`, including provider request
id, query metadata, projected shortcuts, and empty shortcut diagnostics. The `context-stack`
sample proves that `CommandContextStack` scopes command descriptors and projects the GPUI keymap
binding active for the focused key context. The command crate provider lifecycle tests cover
center-issued request ids, bound responses, stale async responses being ignored without mutating
registry sources, explicit `CommandSourceHandle`/`CommandProviderHandle` unregister behavior, and
the `CommandProviderRefreshController` query/loading/response/snapshot pipeline. The command crate
also covers `CommandKeyBindingRegistry`, which lets app/plugin sources contribute command-id keyed
shortcut dictionaries, projects valid entries into concrete GPUI `KeyBinding` values, preserves
GPUI chord and key-context predicate semantics, reports missing-action or parse diagnostics without
panicking, reports same-context command shortcut conflicts, and returns an install report when
app shells append projected bindings into a GPUI keymap. Conflict coverage includes global
no-context bindings that overlap concrete context bindings under GPUI runtime precedence rules.
The UI component command tests now also cover `CommandPaletteProjection`, which adapts a
`CommandCenter` query/keymap projection into a `PreFiltered` `CommandIndexSnapshot`, provider
statuses, shortcut diagnostics, and UI-ready status rows for failed providers plus shortcut drift;
the Command runtime navigation layer, which supports Home/End, configurable Up/Down loop
navigation, Vim-style control aliases, PageUp/PageDown, and Alt+Up/Alt+Down group jumps;
`CommandPaletteController`, which coordinates palette query
changes across provider refresh controllers, refreshes registered synchronous providers, exposes
pending provider requests for app-owned async tasks, keeps compatibility missing-provider ids,
ignores stale async responses through the existing provider request guard, and wraps command-center
query-history navigation so up/down history keys can reuse the current query as a prefix and
restore the draft query at the newest boundary; plus
`CommandProviderPaletteProjection`, which adapts a provider refresh projection into a `PreFiltered`
`CommandIndexSnapshot`, carries loading provider status into `CommandLoadingState`, and lets
`Command::provider_refresh_projection` bind query and snapshot metadata without app-owned snapshot
glue. The Command gallery includes a diagnostics/empty sample that renders provider failure,
shortcut/action drift, and an empty list inside the component-owned command surface.
Run the focused proof with:

```powershell
cargo nextest run -p open-gpui-ui-components command
cargo nextest run -p open-gpui-ui-components command::runtime::tests --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_palette_projection_builds_status_items_from_provider_failures_and_diagnostics command_state_accepts_explicit_status_items --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_palette_controller --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-command center_reports_command_key_binding_conflicts_and_install_report center_reports_global_key_binding_context_conflicts --no-fail-fast
cargo nextest run -p open-gpui-command center_projects_command_key_bindings_into_gpui_keymap center_reports_command_key_binding_projection_diagnostics --no-fail-fast
cargo nextest run -p open-gpui-command center_exposes_query_history_navigation memory_history_promotes_duplicate_queries memory_history_navigates_recent_queries_with_prefix --no-fail-fast
cargo nextest run -p open-gpui-command context_stack keymap_shortcut_projection_can_respect_context_stack center_context_stack_drives_scopes_keymap_and_provider_requests --no-fail-fast
cargo nextest run -p open-gpui-command source_and_provider_handles_unregister_their_runtime_state --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_descriptors --no-fail-fast
cargo nextest run -p open-gpui-ui-components command menu --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery command
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
```

For the reusable command ecosystem, also keep `docs/ui/command-ecosystem.md` current. It records
the split between GPUI `Action`/`Keymap` execution, the app-owned
`open_gpui_command::CommandCenter` facade, scoped source registration/unregistration, availability
guards, shortcut projection, dynamic provider responses, fuzzy search/history ranking, menu
projection, and command-id dispatch.

The focused Tabs test renders real tabs,
preserves the `default_selected` seed on the first frame, rejects disabled tab clicks, keeps manual
arrow navigation as focus-only, and activates focused tabs with Enter and Space. The focused
Toolbar test renders real toolbar items, moves roving focus with arrow/Home keys, skips disabled and
separator items, and activates the focused item with Enter.

The components package also includes low-state primitive coverage for Separator, Kbd, Progress,
Skeleton, Avatar, AvatarGroup, and AvatarGroupCount. Those tests verify resolved state branches,
explicit root/common/prelude exports, theme color intents, stable rendered debug selectors, decorative
separator semantics, progress clamping, indeterminate progress, Avatar fallback initials,
explicit accessible labels, size metrics, `Role::Image`, group visible/hidden counts, overflow
label state, and source metadata staying outside image-loading ownership. The gallery metadata and
short-viewport smoke tests also verify those primitives are listed as official catalog entries and
render visible samples with stable debug selectors.
The public component gate is federated. `component_contract/rows/catalog.rs` owns only the 48
official ids, revisions, families, and required scenario ids. `public_api/common.rs`,
`public_api/default.rs`, and `table/mod.rs` generate typed export facts from their own `pub use`
declarations. Components/Overlay Gallery catalogs own selectors and Story probes, DevTools owns its
immutable metadata projection, and each native target owns a sibling `*.scenarios.toml` artifact.
No shared test manifest or method/source inventory mirrors those owners.

The focused compile/runtime proof is:

```sh
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo test --locked -p open-gpui-ui-components --doc
```

`official_components_match_typed_public_exports` checks every product id against common/default
exports, keeps typed diagnostic facts disjoint from those surfaces, and anchors
`TableBehaviorSnapshot` to the explicit Table diagnostic owner while preserving
`TableVirtualizerSnapshot` as a default restoration input. The compile witness checks root, common,
prelude, explicit Table diagnostic, and GPUI adapter paths. Same-declaration compile-fail doctests
check every diagnostic export against the actual root, common, and prelude facades; a hand-written
leak makes the corresponding doctest compile unexpectedly and fail. `scan-ui-contract` validates
typed export owner/tier/duplicate facts, docs projection drift, invalid/duplicate scenario bindings,
filter-expression injection, and removed central-authority residue, then runs every registered exact
nextest coordinate.

The v0.3 public API freeze also has a workspace-level tier gate:

```sh
cargo run -p xtask -- scan-public-api --check
```

This gate checks root, prelude, common, default, advanced, model, runtime, adapter, and persistence ownership for docking, motion, UI components, UI core, and canvas. Canvas coverage includes the root common facade plus the explicit `adapter`, `persistence`, and `advanced` tiers. It is a deterministic tier scan today and is the integration point for a future rustdoc-json or cargo-public-api snapshot backend.

Accessibility contract coverage now has its own semantic gate. `ComponentA11yContract` validates
role/name/value/action facts without a live platform backend, while the existing GPUI adapter tests
continue to prove role, orientation, toggled-state, and action mapping into GPUI. Run:

```powershell
cargo nextest run -p open-gpui-ui-components a11y --no-fail-fast
```

That gate keeps the renderer-neutral validation vocabulary covered. Official semantic producers
are additionally asserted in the final GPUI `TreeUpdate` with real Focus, Click, Increment,
Decrement, SetValue, selection, and text dispatch; Table has its own multi-node final-tree target.
Static accessibility evidence rows, Gallery claims, and their consumers are absent. They must not
be recreated in place of final-tree and action tests.
The authority and privacy boundary is recorded in
[Semantic accessibility and final-tree authority](knowledge/engineering/decisions/semantic-accessibility-final-tree-authority.md).

Theme portability is guarded by the theme focused gate:

```powershell
cargo nextest run -p open-gpui-ui-components --test theme --test theme_scope --test public_surface --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-foundation-gallery -E 'test(tokens_gallery_renders_sibling_theme_scopes_and_a_deferred_overlay) | test(devtools_gallery_workbench_refreshes_from_shell_live_facts) | test(devtools_gallery_initial_frame_uses_the_window_effective_theme)' --no-fail-fast --locked
cargo run -p xtask -- scan-theme-drift
cargo run -p xtask -- scan-theme-schema
```

That gate keeps app/window/subtree precedence, exact-window refresh, cached child invalidation,
early-return and panic-safe scope restoration, complete-payload deferred opening-generation
capture, delayed native tooltip capture, Gallery sibling scopes, window-effective initial DevTools
capture, runtime `ThemeContext` rendering, complete code-built `ThemeDefinition` registration, and
the JSON loader facade working: `THEME_JSON_SCHEMA_VERSION`, `theme_json_schema`,
`theme_json_string`, `theme_definition_from_json_str`, `theme_definition_from_json_file`,
`register_theme_json_str`, and `register_theme_json_file`. Production component render paths resolve
color intents from
`ThemeResolver::current(window, cx)` or an explicit snapshot. The app-only resolver and direct
default-light `ThemeResolver::resolve` path are absent. Direct GPUI tooltip attachment uses
`Tooltip::scoped`; official components capture delayed builders automatically. Focus-ring painting
follows the same rule: production render paths use `focus_ring_shadow_with_theme(...)` with an
explicit render-time theme context.
Button and TextInput consume the shared typography, spacing, radius, and density recipes; official
overlays and Tooltip consume elevation; Splitter and VirtualizedList consume strict motion policy.
Tests cover explicit-size precedence, reduced-motion safety, source-versus-effective revision,
metadata-only no-ops, complete schema round-trip, atomic invalid replacement, non-color cached-child
refresh, and opening-generation density/motion capture. The committed schema is generated, while
`scan-theme-drift` enforces two real production recipe consumers for every public design token.
The ownership and detached-render boundary are recorded in
[Theme scope resolution and deferred capture](knowledge/engineering/decisions/theme-scope-resolution.md).
Loader failures are structured as `ThemeLoadError` / `ThemeFileField` for unsupported schema
versions, missing identity or nested design facts, unsupported mode/density/motion/token/state
names, invalid elevation values, duplicate or incomplete token/state coverage, and invalid RGB
values. The old color-only shape and `fallback_mode` are rejection cases, not fallback paths.

Collection typeahead has a separate deterministic runtime gate:

```powershell
cargo nextest run -p open-gpui-ui-components --lib typeahead --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-components --test typeahead_runtime --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-components --test layout --test choice --test overlay --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast --locked
```

The private session owns accepted character normalization, the 700ms executor-clock deadline,
repeated-character cycling, and instance lifecycle. Tree, VirtualizedList, Menu, ContextMenu, and
standalone Listbox consume it; Select consumes it only through an open popup Listbox. Tests cover
the exact timeout boundary, no-redraw cycling, IME/modifier propagation, disabled and structural
rows, reveal without selection, reorder/remove by stable key, remount, same-window instances,
equal IDs in different windows, close/reopen generations, and closed Select triggers. Combobox and
Command runtime tests remain the negative gate for editor-owned query input.

The foundation component family gate covers the shipped disclosure, numeric, navigation, display,
action, and feedback additions: Accordion, Collapsible, Slider, NumberInput, ToggleGroup, Link,
Breadcrumb, Tag, and ToastStack. These tests keep one canonical API per family, explicit
root/common/prelude exports, ownership vocabulary, resolved-state purity, official catalog metadata, and
focused Components-page rendering aligned:

```powershell
cargo nextest run -p open-gpui-ui-components --test public_surface --test form --test navigation --test primitives --test theme --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery --test foundation_gallery -E 'test(gallery_contract_metadata_matches_component_rows) | test(gallery_story_contracts_derive_selectors_and_runtime_probes_from_gallery_owners) | test(components_gallery_smoke_focuses_every_focusable_catalog_entry) | test(components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation)'
```

The choice and runtime seams are guarded separately: `choice.rs` owns stable-value matching for
choice models; `collection_typeahead.rs` privately owns collection input timing and acceptance;
`roving_focus.rs` owns shared enabled-item navigation targets; and `menu/runtime.rs` owns
instance-local menu typeahead alongside submenu hover timing and local scroll state. Editable
Command and Combobox queries remain owned by their text-input controllers.
Feedback coverage now promotes `StatusCue` and `EmptyState` as official rendered Components
catalog entries. The focused component tests verify root/common/prelude exports, feedback intent labels,
resolved roles, metrics, and theme color intents. The gallery metadata tests require their
component/state `SIGNALS` entries and stable `gallery:component-status-cue-sample:{id}` /
`gallery:component-empty-state-sample:{id}` selectors, while the short-viewport smoke verifies the
real `status-cue:*:root` and `empty-state:*:root` debug selectors render.
`official_components_match_typed_public_exports` is the public-owner gate for canonical component
rows and their root/common/prelude exports. `gallery_contract_metadata_matches_component_rows` is
the Gallery metadata gate: official Components and Overlay rows must project the exact canonical
id, revision, and family, while Gallery-local adapter/anatomy/state rows carry no product metadata.
`gallery_story_contracts_derive_selectors_and_runtime_probes_from_gallery_owners` is the
story-probe contract gate. It requires official component samples, renderer-neutral state readouts,
and overlay samples to expose a reusable `StoryContract` with truthful public selectors and
user-observable probe operations:
open, dismiss, select, edit, scroll, focus, activate, and read-public-payload. Gallery smokes should
prefer `component_story_contract_for(name)` and `component_story_contracts_for_focus(mode)` before
falling back to raw debug selectors for adapter-internal details. The official sample selector
pairs and state readout selector pairs are derived from those story contracts so focused catalog
traversal and selector metadata stay aligned.
`gallery_story_contracts_derive_selectors_and_runtime_probes_from_gallery_owners` also guards the
pre-renderer state-contract boundary. Gallery-local `state-contract` entries declare a
`state_contract_selector`, do not declare an official `sample_selector`, and stay disjoint from
`official_sample_selector_pairs`.
The current state contracts are `TreeState` and `VirtualizedListState`; their signals cover state,
descriptor, action/result, helper, and payload types. `TreeState` remains a reusable hierarchy
contract even though `Tree` is now an official rendered component, matching the
`VirtualizedListState` / `VirtualizedList` split. The Components page smoke also verifies every
`state_contract_readout_pairs()` selector is visible.
The official Table gate requires `Table`, `TableState`, `VirtualizerState`,
`TableFacetedFilter`, `TableRangeFilter`, `TablePredicateFilter`, `TableColumnVisibility`, role
signals for table rows and cells, and at least one `gallery:component-table-sample:{id}` selector.
Table smokes and state tests assert that rendered row selectors stay bounded by the virtualizer's
visible rows plus overscan, scroll input stays inside the table viewport, sortable header actions
emit state-update payloads, controlled column resize callbacks carry stable sizing payloads,
categorical faceted filters emit controlled exact-token updates, numeric range filters emit
controlled finite-bound updates, predicate filters emit controlled operator/value updates, column
visibility emits controlled hide/show payloads, editable text cells emit controlled exact
`(TableRowIdentity, TableColumnId)` change payloads without triggering row interaction callbacks;
row activation and expansion request payloads stay controlled; source-tree row models keep nested
descendants addressable through exact `TableRowIdentity` lookup; manual source-tree snapshots expose
unloaded/loading/failed child metadata, row-pinning regions split top/center/bottom rows with
keep-pinned and page-only policies, and grouped / expanded row models keep collapsed descendants
addressable through `TableRowModel::row(&TableRowIdentity)`. `TableRowModel::source_rows` and
`unique_source_row` provide explicitly non-exact business-`TableRowId` lookup. Exact source
identities distinguish unique, explicit-instance, and occurrence rows; occurrence identity is
source-snapshot-local, while a caller-owned instance identity survives source replacement and
reorder. Ambiguous or stale edits are inspectable no-ops; controlled
column order changes normalize the complete source order before moving a listed or unlisted column
without taking ownership of visibility or pinning. Virtual focus tests prove the Table-root proxy,
real keyboard continuation, no-steal remounting, and first-row or empty-model fallback. The
Components gallery now carries `release-rollup`, a grouped Table sample that mixes expanded and
collapsed team groups,
exposes aggregate count and score cells, pins the identifier and status columns, and has its own
sticky-header plus inner-scroll smoke. It also carries
`server-paged`, a manual filtering/sorting/pagination sample that renders only the current
app-supplied page snapshot while exposing server-known total row and page counts through the
gallery summary and `TableBehaviorSnapshot`. It also carries `release-resize`, a controlled
column-sizing sample whose resize smoke drags the `name` handle, records the app-owned committed
width, and verifies header and first-row cell widths stay aligned. `filter-board` is also the
faceted-filter proof: it renders a `status` `TableFacetedFilter`, records
`TableFacetedFilterChange` payloads in the sample runtime log, proves selecting `Done` changes the
rendered row window, proves clearing restores it, and confirms popup wheel input does not move the
outer table sample. It also renders a score `TableRangeFilter`, records
`TableRangeFilterChange` payloads in the same runtime log, applies the range to a sample-owned
`TableState` override, proves filtered/final row counts match the core contract, and confirms
popup wheel input stays local. It also renders a name `TablePredicateFilter`, records
`TablePredicateFilterChange` payloads in the same runtime log, applies the operator/value
predicate to a sample-owned `TableState` override, and proves the rendered row window follows the
core filtered row model. `release-matrix` also renders a `TableColumnVisibility` toolbar
control, records `TableColumnVisibilityChange` payloads in the sample runtime log, applies
visibility overrides to the sample-owned `TableState`, proves hiding a metric column removes its
header and cells, proves show-all restores the column, and confirms popup wheel input stays local.
`release-rollup` now also proves controlled column-order changes: the sample runtime log records
`TableColumnOrderChange` payloads, applies the app-owned override to the sample `TableState`, and
shows the score column re-rendering before team while the sample card stays anchored. The focused
component partial-order gate separately starts from an incomplete order, moves an initially
unlisted column under visibility and pinning, and asserts the resulting full source-column order.
`components_gallery_smoke_grouped_table_scroll_stays_inside_sample` is the focused vertical
sticky-header proof: it enters the Table family view, wheels the `release-rollup` body, and
asserts the header band stays fixed while the body row window advances.
`editable-release` is the text-cell editing proof: it renders editable `name` and `team` columns,
keeps `status` read-only, records each `TableCellEditChange` exact logical-row identity, column id,
and apply outcome in the sample runtime log, applies the updated sample-owned `TableState` override,
and proves the changed row text re-renders through the normal Table pipeline.
`release-matrix` is the wide center-column virtualization and column-visibility sample: it pins the
identity and status lanes, exposes fourteen center metrics, locks identity/status visibility, and
has focused smokes that prove off-window center columns unmount/remount, hide/show visibility
changes update rendered headers/cells, and horizontal / popup wheel input remains inside the
sample. `row-pinning` is the row-region sample: it pins top and bottom review rows around a paged center body, exposes
top/center/bottom readouts, and proves center-body wheel input changes the center row window
without moving the fixed row bands or outer sample. The Table adapter keeps the row and column
virtualizers separate internally; public tests assert the resulting two-axis behavior through
`TableBehaviorSnapshot` plus gallery runtime probes. `dependency-tree`
is the source-hierarchy
sample: it proves nested `TableRow` children resolve to visible tree rows,
keeps collapsed descendants addressable by stable id, exposes tree-depth and tree-branch summary
metadata, and drives controlled expansion plus row activation through the gallery runtime log.
`server-tree` is the manual-expansion sample: it preserves the app-supplied source snapshot,
renders unloaded, loading, and failed branch affordances, records loaded-child and load-state
metadata in expansion payloads, and proves that child rows appear only after the gallery runtime
supplies the loaded snapshot.
Core table tests also assert that `TableAggregation` exposes stable built-in aggregate labels,
resolves count, sum, min, max, and average cells for grouped rows without hiding the grouping
column value, and lets `TableState::with_aggregation_fn` resolve named custom aggregate callbacks
with safe empty fallback for unknown names. Core and component tests assert that
`TableColumnPinning` splits visible columns into
left, center, and right regions after visibility/order resolution, ignores unknown or invisible
pinned ids, removes moved columns from their previous pinned side, and exposes matching
header/body region metadata and debug selectors. They also assert `TableRowPinning` deduplicates
each caller-owned target list, preserves target order, resolves exact source/group identities,
expands explicit business-id bulk targets in model order, gives top logical identities precedence
over bottom overlaps, ignores unknown/filtered/collapsed rows, preserves pinned rows outside the
current page by default, supports page-only behavior, feeds only center rows into the vertical
virtualizer, and renders fixed row-pinned bands around the scrollable center body. Final AccessKit
tests additionally prove row, cell, and header `NodeId` stability through column and row pinning,
duplicate source-instance separation, old-node action dispatch, and identity restoration after a
virtual row or column leaves and re-enters the window.
The official Tree gate requires `Tree`, `TreeState`, `TreeMetrics`, tree/tree-item role signals,
and at least one `gallery:component-tree-sample:{id}` selector. Component runtime tests verify
expansion, reveal, and selection payloads; gallery smokes verify keyboard expansion/selection
through the sample runtime log and prove Tree wheel input stays inside the sample viewport.
`TreeChildrenLoadState` adds the lazy-branch gate: unit tests prove expanded unloaded/loading/failed
branches do not synthesize fake child rows, toggle payloads carry loaded-child and load-state
metadata, and loading branches do not repeat toggle requests. The `remote-workspace` gallery sample
proves unloaded, loading, loaded, and failed branch affordances plus runtime payload metadata.
Tree typeahead is covered by a pure state test and a runtime adapter test: the pure helper matches
visible, focusable row labels with wraparound and skips disabled/collapsed rows, while the rendered
adapter reads the shared private session and moves focus without selecting. The `document-outline`
gallery smoke now verifies typing `n o` focuses the visible Notes row after the expand/select path.
Tree and virtualized-list state-contract samples are verified through
`components_page_samples_expose_component_metadata`: Tree readouts assert visible flattening,
disabled-row position skipping, navigation skipping, toggle payloads, and Enter/Space selection
actions; virtualized-list state-contract readouts assert active/selected indices, PageUp/PageDown
clamping, activation payloads, viewport item count, overscan, and semantic scroll strategy labels.
The same metadata test now also checks the official `Tree` sample's role metadata and keyboard
toggle payload, the official `remote-workspace` Tree sample's child-load metadata, plus the
official `VirtualizedList` sample's 10k item count, listbox roles, active/selected state, visible
range, and overscan summary.

The focused Tree proof is:

```powershell
cargo nextest run -p open-gpui-ui-components tree_state_resolves_lazy_branch_load_metadata_without_synthetic_children tree_toggle_payload_includes_child_load_state_and_blocks_loading feedback_tree_and_virtualized_list_public_exports_remain_explicit
cargo nextest run -p open-gpui-ui-components tree_typeahead_targets_visible_focusable_items_from_current_focus tree_runtime_typeahead_focuses_visible_matching_row
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_gallery_smoke_tree_expands_and_selects components_gallery_smoke_tree_lazy_branches_emit_load_metadata
```

The gallery package also includes a compact-shell runtime smoke that switches the gallery to the
compact viewport policy, verifies the derived mobile shell and compact density, scrolls the left
navigation rail to deep pages, and confirms switching away and back resets the page scroll position.

The gallery package also includes Overlay-page runtime smoke coverage for popover, modal dialog,
alert dialog, non-modal sheet, menu, and ContextMenu right-click hotspot opening plus Escape
dismissal. Popover and Dialog smokes open the real component trigger, assert Dialog initial focus,
and assert focus restoration to the trigger after outside press, modal barrier dismissal, and
Escape dismissal. The AlertDialog smoke opens the real trigger, confirms the cancel action gets the
default focus, verifies the primary action closes the dialog, and confirms Escape dismissal
restores focus to the trigger. The Overlay gallery intentionally keeps default-open contract
samples visually closed at page load so modal barriers and floating layers do not block page
scrolling; the metadata rows still report each sample's resolved default-open contract.

The U3/U4 window overlay fleet has a joint completion gate. It covers renderer-neutral focus and
overlay policy, GPUI pointer-session and focus-claim lifecycle, the window focus-scope runtime,
every official overlay adapter, Gallery runtime snapshots, and the redacted DevTools projection:

```powershell
cargo nextest run --locked -p open-gpui-ui-core -E 'test(/^(focus|overlay)::tests::/)' --no-fail-fast
cargo nextest run --locked -p open-gpui -E 'test(/app::test_context::pointer_session_tests::/)' --no-fail-fast
cargo nextest run --locked -p open-gpui dropping_the_focused_handle_does_not_advance_the_explicit_claim_revision --no-fail-fast
cargo check --locked -p open-gpui-ui-components --tests
cargo nextest run --locked -p open-gpui-ui-components -E 'test(/overlay::focus_scope::tests::/)' --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components --test window_overlay_runtime --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components --test overlay --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components --test choice --no-fail-fast
cargo check --locked -p open-gpui-ui-foundation-gallery --tests
$overlayGalleryTests = @(
    'overlay_gallery_smoke_renders_catalog_entries_and_official_samples'
    'overlay_gallery_smoke_dismisses_popover_from_outside_press'
    'overlay_gallery_smoke_opens_tooltip_from_hover_focus_and_ignores_disabled'
    'overlay_gallery_smoke_renders_manual_tooltip_from_state'
    'overlay_gallery_smoke_keeps_hover_card_open_on_outside_press_and_dismisses_on_escape'
    'overlay_gallery_smoke_toggles_hover_card_from_control_surface'
    'overlay_gallery_smoke_closes_dialog_from_modal_barrier_and_escape'
    'overlay_gallery_smoke_controlled_dialog_refusal_keeps_modal_authority'
    'overlay_gallery_smoke_closes_alert_dialog_from_action_and_escape'
    'overlay_gallery_smoke_closes_non_modal_sheet_from_outside_press'
    'overlay_gallery_smoke_closes_menu_from_escape_and_outside_press'
    'overlay_gallery_smoke_opens_menu_submenu_from_hover'
    'overlay_gallery_smoke_nested_popover_menu_dialog_restores_focus_lifo'
    'overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses'
)
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery @overlayGalleryTests --no-fail-fast
cargo check --locked -p open-gpui-devtools --features ui-components
cargo nextest run --locked -p open-gpui-devtools --features ui-components --test framework_adapters framework_adapters_project_window_overlay_runtime_without_raw_layer_ids --no-fail-fast
git diff --check
```

The `open-gpui` lifecycle gates prove stable pointer capture across redraws, routed event-target
versus physical-hover semantics, explicit and automatic release, cancellation on deactivation or
window removal, cross-window isolation, and focus-claim stability when a focused handle is dropped.
The `open-gpui-ui-core` gate owns the canonical focus resolver and overlay policy states, while
the `open-gpui-ui-components` `focus_scope_tests.rs` module (registered as
`overlay::focus_scope::tests`) proves that the GPUI runtime applies those policies to live handles.

`window_overlay_runtime` proves the four runtime phases, parent topology, topmost input arbitration,
modal barriers, controlled refusal, callback reentrancy, exit/reopen, owner release/remount, focus
loops and restoration, and window isolation. The `overlay` and `choice` targets prove the same
authority through all official families, including Popover -> Menu -> controlled Dialog LIFO Escape
and focus restoration, choice/search editor preservation, and passive Tooltip/HoverCard layers.
Gallery must obtain `WindowOverlayRuntime::snapshot()` from the window that rendered the actual
component adapters; a hand-built layer registration is not Gallery evidence. Its nested smoke
checks kind, phase, parent topology, and each restore target, while the refusal smoke keeps Dialog
in `CloseRequested` with modal, keyboard, and focus authority intact.

DevTools consumes `WindowOverlaySnapshot` through the allowlist entry point
`open_gpui_devtools::ui_components::window_overlay_probe_snapshot`. Layer and parent relationships
are snapshot-local opaque ordinals. The projection includes only layer count, kind, phase, presence,
pending open/reason, keyboard eligibility, modal pointer barrier, and focus active/entered facts;
window identity, lifecycle generations, and intent revisions are omitted. Raw `OverlayLayerId`
strings must never enter a DevTools node, property, JSON payload, or serialized capture. The adapter
test uses unique parent and child canary IDs and must fail if either appears anywhere in the
projection or serialization.

The fleet gate requires a visible runtime registration path for Dialog, Sheet, AlertDialog,
Popover, Menu, ContextMenu, Select, Combobox, Command overlay mode, HoverCard, and Tooltip. It also
requires an absence scan for `OverlayLayerHost`, the removed request forwarding helpers, and
component-owned Escape/outside/focus tails. Component state tests alone do not demonstrate window
runtime ownership.

The focused Overlay catalog gates are:

```powershell
cargo nextest run -p open-gpui-ui-foundation-gallery overlay_page_catalog_entries_have_signals_and_sample_selectors
cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_renders_catalog_entries_and_official_samples
```

The `open-gpui-ui-core` overlay tests are the renderer-neutral policy gate for layer kind, presence,
outside-press policy, Escape policy, focus restore intent, initial focus intent, and
`resolve_overlay_placement` side/alignment/fit/trace behavior. The per-window production runtime
consumes those resolvers; the window-free tests are not themselves proof of live registration,
input interception, or focus ownership.
The `open-gpui-ui-components` overlay helper tests cover GPUI mapping for deferred priority, snap
margin, anchor conversion, and placement resolution. Escape and outside-press decisions are
verified through the `ui_core` resolvers and the real per-window runtime rather than request
forwarding helpers.
The U3/U4 `window_overlay_runtime`, `overlay`, and `choice` integration targets above are the
production-authority proof for migrated families. Trigger-anchored components that do not provide measured
trigger/content bounds should not be documented as owning safe-bounds flip/shift at render time
until live placement measurement is wired into that placement path.
For GPUI runtime focus assertions, `VisualTestContext::debug_selector_is_focused` and
`VisualTestContext::focused_debug_selector` are the preferred test hooks. They use test-only
debug-selector-to-focus-handle data and keep focus checks independent from component internals.
The federated component contract keeps adapter-only, renderer-neutral state, primitive, Gallery,
and docs ownership explicit without copying those facts into one row. Table, Tree,
VirtualizedList, and Command expose behavior snapshots or state readouts; renderer assembly plans
stay crate-private unless a future component deliberately promotes a narrower state contract.
For the UI architecture deepening refactor, keep the focused gates below close to the code that
changes them. They cover the component contract rows, public export map, removed primitive
aliases, overlay runtime policy, choice/search behavior, the Command ownership split, the Table
behavior-snapshot and internal render-plan boundary, shared row-window projection, theme registry,
and gallery catalog/conformance/runtime/sample/render module split:

For the non-overlay choice/search, default-surface, motion, and gallery story-contract refactor,
run this focused subset before broader workspace gates:

```sh
cargo fmt --all -- --check
cargo check -p open-gpui-ui-components --tests
cargo nextest run -p open-gpui-ui-components --test choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-motion --no-fail-fast
cargo test -p open-gpui-motion --doc
cargo check -p open-gpui-ui-core --tests
cargo nextest run -p open-gpui-ui-core split --no-fail-fast
cargo nextest run -p open-gpui-ui-core --test headless_contracts --no-fail-fast
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-ui-foundation-gallery component --no-fail-fast
cargo run -p xtask -- scan-ui-contract
git diff --check
```

These commands prove that Select and Combobox keep behavior after their
model/style/render-plan/runtime splits, `choice.rs` remains an internal behavior seam,
`open_gpui_ui_components` no longer exposes broad command/core infrastructure through the curated
default surface, `MotionValue` stays private behind consumed motion controller APIs, and
Listbox/Select/Combobox/Command gallery stories expose state readout selectors for public-payload
assertions.

The window-runtime and choice focus tests should keep the `scroll_surface`, `choice`, `overlay`,
`layout`, and `table` focused gates green. The `choice` gate proves Select, Combobox, and dialog
Command register with the per-window authority and preserve focus plus open-change callback order
after selection, Escape dismissal, outside-press dismissal, and controlled refusal.

For the deep UI framework module refactor, run the focused ownership gates below before the full
workspace gate. They cover runtime theme context, ephemeral semantic projection with final-tree and
action evidence, removed registry history, shared overlay placement,
`open_gpui_ui_core::grid_viewport::RowWindow`, gallery story-contract projection, and
`open_gpui_command::CommandDescriptor` projection:

```powershell
cargo fmt --all
cargo check -p open-gpui-ui-core --tests
cargo check -p open-gpui-ui-components --tests
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-ui-core overlay grid_viewport --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components --lib scroll_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test overlay --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test layout --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo nextest run -p open-gpui-ui-components theme_scope theme a11y menu context_menu command --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_descriptors --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery component --no-fail-fast
cargo run -p xtask -- scan-theme-drift
cargo run -p xtask -- scan-theme-schema
cargo run -p xtask -- scan-ui-contract
rg -n "ThemeResolver::resolve\(" crates/ui_components/src -g "*.rs"; if ($LASTEXITCODE -eq 0) { exit 1 } elseif ($LASTEXITCODE -ne 1) { exit $LASTEXITCODE } else { exit 0 }
rg -n "ThemeRuntime|ThemeResolver::current\(cx\)" crates/ui_components/src examples/ui-foundation-gallery/src -g "*.rs"; if ($LASTEXITCODE -eq 0) { exit 1 } elseif ($LASTEXITCODE -ne 1) { exit $LASTEXITCODE } else { exit 0 }
git diff --check
```

For public export, Gallery metadata, native scenario, or Table diagnostic owner changes, run:

```powershell
cargo fmt --all -- --check
cargo check --locked -p open-gpui-ui-components -p xtask --tests
cargo nextest run --locked -p xtask --lib
cargo nextest run --locked -p open-gpui-ui-components --test public_surface --test table --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery component_catalog_contracts::gallery_contract_metadata_matches_component_rows --no-fail-fast
cargo run --locked -p xtask -- scan-ui-contract
```

For component contract, a11y, gallery conformance, and theme productization work, start from the
reusable UI contract audit before dropping to focused behavior tests:

```powershell
cargo run --locked -p xtask -- scan-ui-contract
cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo check -p open-gpui-ui-components --tests
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-components a11y --no-fail-fast
cargo nextest run -p open-gpui-ui-components theme --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery component_catalog_contracts::gallery_contract_metadata_matches_component_rows --no-fail-fast
```

`scan-ui-contract` checks narrow component rows, same-declaration public export facts, the exact docs
projection, Gallery canonical metadata, test-owned scenario artifacts, and removed
central-authority residue. It executes every registered exact test coordinate, including the
final-tree and real AccessKit action scenarios that verify semantic assembly instead of inferring it
from static claim rows.
Use the narrower
`scan-theme-schema`, `scan-theme-drift`, and focused nextest commands when investigating a specific
failure.

## Interactive Subtree Transform Gate

Changes to `SubtreeTransform`, `ElementGeometry`, targeted input, frame-journal geometry, scene
primitives, renderer conversion, or Motion projection must run the focused U12 gate before broad
workspace verification:

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo fmt --all -- --check
cargo nextest run --locked -p open-gpui --lib transform --no-fail-fast
cargo nextest run --locked -p open-gpui --lib measured tooltip --no-fail-fast
cargo nextest run --locked -p open-gpui --test presentation_surface --no-fail-fast
cargo nextest run --locked -p open-gpui-motion --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components motion_adapter --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery presentation --no-fail-fast
cargo check --locked -p open-gpui-wgpu -p open-gpui-windows --tests
git diff --check
```

The GPUI tests cover checked numeric construction, nested composition, every scene primitive,
transactional late failure, hit testing, explicit local/window event coordinates, pixel and line
wheel semantics, pointer capture rebinding, drag/drop, IME, final AccessKit bounds, deferred and
portal behavior, cache replay, tooltip lifecycle, committed measurement, and displayed debug
geometry. `presentation_surface` also rejects the removed `Transformation`,
`TransformationMatrix`, and `with_transformation` names and guards the opaque renderer ABI.

The Gallery `presentation` smoke uses nested non-uniform transforms with a real Button, delayed
Tooltip owner, TextInput, ScrollArea, drag/drop target, accessibility group, Motion projection, and
committed geometry readout. Static labels or a quad-only sample are not sufficient evidence.

Native CI remains responsible for each renderer on its owning platform. WGPU, DirectX, and Metal
must compile the shared primitive and run their ABI/conversion tests; capable runners additionally
run the designated transformed-pixel and clip smoke. A Windows local run cannot replace Metal or
Linux/native backend evidence.

Run the full component and gallery package gates only after broad contract-table, theme, or gallery
changes:

```powershell
cargo nextest run -p open-gpui-ui-components --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast
```

The Components gallery root keeps `runtime`, `samples`, and render ownership private. Stable
Gallery API names are re-exported explicitly from `components.rs`; sample families live under
`examples/ui-foundation-gallery/src/pages/components/samples/`, runtime probes under
`examples/ui-foundation-gallery/src/pages/components/runtime/`, and render orchestration/readouts
under `examples/ui-foundation-gallery/src/pages/components/render/`. Runtime catalog/Story tests
verify canonical metadata, selectors, probes, and focused-section behavior without parsing Rust
source text.

```powershell
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-core overlay
cargo nextest run -p open-gpui-ui-components --test overlay --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo nextest run -p open-gpui-ui-core virtualizer
cargo nextest run -p open-gpui-ui-components --test layout --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test theme --no-fail-fast
cargo run -p xtask -- scan-theme-drift
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast
```

The binary-level gates above include these focused sentinels:
`official_components_match_typed_public_exports`,
`common_extended_diagnostic_and_adapter_paths_compile`,
`scenario_declarations_reject_duplicate_contracts_and_filter_expressions`,
`scenario_validator_reports_missing_duplicate_unknown_and_owner_drift`,
`scenario_validator_rejects_reused_executable_coordinates`,
`overlay_open_change_helpers_match_core_policies`,
`dialog_runtime_respects_escape_policy_and_restores_trigger_focus`,
`choice_surfaces_share_stable_value_resolution_and_query_normalization`,
`row_model_pipeline_executes_each_stage_before_final_pinning_partition`, `row_window`,
`virtualized_list_behavior_snapshot_uses_item_descriptors_and_virtualizer_contracts`,
`virtualized_list_behavior_snapshot_applies_builder_metrics`,
`table_behavior_snapshot_exposes_center_column_summary_without_window_internals`,
`table_behavior_snapshot_exposes_pinned_column_regions`, `theme_registry`, `theme_resolver`,
`theme_snapshots`, `gallery_contract_metadata_matches_component_rows`,
`gallery_story_contracts_derive_selectors_and_runtime_probes_from_gallery_owners`,
`choice_search_story_contracts_expose_state_readouts_and_product_metadata`,
`components_gallery_smoke_focused_choice_search_state_readouts_render`,
`components_gallery_smoke_focuses_catalog_family_and_restores_all_mode`, and
`components_gallery_smoke_focuses_every_focusable_catalog_entry`.

The theme drift scan is the focused gate for component color recipes and built-in theme token
coverage. It requires all `ThemeResolver::*_colors` component calls to be implemented and listed
in `crates/ui_components/src/theme/recipes.rs`, rejects component-local `impl ThemeResolver`
extensions, and checks that light, dark, and high-contrast palettes expose the same token/state
shape. Add or move recipes in the theme module first, then update the catalog entry in the same
patch.

The `open-gpui-ui-components` public contract tests should also keep
`public_resolved_state_contracts_avoid_gpui_runtime_types` passing. That test is the hard
headless-readiness guard for public resolved-state structs: it prevents `Window`, `App`,
`Context`, `RenderOnce`, `IntoElement`, `ElementId`, `Entity`, focus handles, scroll handles, and
callback storage from entering state contracts. The companion extraction-blocker inventory tests in
`open-gpui-ui-components` and `open-gpui-ui-core` pin the extraction gate deliberately. Component
public-state blockers are currently empty: resolved overlay contracts expose `OverlayResolvedState`, while
`GpuiOverlayState` stays in the GPUI adapter helper surface for deferred priority and snap margin.
Public component metrics and accessibility state now use neutral UI-core vocabulary; adding public
GPUI `Pixels`, `Bounds`, `Point`, or `Size` aliases to resolved-state contracts should fail the
guard inventory. `open-gpui-ui-core` is now renderer-neutral: it has no `open_gpui` dependency,
no UI-core source references to `open_gpui`, and no `UiPx` conversion impls for GPUI style types.
Adaptive policies accept neutral `UiPx` thresholds and inputs instead of GPUI `Pixels`; GPUI
callers should convert their concrete window or viewport width at the adapter boundary before
invoking UI-core adaptive helpers. The companion strict-boundary inventory must stay empty.
The public-surface compile witness keeps the intentionally public GPUI helper paths usable:
`TextInputController`, `FieldControl`, `UiA11yElementExt`, `VirtualizedListGpuiExt`, externally
supplied `ScrollHandle`, `focus_ring_shadow_with_theme`, `GpuiOverlayState`,
accessibility/geometry conversions, and adapter scheduling helpers remain supported under
`open_gpui_ui_components::gpui_adapter`. The typed common-tier gate rejects registering its named
adapter-only surfaces as common exports; it does not infer helper ownership from Rust source text.
Any deliberate root or prelude projection is therefore an explicit public-API change that must move
into the same-declaration tier authority and its compile gates.
`FocusRing` itself uses neutral `UiPx`; only the GPUI focus-ring shadow helper returns `BoxShadow`,
and production render paths should use the explicit-theme helper.

When changing GPUI accessibility repair or component metadata that creates explicit cross-node
relationships, also run:

```sh
cargo check -p open-gpui
cargo nextest run -p open-gpui --lib window::a11y::tests::repair_tree_update
```

Manual UI foundation dogfood should use the dedicated gallery after the automated checks pass:

```sh
cargo run -p open-gpui-ui-foundation-gallery
cargo run -p open-gpui-ui-foundation-gallery -- --page components
```

1. Open `Tokens` and confirm the semantic token registry shows surface, text, accent, focus ring,
   destructive, overlay, and modal overlay keys without introducing a styled component layer.
2. Open `Sizing & Density`, switch between compact and desktop from the summary panel, and confirm
   the highlighted density and default size change with the foundation policies.
3. Open `Adaptive`, use the same compact/desktop switch, and confirm device samples show mobile /
   desktop shell mode, compact / regular / expanded class, and panel samples show compact / medium /
   wide classes.
4. Open `Focus & A11y`, tab through the focusable controls, confirm the focus-visible outline is
   visible, click the counter and reset controls, and toggle the switch. The visible counter and
   switch state should match the accessible role/state vocabulary shown by the page.
5. Open `Overlay`, click `open overlay`, confirm the anchored popover appears from the trigger, then
   close it from the popover or press Escape. The geometry readout should keep anchor, layout,
   visual, preferred, and safe-window rectangles visible. The behavior contract matrix should show
   distinct tooltip, popover, dialog, and menu policies for presence, outside press, Escape, focus,
   underlay blocking, and GPUI adapter fields such as deferred priority and snap margin. In the
   Tooltip samples, hover `Hover or focus`, tab to `Focus only`, and confirm each reveals
   descriptive tooltip content while `Disabled` remains unfocusable and closed; `Manual delayed`
   should stay visible and report its custom delay policy. In the HoverCard samples, confirm
   `Profile preview` reports its default-open interactive contract without visually blocking the
   page at load, `Focus preview` opens only from keyboard focus, and `Manual card` opens and closes
   from its gallery control with pass-through or consume outside-press metadata shown in the state
   row. In the Popover samples, confirm `Default open` reports the default-open contract
  without visually blocking the page at load, `Controlled` opens and closes from its gallery
  control, Escape closes the controlled popover, outside press closes the visible popovers, and the
  `Consume outside` sample reports a consuming outside-press policy while `Disabled` remains
  closed. In the Dialog samples, open and close `Controlled modal`, confirm Escape and the modal
  barrier can close it without activating underlay controls, confirm `Default open` reports a
  blocking modal layer without visually blocking the page at load, confirm `Outside ignored`
  reports the sticky outside policy, and confirm `Disabled` stays closed. In the AlertDialog
  samples, open `Delete project`, confirm the destructive action is explicit, cancel receives the
  default focus, outside press is consumed without dismissing, Escape closes it, and focus returns
  to the trigger; confirm the safe cancel sample reports its default-open and modal-underlay
  contract without visually blocking the page at load. In the Sheet samples, confirm the left modal
  sheet reports blocking underlay input, the right non-modal sheet opens from its gallery control
  and reports pass-through outside behavior without a blocking modal barrier, and the bottom sticky
  sheet reports bottom-edge attachment, hidden close affordance, and ignored outside press. In the
  Menu samples, confirm arrow keys move roving focus over enabled
   action items while skipping separators and disabled items, Enter/Space activates the focused
   action and closes the menu, Escape closes the controlled menu, and `Outside ignored` keeps its
   explicit outside policy. In the ContextMenu samples, right-click the hotspot and confirm the
   menu opens from the pointer point, snaps inside the window near edges, and closes on outside
   press or Escape.
6. Open `Components`, or start there directly with
   `cargo run -p open-gpui-ui-foundation-gallery -- --page components`, and confirm Button, Badge,
   Accordion, Collapsible, Slider, NumberInput, ToggleGroup, Link, Breadcrumb, Tag, ToastStack,
   IconButton, Separator, Kbd, Progress, Skeleton, Avatar, ScrollArea, Splitter, Switch, Checkbox,
   RadioGroup, Toggle, Label, TextInput, Textarea, Field, Tabs, Toolbar, Sidebar, Listbox, Select,
   Combobox, Command, Table, and VirtualizedList samples render with enabled, disabled, selected, checked, unchecked,
   indeterminate, pressed, invalid, required, read-only, placeholder, value, help, error,
   decorative, semantic, indeterminate-progress, fallback-initial,
   source-metadata, roving-focus, popup, overflow-axis, scroll-reset, resize-constraint, row-model,
   and virtualized-viewport states. The Badge, Kbd, Skeleton, and non-removable Tag samples should
   remain display-only.
   The Accordion and Collapsible samples should expose stable disclosure values, disabled rows, and
   open-state readouts. Slider and NumberInput samples should expose clamped min/max/step metadata,
   disabled/read-only or invalid states, and keyboard or step payload semantics. ToggleGroup should
   expose single and multiple stable-value selection with disabled-item skipping. Link and
   Breadcrumb should expose accessible navigation labels and activation metadata. Tag should expose
   removable and disabled-remove metadata. ToastStack should expose visible stack ordering,
   overflow, timeout pruning, dismiss reasons, and action metadata without owning timers.
   Use a few catalog cards, such as Table, Tree, and VirtualizedList, to enter focused
   component-family mode; confirm unrelated samples are hidden, the section directory stays
   available, nested sample scrolling still stays inside the sample, and `All components` restores
   the full conformance page with the page scroll reset. The Separator samples should distinguish semantic and
   decorative roles. The Progress samples should cover determinate and indeterminate values, with
   indeterminate progress rendering as a short non-percentage segment rather than a fixed 33% fill.
   The Avatar samples should show derived fallback initials, explicit fallback text, explicit
   accessible labels, and source metadata without owning image loading. The IconButton samples
   should be square controls with visible focus and explicit accessible labels. The ScrollArea samples should cover vertical overflow, horizontal overflow,
   and two-axis overflow; wheel or trackpad scrolling should stay inside each constrained viewport
   while the state readout reports the expected axis and reset policy. Scroll each constrained
   ScrollArea once, then continue scrolling the same viewport after the content has moved; it should
   keep moving instead of snapping back to the origin after the redraw caused by the first scroll.
   The gallery navigation rail should also scroll independently inside its own viewport so deep
   sections remain reachable on compact windows. The vertical Tabs sample should keep its tab rail
   scrollable inside the constrained gallery card, and the focused component smoke now verifies the
   shared `ScrollArea` viewport directly through `tabs_vertical_tablist_scrolls_when_constrained`.
   The Splitter samples should
   show horizontal and vertical panel groups, stable handle affordances, min/max fraction readouts,
   collapsed-panel metadata, and pointer-drag resizing without changing surrounding layout. Drag the
   vertical collapsed sample far enough to restore the collapsed panel, then confirm subsequent
   dragging resizes it normally. The RadioGroup samples should
   cover vertical required selection and horizontal navigation that skips disabled items. The Toggle
   samples should expose button-like pressed state without behaving like a checkbox. The Tabs
   samples should cover horizontal automatic activation and vertical manual activation; use arrow
   keys, Home/End, Enter, and Space to confirm focus movement and activation behavior. The vertical
    sample should keep its tab rail scrollable inside the constrained gallery card. The Toolbar
    samples should expose horizontal and vertical command groups; use arrow keys plus Home/End to
    confirm roving focus skips disabled items and separators. Use Enter or Space to activate action
    items, and Space only to activate toggle items. The component runtime smoke now verifies the rendered Toolbar keyboard path
    for disabled-item/separator skipping and activation payloads. The Sidebar samples should expose
   expanded, icon-collapsed, and long scrollable navigation; icon collapse should hide visible labels
   while keeping item labels explicit, disabled and duplicate values should fail closed, and the long
   sidebar should scroll inside its sample frame. Semantic activation coverage verifies pointer,
   Enter/Space key-up, AccessKit, and programmatic parity, item-handler override, and focus repair
   without stealing external focus. The component smoke verifies the shared `ScrollArea` viewport through
   `sidebar_long_navigation_scrolls_inside_shared_scroll_area`, and the gallery smoke verifies the
   long sidebar's internal viewport moves relative to its sample card. The Listbox samples should
   expose
   grouped options, disabled option skipping, selected and active descendant metadata, empty-state
   behavior, and keyboard navigation/activation with Up/Down/Home/End plus Enter/Space. The
   component runtime smoke now verifies rendered Listbox disabled clicks, selection-free arrow
   navigation, disabled/separator skipping, and option/listbox callback parity for keyboard
   activation. The Select
   samples should expose closed, controlled-open, and disabled states; confirm the trigger label
   reflects the selected option, the open sample uses a non-modal dismissible listbox popup with a
   scrollable long option set, Escape/outside press dismisses it, disabled empty select remains
   closed, and the state readout keeps trigger-selected value distinct from popup listbox selection.
   The component runtime smoke now verifies rendered Select trigger opening, disabled popup
   option rejection, click selection, keyboard popup selection that skips disabled rows, selection
   payloads, and ordered popup close callbacks. The Combobox samples should expose editable
   filtering, selected value metadata that does not disappear when the query hides the selected
   option, an empty filtered state, disabled input/popup suppression, and visible query metadata.
   The component runtime
   smoke now verifies real Combobox text-input editing, filtered popup options, filtered option
   click selection, and close callbacks. The Command samples should expose ranked search results,
   selected chips for multi-select, stable selected values independent of result order, a 10k-item
   virtualized command result window, app-owned
   indexed/loading metadata, shortcut labels, inline and dialog-backed presentation, and modal
   dialog outside/Escape dismissal while preserving the Components page scrollability. The component
   runtime smoke now verifies real Command text-input editing, inline filtering, keyboard
   activation, shortcut payloads, non-dialog content persistence, multi-select toggling, virtualized
   scrolling/reveal behavior, and app-owned index snapshot state. The default TextInput
   sample should accept real text editing through the controller-backed path, and the password
   sample should show masked display metadata while preserving the underlying value contract. The
   Textarea samples should expose placeholder, filled, overflowing, and invalid states; wheel input
   inside the overflowing textarea should scroll its multiline content without moving the sample
   card or outer Components page. The
   gallery remains scrollable and keeps focus visible when the page overflows. The Table samples
   should expose the `release-queue` 10k-row virtualized window,
    the filtered/sorted/paginated `filter-board` model with working status `TableFacetedFilter`,
    score `TableRangeFilter`, and name `TablePredicateFilter` controls,
    the controlled `release-resize` sizing
    sample, the grouped and sticky pinned `release-rollup` model with left/right fixed lanes and a
    horizontally scrollable center lane, the wide `release-matrix` center-column window with a
    working `TableColumnVisibility` control, the source-tree `dependency-tree` sample with nested
    rows and controlled expansion, stable selected row ids, the editable `editable-release`
    text-cell sample with app-owned row updates, the `multiline-release` textarea-cell sample with
    newline-preserving app-owned row updates, table/row/cell accessibility metadata, sortable header
    metadata, resize handle metadata, row activation, expansion, column-visibility, and cell-edit
    log entries, and internal body viewports that scroll
    without moving the outer Components page.
    The Tree sample should expose `document-outline`,
    tree/tree-item accessibility metadata, expandable `Paper` children, a state readout, an inner
    viewport that scrolls without moving the outer Components page, and selection/toggle events
    through the gallery sample runtime log. The VirtualizedList sample should expose the
    `release-navigation` 10k-item window, listbox/listbox-option roles, active/selected
    metadata, visible/overscan readouts, an internal viewport that scrolls without moving the
    outer Components page, card-chrome wheel containment, and PageDown plus Enter/Space activation
    through the gallery sample runtime log. The app should stay open after opening `Components`;
    an `accesskit_consumer`
   panic during that navigation is a
   regression in the accessibility repair gate. The Components page also serves as a conformance
   surface: confirm the visible component catalog distinguishes official components from
    adapter-only helpers and internal anatomy, and confirms Separator, Kbd, Progress, Skeleton, and
    Avatar are official entries with state types, then confirm the visible gate cards for explicit
    crate exports, gallery metadata, ScrollArea redraw persistence, Splitter runtime constraints,
    Tabs overflow, `table-virtualization`, `tree-renderer`, `virtualized-list-renderer`, and
    explicit accessible metadata on icon-only and label-association samples.
   The Overlay Menu and ContextMenu samples should expose action, checkbox, radio, separator,
   disabled, submenu, typeahead, controlled-open, outside-policy, and point-anchor variants. Use
   `cargo nextest run -p open-gpui-ui-components menu` and `cargo nextest run -p
   open-gpui-ui-components context_menu` to verify rich item payloads, pure typeahead,
   visible-submenu keyboard navigation, submenu hover delay / close timing, local menu scrollability,
   context-menu reuse, and long-menu wheel containment through
   `context_menu_runtime_long_menu_scroll_stays_inside_surface`. Use `cargo nextest run -p
   open-gpui-ui-components
   menu_runtime_hover_opens_submenu_and_preserves_child_focus
   menu_runtime_hover_switches_between_submenu_branches` together with the menu family command to
   cover the hover-delay runtime. Use
   `cargo nextest run -p open-gpui-ui-foundation-gallery
   overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
   overlay_page_context_menu_samples_expose_point_anchor_contracts
   overlay_page_catalog_entries_have_signals_and_sample_selectors
   overlay_gallery_smoke_closes_menu_from_escape_and_outside_press
   overlay_gallery_smoke_opens_menu_submenu_from_hover
   overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses` plus `cargo check -p
   open-gpui-ui-foundation-gallery --tests` after changing the overlay menu family.
7. Re-run `cargo nextest run -p open-gpui-ui-components` and `cargo nextest run -p
   open-gpui-ui-foundation-gallery` if a manual check exposes a component or gallery regression.

For UI component productization checkpoint work, additionally review
`docs/adr/0008-open-gpui-ui-component-productization-roadmap.md` after the automated component
tests pass. If a future task explicitly reopens extraction, also review
`docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md` and
`docs/adr/0007-open-gpui-ui-headless-boundary-design.md`. The checkpoint should continue to
identify which behavior is neutral, which behavior remains GPUI adapter-owned, and why the current
crates remain the active product boundary.

CI runs a three-platform matrix for pushes to `master` / `main`, pull requests, and manual workflow
dispatches:

- Windows runs the same local gate, `cargo nextest run -p xtask`,
  `cargo nextest run -p open-gpui-docking-native --no-fail-fast`, and
  `cargo check -p open-gpui-windows --all-features --locked`.
- Linux runs `cargo check -p open-gpui-linux --all-features --locked` after installing the system
  headers needed for Wayland, X11, fontconfig, freetype, and pkg-config.
- macOS runs `cargo check -p open-gpui-macos --features font-kit --locked`.
- All three platforms run `cargo check -p open-gpui-wgpu --features font-kit --locked`.

Run the native renderer smoke explicitly with:

```sh
cargo run -p xtask -- renderer-smoke
```

That command runs the focused `open-gpui-wgpu` smoke test that requests a real native `wgpu` adapter and
device, creates the renderer bind group layouts, and builds the core render pipelines. It is not
part of the default `verify` gate because it depends on local GPU, driver, and session availability.

Run the docking smoke surface explicitly after changing `open-gpui-docking`:

```sh
cargo fmt --all -- --check
cargo check --tests -p open-gpui-docking
cargo check -p open-gpui-docking-minimal --locked
cargo nextest run -p open-gpui-docking
cargo nextest run -p open-gpui-docking-native --no-fail-fast
cargo check -p open-gpui-docking-native
cargo run -p open-gpui-docking-minimal
cargo run -p open-gpui-docking-native
```

When changing docking drop routing, delivery, or public surface boundaries, also run:

```sh
cargo nextest run -p open-gpui-docking drop_target viewport_drop_route viewport_drop_delivery public_surface_tests --no-fail-fast
```

For docking presentation/preview/motion work, the focused semantic gates are:

```sh
cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast
```

The docking native example exercises the public multi-window setup: applications build one
`DockController`, wrap it in a `DockViewportRuntimeHandle`, register window-close cleanup, and open
controller-backed primary and secondary `DockHost` viewports. The runtime panel reports both the
last route target and the route selection source, so dogfood runs can distinguish trusted hovered
window routes, window-stack fallback routes, focus-stamp fallback routes, and current-facts
rejections. It also reports the current platform viewport capability snapshot, splitting route facts
from placement facts so platform-boundary regressions are visible during native dogfood. The
placement restore line reports matched and missing restored windows, and the tear-off status line
reports whether a viewport opened from suggested bounds or drag-source geometry, so placement
authority regressions are visible in the same panel.

Docking target previews are scene-owned. During dogfood, every target-window preview should be
explainable from the same capability model: `DockPresentationScene` resolves panes, tab bars,
tab labels, splitters, floating containers, focus regions, and overlay anchors; `DockPreviewScene`
describes allowed/rejected target facts; `DockVisualAffordanceScene` gives stable layer identity for
preview bodies, guide boxes, tab insertion, payload tabs, route markers, and rejected state;
`DockTransitionPlan` describes motion/reduced-motion semantics; divider hit maps, zoom/focus state,
accessibility descriptors, and runtime diagnostics consume the same descriptor path. Rendering must
not recreate guide availability independently from those descriptors. Debug selectors reflect that
contract:
target-stack guides use `dock:<space>:drop-guide:inner:<tabs>:<zone>`, root/host guides use
`dock:<space>:drop-guide:outer:<zone>`, the split body is exposed separately from the full target
preview container as `dock:<space>:drop-preview:body`, and center/tab docking exposes a
`dock:<space>:drop-preview:tab-insertion` affordance before payload tab previews.

Manual native docking dogfood should use the same example after the automated checks pass:

1. Launch `cargo run -p open-gpui-docking-native` and confirm the app opens `Docking demo`,
   `Docking preview`, and `Empty central dogfood` windows.
2. Drag a primary-class tab from `Docking demo` into another primary-compatible target; the preview
   must appear in the destination window and release must select the moved item there.
3. Drag the `Preview` / `Diff` secondary-class stack from `Docking preview` back into `Docking demo`;
   item order and the active tab must be preserved.
4. Drag `Preview` / `Diff` over `Empty central dogfood`; the route must render as rejected and
   release must not mutate the graph because the central space only accepts central-class panels.
5. Use `Restore central note` from the runtime status panel; the `Central note` panel must reopen in
   the empty central window and recover the central-region identity instead of becoming ordinary
   root-only content.
6. Drag a tab or stack outside every docking window; a new runtime-backed viewport must open before
   the graph moves the source payload.
7. Dock the torn-off viewport content back into an existing window; the destination window must
   activate and the moved item must become the selected tab.
8. Move runtime-opened windows across displays, choose `Save placement`, then use `Reopen closed
   demo viewports`; restored placement should use saved bounds only as placement input while live
   drag routing continues to use current viewport bounds. On macOS, windows on a secondary display
   should keep non-overlapping desktop-space bounds while routing between viewports.
9. Exercise the runtime panel close-policy controls for prevent, retain, and merge-back behavior;
   closing a viewport must match the selected policy without losing descriptor-backed panel restore
   or leaving a stale cross-window route preview in another viewport.
10. Start a cross-window drag, hover a valid target, then move to an area of the same viewport with
   no current dock target before releasing; the previous preview must not commit from stale target
   state.
11. Drag over the empty central dogfood window; empty central-space preview, rejection, and
   passthrough behavior must match the visible policy state.
12. While dragging over a valid tabs target, confirm the guide affordance is target-owned and
    box-shaped rather than a floating five-button cluster. Center hover should show a center box,
    side hover should highlight only the corresponding side box, and inactive boxes should remain
    visibly weaker than the active one.
13. Hover the center of a compatible target and confirm the destination window renders one dock
    preview plus one contained payload tab preview. Hover any split edge and confirm the preview
    becomes an edge band and the payload tab preview disappears.
14. Reproduce a nested-target case by docking into a child region, then dragging another tab into
    the remaining nested leaf. Confirm hovering the left or right side of that nested leaf resolves
    inside the nested leaf itself rather than snapping to the neighboring region or to the root
    edge.
15. Drag a tab or stack outside every valid host. Confirm the route marker reads as tear-off or
    rejection only; no fake blue dock target or payload tooltip should appear at the source.
16. Drag the two-tab `Preview` / `Diff` stack over a compatible target stack center. The
    destination preview must show a shared preview body plus two selected-tab-like payload tab
    previews in payload order; the previews should clip to the target tab bar instead of becoming a
    single dark rectangle.
17. Repeat the same two-tab stack drag across windows. The target window must render the same
    payload tab preview structure as a local hover, while the source window shows only route-marker
    feedback when applicable.
18. Hover a side drop box for the root central leaf and a side drop box for a nested child leaf.
    The root central leaf should use outer split semantics; the nested child leaf should keep
    inner split semantics.
19. Hover rejected center and route targets. The target preview must use rejected tokens, suppress
    payload tab previews, and leave the graph unchanged on release.
20. Start a routed cross-window hover, then hover a different compatible viewport before release.
    The old target window must lose both its target preview and payload tab previews, while the new
    target owns the current preview.
21. Press Escape during a routed drag after a target preview appears. The source route marker,
    target preview, runtime drag session, and GPUI active drag must all clear in one frame.

Current docking multi-viewport capability states:

- Platform viewport windows require two gates. `DockPolicy::allow_platform_viewports` is the
  workspace/app opt-in, while `PlatformViewportCapabilities::platform_viewport_windows` is the
  backend fact that independent application viewport windows exist. The capability defaults to
  false; macOS, Windows, X11, and the test platform opt in, while Web and Wayland fail closed.
  When policy allows tear-off but the backend capability is false, outside-viewport route preview
  records `PlatformViewportWindowsUnsupported`, and explicit `open_viewport` / tear-off runtime
  opens return `Unsupported` before creating a GPUI window. Single-window docking and in-window
  split/merge/floating interactions remain available on web. Automated owners:
  `viewport_runtime_handle_drop_route_fails_closed_when_platform_viewport_windows_unsupported`,
  `viewport_runtime_open_viewport_fails_closed_when_platform_viewport_windows_unsupported`, and
  `viewport_runtime_tear_off_fails_closed_when_platform_viewport_windows_unsupported`.
- Coordinate facts are explicit runtime state. `DockViewportCoordinateStatusRecord` reports whether
  each registered viewport is using shared global-screen bounds or receiver-local window bounds, and
  the runtime panel exposes that generation next to the route selection source. Mixed-DPI and
  display-ambiguous backends should fail closed or degrade to local-only routing until the platform
  backend can publish stronger facts. Automated owners: `host_viewport_route_tests` and
  `viewport_lifecycle_record_reports_window_local_coordinate_status`.
- Docking policy scenes, divider hit maps, floating-model deltas, and local drop facts remain in
  absolute layout coordinates. Cross-window routing, global-screen conversion, and tear-off source
  bounds remain in window/display coordinates. The viewport snapshot retains GPUI's opaque
  committed `ElementGeometry` to convert between them; a transform-only frame advances route facts
  so a proof captured under old displayed geometry cannot authorize a later release.
- Viewport flags are capability-gated platform sync requests. No-input can be applied when a
  backend advertises native pointer-input routing; no-focus-on-appearing, no-focus-on-click, alpha,
  topmost, and no-taskbar use `PlatformViewportFlagCapabilities` and are recorded as unsupported
  requests until a backend exposes real live mutation support. The native runtime panel reports
  both capability snapshots and the latest applied/skipped/unsupported sync counts. Automated owners:
  `host_viewport_platform_capability_tests`,
  `viewport_runtime_syncs_supported_options_when_reusing_window`, and
  `empty_central_passthrough_syncs_window_pointer_input`.
- Preview proof is semantic rather than pixel-perfect. `DockPreviewVisualDescriptor` records the
  allowed/rejected decision, active layer, active zone, tab insertion descriptor, payload tab
  previews, and route-preview marker shape, while debug selectors continue to anchor rendered
  dogfood checks. Presentation, overlay, transition, zoom/focus, divider hit map, and accessibility
  descriptors are covered by focused tests. The native runtime panel exposes preview capability as
  `preview proof: presentation-scene+real-content-reveal+overlay-motion+tab-insertion+retargeting+splitter-motion+zoom-focus+divider-hit-map+corner-drag+a11y+route-cleanup+reduced-motion`
  and motion runtime capability as
  `motion proof: shared-runtime+run-state+scalar-value+scalar-sample+explicit-models+policy-gates+layout-projection+projection-clips+sampled-progress+retargeted-identity+reduced-motion-final-state+high-frequency-bypass`.
  The transition executor currently productizes sampled pane, divider, visual-affordance, zoom, and
  focus motion on top of explicit timeline or spring scalar models.
  Overlay-scene-to-transition conversion for tab insertion, payload ghosts, route markers, and
  rejected state is descriptor proof, not an every-frame drag-preview animation guarantee.
  Automated owners: `host_presentation_scene_tests`, `host_viewport_preview_visual_tests`,
  `host_transition_tests`, `host_zoom_focus_tests`, `host_divider_hit_map_tests`, and
  `host_accessibility_tests`. Transparent payload-window rendering, platform accessibility mapping,
  and screenshot or pixel-regression baselines remain explicitly deferred follow-up work.
- Routed overlay cleanup is fail-closed. Source-window route markers and target-window previews are
  separately renderable, but releases revalidate against current viewport facts instead of trusting
  cached preview state. Starting a new routed drag clears the previous session's routed preview,
  replacing a route target removes stale previews from the old target window, and Escape clears the
  GPUI active drag plus all routed preview state. Automated owners:
  `host_viewport_preview_tests`, `host_transition_tests`, and `host_render_tests`.
- Test ownership is split by concern. Route, lifecycle, placement, close, preview, platform
  capability, and visual-proof assertions live in focused `host_viewport_*_tests` modules; the old
  monolithic runtime test files have been deleted. Rendered native dogfood tests remain
  end-to-end integration coverage.
- `DockViewportRuntimeHandle` remains the application-facing facade. Platform sync and pointer-input
  requests now live behind `viewport_platform_sync`, window effects live behind
  `viewport_runtime_effects`, route/scene/close handle methods are split into
  `viewport_runtime_handle::{route_ops,scene_ops,close_ops}`, and coordinate facts live with the
  viewport registry/status model. New tests should target those owning modules instead of adding
  crate-private pass-throughs to the handle.

Before publishing a crate, confirm that the packaged archive carries the expected attribution files:

```sh
cargo package -p open-gpui --list --allow-dirty
```

For the canvas crate specifically, run:

```sh
cargo package -p open-gpui-canvas --list --allow-dirty
cargo publish -p open-gpui-canvas --dry-run --allow-dirty
```

Every published Open GPUI crate should include `README.md`, `LICENSE-APACHE`, and `NOTICE`. Cargo
does not package files outside a crate root through `include`, so each publishable crate root keeps
its own `NOTICE` copy.

The import-boundary scan rejects dependency files that reintroduce Zed's GPL tracing stack
(`ztracing`, `ztracing_macro`, `zlog`), the old `zed-sum-tree` dependency, the Zed monorepo as a
Cargo git dependency, retired Zed Git fork sources that have already been migrated, or the removed
Zed `perf` crate dependency. The retired `zed-scap` package and `zed-industries/scap` Git source
are also rejected now that screen capture resolves through the Open GPUI-owned
`open-gpui-scap` fork. The old crates.io `zed-font-kit` package is retired and should not be
reintroduced; font-kit resolves through the Open GPUI-owned fork configured in the crate manifests.
