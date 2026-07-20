# Welcome to Open GPUI!

Open GPUI is a hybrid immediate and retained mode, GPU accelerated UI framework
for Rust, forked from Zed's GPUI framework code.

## Getting Started

Open GPUI is still in active development and is pre-1.0. There will often be breaking changes between versions. The current workspace MSRV is Rust 1.92. Add the following to your `Cargo.toml`:

```toml
open_gpui = { package = "open-gpui", version = "0.2.0" }
open_gpui_platform = { package = "open-gpui-platform", version = "0.2.0" }
```

- [Ownership and data flow](src/_ownership_and_data_flow.rs)
- [Accessibility](src/_accessibility.rs)

Everything in Open GPUI starts with an `Application`. You can create one with `open_gpui_platform::application()`, and kick off your application by passing a callback to `Application::run()`. Inside this callback, you can create a new window with `App::open_window()`, and register your first root view.

## What This Crate Owns

`open-gpui` owns the application, window, element, entity, input, text, accessibility, and rendering-facing API that applications use directly. It does not choose a backend by itself; use `open-gpui-platform` for target-specific application construction.

Native backends cover macOS, Windows, Linux, and FreeBSD through platform crates. WebAssembly uses `open-gpui-web` through `open-gpui-platform` on `wasm32-unknown-unknown`. Backend capabilities such as WebGPU, screen capture, platform viewport windows, and compositor features remain runtime facts.

For higher-level controls, use `open-gpui-ui-components`. For deterministic UI motion primitives, use `open-gpui-motion`. For retained dock spaces, use `open-gpui-docking`.

## Interactive Subtree Geometry

Use `SubtreeTransform` when axis-aligned scale or translation must apply consistently to an
interactive subtree. The transform runs after layout, so measurement, flex/grid placement, scroll
extent, and sibling flow do not change:

```rust
use open_gpui::{
    ParentElement as _, SubtreeTransform, SubtreeTransformExt as _, SubtreeTransformOrigin, div,
    point, px, size,
};

let transform = SubtreeTransform::try_new(
    size(1.2, 0.9),
    point(px(8.0), px(-4.0)),
    SubtreeTransformOrigin::CENTER,
)
.expect("finite positive transform");

let content = div().child("Interactive content").with_subtree_transform(transform);
```

The supported public contract is finite positive normal axis scale, finite logical-pixel translation, and
a post-layout origin. Rotation, skew, arbitrary affine matrices, and 3D are intentionally absent.
Use `TargetedEvent` helpers for target-local input coordinates and `ElementGeometry` or
`measured_element` for committed layout/displayed geometry. Raw platform event positions remain in
window coordinates.

See [ADR 0021](../../docs/adr/0021-open-gpui-interactive-subtree-transform-authority.md) for the
cross-channel and numeric-failure contract.

## Layout-Preserving Subtree Presentation

Use `SubtreePresentation` when a whole subtree must retain its layout slot while changing its
participation in rendering and interaction:

```rust
use open_gpui::{
    ParentElement as _, SubtreePresentation, SubtreePresentationExt as _, div,
};

let inert_content = div()
    .child("Painted, but excluded from input, focus, IME, and accessibility")
    .with_subtree_presentation(SubtreePresentation::Inert);
```

`Visible` participates in every channel. `Inert` keeps layout and paint but suppresses input,
focus, IME, tooltips, overlay intent, and accessibility. `Hidden` keeps layout only. A suppressive
ancestor always wins, including through transforms, deferred elements, and cached views. Returning
to `Visible` rebuilds current participation without replaying stale input or focus claims.

Use `Display::None` when the subtree must leave layout. Use component disabled state when a control
should remain discoverable with disabled semantics. See
[ADR 0022](../../docs/adr/0022-open-gpui-subtree-presentation-authority.md) for the full matrix and
committed-frame cleanup contract.

### Focus Observation

`on_focus_in` observes effective focus entering a handle or descendant while the platform window
is active. `on_focus_committed` observes one handle becoming the exact committed local focus;
`on_focus_committed_in` observes committed focus entering a handle or descendant. Both committed
observers work while the platform window is inactive, and later platform activation does not
replay them. `Window::focused` reads current intent and may be provisional during a candidate
render; `Window::committed_focus` reads the exact leaf from the last committed window-local tree.

Use `focus_with_completion` or `blur_with_completion` for a retained transaction that must settle
one exact or empty focus-authority request. The callback receives `Committed`, `Rejected`, or
`Superseded` after final rendered membership is known. A request issued after the current frame
seals input and accessibility authority is qualified in one later platform frame, so late
presentation failure cannot produce stale success bookkeeping and cached commit replay cannot
recursively redraw inside one effect cycle. `Window::blur` and `Window::disable_focus` take `cx`
so superseded completion callbacks are always scheduled.

### Dependencies

Open GPUI has various system dependencies that it needs in order to work.

#### macOS

On macOS, Open GPUI uses Metal for rendering. In order to use Metal, you need to do the following:

- Install [Xcode](https://apps.apple.com/us/app/xcode/id497799835?mt=12) from the macOS App Store, or from the [Apple Developer](https://developer.apple.com/download/all/) website. Note this requires a developer account.

> Ensure you launch Xcode after installing, and install the macOS components, which is the default option.

- Install [Xcode command line tools](https://developer.apple.com/xcode/resources/)

  ```sh
  xcode-select --install
  ```

- Ensure that the Xcode command line tools are using your newly installed copy of Xcode:

  ```sh
  sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
  ```

## The Big Picture

Open GPUI offers three different [registers](<https://en.wikipedia.org/wiki/Register_(sociolinguistics)>) depending on your needs:

- State management and communication with `Entity`'s. Whenever you need to store application state that communicates between different parts of your application, you'll want to use Open GPUI's entities. Entities are owned by Open GPUI and are only accessible through an owned smart pointer similar to an `Rc`. See the `app::context` module for more information.

- High level, declarative UI with views. All UI in Open GPUI starts with a view. A view is simply an `Entity` that can be rendered, by implementing the `Render` trait. At the start of each frame, Open GPUI will call this render method on the root view of a given window. Views build a tree of `elements`, lay them out and style them with a tailwind-style API, and then give them to Open GPUI to turn into pixels. See the `div` element for an all purpose swiss-army knife of rendering.

- Low level, imperative UI with Elements. Elements are the building blocks of UI in Open GPUI, and they provide a nice wrapper around an imperative API that provides as much flexibility and control as you need. Elements have total control over how they and their child elements are rendered and can be used for making efficient views into large lists, implement custom layouting for a code editor, and anything else you can think of. See the `element` module for more information.

Each of these registers has one or more corresponding contexts that can be accessed from all Open GPUI services. This context is your main interface to Open GPUI, and is used extensively throughout the framework.

## Other Resources

In addition to the systems above, Open GPUI provides a range of smaller services that are useful for building complex applications:

- Actions are user-defined structs that are used for converting keystrokes into logical operations in your UI. Use this for implementing keyboard shortcuts, such as cmd-q. See the `action` module for more information.

- Platform services, such as `quit the app` or `open a URL` are available as methods on the `app::App`.

- An async executor that is integrated with the platform's event loop. See the `executor` module for more information.,

- The `[open_gpui::test]` macro provides a convenient way to write tests for your Open GPUI applications. Tests also have their own kind of context, a `TestAppContext` which provides ways of simulating common platform input. See `app::test_context` and `test` modules for more details.

Currently, the best way to learn about these APIs is to read the Open GPUI examples and the framework source. This repository is a fork of Zed GPUI, but it is maintained as an independent Open GPUI workspace.

## Verification

For focused changes in this crate, run:

```sh
cargo check -p open-gpui --tests --locked
cargo run -p xtask -- verify-release-docs
```

For end-to-end workspace confidence, use the root `cargo run -p xtask -- verify` gate.

## License and Attribution

Open GPUI is licensed under Apache-2.0 and is forked from Zed's Apache-2.0 GPUI framework code. See this package's `LICENSE-APACHE` and `NOTICE` files for fork attribution, and the repository root `README.md` for full dependency attribution and release license guidance.
