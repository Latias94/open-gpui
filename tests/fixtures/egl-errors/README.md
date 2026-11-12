# EGL binding error regression

This fixture consumes the real `khronos-egl` public binding. It has no Cargo manifest and
does not change workspace resolution. Compile it with `rustc` against the exact source below.
The upstream crate's own tests remain the primary regression owner; this consumer adds the
dynamic loader and the `initialize` wrapper used by WGPU.

## Fixed source and license

- Zed integration: [`19c69782eb3c1d99b79878d82efa935d565910f9`](https://github.com/zed-industries/zed/commit/19c69782eb3c1d99b79878d82efa935d565910f9).
- Binding: [`zed-industries/khronos-egl@42214b36347b108fd9630ca6254b96dde59e04fe`](https://github.com/zed-industries/khronos-egl/tree/42214b36347b108fd9630ca6254b96dde59e04fe).
- Package: `khronos-egl 6.0.0`, edition 2021, manifest license `MIT/Apache-2.0`.
- Both [MIT](https://raw.githubusercontent.com/zed-industries/khronos-egl/42214b36347b108fd9630ca6254b96dde59e04fe/LICENSE-MIT)
  and [Apache-2.0](https://raw.githubusercontent.com/zed-industries/khronos-egl/42214b36347b108fd9630ca6254b96dde59e04fe/LICENSE-APACHE)
  texts are present and byte-identical to registry 6.0.0. Preserve both if vendoring.
- The pin's parent is `5605af0` (version 6.0.0); its `src/lib.rs` is byte-identical to the
  installed registry 6.0.0 source. The pinned commit changes only `src/lib.rs`.
  Manifest, dependencies, features, build script and license texts are unchanged.
- Upstream submission: [khronos-egl#37](https://github.com/timothee-haudebourg/khronos-egl/pull/37).
  This review establishes the fixed commit, without assuming a later release contains it.
- Fixed `src/lib.rs` SHA-256:
  `0ac61d8b11e61846a5360b53ba9c3b7f6ec65b6d7de049e74074c15524cdbbae`.

## Complete wrapper audit

All 39 former `Err(self.get_error().unwrap())` branches call `get_call_error()` at the pin.
The helper reads the native error exactly once and uses `unwrap_or(Error::Unreported)`.
Every success branch returns before that helper. The full set is:

| EGL version | Guarded public wrappers |
| --- | --- |
| 1.0 (21) | `matching_config_count`, `choose_config`, `copy_buffers`, `create_context`, `create_pbuffer_surface`, `create_pixmap_surface`, `create_window_surface`, `destroy_context`, `destroy_surface`, `get_config_attrib`, `get_config_count`, `get_configs`, `initialize`, `make_current`, `query_context`, `query_string`, `query_surface`, `swap_buffers`, `terminate`, `wait_gl`, `wait_native` |
| 1.1 (4) | `bind_tex_image`, `release_tex_image`, `surface_attrib`, `swap_interval` |
| 1.2 (4) | `bind_api`, `create_pbuffer_from_client_buffer`, `release_thread`, `wait_client` |
| 1.5 (10) | `create_sync`, `destroy_sync`, `client_wait_sync`, `get_sync_attrib`, `create_image`, `destroy_image`, `get_platform_display`, `create_platform_window_surface`, `create_platform_pixmap_surface`, `wait_sync` |

`choose_first_config` delegates to guarded `choose_config`. Attribute-list checks and empty
buffer shortcuts retain their previous behavior. Optional-handle getters such as `get_display`
and `get_current_context` retain `None` for null handles. `get_error()` still returns `None`
for `SUCCESS`; `Error::try_from(SUCCESS)` still returns `Err(SUCCESS)`. Only a failed native
call synthesizes `Unreported`, whose `native()` and `Int::from` report the observed `SUCCESS`.
Unknown, nonstandard native error integers still reach the existing `try_into().unwrap()`;
this specific fix handles failed calls reporting `EGL_SUCCESS`.

The eight upstream unit tests use `Instance<MockEgl>` to exercise the actual wrapper bodies:
both window-surface constructors get failure/SUCCESS, real-error and success/no-query controls;
`wait_gl` gets the failed-boolean control; the last test checks the exact `Error::Unreported`
variant's native conversions and diagnostic. They do not use the dynamic loader.

## ABI and downstream path

The binding retains `extern "system"` for native declarations, dynamic function pointers and
provider methods. On Windows this preserves the system calling convention; on Linux it uses
the platform C convention. The Windows/Linux handle aliases remain pointers, `Boolean` remains
`c_uint`, `Int` remains `i32` and `Attrib` remains `usize`. No production native signature or
dynamic table layout changes. Adding the public Rust enum variant is source-breaking for an
exhaustive downstream match; it is not a stable C ABI addition.

The inspected WGPU source is exactly
[`cddb2f5d80dc0009b2ef6767635140cacf90a007`](https://github.com/gfx-rs/wgpu/tree/cddb2f5d80dc0009b2ef6767635140cacf90a007),
whose workspace requests `khronos-egl = "6"` and whose native GLES feature enables `dynamic`.

- Linux/FreeBSD: `open-gpui-linux` X11/Wayland windows construct `WgpuRenderer`;
  `WgpuContext::instance` requests `VULKAN | GL`; WGPU Core's `try_add_hal(Gles)` enters
  `wgpu-hal/src/gles/egl.rs`. It loads `DynamicInstance<EGL1_4>`, obtains a display and calls
  `Inner::create -> initialize_display -> egl.initialize`. Initialization errors become
  `InstanceError` with their source; Core can retain other successfully initialized backends.
- Surface configuration reaches `create_platform_window_surface` through the EGL 1.5 upcast,
  or `create_window_surface` as fallback. This is the integration commit's reported Wayland
  panic scenario. Wrapper errors now reach WGPU's existing `SurfaceError::Lost` path.
  Context creation, `make_current` and `swap_buffers` also use guarded wrappers.
- Windows: ordinary GPUI uses its DirectX renderer. WGPU GLES defaults to WGL, selected by
  `wgpu-hal/src/gles/mod.rs`; only `cfg(windows_angle)` selects the EGL owner and `libEGL.dll`.
  ANGLE's D3D11 platform-display route then uses the same initialization/context/surface
  wrappers. A Windows default build alone cannot qualify ANGLE execution.
- WGPU's context robustness match singles out `BadAttribute | BadMatch | BadConfig`, then
  has `Err(e) => return Err(e)`. `Unreported` is accepted by this catch-all; other inspected
  EGL error paths format/map/propagate the error without exhaustive variant matching.
- Browser WebGL uses the web owner, not native EGL. Emscripten has a separate static EGL
  selection. A lockfile containing EGL does not by itself prove native runtime reachability.

## Dynamic consumer

`mock_egl.rs` builds a small `cdylib` exporting all 44 correctly typed EGL 1.0-1.5 symbols.
It imports the real binding's FFI types and uses `extern "system"`. Four exercised symbols
implement result/error injection and valid success outputs. The other 40 abort if called;
they exist to satisfy the real versioned loader, without pretending to implement a driver.
Two extra symbols set the response and count error reads. Tests serialize the process-global
mock state with a mutex. Every test loads an explicit fixture path; no system EGL, display,
window system or GPU is involved.

`egl_error_tests.rs` uses the same concrete `DynamicInstance<EGL1_4>` type as WGPU, including
the 1.5 upcast. Its three tests each call `initialize`, `create_window_surface` and
`create_platform_window_surface`. They check failed/SUCCESS, two preserved native errors,
and successful/no-error-query behavior. Successful initialization must return `(1, 5)`;
surfaces must be nonnull; an error read must consume the stored error.

The dynamic test checks the unreported case's native value and exact diagnostic so the
identical source also compiles against unfixed registry 6.0.0. The upstream tests separately
assert `Err(Error::Unreported)` directly. Both layers execute production binding wrappers;
only the upstream layer substitutes the Rust provider trait. The dynamic layer substitutes
native symbols through the actual loader.

## Executed receipt and commands

On 2026-11-13, macOS `aarch64-apple-darwin`, `rustc 1.99.1`:

- Exact pinned crate unit tests: **8 passed**.
- Dynamic consumer, exact pinned binding: **3 passed**, covering all three wrappers.
- Identical dynamic consumer, registry 6.0.0 binding: **2 passed, 1 failed**. Failure occurs
  in the first wrapper, `initialize`, at registry `src/lib.rs:1065:27`,
  `called Option::unwrap() on a None value`; that test stops before its other two calls.
- Binding compilation emits the pre-existing `get_proc_address` nonnullable-function-pointer
  warning. No native driver execution, Windows/Linux compilation, MSRV gate or patched
  repository Cargo resolution is claimed. No Cargo command was run for this fixture.

The exact source snapshot and executables from this run are in
`/tmp/open-gpui-egl-XTkZQF`. From the repository root, these zsh commands rerun/build the
consumer using the existing host dependency artifacts (all must match the selected rustc):

```zsh
EGL_TASK=/tmp/open-gpui-egl-XTkZQF
EGL_LIBC="$PWD/target/debug/deps/liblibc-3a9c9ac3c167ae7a.rlib"
EGL_LOADING="$PWD/target/debug/deps/liblibloading-7934645bf025c797.rlib"
EGL_FEATURES=(--cfg 'feature="1_0"' --cfg 'feature="1_1"' --cfg 'feature="1_2"' --cfg 'feature="1_3"' --cfg 'feature="1_4"' --cfg 'feature="1_5"')

rustc --test --crate-name khronos_egl --edition=2021 "${EGL_FEATURES[@]}" --extern "libc=$EGL_LIBC" -L dependency=target/debug/deps "$EGL_TASK/fixed/src/lib.rs" -o "$EGL_TASK/upstream-unit-tests"
"$EGL_TASK/upstream-unit-tests" --test-threads=1

rustc --crate-name khronos_egl --crate-type rlib --edition=2021 "${EGL_FEATURES[@]}" --cfg 'feature="dynamic"' --extern "libc=$EGL_LIBC" --extern "libloading=$EGL_LOADING" -L dependency=target/debug/deps "$EGL_TASK/fixed/src/lib.rs" -o "$EGL_TASK/libkhronos_egl_fixed.rlib"
rustc --crate-name egl_error_mock --crate-type cdylib --edition=2021 tests/fixtures/egl-errors/mock_egl.rs --extern "khronos_egl=$EGL_TASK/libkhronos_egl_fixed.rlib" -L dependency=target/debug/deps -o "$EGL_TASK/libegl_error_mock.dylib"
rustc --test --edition=2021 tests/fixtures/egl-errors/egl_error_tests.rs --extern "khronos_egl=$EGL_TASK/libkhronos_egl_fixed.rlib" --extern "libloading=$EGL_LOADING" -L dependency=target/debug/deps -o "$EGL_TASK/dynamic-fixed-tests"
OPEN_GPUI_EGL_MOCK="$EGL_TASK/libegl_error_mock.dylib" "$EGL_TASK/dynamic-fixed-tests" --test-threads=1

OPEN_GPUI_EGL_MOCK="$EGL_TASK/libegl_error_mock.dylib" "$EGL_TASK/dynamic-registry-tests" --test-threads=1
```

To rebuild the negative control, compile the same binding command with registry 6.0.0's
`src/lib.rs` and output `libkhronos_egl_registry.rlib`; compile the same test source against
that rlib and output `dynamic-registry-tests`. The same mock works because the native ABI
is unchanged. On Linux use `.so`, and on Windows `.dll`, in the mock output and environment
variable; host dependency artifacts must be rebuilt on that host before reusing these commands.

To reconstruct the snapshot without touching `repo-ref/zed`, clone the official binding
repository into a task temporary directory, then extract `src/lib.rs` and license files
with `git show 42214b36347b108fd9630ca6254b96dde59e04fe:PATH`. The inspected source was extracted
this way from an independent temporary clone. Do not apply the fix to registry/cache sources.

## Root patch handoff

Add this entry to each root's `[patch.crates-io]` section; declaring a library dependency
does not export its root patches to consumers:

```toml
khronos-egl = { git = "https://github.com/zed-industries/khronos-egl", rev = "42214b36347b108fd9630ca6254b96dde59e04fe" }
```

The complete repository-owned root inventory, excluding references and build artifacts, is
below. Apply the same entry to all eight roots if maintaining a uniform root source policy.
Only the first four currently resolve EGL in their existing lockfiles; the other four have
no current EGL edge, so their entry can be unused and is not a runtime fix.

| Root manifest path | Existing lock / EGL state at inspection |
| --- | --- |
| `Cargo.toml` | `Cargo.lock`: registry EGL 6.0.0; pinned WGPU 30.0.0 |
| `tests/fixtures/dependency-consumer/Cargo.toml` | Sibling lock: registry EGL 6.0.0; pinned WGPU 30.0.0 |
| `crates/gpui_web/examples/smoke_web/Cargo.toml` | Sibling lock: registry EGL 6.0.0; pinned WGPU 30.0.0 |
| `crates/gpui_web/examples/hello_web/Cargo.toml` | Sibling lock: registry EGL 6.0.0; registry WGPU HAL 29.0.4; no WGPU upgrade is implied |
| `tests/fixtures/hot-reload-consumer/Cargo.toml` | Existing lock has no EGL or WGPU HAL |
| `tests/fixtures/test-support-consumer/Cargo.toml` | Existing lock has no EGL or WGPU HAL |
| `crates/gpui_ios/tests/consumer/Cargo.toml` | No existing lock; no existing EGL resolution receipt |
| `tests/fixtures/arena-miri/Cargo.toml` | Existing lock contains only the independent arena/log closure; no EGL |

`tests/fixtures/hot-reload-consumer/panel/Cargo.toml` is a workspace member, not another root.
The new EGL fixture has no manifest and therefore creates no ninth root.

The integration selects the patch in the four EGL-bearing roots and the new iOS platform
consumer. The three Core-only roots have no EGL edge and receive no unused patch. The
inspection table above preserves the original finding. The legacy ignored hello-Web lock
has subsequently been regenerated from its old 0.1/WGPU-29 graph; it is not a tracked
dependency artifact. Root, independent consumer and tracked smoke-Web locks preserve their
pre-existing selected package versions while adding the required new platform owners.

After declaring the exact root pin, use `cargo update -p khronos-egl --manifest-path PATH`
online to replace the registry source and checksum with the Git source below. An offline
refresh can choose older locally available registry packages; inspect and avoid unrelated
resolver changes. Review the lock diff:

```text
git+https://github.com/zed-industries/khronos-egl?rev=42214b36347b108fd9630ca6254b96dde59e04fe#42214b36347b108fd9630ca6254b96dde59e04fe
```

Then serialize locked Linux/FreeBSD GLES production and independent-consumer checks, inspect
`cargo tree --locked --target x86_64-unknown-linux-gnu -i khronos-egl` for each affected root,
and qualify Windows ANGLE separately if claiming that route. The source and headless binding
receipts above do not establish native compositor, ANGLE, GPU presentation or full framework
qualification. This fixture does not update the canonical reconciliation ledger.
