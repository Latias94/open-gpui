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
The `open-gpui-ui-components` public contract tests should also keep
`public_resolved_state_contracts_avoid_gpui_runtime_types` passing. That test is the hard
headless-readiness guard for public resolved-state structs: it prevents `Window`, `App`,
`Context`, `RenderOnce`, `IntoElement`, `ElementId`, `Entity`, focus handles, scroll handles, and
callback storage from entering state contracts. The companion extraction-blocker inventory tests in
`open-gpui-ui-components` and `open-gpui-ui-core` pin the remaining `GpuiOverlayState`, direct
focus/a11y re-export, and adaptive `Pixels` usage so later extraction-prep work can shrink that
allowlist deliberately. Public component metrics now use neutral `UiPx`; adding public GPUI
`Pixels`, `Bounds`, `Point`, or `Size` aliases to resolved-state contracts should fail the guard
inventory. `UiPx` still carries GPUI style conversion impls in UI core as a transitional adapter
convenience until the strict crate boundary is split.

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
   underlay blocking, and GPUI adapter fields such as deferred priority and snap margin. In the
   Tooltip samples, hover `Hover or focus`, tab to `Focus only`, and confirm each reveals
   descriptive tooltip content while `Disabled` remains unfocusable and closed; `Manual delayed`
   should stay visible and report its custom delay policy. In the HoverCard samples, confirm
   `Profile preview` starts visible and can stay open while pointer or keyboard focus is on the
   trigger/content, `Focus preview` opens only from keyboard focus, and `Manual card` opens and
   closes from its gallery control with pass-through or consume outside-press metadata shown in the
   state row. In the Popover samples, confirm `Default open` starts visible, `Controlled` opens
   and closes from its gallery control, Escape closes the controlled popover, outside press closes
   the visible popovers, and the
   `Consume outside` sample reports a consuming outside-press policy while `Disabled` remains
   closed. In the Dialog samples, open and close `Controlled modal`, confirm Escape and the modal
   barrier can close it without activating underlay controls, confirm `Default open` reports a
   blocking modal layer, confirm `Outside ignored` remains open on outside press, and confirm
   `Disabled` stays closed. In the AlertDialog samples, open `Delete project`, confirm the
   destructive action is explicit, cancel receives the default focus, outside press is consumed
   without dismissing, Escape closes it, and focus returns to the trigger; confirm the safe cancel
   sample starts open and keeps the underlay blocked until an explicit action closes it. In the
   Sheet samples, confirm the left modal sheet blocks underlay input and closes by Escape/outside
   press/close control, the right non-modal sheet reports pass-through outside behavior without a
   blocking modal barrier, and the
   bottom sticky sheet is attached to the bottom edge, hides the close affordance, and ignores
   outside press. In the Menu samples, confirm arrow keys move roving focus over enabled
   action items while skipping separators and disabled items, Enter/Space activates the focused
   action and closes the menu, Escape closes the controlled menu, and `Outside ignored` keeps its
   explicit outside policy. In the ContextMenu samples, right-click the hotspot and confirm the
   menu opens from the pointer point, snaps inside the window near edges, and closes on outside
   press or Escape.
6. Open `Components`, or start there directly with
   `cargo run -p open-gpui-ui-foundation-gallery -- --page components`, and confirm Button, Badge,
   IconButton, ScrollArea, Splitter, Switch, Checkbox, RadioGroup, Toggle, Label, TextInput, Field,
   Tabs, Toolbar, Sidebar, Listbox, Select, Combobox, and Command samples render with enabled,
   disabled, selected, checked, unchecked, indeterminate,
   pressed, invalid, required, read-only, placeholder, value, help, error, control-association,
   roving-focus, popup, overflow-axis, scroll-reset, and resize-constraint states. The Badge samples should
   remain display-only. The IconButton samples should be square controls with visible focus and
   explicit accessible labels. The ScrollArea samples should cover vertical overflow, horizontal overflow,
   and two-axis overflow; wheel or trackpad scrolling should stay inside each constrained viewport
   while the state readout reports the expected axis and reset policy. Scroll each constrained
   ScrollArea once, then continue scrolling the same viewport after the content has moved; it should
   keep moving instead of snapping back to the origin after the redraw caused by the first scroll.
   The Splitter samples should
   show horizontal and vertical panel groups, stable handle affordances, min/max fraction readouts,
   collapsed-panel metadata, and pointer-drag resizing without changing surrounding layout. Drag the
   vertical collapsed sample far enough to restore the collapsed panel, then confirm subsequent
   dragging resizes it normally. The RadioGroup samples should
   cover vertical required selection and horizontal navigation that skips disabled items. The Toggle
   samples should expose button-like pressed state without behaving like a checkbox. The Tabs
   samples should cover horizontal automatic activation and vertical manual activation; use arrow
   keys, Home/End, Enter, and Space to confirm focus movement and activation behavior. The vertical
   sample should keep its tab rail scrollable inside the constrained gallery card. The Toolbar
   samples should expose horizontal and vertical command groups; use arrow keys plus Home/End to
   confirm roving focus skips disabled items and separators, and use Enter/Space to activate
   action/toggle items. The Sidebar samples should expose expanded, icon-collapsed, and long
   scrollable navigation; icon collapse should hide visible labels while keeping item labels
   explicit, disabled items should be skipped, and the long sidebar should scroll inside its sample
   frame without making the full Components page unscrollable. The Listbox samples should expose
   grouped options, disabled option skipping, selected and active descendant metadata, empty-state
   behavior, and keyboard navigation/activation with Up/Down/Home/End plus Enter/Space. The Select
   samples should expose closed, controlled-open, and disabled states; confirm the trigger label
   reflects the selected option, the open sample uses a non-modal dismissible listbox popup with a
   scrollable long option set, Escape/outside press dismisses it, and disabled empty select remains
   closed. The Combobox samples should expose editable filtering, selected value metadata that does
   not disappear when the query hides the selected option, an empty filtered state, and disabled
   input/popup suppression. The Command samples should expose grouped command items, shortcut
   labels, loading and empty states, inline and dialog-backed presentation, and modal dialog
   outside/Escape dismissal while preserving the Components page scrollability. The default TextInput
   sample should accept real text editing through the
   controller-backed path, while the gallery remains scrollable and keeps focus visible when the
   page overflows. The app should stay open after opening `Components`; an `accesskit_consumer`
   panic during that navigation is a
   regression in the accessibility repair gate. The Components page also serves as a conformance
   surface: confirm the visible gate cards for explicit crate exports, gallery metadata, ScrollArea
   redraw persistence, Splitter runtime constraints, Tabs overflow, and explicit accessible
   metadata on icon-only and label-association samples.
7. Re-run `cargo nextest run -p open-gpui-ui-components` and `cargo nextest run -p
   open-gpui-ui-foundation-gallery` if a manual check exposes a component or gallery regression.

For headless-readiness checkpoint work, additionally review `docs/adr/0006-open-gpui-ui-headless-
extraction-checkpoint.md` after the automated component tests pass. The checkpoint should continue
to identify which behavior is neutral, which behavior remains GPUI adapter-owned, and why a
standalone `open-gpui-ui-headless` crate is or is not ready.

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
