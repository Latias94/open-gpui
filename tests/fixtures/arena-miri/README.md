# Element Arena Miri Tests

This independent package runs the production `crates/gpui/src/arena.rs` directly, including its
existing tests. It copies no allocator code and avoids the native platform and renderer graph.

Run serially from the repository root after installing the nightly Miri and rust-src components:

```sh
cargo +nightly miri test --locked --manifest-path tests/fixtures/arena-miri/Cargo.toml --target-dir target
```

Repeat with `MIRIFLAGS=-Zmiri-tree-borrows` to exercise Tree Borrows. The default interpreter uses
Stacked Borrows. Use one build job and the same ordinary target directory as other local Cargo
work; do not run this concurrently with workspace builds.

This fixture checks allocator alignment, chunk spill/reuse, nested-clear deferral, held references
and invalidation. It does not compile the complete App, TLS scope or Window, so it does not
qualify their full aliasing, native rendering, IME or operating-system lifecycle behavior. Those
remain separate framework and native regression gates.
