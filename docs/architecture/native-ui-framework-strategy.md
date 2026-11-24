# Native UI Framework Strategy

Open GPUI's UI ecosystem is a Rust-first native UI framework, not a web component registry port.
The official implementation surface ships as Cargo crates, and the source code plus focused contract tests are the primary discovery and verification surface.

## Distribution Model

`open-gpui-ui-core` owns renderer-neutral primitives such as tokens, sizing, accessibility vocabulary, overlay policy, and table/virtualizer data contracts.
`open-gpui-ui-components` owns concrete GPUI adapters and official component APIs.
Applications depend on those crates through Cargo; copied source is not the upgrade or compatibility authority.
Applications provide command items, menu descriptors, and action handlers. GPUI owns keymaps and
action dispatch. Shared table, virtualizer and grid contracts live in `open_gpui_ui_core`.

Open GPUI no longer ships a generated component registry manifest, scaffold recipe manifest, or registry JSON/schema artifact.
ADR 0014 supersedes the hybrid registry experiment because it duplicated source facts that AI agents and maintainers can read directly from the crate.

## Contract Authority

The canonical source of component product metadata remains
`crates/ui_components/src/component_contract/`. Those typed rows own only official id, revision,
family, and required scenario ids. Public modules own exports and integration tests own executable
coordinates and runtime probes.

The reusable local checks are:

1. Update component contract rows only when official identity, revision, family, or required
   product scenarios change.
2. Run `cargo nextest run --locked -p open-gpui-ui-components --test public_surface`.
3. Run focused component, a11y, theme, or table/tree/menu tests for behavior that changed.
4. Update docs and ADRs only when crate boundaries, public APIs, or compatibility rules change.

## Non-Goals

The current strategy does not include:

- a hosted component registry;
- `gpui add` or another public CLI that edits application source;
- a shadcn-style source-copy package manager;
- a generated component registry JSON/schema artifact;
- scaffold recipe metadata as a public API;
- a separate `open-gpui-ui-headless` crate as part of the active roadmap.

Those ideas can be revisited only if repeated real application work shows that source inspection plus typed contract tests are not enough.
