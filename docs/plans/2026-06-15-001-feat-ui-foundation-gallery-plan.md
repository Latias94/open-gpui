---
title: "feat: Add UI foundation gallery"
type: "feat"
date: "2026-06-15"
---

# feat: Add UI foundation gallery

## Summary

Add a standalone gallery example that consumes `open-gpui-ui-core` directly and stays pure foundation.
The gallery will exercise tokens, sizing and density, adaptive layout, focus and accessibility, and
overlay behavior so the foundation API is forced to prove itself in a real consumer.

---

## Problem Frame

`open-gpui-ui-core` now exists, but it still lacks a real consumer that makes the foundation shape
visible. Without a dogfood surface, the crate can stay technically correct while still missing the
ergonomics and gaps that only appear when a full screen uses several foundation helpers together.

The current `examples/smoke-native` package is canvas-specific, so it is the wrong place to validate
the UI foundation. This plan creates a separate gallery package so the foundation signal stays clean
and reusable.

---

## Requirements

### Consumer package

- R1. Create a new workspace example package named `open-gpui-ui-foundation-gallery`.
- R2. Keep the package dependency surface limited to `open_gpui`, `open_gpui_ui_core`, and
  `open_gpui_platform` unless tests need an additional test-only dependency.
- R3. Make the package build as a small library plus a thin binary entrypoint so the shell and pages
  are testable.

### Foundation gallery

- R4. Render a pure foundation shell with separate sections for tokens, sizing and density,
  adaptive layout, focus and accessibility, and overlay behavior.
- R5. Include at least one compact or desktop switch and one focusable control so the gallery
  exercises the foundation contracts directly.
- R6. Surface any missing helper or vocabulary gap in `open-gpui-ui-core` instead of introducing a
  styled component layer.

### Verification and guardrails

- R7. Add targeted tests for the gallery shell and the helper behavior it relies on.
- R8. Update the verification docs so the gallery becomes part of the normal UI foundation dogfood
  path.

---

## Key Technical Decisions

- **Use a dedicated gallery package, not `smoke-native`.** `smoke-native` is canvas-oriented and
  would blur the signal we want from the UI foundation.
- **Keep the gallery pure foundation.** The consumer should prove `open-gpui-ui-core` directly
  without an `open-gpui-ui` dependency or a styled abstraction.
- **Use the reference set intentionally.** `fret` is the main architecture reference, especially
  for layering and adaptive shell strategy; `gpui-component` is the GPUI-native implementation
  seed; external desktop UI libraries stay comparison-only for behavior contracts.
- **Split the package into library and binary targets.** The library will hold shell and page state,
  while the binary stays a thin launcher.
- **Let the gallery drive only real foundation gaps.** If the consumer needs a helper that does not
  exist yet, the fix belongs in `open-gpui-ui-core`.
- **Use verification docs as the manual dogfood contract.** The gallery should become a repeatable
  path, not just a one-off demo.

---

## Output Structure

```text
examples/ui-foundation-gallery/
  Cargo.toml
  src/
    lib.rs
    main.rs
    shell.rs
    pages/
      mod.rs
      tokens.rs
      sizing.rs
      adaptive.rs
      focus_a11y.rs
      overlay.rs
  tests/
    foundation_gallery.rs
```

---

## Implementation Units

### U1. Scaffold the gallery package

- **Goal:** Add the new workspace example package and keep the entrypoint thin.
- **Requirements:** R1, R2, R3, R7.
- **Dependencies:** None.
- **Files:** `Cargo.toml`, `examples/ui-foundation-gallery/Cargo.toml`,
  `examples/ui-foundation-gallery/src/lib.rs`, `examples/ui-foundation-gallery/src/main.rs`,
  `examples/ui-foundation-gallery/tests/foundation_gallery.rs`.
- **Approach:** Register the package in the workspace, wire a library target that exports the shell,
  and keep the binary responsible only for opening the window and mounting the gallery root.
- **Patterns to follow:** `examples/docking-native/Cargo.toml`, `examples/smoke-native/Cargo.toml`,
  `examples/canvas-notes/src/main.rs`.
- **Test scenarios:** the workspace resolves the new package; the package compiles with the minimal
  dependency set; the binary boots into an empty gallery shell; the new package does not pull in a
  styled component crate.
- **Verification:** `open-gpui-ui-foundation-gallery` is a first-class workspace member and can be
  compiled independently.

### U2. Build the foundation gallery shell

- **Goal:** Render the top-level gallery layout and the foundation section pages.
- **Requirements:** R4, R5.
- **Dependencies:** U1.
- **Files:** `examples/ui-foundation-gallery/src/shell.rs`,
  `examples/ui-foundation-gallery/src/pages/mod.rs`,
  `examples/ui-foundation-gallery/src/pages/tokens.rs`,
  `examples/ui-foundation-gallery/src/pages/sizing.rs`,
  `examples/ui-foundation-gallery/src/pages/adaptive.rs`.
- **Approach:** Keep the shell visually simple and foundation-led. Use a stable navigation surface,
  a content area for the current page, and small controls for switching between compact and desktop
  views. Render raw token and sizing vocabulary so the page reflects the foundation API directly.
- **Patterns to follow:** `crates/gpui/examples/README.md`, `crates/gpui/src/elements/anchored.rs`,
  `crates/ui_core/src/adaptive.rs`, `crates/ui_core/src/tokens.rs`, `crates/ui_core/src/sizing.rs`.
- **Test scenarios:** each section is reachable from the shell; the token page shows the semantic
  vocabulary; the sizing page exposes the density vocabulary; the adaptive toggle changes the
  visible shell class without changing package dependencies.
- **Verification:** the gallery can present the foundation slices as separate pages and switch
  between compact and desktop presentation.

### U3. Wire focus, accessibility, and overlay behavior

- **Goal:** Make the gallery prove the interactive foundation helpers instead of only rendering them.
- **Requirements:** R4, R5, R6, R7.
- **Dependencies:** U2.
- **Files:** `examples/ui-foundation-gallery/src/pages/focus_a11y.rs`,
  `examples/ui-foundation-gallery/src/pages/overlay.rs`,
  `examples/ui-foundation-gallery/tests/foundation_gallery.rs`.
- **Approach:** Add a small focusable control cluster, an accessibility summary surface, and an
  anchored overlay demo. Keep the code on GPUI primitives plus `ui_core` helpers so it stays honest
  to the foundation boundary.
- **Patterns to follow:** `crates/gpui/examples/focus_visible.rs`, `crates/gpui/examples/tab_stop.rs`,
  `crates/gpui/examples/popover.rs`, `crates/gpui/src/_accessibility.rs`,
  `crates/ui_core/src/focus.rs`, `crates/ui_core/src/a11y.rs`, `crates/ui_core/src/overlay.rs`.
- **Test scenarios:** tab focus lands on the focusable demo control; escape dismisses the overlay;
  the accessibility surface exposes the expected role and state vocabulary; the overlay respects the
  gallery bounds and trigger anchor.
- **Verification:** the gallery shows focus and overlay behavior without introducing a custom widget
  layer.

### U4. Add dogfood verification and documentation

- **Goal:** Make the gallery part of the normal UI foundation verification path.
- **Requirements:** R7, R8.
- **Dependencies:** U2, U3.
- **Files:** `docs/verification.md`, `examples/ui-foundation-gallery/tests/foundation_gallery.rs`.
- **Approach:** Add a manual dogfood entry for the gallery with the compact/desktop, focus/a11y, and
  overlay checks. Keep the verification note focused on how to use the gallery and what behavior to
  inspect.
- **Patterns to follow:** `docs/verification.md` current gate structure and manual dogfood sections.
- **Test scenarios:** the verification doc names the new gallery package; a reviewer can use the doc
  to reproduce the same checks after a fresh build; the manual path points to the same gallery pages
  used by the tests.
- **Verification:** the gallery is discoverable as the first UI foundation smoke surface.

---

## Alternatives Considered

### Option A: New gallery example package

**Pros:** clean foundation signal, testable package boundary, reusable dogfood surface.

**Cons:** adds a new workspace member and a small amount of scaffolding.

**Decision:** chosen.

### Option B: Repurpose `examples/smoke-native`

**Pros:** less scaffolding.

**Cons:** canvas-specific dependencies would blur the UI foundation signal.

**Decision:** rejected.

### Option C: Keep iterating `open-gpui-ui-core` without a consumer

**Pros:** no new package.

**Cons:** no dogfood surface, so foundation gaps stay speculative.

**Decision:** rejected.

---

## Success Metrics

| Metric | Target | Measurement |
| --- | --- | --- |
| Workspace integration | New gallery package is a first-class workspace member | `cargo metadata` and workspace manifest review |
| Foundation coverage | Gallery exposes tokens, sizing, adaptive, focus/a11y, and overlay as separate pages | code review and manual run |
| Boundary discipline | Gallery depends on `open_gpui` and `open_gpui_ui_core`, not a styled component crate | manifest review |
| Verification path | `docs/verification.md` points to the gallery as the UI foundation dogfood surface | doc review |
| Testability | Gallery shell and helper behavior have targeted tests | `cargo nextest run` for the example package |

---

## Scope Boundaries

### Deferred for later

- A styled `open-gpui-ui` crate.
- Wenli reader shell polish.
- Rich text, editor, table, chart, and webview extensions.
- Full gallery visual design work beyond foundation clarity.

### Outside this product's identity

- Canvas or docking behavior.
- Runtime core changes that are not proven by the gallery consumer.
- A second component system that shadows `open-gpui-ui-core`.

---

## System-Wide Impact

This adds a new workspace package and a new manual verification surface. It does not change runtime
contracts, but it does establish a canonical dogfood path for the UI foundation.

---

## Risks & Dependencies

| Risk | Impact | Mitigation |
| --- | --- | --- |
| The gallery becomes a thin demo with no real pressure on `ui_core` | Medium | Keep the pages interactive and keep the dependency surface minimal. |
| The new example drifts toward a styled component library before the foundation is ready | High | Keep the package pure foundation and reject `open-gpui-ui` dependencies. |
| The gallery stays unverified and becomes hard to trust | Medium | Add targeted tests and wire the package into `docs/verification.md`. |
| `ui_core` still has missing ergonomics after the consumer lands | Medium | Fix only the missing foundation gap and keep the fix inside `open-gpui-ui-core`. |

---

## Documentation / Operational Notes

Update `docs/verification.md` with the gallery as the manual UI foundation path. Keep the note
short and practical so the example is easy to rediscover later.

---

## Sources & Research

- `fret` workspace, especially `fret-ui-kit` and `fret-ui-shadcn`
- `repo-ref/gpui-component`
- `docs/adr/0004-open-gpui-component-library-strategy.md`
- `docs/knowledge/engineering/current-state.md`
- `docs/knowledge/engineering/decisions/open-gpui-ui-foundation-first.md`
- `docs/knowledge/engineering/sessions/open-gpui-component-library-handoff.md`
- `crates/ui_core/src/lib.rs`
- `crates/ui_core/src/tokens.rs`
- `crates/ui_core/src/sizing.rs`
- `crates/ui_core/src/adaptive.rs`
- `crates/ui_core/src/focus.rs`
- `crates/ui_core/src/a11y.rs`
- `crates/ui_core/src/overlay.rs`
- `crates/gpui/examples/README.md`
- `crates/gpui/examples/focus_visible.rs`
- `crates/gpui/examples/tab_stop.rs`
- `crates/gpui/examples/popover.rs`
- `crates/gpui/src/_accessibility.rs`
- `crates/gpui/src/elements/anchored.rs`
- `examples/smoke-native/Cargo.toml`
- `examples/smoke-native/src/main.rs`
- `examples/docking-native/Cargo.toml`
- `../../../fret/ecosystem/fret-ui-kit/src/lib.rs`
- `../../../fret/ecosystem/fret-ui-kit/src/adaptive.rs`
- `../../../fret/ecosystem/fret-ui-kit/src/overlay.rs`
- `repo-ref/gpui-component/crates/ui/src/lib.rs`
- `repo-ref/gpui-component/README.zh-CN.md`
- `repo-ref/zed/crates/gpui/examples/anchor.rs`
- `repo-ref/zed/crates/gpui/examples/list_example.rs`
- `repo-ref/zed/crates/gpui/examples/uniform_list.rs`
- `repo-ref/zed/crates/gpui/examples/window_positioning.rs`
