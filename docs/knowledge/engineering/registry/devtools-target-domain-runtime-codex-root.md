---
type: "Work Registration"
title: "DevTools target domain runtime"
description: "Target/domain/event capture runtime refactor for open-gpui DevTools."
timestamp: 2026-07-09T03:29:24Z
status: "active"
last_seen: 2026-07-09T04:20:00Z
registration_id: "devtools-target-domain-runtime-codex-root"
producer_id: "codex-root"
source_workspace: "F:\\SourceCodes\\Rust\\open-gpui"
related_plan: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
git_branch: "main"
git_commit: "37eab2e5"
latest_link: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
---

# Scope

Execute U1-U7 from the target/domain/runtime plan; keep DevTools local, read-only, renderer-neutral, and redaction-first.

# Current Claim

Plan reviewed and revised after headless doc review. U1-U3 capture core slice is committed and pushed in `37eab2e5`.

# Latest Links

- docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md

# Handoff

Start U4. The MVP checkpoint passed: `DevtoolsInspectorState::from_capture()` projects target/domain/event rows and DevTools all-features tests pass.

# Citations
