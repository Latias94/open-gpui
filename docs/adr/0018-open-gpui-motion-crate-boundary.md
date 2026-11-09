# ADR 0018: Open GPUI Motion Crate Boundary

**Status**: Accepted
**Date**: 2026-07-06
**Last Revised**: 2026-11-09 (official Spring baseline and domain sampling)

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

Create `open-gpui-motion` as the renderer-neutral home for shared sampling, motion policy and
geometry. GPUI retains the official public element-animation interface and its UI lifecycle.

The crate owns:

- motion preferences, durations, easing, and reduced-motion final-state semantics;
- deterministic timeline sampling and the official pure `SpringConfig` / `SpringState` solver,
  including ramp targets, transition matrices, settling and duration-easing adaptation;
- `MotionClockSample` values that map adapter clocks into deterministic elapsed `Duration`
  samples and clamp non-monotonic elapsed time;
- policy-resolved execution plans and public frame-demand aggregation through
  `MotionFrameDemand::combine` / `combine_all`;
- scalar tracks, scalar controllers, cancellation, explicit finish, terminal pruning, completion,
  and retargeting;
- `MotionFrameHost` reset reasons for adapter-owned motion epochs such as retarget, cancel,
  finish, terminal pruning, and stable motion identity changes;
- renderer-neutral motion geometry and projection/reveal/clip helpers.

`open-gpui-ui-core` no longer declares or re-exports motion modules. Splitter, UI components, and
docking import `open_gpui_motion` directly and perform adapter-local conversions between
`MotionRect` and their own geometry types.

The November 9 refinement adopts the complete Spring mechanism from fixed Zed
`635026fabc38440fce5e1767d125026a17b12012`, introduced by
`8b1497dbd22fb06f5838a7c0b84a1e54fafa71bc` with the final executor-clock correction.
The motion crate owns the single pure solver. GPUI reexports those same configuration/state
types and easing function, and owns typed targets/interpolation, SpringAnimation builders,
playback, Element-ID state, UI clocks and frame requests. The solver imports no GPUI geometry,
App, Window, renderer or scheduler. Existing domain damping formulas are removed rather than
kept as a second implementation.

Domain sampling uses explicit elapsed time and the shared solver. `MotionSpringOptions` groups
its official configuration and positional epsilon with separately documented policy hints;
review-duration and bounce hints do not measure settling time or physical overshoot. The old
MotionSpringPhysics and independently configurable rest-speed threshold are removed. Official
settling compares velocity with epsilon times natural frequency. Domain input sanitization
does not alter the official configuration constructor's preconditions.

GPUI elements preserve the complete official lifecycle: first mount defaults to the target
unless `from` supplies an initial value; an existing element advances under its old target and
configuration before applying new inputs. Paused retains velocity, Stopped clears it, Completed
resolves the latest target and Cancelled restores the original initial value. Domain cancellation
that freezes a current sample remains a distinct operation; it is not an alias for Cancelled.
Raw official phases may overshoot, while domain progress/reveal clipping stays explicit.

Splitter, keyed list-indicator, docking projection and bring-into-view retain domain identity,
layout/input and completion authority. A property has one live motion owner; a domain run must
not be layered with another element Spring driving the same value. GPUI adapters treat App
reduced motion as a policy floor, while the pure solver does not query an application singleton.
UI playback and reduced-motion preference do not define a game simulation clock or establish
cross-platform bitwise determinism.

The stable surface is limited to contracts proven by current first-party consumers. Presence,
keyframes, repeat/reverse/speed controls, public value subscriptions, domain property builders, DOM/CSS
parsing, React hooks, WAAPI behavior, browser-native acceleration, and full shared-layout
orchestration are not accepted as stable core promises.
For v0.2.0, `VirtualizedList` extends the proof set by consuming scalar controller samples for a
stable-key active-descendant indicator that animates paint-only chrome while leaving row layout,
scroll offsets, focus, hit testing, selection, and accessibility state under the component adapter.

## Consequences

- Existing in-repo callers must migrate from `open_gpui_ui_core::Motion*` to
  `open_gpui_motion::Motion*`.
- Motion can be checked and documented independently from UI components and docking.
- Import-boundary checks must prevent `open-gpui-motion` from depending on UI, docking, platform,
  web, or renderer crates.
- Adapter crates remain responsible for rendering, input, focus, accessibility, hit testing,
  semantic ownership, and GPUI frame requests.
- `open-gpui-motion` exposes frame demand, but it does not own a global scheduler or call
  `request_frame`; adapters aggregate demand and decide when to ask GPUI for another frame.
- Adapters must reset their local `MotionFrameHost` with an explicit reason when a retarget,
  cancellation, completion, terminal prune, or stable identity change starts a new motion epoch.
- ADR 0017's decision to defer a public value graph remains current. This refinement supersedes
  ADR 0016/0017's duplicate spring solver and broad deferral of official application builders and
  playback; their domain ownership, explicit time and projection contracts remain current.

## Related Documents

- `crates/motion/README.md`
- `docs/adr/0015-ui-motion-runtime-foundation.md`
- `docs/adr/0016-ui-motion-spring-foundation.md`
- `docs/adr/0017-ui-motion-value-foundation.md`
- `docs/plans/2026-07-06-002-refactor-open-gpui-motion-system-plan.md`
