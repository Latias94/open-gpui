# Editor framework evolution evidence

The November 13 implementation follows the [framework evolution plan](../../../plans/2026-11-13-1145-refactor-editor-framework-evolution-plan.md).
The user subsequently authorized breaking changes and replacement of qualified implementations.
This authorizes implementation and local commits; it does not waive submission, shutdown or native qualification requirements.

## Source and compiler boundary

| Input | Revision | Scope |
| --- | --- | --- |
| Open-GPUI implementation base | `22399cb09e6399f6d247cc60bc665b45ab92d5df` | Plan commit retained on `refactor/editor-framework-evolution`. |
| Official GPUI adoption endpoint | `27bbbc1cc480a7500cec122f120e796d1fb8500c` | Existing reconciled baseline; no new core port is implied by adapter work. |
| dockspace | `46da973bcc8db7cb50c72aef0f65b6acd6b59338` | Immutable Git dependency, defaults disabled, `serde` enabled. Apache-2.0 core. |

The production adapter consumes only `dockspace`, not its egui/winit adapters or its standalone GPUI harness.
The source harness's existing uncommitted changes are excluded from this dependency and remain untouched.
The adapter keeps one complete `DockspaceSession` as its logical authority; it does not mirror that session into the incumbent graph/controller.

An independently locked temporary consumer compiled the core and `serde` feature on macOS ARM64 with Rust 1.93.0 using `--ignore-rust-version`.
The same lockfile compiled with Rust 1.95.0 without that override.
The override is diagnostic evidence, not a supported consumption command: the selected immutable package declares Rust 1.95.
Accordingly `open-gpui-docking` explicitly declares Rust 1.95; the workspace default and other framework packages remain at 1.93.
Lowering the shared crate's published compiler floor requires a later upstream manifest change and qualification of its own package/test closure.

Commands used for this dependency probe:

```text
CARGO_BUILD_JOBS=2 cargo +1.93.0 check --manifest-path <temporary-consumer>/Cargo.toml --ignore-rust-version --all-features
CARGO_BUILD_JOBS=2 cargo +1.95.0 check --locked --manifest-path <temporary-consumer>/Cargo.toml --all-features
```

Both completed successfully. The temporary consumer used the clean core source at the recorded dockspace revision.
The repository lockfile independently records the immutable Git source and has one GPUI source identity.
Compiler success does not qualify native windows, input, renderer submission or Nara's Windows consumer.

## Ownership obligations

ADRs 0030 and 0031 continue to govern physical ownership: accepted candidates are distinct from submitted outputs, native close intent is distinct from destruction, and retirement requires the original renderer/capture/native facts.
One physical Window must aggregate all its logical surface outputs behind one original exact frame ticket.
A deferred Window must not retain the session's affine update lease and starve another Window.
Each View occurrence uses a separate GPUI entity; sharing a document does not authorize mounting the same entity twice.

The migration explicitly selects weighted N-ary layout semantics. Binary structural identities, absolute `SizeRef` behavior, rounding and central remainder rules cannot be silently presented as equivalent.
The migration receipt preserves their durable provenance and records differences for the host. It must never recreate an authoritative legacy graph.
