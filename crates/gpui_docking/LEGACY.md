# Legacy Docking Compatibility

Enable `open-gpui-docking/legacy-docking` only for an explicitly incumbent
integration. Import its facade from `open_gpui_docking::legacy::prelude`, model
and commands from `legacy::model`, direct hosts from `legacy::runtime`, and
records from `legacy::advanced`. The crate root always denotes the shared-session
implementation, including when Cargo unifies the compatibility feature.

The old `DockSurface::builder`, lazy factories, revision subscriptions,
stable-item activation completions and typed viewport outcomes retain their old
contracts in this namespace. They do not operate on a shared `DockspaceSession`.
Do not attach both semantic runtimes to one workspace or use the old controller
as recovery for a new surface. See the [migration guide](../../docs/ui/migration-v0.3.md#default-shared-docking-session)
for the new lifecycle and persistence entry points.

The feature retains the existing deterministic scenarios, native Windows input
qualification and browser unsupported-native-window qualification. Old benchmarks
require both `legacy-docking` and `test-support`. The native dogfood example also
requires its package's `legacy-docking` feature:

```sh
cargo nextest run -p open-gpui-docking --all-features --lib --test-threads=1
cargo nextest run -p open-gpui-docking-native --features legacy-docking --test-threads=1
cargo run -p open-gpui-docking-native --features legacy-docking
```

DevTools' displaced runtime projection is `open_gpui_devtools::legacy_docking`
and requires `open-gpui-devtools/legacy-docking`. Its ordinary `docking` feature
inspects the shared-session implementation instead.

Compatibility retention is not new-platform evidence. Deleting this engine
requires the declared replacement scenarios, particularly physical capture,
current native hit stacks, mixed DPI, OS IME and close/retirement on each owning
backend. The [cutover record](../../docs/knowledge/engineering/verification/dockspace-default-cutover-20261114.md)
separates executed replacement checks from pending platform work. Legacy v2/v3
persistence conversion already lives in `persistence` and needs none of this
runtime or its feature.
