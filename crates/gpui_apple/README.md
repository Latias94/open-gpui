# Open GPUI Apple

Shared Apple framework services extracted from the Apache-2.0 Zed GPUI implementation:
Metal rendering and atlas resources, task dispatch, CoreText/OpenType, Keychain, process
information and Metal display-link support. `open-gpui-macos` retains AppKit application,
window and input ownership.

The crate exposes the official shared macOS/iOS substrate. It does not provide an iOS
application backend. Enable `font-kit` for `AppleTextSystem`, `runtime_shaders` for runtime
Metal shader compilation, and `test-support` or `bench-support` for their existing renderer
consumers. Native services and rendering require an Apple target and their documented native
thread/resource ownership.

See the workspace [upstream sync plan](../../docs/plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#shared-apple-framework-successor)
and [verification guide](../../docs/verification.md) for implementation provenance and actual
platform qualification.
