# Refine gallery catalog metadata

Created: 2026-06-18
Origin: current UI gallery architecture review and selector unification pass

## Problem Frame

`ComponentCatalogEntry` is still shallow. The Components page shell and its conformance test still
carry selector knowledge outside the catalog, so the catalog behaves like a naming table instead of
the seam for official gallery metadata.

## Scope

Deepen the catalog so official sample selector knowledge comes from `pages/components.rs`, and the
Components gallery test derives its selector list from that catalog metadata instead of from a local
sample-builder scan. Keep rendered samples, visible gallery cards, and component runtime behavior
unchanged. Do not add a new crate.

## Implementation Units

### U1 - Add catalog-owned selector metadata

Files:
- `examples/ui-foundation-gallery/src/pages/components.rs`

Goal:
- Give each official catalog entry a stable sample selector description that the test layer can
  query without rebuilding sample data.

Test scenarios:
- Every official catalog entry exposes exactly one selector value.
- Non-official entries remain explicit and do not pretend to be gallery samples.
- Selector values stay unique across the official catalog.

### U2 - Derive gallery coverage from the catalog

Files:
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`

Goal:
- Remove the local official-sample selector table built from sample builders.
- Derive the official selector list from `COMPONENT_CATALOG` and keep the existing catalog-name and
  signal coverage assertions.

Test scenarios:
- The official selector list matches the official catalog set.
- The Components smoke still finds every official sample on the rendered page.
- The catalog conformance assertion still fails if a required signal is missing.

### U3 - Verify and record the new seam

Files:
- `docs/knowledge/engineering/current-state.md`
- `docs/knowledge/engineering/log.md`

Goal:
- Run the gallery and component verification gates, then record the new catalog seam in engineering
  memory so the next session resumes from the deeper catalog state.

Test scenarios:
- `cargo fmt --all --check`
- `cargo check -p open-gpui-ui-foundation-gallery --tests`
- `cargo nextest run -p open-gpui-ui-foundation-gallery`
- `cargo nextest run -p open-gpui-ui-components`

## Existing Patterns To Follow

- `examples/ui-foundation-gallery/src/pages/components.rs` for catalog shape and conformance gates.
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs` for the existing selector coverage
  and gallery smoke style.

## Risks

- A selector table can drift if the catalog metadata and gallery sample helpers diverge again.
- Overfitting the catalog to today’s selector list could make future sample growth awkward, so keep
  the metadata small and explicit.
