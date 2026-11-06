# Independent dependency consumer

This application is its own Cargo workspace. It consumes Open GPUI through path dependencies
without inheriting the repository workspace's dependency resolution or lockfile. Its own
`Cargo.lock` records the consumer resolution.

The application pins Taffy 0.10.0, WGPU 30.0.1, and, on macOS, Core Foundation 0.10.1 to exercise
compatible dependency choices independently of the library workspace. Keep these exact pins in
the application fixture; library dependencies should allow compatible patch releases. The direct
WGPU dependency requests only its `std` feature for API types.

The optional `wgpu-renderer` feature also includes `open-gpui-wgpu` in the consumer graph, so the
all-features gate compiles the actual renderer against the application's pinned WGPU 30.0.1.
Cargo unifies the renderer's WGPU feature requests with the direct dependency's features. This
compile check does not select WGPU as the Windows platform backend or establish GPU resource
sharing.

The source composes a GPUI `Render` view, a semantic button activation handler, custom canvas
painting, and a `DockSurface` panel factory. Taffy's size type supplies the preview dimensions;
WGPU's texture format type supplies the preview label. It adds no document or history model.

The platform dependency enables `x11` and `wayland` on Linux and FreeBSD, and `font-kit` on macOS.
Windows uses the shared platform dependency without additional fixture-selected features.

Run these commands from the repository root. For dependency maintenance, generate or refresh the
consumer lockfile, then review and commit it:

```text
cargo generate-lockfile --manifest-path tests/fixtures/dependency-consumer/Cargo.toml
```

The regular fixture gate checks all application targets with all features, including the WGPU
renderer, against that lockfile and reuses the repository's build directory with one build job:

```text
cargo check --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-targets --all-features --jobs 1 --target-dir target
```

Run the same gate separately on Windows, Linux, and macOS in CI. Record results per host;
a Windows result does not qualify compilation on macOS, Linux, or FreeBSD.

This is a source compilation and dependency resolution gate. It requires the normal native build
prerequisites for the host platform. No application launch or test framework is required. Passing
the gate does not qualify native window behavior, visual output, GPU execution, or the declared
minimum Rust version. Platform runtime features and renderer execution need separate validation.

The optional public text tests consume `IndentAdjustment`, Unicode wrapping, Middle truncation and
the fluent style API through this independent lockfile. Run them separately from compilation:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test wrapping --test-threads=1 --no-fail-fast --no-tests fail
```

These tests use `NoopTextSystem` for deterministic boundary and run checks. They do not launch the
application or establish real-font, native, GPU or compiler-floor qualification. The framework's
separate wrapping consumer uses actual Cosmic fonts and public headless paint.
