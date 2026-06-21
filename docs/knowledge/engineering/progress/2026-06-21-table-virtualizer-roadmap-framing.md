---
type: Work Progress
title: Table and virtualizer roadmap framing
description: "Planning note for the next table / virtualizer series using fret and TanStack references."
timestamp: 2026-06-21T23:59:00Z
tags: ["open-gpui", "table", "virtualizer", "tanstack", "fret", "planning"]
source_session: "019ec6c8-5566-7062-8458-21ebe1360573"
git_branch: "main"
---

# Summary

The next series is table / virtualizer design, not a standalone headless crate.

Local reference checkouts now include `repo-ref/fret`, `repo-ref/tanstack-table`, and
`repo-ref/tanstack-virtual`.

The likely shape is a renderer-neutral table state and row-model contract, a reusable virtualizer
contract, and GPUI adapters plus gallery recipes on top.

# Details

`fret` is the best layering reference because it keeps thin facades separate from deep behavior
modules.

TanStack Table is the semantic reference for row-model ordering, stable ids, and state shape.

TanStack Virtual is the semantic reference for viewport count, item keys, overscan, measurement,
anchor behavior, and snapshot/restore.

The current product boundary should stay in the existing UI crates. The plan should leave future
headless extraction open as a later decision, not as the active roadmap.

# Plan Outcome

`docs/plans/2026-06-21-001-feat-ui-table-virtualizer-roadmap-plan.md` now captures the roadmap.
It keeps the work inside the current UI crates and splits execution into table-core v0,
virtualizer metrics/range v0, GPUI adapter, gallery conformance, and follow-up growth rules.

# Next Action

Start U1 and U2 from the roadmap, then add U6 before building the GPUI adapter slice.

# Citations

[1] `docs/knowledge/engineering/current-state.md`
[2] `repo-ref/fret/docs/adr/0100-headless-table-engine.md`
[3] `repo-ref/fret/docs/adr/0042-virtualization-and-large-lists.md`
[4] `repo-ref/tanstack-table/docs/guide/row-models.md`
[5] `repo-ref/tanstack-virtual/docs/api/virtualizer.md`
[6] `docs/plans/2026-06-21-001-feat-ui-table-virtualizer-roadmap-plan.md`
