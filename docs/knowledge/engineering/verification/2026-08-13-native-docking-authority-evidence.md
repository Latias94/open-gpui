---
type: Verification Evidence
title: Native docking authority convergence and Windows runner gate
status: pending
timestamp: 2026-08-13
git_branch: refactor/ui-framework-authority-convergence
git_commit: a36e5f90d919aa165510a42730d52ed23d624455
verified_by:
  - cargo nextest run --locked -p open-gpui --no-fail-fast --test-threads 1
  - cargo nextest run --locked -p open-gpui-windows --no-fail-fast --test-threads 1
  - cargo nextest run --locked -p open-gpui-docking --no-fail-fast --test-threads 1
  - cargo nextest run --locked -p open-gpui-docking-native --no-fail-fast --test-threads 1
  - cargo run --locked -p xtask -- scan-theme-drift
  - cargo run --locked -p xtask -- scan-theme-schema
  - cargo run --locked -p xtask -- scan-ui-contract
  - cargo run --locked -p xtask -- scan-public-api
  - cargo run --locked -p xtask -- scan-import-boundary
  - cargo run --locked -p xtask -- scan-doc-links
  - cargo run --locked -p xtask -- verify-release-docs
  - cargo fmt --all -- --check
  - git diff --check
---

# Scope

This evidence fixes the deterministic verification boundary for commit
`a36e5f90d919aa165510a42730d52ed23d624455`. The commit converges the U27-U29
native docking slice around exact authority rather than elapsed-time or rectangle heuristics:

- point-scoped native hit stacks preserve provisional and no-input pass-through generations;
- captured drag, provisional placement, destination semantics, and committed-loss recovery use
  exact identities, generations, and receipts;
- DockGraph canonicalization retains only transaction-authorized staging roots and performs one
  post-mutation sweep;
- Windows returning test workers settle application, ingress, platform-message-window, and process
  lifetime authority before reporting success;
- native scenarios are selected from one typed manifest and run through an exact-SHA workflow;
- mixed-DPI verification compares the immutable release placement intent with the resulting HWND
  client bounds instead of recording the observed bounds as their own oracle.

This record supersedes no historical evidence. In particular, the 2026-08-09 native docking
evidence remains an immutable record for its earlier commit and smaller scenario slice.

# Deterministic results

- `open-gpui`: 765 passed, 0 skipped; nextest run id
  `af1308ca-937d-4071-bc07-27b235c0ff3d`.
- `open-gpui-windows`: 107 passed, 1 skipped; nextest run id
  `a9f61e5f-e823-462d-bae2-5fc365c9f542`.
- `open-gpui-docking`: 1,344 passed, 0 skipped; nextest run id
  `c672b3b6-6bf8-40c4-896b-98672820bdd0`.
- `open-gpui-docking-native`: 27 passed, 9 skipped; nextest run id
  `9229527b-5e46-46a0-b9dd-4dfa85580434`.
- The returning Windows finalization probes passed 3/3; nextest run id
  `0e004065-47f9-4347-9550-413a555de614`.
- The destination-semantics generation-bound watchdog probes passed 2/2; nextest run id
  `f0c46b6c-f06a-434c-a31c-559a1759687a`.
- Theme drift, theme schema, federated UI contract, public API tier, import boundary, public doc
  link, and release-document scanners passed.
- `cargo fmt --all -- --check` passed.
- `git diff --check` passed. Git reported only the repository's expected LF-to-CRLF conversion
  warnings on this Windows checkout; it reported no whitespace error.

# Native runner contract

The real OS gate is `.github/workflows/native-windows-interactive.yml`. It runs only on trusted
`merge_group`, trusted `main`/`master` pushes, or an explicit full-SHA `workflow_dispatch`. It does
not execute untrusted pull-request code on the self-hosted interactive machine. The runner must
attest that it is ephemeral, one-job-only, deny-by-default for network access, attached to the
expected account SID, and running in a non-service interactive Windows session.

The workflow invokes `cargo run --locked -p xtask -- native-windows-interactive`, which runs the
Windows input/capture sentinel and every scenario in
`examples/docking-native/tests/native_windows_interactive.native-scenarios.toml`. The manifest
currently owns eight Dock scenarios covering source capture, opaque occlusion, surface shutdown,
same-HWND promotion, committed destination loss, process convergence, no-input pass-through, and
mixed-DPI placement.

# Pending evidence

The local machine cannot certify the remaining native matrix: ambient application windows cover
the desktop work area, and the active session does not provide the required distinct mixed-DPI
display topology. The following evidence therefore remains pending for the exact implementation
commit:

- the Windows native input/capture sentinel;
- all eight manifest-owned ignored Dock scenarios;
- the post-scenario process, HWND, capture, and native-resource census on the ephemeral runner;
- exact mixed-DPI client-bounds convergence on two physical DPI domains;
- the trusted workflow run URL and run identifier.

No local skip is counted as product success. This document may be changed to `status: complete`
only after the dedicated workflow checks out the exact commit and all native scenarios pass.

# References

- Plan: `docs/plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md`
- Workflow: `.github/workflows/native-windows-interactive.yml`
- Scenario manifest:
  `examples/docking-native/tests/native_windows_interactive.native-scenarios.toml`
- Native process harness:
  `examples/docking-native/src/native_interactive_tests/process_harness.rs`
- Previous evidence:
  `docs/knowledge/engineering/verification/2026-08-09-native-docking-multiviewport-evidence.md`
