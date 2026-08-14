---
type: Verification Evidence
title: Native viewport placement authority and Dear ImGui parity review
status: pending
timestamp: 2026-08-14
git_branch: refactor/ui-framework-authority-convergence
git_commit: 4a350631dfdb62feaa5367f3fe3df2cbde68682f
verified_by:
  - cargo nextest run --locked -p open-gpui -p open-gpui-windows -p open-gpui-docking -p open-gpui-docking-native --test-threads 1 --no-fail-fast
  - target/debug/xtask.exe verify-release-docs
  - target/debug/xtask.exe scan-public-api --check
  - cargo fmt --all -- --check
  - git diff --check
---

# Scope

This evidence binds the U29 native viewport placement slice to commit
`4a350631dfdb62feaa5367f3fe3df2cbde68682f`. It does not rewrite or supersede the
2026-08-13 native docking evidence. The implementation converges four previously separable
authorities into one generation-bound placement protocol:

- the physical client bounds requested by Dock;
- the complete target-display observation, including topology generation, full/work bounds, and
  scale;
- the coherent native geometry and platform-facts sample read back after placement;
- the provisional session purpose: live route movement or the final MouseUp release.

The same change also keeps terminal application shutdown, native event ingress, provisional window
opening, HWND retirement, and process-worker convergence on explicit retained authority rather than
elapsed-time success.

# Dear ImGui / Winit comparison

The local `repo-ref/dear-imgui-rs` review used these recent upstream changes:

- `fd6ba908` — preserve secondary viewport client geometry across undecorated Windows windows,
  decoration changes, physical-coordinate DPI boundaries, and delayed X11 frame extents;
- `194cfee2` — explicitly wake event-driven viewport geometry reconciliation;
- `ef28268c` — publish complete detached-monitor transactions.

The applicable lessons are contract-level, not architectural copying. Open GPUI keeps its retained
window/session authority and does not adopt Winit callback tables or Dear ImGui's immediate-mode
viewport owner. The Windows implementation now provides the stronger guarantees below:

- client bounds are authoritative; outer bounds are a platform-derived implementation detail;
- target-monitor DPI is used for the initial outer-frame calculation;
- `SetWindowPos` is followed by exact `GetClientRect`/`ClientToScreen` readback and one deterministic
  correction when native decoration or `WM_DPICHANGED` changes the client result;
- a hidden provisional window reaches exact physical placement first, submits a later non-empty
  frame for that placement generation, and is then revealed without another move or resize;
- one coherent native sample produces both the provisional final-placement facts and the generic
  `WindowPlatformFacts`, preventing promotion from joining two different native moments;
- a matching client rectangle from a different display publication is not classified as `Exact`;
- live route movement and final release share the placement domain but carry different typed
  purposes, so an intermediate route placement cannot satisfy promotion.

# Deterministic results

The combined nextest run used one test thread and completed with run id
`5b4d5bf3-11fd-4b91-868c-3883b6a61827`:

- 2,264 tests passed;
- 10 tests were skipped by configuration;
- no test failed.

The skipped set includes environment-owned native-interactive scenarios; no skip is counted as
native product success. Release-document verification and public-API tier scanning passed. `cargo
fmt --all -- --check` and `git diff --check` passed; Git emitted only this Windows checkout's
expected LF-to-CRLF conversion warnings.

# Pending native evidence

This record remains `pending`. The local interactive desktop could not provide a fail-closed open
desktop point because ambient top-level windows cover the available work area, and it cannot provide
the required controlled mixed-DPI topology. A trusted ephemeral Windows runner must still certify
the exact commit with:

- a real captured drag that keeps the same provisional HWND moving through A -> B -> C while the
  button remains held;
- MouseUp placement locked at A while an immediately newer physical move to B cannot redirect the
  commit;
- negative-origin and low-to-high/high-to-low DPI movement with exact client-bounds readback;
- hidden physical placement, post-placement submitted frame, and reveal-only native show order;
- exact HWND lifecycle and resource census before worker process exit;
- no-input pass-through, opaque-barrier, capture-loss, destination-close, and application-shutdown
  convergence on the manifest-owned native matrix.

# Residual U32 work

The review confirmed one non-blocking performance risk that must not be solved by a second shallow
cache. Native mouse movement currently performs repeated source/target display construction so it
can prove a stable sample. U32/KTD38 must replace those independent queries with one complete,
immutable display publication whose topology generation, identities, geometry, DPI, and primary
selection are committed atomically.

U32 must also complete the platform matrix:

- stable detached-display identity rather than raw native-handle identity;
- no partial display enumeration publication;
- one topology publication generation rather than per-window message fan-out counts;
- X11 asynchronous frame-extents reconciliation and explicit wake-up;
- macOS owning-platform client-geometry evidence;
- display removal/reuse and topology-change fail-closed tests.

# References

- Plan: `docs/plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md`
- Previous evidence:
  `docs/knowledge/engineering/verification/2026-08-13-native-docking-authority-evidence.md`
- Windows placement implementation: `crates/gpui_windows/src/window.rs`
- GPUI placement contract: `crates/gpui/src/platform.rs`
- Dock live-undock runtime: `crates/gpui_docking/src/surface/live_undock_runtime.rs`
- Native scenario driver: `examples/docking-native/src/native_interactive_tests.rs`

