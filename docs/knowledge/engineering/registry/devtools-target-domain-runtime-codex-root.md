---
type: "Work Registration"
title: "DevTools target domain runtime"
description: "Target/domain/event capture runtime refactor for open-gpui DevTools."
timestamp: 2026-07-09T03:29:24Z
status: "active"
last_seen: 2026-07-09T03:51:43Z
registration_id: "devtools-target-domain-runtime-codex-root"
producer_id: "codex-root"
source_workspace: "F:\\SourceCodes\\Rust\\open-gpui"
related_plan: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
git_branch: "main"
git_commit: "9995c0c6"
latest_link: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
---

# Scope

Execute U1-U7 from the target/domain/runtime plan; keep DevTools local, read-only, renderer-neutral, and redaction-first.

# Current Claim

Plan reviewed and revised after headless doc review. Current workspace also contains uncommitted U1/U3 Rust protocol scaffolding that must be audited before editing.

# Latest Links

- docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md

# Handoff

Start by auditing target.rs/domain.rs/event.rs, registry collect_capture, timeline projection, and new tests; do not create duplicate protocol modules.

# Citations
