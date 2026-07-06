# Canvas Jellyflow Showcase

`open-gpui-canvas-jellyflow` is an optional showcase for integrating
`open-gpui-canvas` with the external Jellyflow graph runtime.

It is intentionally excluded from the default Open GPUI workspace because it
depends on sibling repositories that are not part of a normal checkout:

- `../../../../crates/jellyflow`
- `../../../../crates/jellyflow-open-gpui`

Use the normal workspace examples first:

```sh
cargo run -p open-gpui-canvas-notes
cargo run -p open-gpui-docking-native
cargo run -p open-gpui-ui-foundation-gallery
```

When the sibling Jellyflow repositories are present, run this showcase
explicitly:

```sh
cargo run --manifest-path examples/canvas-jellyflow/Cargo.toml
```
