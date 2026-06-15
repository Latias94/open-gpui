# Engineering Memory Update Log

## 2026-06-15
* **Update**: Completed U4 of the UI foundation gallery plan: `docs/verification.md` now documents focused `open-gpui-ui-core` / gallery checks and the manual `cargo run -p open-gpui-ui-foundation-gallery` dogfood path; package checks and nextest runs pass.
* **Update**: Completed U3 of the UI foundation gallery plan: focus/a11y and overlay now have interactive demos backed by `open-gpui-ui-core` focus/a11y/overlay vocabulary, and `cargo nextest run -p open-gpui-ui-foundation-gallery` passes 10/10 tests.
* **Update**: Completed U2 of the UI foundation gallery plan: tokens, sizing/density, and adaptive pages now render real `open-gpui-ui-core` data models, the shell has a compact/desktop switch, and `cargo nextest run -p open-gpui-ui-foundation-gallery` passes 8/8 tests.
* **Update**: Completed U1 of the UI foundation gallery plan by adding `examples/ui-foundation-gallery` as a workspace package with a small library, thin binary, pure foundation dependency surface, empty shell, section registry, and passing `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Wrote the first follow-up plan at `docs/plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md` and locked the first consumer choice to a dedicated pure-foundation gallery example.
* **Update**: Recorded the reference repository set for the Open GPUI UI strategy: `fret`, `fret-ui-kit`, `fret-ui-shadcn`, `gpui-component`, plus broader open source comparators such as Flutter, Jetpack Compose, Radix UI, React Aria, React Spectrum, shadcn/ui, and Apple HIG / SwiftUI.
* **Update**: Implemented the first Open GPUI UI foundation slice on `feat/open-gpui-ui-core` with the new `open-gpui-ui-core` crate, sizing/adaptive/token/overlay helpers, a11y/focus re-exports, and passing `cargo nextest run -p open-gpui-ui-core`.
* **Update**: Updated ADR 0004 to prioritize a11y, focus, overlay, tokens, sizing, density, and adaptive layout before broad component rollout; added decision and session memory for the UI foundation-first strategy.
* **Initialization**: Created engineering wiki memory bundle.
