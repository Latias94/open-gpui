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

CI runs the same gate on Windows for pushes to `master`, pull requests, and manual workflow
dispatches. It also runs the `xtask` unit tests with nextest.

The import-boundary scan rejects dependency files that reintroduce Zed's GPL tracing stack
(`ztracing`, `ztracing_macro`, `zlog`), the old `zed-sum-tree` dependency, the Zed monorepo as a
Cargo git dependency, or the removed Zed `perf` crate dependency.

The scan intentionally does not reject the current external Zed-maintained forks that are still
tracked as follow-up debt: `zed-scap`, `zed-font-kit`, and Zed's `wgpu` fork.
