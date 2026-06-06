# Verification

Run the local Open GPUI gate with:

```sh
cargo run -p xtask -- verify
```

The gate runs:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo check -p open-gpui-smoke-native`
- `cargo run -p xtask -- scan-import-boundary`

The import-boundary scan rejects dependency files that reintroduce Zed's GPL tracing stack
(`ztracing`, `ztracing_macro`, `zlog`), the old `zed-sum-tree` dependency, the Zed monorepo as a
Cargo git dependency, or the removed Zed `perf` crate dependency.

The scan intentionally does not reject the current external Zed-maintained forks that are still
tracked as follow-up debt: `zed-reqwest`, `zed-scap`, `zed-font-kit`, `zed-xim`, and Zed's `wgpu`
fork.
