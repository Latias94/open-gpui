---
type: Verification Evidence
title: Dockspace Editor Reconstruction and Windows Focus Ordering
status: verified
timestamp: 2026-11-16T22:25:00+08:00
git_commit: 7509ac68a777340884cd3d894e44926f5cd860d2
---

# Dockspace Editor Reconstruction and Windows Focus Ordering

## Scope

This follow-up starts at Open-GPUI
`959bd16dc09a9529c0308dae7e6be5bf42164e4c` with dockspace fixed at
`e8120864e06de445e1b588b07d6b322bb7125e41`. It applies the already qualified
failed-generation termination interface to the application's
[dockspace editor example](../../../../examples/dockspace-editor/README.md).
This repository's example is the application consumer; Nara's editor is outside
the change.
The focus correction is committed at
`31978b3698acd96e162c4d9e25b5a11072e7273c`; the application example, native smoke,
binary stack configuration and CI update are committed at
`7509ac68a777340884cd3d894e44926f5cd860d2`.

## Application ownership

An independent recovery control window retains the workspace's original
`EditorView` entities and shared document. An explicit rebuild copies the last
committed logical layout before native drain can recover Child content into its
anchor. Normal drain and explicitly selected failed-generation termination both
wait for `when_native_terminated` before creating a new managed session/provider.
The callback uses a weak application owner and defers entity reentry because the
native pump may complete synchronously inside the initiating owner update.

The old session keeps its failed native input and other protocol owners. Only
committed topology and application-owned pane data cross into the successor.
Obsolete per-window prepaint observations are cleared after physical termination;
the pane entities, local drafts and shared document remain intact. Closed tabs
stay closed. A failed open retains its original generation and rollback tickets
until termination, after which reconstruction can be retried explicitly.

Closing the recovery control overrides a pending rebuild and retires the managed
group first. A terminal `Closing` state prevents queued UI actions from reopening
the workspace. Process-wide App shutdown also cancels an already queued rebuild.
The independent control keeps the normal application alive under Windows'
`LastWindowClosed` policy while its docking windows retire.

## Defects found by native verification

The first Windows debug launch overflowed the default 1 MiB main-thread stack
before displaying a pane. CDB stopped at `__chkstk` in
`DockEngine::reduce_tick_candidate`, reached through the finite
`prepare_host_frame_owned` / `DockspaceHostFrame::prepare_commit` /
`DockSurface::bind_native_root` call chain. There was no repeating recursion in
that stack. An isolated executable copy changing only PE `SizeOfStackReserve`
from 1 MiB to 8 MiB passed that point. The example now sets the corresponding
Windows linker option in its own `build.rs`. The final native binary reports an
8,388,608-byte reserve and an unchanged 4,096-byte commit. Workspace-wide linker
options and the UI event-loop thread are unchanged.

The existing native smoke could also exit when its final window closed, before
printing its success receipt. Both bounded editor smoke modes now use explicit
process exit and print their final receipt before requesting quit. Exit status
alone does not qualify either smoke.

Once startup passed the stack boundary, native verification exposed a real focus
ordering error. A platform focus hint could name an existing retained window
whose preceding visibility snapshot made known Dock focus unavailable. The
adapter mapped that hint using binding existence alone. Reducing the recorded
prefix then failed with `GlobalFocusBindingUnavailable` / `OperationConflict`,
blocking first content output even though the GPUI windows themselves submitted.

The original FIFO now keeps `FocusedWindow` at its front until earlier native
input commits, when global focus observation is supported. Unsupported focus
hints remain ignored without adding a needless commit boundary. Mapping then calls the core's
`native_binding_can_report_focus`: hidden, minimized, unobserved and cleanup-only
bindings map to Unknown. Visible pending bindings remain eligible under the core
contract. No observation is dropped or reordered. Initial-focus coverage counts
all delivered focus reports after committed inventory; the new regression checks
hidden/minimized/unknown presentation followed by visible restoration and exactly
one committed focus observation for each hint.

The regression failed before the fix in nextest run
`30ab3098-cca4-4234-a6b7-9ab241239453`. All 20 native transport regressions then
passed in `262c69a4-96c5-4d00-94ae-e5539cfbda13`.
The first complete leak-detection run
`aa727167-b089-4d6f-b681-2df2ba323207` exposed the unsupported-focus batching
regression in the original failed-open rollback-custody test (164/165 passed).
The final capability guard preserves that test's original native-terminal
assertion rather than changing its expected result.

## Verification

All Cargo builds and nextest execution were serialized with `CARGO_BUILD_JOBS=1`
and `--test-threads=1`, reusing the workspace `target`. The environment was
Windows 11, x86_64 MSVC, Rust 1.100.0. The five application recovery regressions
passed in `4beb94de-1092-4ca7-8b87-d51e2838bf16`, covering delayed renderer/native
terminal, failed input retention, fresh bindings, pre-drain Child topology,
closed-pane state, isolated sibling survival, partial reconstruction retry,
control-close cancellation and AppShutdown cancellation.

The final complete run enabled `GPUI_LEAK_DETECTION=1` and
`GPUI_TEST_EXPECT_LEAK_DETECTION=on`, matching the hosted quality job:

```text
cargo nextest run --locked -p open-gpui-docking -p open-gpui-dockspace-editor --test-threads=1 --no-fail-fast --no-tests fail
```

It passed **165/165** tests (160 docking, five editor) in
`ceca5851-290d-4f51-9f17-81a4add6a2c3`. The failed-generation recovery test clicks
the rendered control's actual rebuild button; diagnostics follow the controls in
a scrollable window so long errors do not obscure the recovery entry point.
`cargo fmt --all --check`, `git diff --check`, the public API scan and the public
documentation link scan also passed. Independent read-only lifecycle review and
its incremental follow-up returned no blocking findings.

Local native verification first passed at 22:09 and was repeated against the
final production source at 22:22 on November 16, 2026 (+08:00):

| Command argument | Result | Required receipt |
| --- | --- | --- |
| `--smoke-recovery` | Exit 0 | `DOCKSPACE RECOVERY SMOKE OK:` |
| `--smoke-native` | Exit 0 | `DOCKSPACE NATIVE SMOKE OK:` |

The recovery smoke observes actual Submitted frames before and after recovery,
injects duplicate press records through the public native ingress, awaits a
failed physical termination, checks that the old rejected input remains with its
original session, verifies fresh native bindings and identical pane entities,
edits the recovered View and finally requires normal drain. Its timeout diagnostic
contains the original host failure, pane prepaint observations and actual Window
presentation facts.

The native smoke separately covers Child creation, actual Submitted frames,
retained local/shared edits after movement, dependent/anchor native terminals,
binding quiescence and an isolated sibling that remains usable before retiring.
Both commands ran serially against the normal built binary after the production
focus fix. These are focused programmatic native smokes, not receipts for the
complete manifest-owned interactive gate.

Evidence retained in the local temporary directory:

- `%TEMP%/open-gpui-editor-native-stack-20261116.log`
- `%TEMP%/open-gpui-focus-visibility-red-20261116.log`
- `%TEMP%/open-gpui-focus-visibility-green-20261116.log`
- `%TEMP%/open-gpui-editor-recovery-nextest-20261116.log`
- `%TEMP%/open-gpui-editor-recovery-final-default-20261116.log`
- `%TEMP%/open-gpui-editor-recovery-final-default-green-20261116.log`
- `%TEMP%/open-gpui-editor-recovery-final-qualified-20261116.log`
- `%TEMP%/open-gpui-editor-recovery-build-20261116.log`
- `%TEMP%/open-gpui-editor-recovery-final-build-20261116.log`
- `%TEMP%/open-gpui-editor-recovery-native-20261116.log`
- `%TEMP%/open-gpui-editor-native-smoke-20261116.log`
- `%TEMP%/open-gpui-editor-recovery-final-native-20261116.log`
- `%TEMP%/open-gpui-editor-final-native-smoke-20261116.log`

## Qualification limits

The five TestPlatform recovery tests are added to hosted CI. Neither native smoke
is added to GitHub-hosted CI. Local native fault injection qualifies the protocol
failure path with real windows and a renderer; its injected button records do not
claim physical mouse, OS capture, display disconnect/reconnect or IME coverage.
No mixed-DPI check was repeated in the single-display remote session. No macOS,
Linux or Windows GNU runtime was exercised in this follow-up.

Reconstruction is an in-memory application example. It does not implement durable
file recovery, external pane-key persistence, application undo history or saved
native placement. Those remain application responsibilities as described in
[ADR 0034](../../../adr/0034-shared-dockspace-session-integration.md).
