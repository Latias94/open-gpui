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

For focused `open-gpui-canvas` work, run:

```sh
cargo fmt -p open-gpui-canvas
cargo check -p open-gpui-canvas --benches
cargo nextest run -p open-gpui-canvas
cargo check -p open-gpui-smoke-native
```

The canvas crate also has a large-canvas Criterion baseline:

```sh
cargo bench -p open-gpui-canvas --bench large_canvas
```

Use the benchmark to compare spatial-index, visible-query, and paint-frame culling changes. It is
not part of the default CI gate because benchmark timing is runner-dependent.

For focused `open-gpui-ui-core`, `open-gpui-ui-components`, or UI foundation gallery work, run:

```sh
cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-ui-core
cargo check -p open-gpui-ui-components
cargo check -p open-gpui-ui-foundation-gallery
cargo nextest run -p open-gpui-ui-core
cargo nextest run -p open-gpui-ui-components
cargo nextest run -p open-gpui-ui-foundation-gallery
```

The `open-gpui-ui-core` overlay tests are the renderer-neutral gate for shared overlay behavior.
They should cover layer kind, presence, outside-press policy, Escape policy, focus restore intent,
initial focus intent, and placement input without opening a GPUI window.
The `open-gpui-ui-components` overlay helper tests should cover the GPUI adapter mapping for
deferred priority, snap margin, anchor conversion, outside-press open-change, and Escape
open-change without introducing a global overlay runtime.

When changing GPUI accessibility repair or component metadata that creates explicit cross-node
relationships, also run:

```sh
cargo check -p open-gpui
cargo nextest run -p open-gpui --lib window::a11y::tests::repair_tree_update
```

Manual UI foundation dogfood should use the dedicated gallery after the automated checks pass:

```sh
cargo run -p open-gpui-ui-foundation-gallery
cargo run -p open-gpui-ui-foundation-gallery -- --page components
```

1. Open `Tokens` and confirm the semantic token registry shows surface, text, accent, focus ring,
   destructive, overlay, and modal overlay keys without introducing a styled component layer.
2. Open `Sizing & Density`, switch between compact and desktop from the summary panel, and confirm
   the highlighted density and default size change with the foundation policies.
3. Open `Adaptive`, use the same compact/desktop switch, and confirm device samples show mobile /
   desktop shell mode, compact / regular / expanded class, and panel samples show compact / medium /
   wide classes.
4. Open `Focus & A11y`, tab through the focusable controls, confirm the focus-visible outline is
   visible, click the counter and reset controls, and toggle the switch. The visible counter and
   switch state should match the accessible role/state vocabulary shown by the page.
5. Open `Overlay`, click `open overlay`, confirm the anchored popover appears from the trigger, then
   close it from the popover or press Escape. The geometry readout should keep anchor, layout,
   visual, preferred, and safe-window rectangles visible. The behavior contract matrix should show
   distinct tooltip, popover, dialog, and menu policies for presence, outside press, Escape, focus,
   underlay blocking, and GPUI adapter fields such as deferred priority and snap margin.
6. Open `Components`, or start there directly with
   `cargo run -p open-gpui-ui-foundation-gallery -- --page components`, and confirm Button, Badge,
   IconButton, Switch, Checkbox, RadioGroup, Toggle, Label, TextInput, Field, and Tabs samples
   render with enabled, disabled, selected, checked, unchecked, indeterminate, pressed, invalid,
   required, read-only, placeholder, value, help, error, control-association, and roving-focus
   states. The Badge samples should remain display-only. The IconButton samples should be square
   controls with visible focus and explicit accessible labels. The RadioGroup samples should cover
   vertical required selection and horizontal navigation that skips disabled items. The Toggle
   samples should expose button-like pressed state without behaving like a checkbox. The Tabs
   samples should cover horizontal automatic activation and vertical manual activation; use arrow
   keys, Home/End, Enter, and Space to confirm focus movement and activation behavior. The vertical
   sample should keep its tab rail scrollable inside the constrained gallery card. The default
   TextInput sample should accept real text editing through the controller-backed path, while the
   gallery remains scrollable and keeps focus visible when the page overflows. The app should stay
   open after opening `Components`; an `accesskit_consumer` panic during that navigation is a
   regression in the accessibility repair gate.
7. Re-run `cargo nextest run -p open-gpui-ui-components` and `cargo nextest run -p
   open-gpui-ui-foundation-gallery` if a manual check exposes a component or gallery regression.

CI runs a three-platform matrix for pushes to `master` / `main`, pull requests, and manual workflow
dispatches:

- Windows runs the same local gate, `cargo nextest run -p xtask`,
  `cargo nextest run -p open-gpui-docking-native --no-fail-fast`, and
  `cargo check -p open-gpui-windows --all-features --locked`.
- Linux runs `cargo check -p open-gpui-linux --all-features --locked` after installing the system
  headers needed for Wayland, X11, fontconfig, freetype, and pkg-config.
- macOS runs `cargo check -p open-gpui-macos --features font-kit --locked`.
- All three platforms run `cargo check -p open-gpui-wgpu --features font-kit --locked`.

Run the native renderer smoke explicitly with:

```sh
cargo run -p xtask -- renderer-smoke
```

That command runs the focused `open-gpui-wgpu` smoke test that requests a real native `wgpu` adapter and
device, creates the renderer bind group layouts, and builds the core render pipelines. It is not
part of the default `verify` gate because it depends on local GPU, driver, and session availability.

Run the docking smoke surface explicitly after changing `open-gpui-docking`:

```sh
cargo nextest run -p open-gpui-docking
cargo nextest run -p open-gpui-docking-native --no-fail-fast
cargo check -p open-gpui-docking-native
cargo run -p open-gpui-docking-native
```

The docking native example exercises the public multi-window setup: applications build one
`DockController`, wrap it in a `DockViewportRuntimeHandle`, register window-close cleanup, and open
controller-backed primary and secondary `DockHost` viewports.

Manual native docking dogfood should use the same example after the automated checks pass:

1. Launch `cargo run -p open-gpui-docking-native` and confirm the app opens `Docking demo`,
   `Docking preview`, and `Empty central dogfood` windows.
2. Drag a primary-class tab from `Docking demo` into another primary-compatible target; the preview
   must appear in the destination window and release must select the moved item there.
3. Drag the `Preview` / `Diff` secondary-class stack from `Docking preview` back into `Docking demo`;
   item order and the active tab must be preserved.
4. Drag `Preview` / `Diff` over `Empty central dogfood`; the route must render as rejected and
   release must not mutate the graph because the central space only accepts central-class panels.
5. Use `Restore central note` from the runtime status panel; the `Central note` panel must reopen in
   the empty central window and recover the central-region identity instead of becoming ordinary
   root-only content.
6. Drag a tab or stack outside every docking window; a new runtime-backed viewport must open before
   the graph moves the source payload.
7. Dock the torn-off viewport content back into an existing window; the destination window must
   activate and the moved item must become the selected tab.
8. Move runtime-opened windows across displays, choose `Save placement`, then use `Reopen closed
   demo viewports`; restored placement should use saved bounds only as placement input while live
   drag routing continues to use current viewport bounds. On macOS, windows on a secondary display
   should keep non-overlapping desktop-space bounds while routing between viewports.
9. Exercise the runtime panel close-policy controls for prevent, retain, and merge-back behavior;
   closing a viewport must match the selected policy without losing descriptor-backed panel restore
   or leaving a stale cross-window route preview in another viewport.
10. Start a cross-window drag, hover a valid target, then move to an area of the same viewport with
   no current dock target before releasing; the previous preview must not commit from stale target
   state.
11. Drag over the empty central dogfood window; empty central-space preview, rejection, and
   passthrough behavior must match the visible policy state.

Current platform caveats for docking multi-viewport dogfood:

- Windows mixed-DPI displays and Wayland global toplevel positions are not yet normalized into one
  explicit GPUI coordinate type. Treat cross-display routing results on those backends as areas for
  follow-up platform API work, not as proof of full ImGui PlatformIO parity.
- No-input, no-focus-on-appearing, alpha, topmost, and no-taskbar viewport flags are not modeled in
  GPUI's platform trait yet.

Before publishing a crate, confirm that the packaged archive carries the expected attribution files:

```sh
cargo package -p open-gpui --list --allow-dirty
```

For the canvas crate specifically, run:

```sh
cargo package -p open-gpui-canvas --list --allow-dirty
cargo publish -p open-gpui-canvas --dry-run --allow-dirty
```

Every published Open GPUI crate should include `README.md`, `LICENSE-APACHE`, and `NOTICE`. Cargo
does not package files outside a crate root through `include`, so each publishable crate root keeps
its own `NOTICE` copy.

The import-boundary scan rejects dependency files that reintroduce Zed's GPL tracing stack
(`ztracing`, `ztracing_macro`, `zlog`), the old `zed-sum-tree` dependency, the Zed monorepo as a
Cargo git dependency, retired Zed Git fork sources that have already been migrated, or the removed
Zed `perf` crate dependency. The retired `zed-scap` package and `zed-industries/scap` Git source
are also rejected now that screen capture resolves through the Open GPUI-owned
`open-gpui-scap` fork. The old crates.io `zed-font-kit` package is retired and should not be
reintroduced; font-kit resolves through the Open GPUI-owned fork configured in the crate manifests.
