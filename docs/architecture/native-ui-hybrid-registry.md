# Native UI Hybrid Registry

Open GPUI uses a hybrid registry model for its native UI framework. Cargo remains the distribution
authority for official components, while generated metadata makes the component ecosystem
inspectable, scaffoldable, and verifiable.

This page is the implementation guide for ADR 0013.

## Authorities

The official shipped surface stays in Rust crates:

- `open-gpui-ui-core` owns renderer-neutral tokens, sizing, accessibility vocabulary, overlay
  policy, split/motion helpers, and portable contracts.
- `open-gpui-ui-components` owns official GPUI component APIs, adapters, default exports, and the
  typed component contract registry.
- `examples/ui-foundation-gallery` owns gallery dogfood evidence, sample selectors, and story
  probes.

The registry is metadata over those authorities. It is not a package manager, marketplace, or
source-copy component registry.

## Manifest API

The public manifest API lives under `open_gpui_ui_components::component_contract`:

- `COMPONENT_REGISTRY_MANIFEST_VERSION`
- `component_registry_manifest()`
- `component_registry_manifest_schema()`

Version 1 entries are deterministic projections of typed contract facts. They include component
identity, ownership class, family, source home, docs token, docs status, gallery status, root and
prelude export intent, API inventory summary, scaffold recipes, generated file intent, and
verification owners.

Manifest entries must stay renderer-neutral. They must not contain GPUI runtime handles, callbacks,
`Window`, `App`, `Context`, `Element`, focus handles, or scroll handles.

## Committed Artifacts

The committed artifacts are:

- `docs/registry/open-gpui-component-registry-v1.json`
- `docs/schemas/open-gpui-component-registry-v1.schema.json`

Regenerate them after component contract or recipe metadata changes:

```powershell
cargo run -p open-gpui-ui-components --example export_component_registry --quiet
cargo run -p open-gpui-ui-components --example export_component_registry_schema --quiet
```

Then verify drift and internal references:

```powershell
cargo run -p xtask -- scan-ui-registry
```

`scan-ui-registry` compares generated output with committed artifacts and checks required registry
rows, scaffold recipe ids, recipe source-component references, generated file intents, and
verification gates. `xtask verify` runs this scan before `scan-ui-contract` so ecosystem drift fails
before broader docs, gallery, accessibility, or theme claims are evaluated.

## Scaffold Recipes

Recipes live in `crates/ui_components/src/component_contract/recipes.rs`. The first recipe ids are:

- `table-filters-toolbar`
- `field-control-composition`
- `themed-surface-wrapper`
- `gallery-story-sample`

Recipes describe app-owned, Cargo dependency, or gallery-owned starting points. They declare source
components, required imports, generated file intent, customization boundaries, verification gates,
and `ScaffoldRecipeOutputOwnership`.

Recipe output uses the public classes `AppOwnedSource`, `CargoDependencySnippet`, and
`GalleryStorySample`. Official component implementations remain Cargo-owned even when a recipe
scaffolds local wrapper, dependency, or sample code.

## Add Modify Verify Loop

Use this loop for component ecosystem changes:

1. Add or update typed rows in `crates/ui_components/src/component_contract/`.
2. Add or update scaffold recipe metadata when a composition starter is useful.
3. Regenerate `docs/registry/open-gpui-component-registry-v1.json`.
4. Regenerate `docs/schemas/open-gpui-component-registry-v1.schema.json`.
5. Run `cargo run -p xtask -- scan-ui-registry`.
6. Run `cargo run -p xtask -- scan-ui-contract`.
7. Run the focused `cargo nextest` gates for the component, gallery, accessibility, or theme
   behavior that changed.
8. Update docs, engineering memory, and ADRs only when names, ownership, or compatibility rules
   change.

Gallery evidence remains a verification input, not the source of component truth. Gallery tests may
consume manifest rows, but the component crate must not import gallery selector constants.

## Non-Goals

This architecture does not ship:

- a hosted registry service;
- `gpui add` or another public CLI that edits application source;
- a shadcn-style source-copy registry as the official component distribution model;
- third-party registry publishing;
- manifest version negotiation beyond schema version 1;
- a new `open-gpui-ui-headless` crate.

These may be revisited by later ADRs after the Cargo-first metadata registry proves stable under
real component, gallery, theme, and accessibility changes.
