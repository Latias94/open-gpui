# Native UI Framework Strategy

Open GPUI's UI ecosystem is a Rust-first native UI framework, not a web component registry port.
The official implementation surface ships as Cargo crates, and the source code plus focused contract tests are the primary discovery and verification surface.

## Distribution Model

`open-gpui-ui-core` owns renderer-neutral primitives such as tokens, sizing, accessibility vocabulary, overlay policy, and table/virtualizer data contracts.
`open-gpui-ui-components` owns concrete GPUI adapters and official component APIs.
Applications depend on those crates through Cargo; copied source is not the upgrade or compatibility authority.
The curated `open_gpui_ui_components` default surface should not masquerade as the owner of command
runtime or broad renderer-neutral infrastructure. Applications import command infrastructure from
`open_gpui_command` and shared core table/virtualizer/grid contracts from `open_gpui_ui_core`,
while component modules may still expose the component-facing state and helper types that make a
specific official component API coherent.

Open GPUI no longer ships a generated component registry manifest, scaffold recipe manifest, or registry JSON/schema artifact.
ADR 0014 supersedes the hybrid registry experiment because it duplicated source facts that AI agents and maintainers can read directly from the crate.

## Contract Authority

The canonical source of component product metadata remains
`crates/ui_components/src/component_contract/`. Those typed rows own only official id, revision,
family, and required scenario ids; they are not an external ecosystem registry. Public modules own
exports, Gallery owns selectors and runtime probes, DevTools owns its projection, native test
targets own exact executable coordinates, and docs own the human-readable projection.

The reusable local checks are:

1. Update component contract rows only when official identity, revision, family, or required
   product scenarios change.
2. Run `cargo run -p xtask -- scan-ui-contract`.
3. Run focused component, gallery, a11y, theme, or table/tree/menu tests for behavior that changed.
4. Update docs and ADRs only when crate boundaries, public APIs, or compatibility rules change.

Gallery selectors and story probes remain owned by `examples/ui-foundation-gallery`.
Gallery tests join canonical metadata to their own rows, but `open-gpui-ui-components` must not
import Gallery selector constants. Each integration target binds required scenarios through a
sibling `*.scenarios.toml`; `scan-ui-contract` validates and runs those exact nextest coordinates.
Official choice/search stories should expose both sample selectors and state-readout selectors so
tests can prove public payloads without relying on renderer internals.

## Non-Goals

The current strategy does not include:

- a hosted component registry;
- `gpui add` or another public CLI that edits application source;
- a shadcn-style source-copy package manager;
- a generated component registry JSON/schema artifact;
- scaffold recipe metadata as a public API;
- a separate `open-gpui-ui-headless` crate as part of the active roadmap.

Those ideas can be revisited only if repeated real application work shows that source inspection plus typed contract tests are not enough.
