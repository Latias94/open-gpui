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

The feature retains the incumbent deterministic scenarios and benchmarks. The
Windows physical-input manifest and browser smoke use the default shared session.
The native dogfood example and the optional physical latency comparator require
the example package's `legacy-docking` feature. Legacy CPU benchmarks also require
`test-support`.

```sh
cargo nextest run -p open-gpui-docking --all-features --lib --test-threads=1
cargo nextest run -p open-gpui-docking-native --features legacy-docking --bin open-gpui-docking-native --test-threads=1
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

The [shared-session migration record](../../docs/knowledge/engineering/verification/dockspace-shutdown-migration-20261115.md)
records the ten Windows behavior commits and the exact fixed dependency. The
manifest runs all ten journeys on the shared managed session through
`--test native_windows_interactive`. The dogfood and the explicit compatibility
tests still consume legacy APIs.
