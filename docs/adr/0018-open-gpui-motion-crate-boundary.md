# ADR 0018: Open GPUI Motion Crate Boundary

**Status**: Accepted
**Date**: 2026-07-06

## Context

ADR 0015, ADR 0016, and ADR 0017 built a renderer-neutral motion foundation inside
`open-gpui-ui-core`. That made the first Splitter and docking proofs possible, but it also made the
ownership boundary misleading: motion was becoming a general Open GPUI primitive while living under
the UI foundation crate.

The new component-library work needs motion to be reusable by components, docking, canvas-like
surfaces, and future adapters without forcing those crates through `open-gpui-ui-core`. Open GPUI is
still pre-1.0, and the motion API was not part of v0.1.0, so preserving compatibility aliases would
only create a stale public path.

## Decision

Create `open-gpui-motion` as the only public home for shared motion contracts.

The crate owns:

- motion preferences, durations, easing, and reduced-motion final-state semantics;
- deterministic timeline and spring scalar sampling;
- policy-resolved execution plans and frame-demand reporting;
- scalar tracks, scalar controllers, cancellation, completion, and retargeting;
- renderer-neutral motion geometry and projection/reveal/clip helpers.

`open-gpui-ui-core` no longer declares or re-exports motion modules. Splitter, UI components, and
docking import `open_gpui_motion` directly and perform adapter-local conversions between
`MotionRect` and their own geometry types.

The stable surface is limited to contracts proven by current first-party consumers. Presence,
keyframes, repeat/reverse/speed controls, public value subscriptions, high-level builders, DOM/CSS
parsing, React hooks, WAAPI behavior, browser-native acceleration, and full shared-layout
orchestration are not accepted as stable core promises.

## Consequences

- Existing in-repo callers must migrate from `open_gpui_ui_core::Motion*` to
  `open_gpui_motion::Motion*`.
- Motion can be checked and documented independently from UI components and docking.
- Import-boundary checks must prevent `open-gpui-motion` from depending on UI, docking, platform,
  web, or renderer crates.
- Adapter crates remain responsible for rendering, input, focus, accessibility, hit testing,
  semantic ownership, and GPUI frame requests.
- ADR 0017's decision to defer a public value graph remains current; this ADR supersedes only the
  location and import path of the proven shared motion contracts.

## Related Documents

- `crates/motion/README.md`
- `docs/adr/0015-ui-motion-runtime-foundation.md`
- `docs/adr/0016-ui-motion-spring-foundation.md`
- `docs/adr/0017-ui-motion-value-foundation.md`
- `docs/plans/2026-07-06-002-refactor-open-gpui-motion-system-plan.md`
