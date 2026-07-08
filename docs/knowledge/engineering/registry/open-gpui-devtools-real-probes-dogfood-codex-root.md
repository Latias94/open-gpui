---
type: "Work Registration"
title: "Open GPUI DevTools real probes dogfood"
description: "Registration for Open GPUI DevTools real probes dogfood."
timestamp: 2026-07-08T15:39:07Z
status: "verified"
last_seen: 2026-07-09T00:56:22+08:00
producer_id: "codex-root"
related_plan: "docs/plans/2026-07-08-004-feat-devtools-real-probes-dogfood-plan.md"
git_branch: "feat/devtools-real-probes-dogfood"
---

# Scope

Implement the real DevTools probe and gallery dogfood plan in
`docs/plans/2026-07-08-004-feat-devtools-real-probes-dogfood-plan.md`.
The branch should replace static demo snapshots with public ecosystem
adapter snapshots while preserving feature-gated crate boundaries.


# Current Claim

Root agent owns the feature branch `feat/devtools-real-probes-dogfood`.
Implementation units U1-U5 and focused verification are complete. Final
merge to local `main` and push to `origin/main` remain after the U5 commit.


# Latest Links

- Plan: `docs/plans/2026-07-08-004-feat-devtools-real-probes-dogfood-plan.md`
- Plan commit: `a885ea47`
- Progress: `docs/knowledge/engineering/progress/2026-07-08-open-gpui-devtools-real-probes-dogfood.md`
- Verification: `docs/knowledge/engineering/verification/open-gpui-devtools-real-probes-dogfood-20260709.md`

# Handoff

U1-U5 are implemented and focused verification passed. U5 includes final redaction hardening for
`ProbeId`, `SnapshotKind::Custom`, redaction summaries, invalid email scanning, and separated
sensitive key/value strings.

Progress: `docs/knowledge/engineering/progress/2026-07-08-open-gpui-devtools-real-probes-dogfood.md`
Verification: `docs/knowledge/engineering/verification/open-gpui-devtools-real-probes-dogfood-20260709.md`


# Citations
