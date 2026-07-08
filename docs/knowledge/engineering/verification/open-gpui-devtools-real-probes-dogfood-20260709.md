---
type: Verification Evidence
title: Open GPUI DevTools real probes dogfood verification
status: verified
timestamp: 2026-07-09T00:56:22+08:00
git_branch: feat/devtools-real-probes-dogfood
git_commit: 9885f5d8-plus-u5-final-diff
related_plan:
  - docs/plans/2026-07-08-004-feat-devtools-real-probes-dogfood-plan.md
tags:
  - devtools
  - form
  - resource
  - ui-components
  - motion
  - docking
  - verification
---

# Summary

The real-probe dogfood plan passed focused local verification on Windows after the final U5
redaction hardening and documentation pass. The final diff keeps DevTools read-only, replaces static
gallery snapshots with registry-collected first-party probes, and enforces redaction on probe ids,
custom snapshot-kind labels, diagnostics, payload strings, redaction notes, email-like text,
filesystem paths, and same-token or separated sensitive key/value text.

# Passed Gates

- `cargo fmt -p open-gpui-devtools -p open-gpui-form -p open-gpui-resource -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery -p open-gpui --check`: passed.
- `git diff --check`: passed before final memory write.
- `cargo check -p open-gpui-devtools --tests --locked`: passed.
- `cargo check -p open-gpui-devtools --features form,resource --tests --locked`: passed.
- `cargo check -p open-gpui-devtools --features gpui,motion,docking --tests --locked`: passed.
- `cargo check -p open-gpui-devtools --no-default-features --features form --tests --locked`: passed.
- `cargo check -p open-gpui-devtools --no-default-features --features resource --tests --locked`: passed.
- `cargo check -p open-gpui-devtools --no-default-features --features motion --tests --locked`: passed.
- `cargo check -p open-gpui-devtools --no-default-features --features docking --tests --locked`: passed.
- `cargo check -p open-gpui-devtools --no-default-features --features ui-components --tests --locked`: passed.
- `cargo check -p open-gpui-devtools --no-default-features --features gpui --tests --locked`: passed.
- `cargo check -p open-gpui-ui-components --tests --locked`: passed.
- `cargo check -p open-gpui-ui-foundation-gallery --tests --locked`: passed.
- `cargo check -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools --tests --locked`: passed.
- `cargo nextest run -p open-gpui-devtools --no-fail-fast --locked`: passed 20/20.
- `cargo nextest run -p open-gpui-devtools --features form,resource form_resource_adapters --no-fail-fast --locked`: passed 3/3.
- `cargo nextest run -p open-gpui-devtools --features gpui,motion,docking framework_adapters --no-fail-fast --locked`: passed 5/5.
- `cargo nextest run -p open-gpui-ui-components form resource public_surface --no-fail-fast --locked`: passed 48/48.
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools form resource component_sample_contracts --no-fail-fast --locked`: passed 20/20.
- `cargo test -p open-gpui-devtools --doc --locked`: passed 3/3.
- `cargo run -p xtask -- verify-release-docs`: passed.
- `cargo run -p xtask -- scan-doc-links`: passed.
- `cargo run -p xtask -- scan-import-boundary`: passed.
- `cargo run -p xtask -- scan-theme-drift`: passed.
- `cargo run -p xtask -- scan-theme-schema`: passed.
- `cargo metadata --no-deps --format-version 1`: passed.
- `python <engineering-wiki-memory>/scripts/wiki_memory.py validate --root docs\knowledge\engineering`: passed with existing rollup-size, stale `current-state.md`, and historical absolute-path warnings.

# Review Fixes

- `ProbeId::new` and `ProbeId` deserialization now sanitize ids before storage and export.
- `SnapshotKind::Custom` labels are sanitized in envelopes, `as_label()`, serialization, and
  deserialization.
- `SnapshotKind::as_label()` now returns `Cow<'_, str>` so custom labels can be sanitized without
  forcing allocation for built-in kinds.
- The sanitizer continues scanning after invalid `@` candidates, so a malformed token before a real
  email does not stop redaction.
- The sanitizer redacts same-token assignments, separated `key: value` and `key = value` pairs,
  `bearer` values, and bare sensitive-key followed-by-value pairs such as `token raw-value`.

# Behavior Notes

- `open-gpui-devtools` default builds remain renderer-neutral.
- `form`, `resource`, `motion`, `docking`, `ui-components`, and `gpui` feature gates compile in
  isolation; `gpui` intentionally enables `ui-components` for the read-only inspector surface.
- The UI foundation gallery DevTools page collects current sample state through `DevtoolsRegistry`
  and tests reject static demo snapshot builders.
- Runtime facts without a committed public snapshot, such as unmounted scroll viewport and docking
  viewport state in the gallery, surface as sanitized diagnostics rather than invented fixtures.

# Citations

- Plan: `docs/plans/2026-07-08-004-feat-devtools-real-probes-dogfood-plan.md`
- Progress: `docs/knowledge/engineering/progress/2026-07-08-open-gpui-devtools-real-probes-dogfood.md`
- Work registration: `docs/knowledge/engineering/registry/open-gpui-devtools-real-probes-dogfood-codex-root.md`
- Verification guide: `docs/verification.md`
