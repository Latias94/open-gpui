# Native UI Framework Strategy

Open GPUI's UI ecosystem is a Rust-first native UI framework, not a web component registry port.
The official implementation surface ships as Cargo crates. The component registry is metadata that
makes those crates inspectable, scaffoldable, verifiable, and easier for agents to navigate.

The detailed implementation workflow is recorded in
[Native UI Hybrid Registry](native-ui-hybrid-registry.md).

## Distribution Model

`open-gpui-ui-core` owns renderer-neutral primitives such as tokens, sizing, accessibility
vocabulary, overlay policy, and table/virtualizer data contracts. `open-gpui-ui-components` owns the
concrete GPUI adapters and official component APIs. Applications depend on those crates through
Cargo; copied source is not the upgrade or compatibility authority.

The metadata registry complements Cargo distribution:

- `docs/registry/open-gpui-component-registry-v1.json` is the committed component registry
  artifact generated from `component_contract`.
- `docs/schemas/open-gpui-component-registry-v1.schema.json` is the committed schema for registry
  consumers.
- `cargo run -p open-gpui-ui-components --example export_component_registry --quiet` regenerates
  the registry artifact.
- `cargo run -p open-gpui-ui-components --example export_component_registry_schema --quiet`
  regenerates the schema artifact.
- `cargo run -p xtask -- scan-ui-registry` compares both artifacts with generated output and checks
  recipe references.

## Registry Authority

The canonical source of component metadata remains
`crates/ui_components/src/component_contract/`. The manifest is a deterministic projection of that
typed registry; it does not replace the Rust rows.

The public manifest API is:

- `open_gpui_ui_components::component_contract::COMPONENT_REGISTRY_MANIFEST_VERSION`;
- `open_gpui_ui_components::component_contract::component_registry_manifest()`;
- `open_gpui_ui_components::component_contract::component_registry_manifest_schema()`.

Schema version `1` is the current compatibility surface for local tooling and committed artifacts.

Manifest entries describe:

- component or public-surface name;
- ownership class;
- family;
- docs status and docs token;
- gallery status and evidence owner;
- source home and source inputs;
- root/prelude export intent;
- API inventory summary when a row has public component API data;
- local verification owners.

## Scaffold Recipes

Scaffold recipes are app-owned, Cargo dependency, or gallery-owned starter metadata. They declare
recipe ids, source components, generated-file intents, required imports, customization boundaries,
verification gates, and output ownership.
`ScaffoldRecipeOutputOwnership` currently has three public classes:
`AppOwnedSource`, `CargoDependencySnippet`, and `GalleryStorySample`.

The initial recipe set is intentionally small:

- `table-filters-toolbar`;
- `field-control-composition`;
- `themed-surface-wrapper`;
- `gallery-story-sample`.

Recipes may help local applications start from a proven composition pattern, but they do not make
official components source-copy packages. Official component implementations remain Cargo-owned.

## Evidence Loop

The normal ecosystem loop is:

1. Add or modify typed registry rows in `component_contract`.
2. Add or update recipe metadata only when a composition starter is useful.
3. Regenerate the registry and schema artifacts.
4. Run `cargo run -p xtask -- scan-ui-registry`.
5. Run `cargo run -p xtask -- scan-ui-contract` and focused component/gallery tests for behavior.
6. Update docs and ADRs when artifact names, commands, or ownership rules change.

Gallery selectors and story probes remain owned by `examples/ui-foundation-gallery`. Gallery tests
consume the manifest and prove that rows claiming gallery evidence have catalog, selector, or story
coverage. The component crate never imports gallery selector constants.

## Non-Goals

This strategy does not introduce a hosted marketplace, a shadcn-style source-copy package manager,
or a separate `open-gpui-ui-headless` crate as part of the current work. Those may become future
projects after the Cargo-first metadata registry, verification gates, and component contracts stay
stable under real changes.
