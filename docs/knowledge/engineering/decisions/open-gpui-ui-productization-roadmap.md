---
type: "Decision"
title: "Open GPUI UI component productization roadmap"
description: "Treat current UI crates as the active product boundary and defer standalone headless extraction."
timestamp: 2026-06-17T04:01:25Z
tags: ["open-gpui", "ui", "components", "productization", "adr"]
status: "active"
source_session: "019eca05-d03c-74d1-8703-fff1e230e4ff"
git_branch: "feat/open-gpui-ui-core"
---

# Decision

Open GPUI should productize the current UI crates before reopening standalone headless extraction.
`open-gpui-ui-core`, `open-gpui-ui-components`, and `examples/ui-foundation-gallery` are the active
product boundary for the next phase.

As of the 2026-07-01 follow-up, the next productization slice is narrower than broad component
rewrites: split the component contract registry, add accessibility contract gates, and add a theme
JSON schema plus file-loader facade. Do not treat remaining 1k+ component files or
`open-gpui-ui-headless` as current roadmap work.

# Context

The previous strict-boundary and extraction-design work proved that UI core can stay renderer
neutral and that several behavior families could move later. That evidence is still useful, but it
was starting to make the next session look like a crate-extraction task.

The current product risk is different: the component surface is broad enough that registry
ownership, accessibility contracts, theme portability, gallery conformance, verification notes, and
memory need to tell one coherent product story.

# Alternatives

- Continue directly into a behavior-crate extraction plan.
- Create a standalone `open-gpui-ui-headless` crate now.
- Productize the current crates first and keep extraction as a future explicit decision.

The third path is chosen because it improves the shipped surface without freezing a new package
boundary before the current component contracts are stable.

# Consequences

- ADR 0008 is the active roadmap decision for the next UI component phase.
- ADR 0006 and ADR 0007 remain historical boundary references, not the next implementation step.
- Future work should start with registry, accessibility, and theme productization, then use the
  gallery and verification docs as release gates.

# Citations

[1] [ADR 0008](../../../adr/0008-open-gpui-ui-component-productization-roadmap.md)
[2] [Productization roadmap plan](../../../plans/2026-06-17-003-feat-ui-component-productization-roadmap-plan.md)
[3] [Current State](../current-state.md)
