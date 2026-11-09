# Open GPUI Motion

`open-gpui-motion` contains renderer-neutral motion primitives for Open GPUI components and domain
crates. It samples layout-like UI motion from caller-owned elapsed time. It does not own a UI clock
or scheduler, and analytical sampling does not guarantee cross-platform bitwise simulation results.

## What This Crate Owns

- Motion preferences, duration tokens, easing tokens, and immediate reduced-motion semantics.
- `MotionTransition` as the root facade for duration, spring, immediate, and reduced-motion
  transitions selected by product intent.
- The upstream pure `SpringConfig`, `SpringState`, and `sampled_easing` solver, shared by domain
  motion and GPUI's typed Spring API through the same Rust types.
- `MotionSpringOptions` for a shared configuration, one positional epsilon and separate
  `MotionSpringPolicyHints`. Domain presets do not define a second physical solver.
- Timeline and spring scalar sampling from explicit elapsed time, with `Instant` conversion kept in
  adapter code instead of the motion lifecycle.
- `MotionScalarRun` and `MotionProgressRun` for policy-resolved scalar and normalized 0..1 adapter
  runs.
- Scalar controllers, retargeting, cancellation, explicit finish, terminal pruning, raw execution
  plans, and low-level policy input validation in `open_gpui_motion::advanced`.
- `MotionProgressSequence` for composing many keyed scalar tracks with absolute starts, append,
  with-previous, after-previous, and staggered insertion while preserving renderer-neutral sampling.
- `MotionClockSample` for mapping adapter `Instant` values into deterministic controller
  `Duration` samples with non-monotonic elapsed time clamped.
- `MotionFrameDemand::combine` and `MotionFrameDemand::combine_all` for aggregating many motion
  sources into one adapter frame request.
- `MotionFrameDriver` for keeping adapter-owned frame request decisions consistent without
  depending on a GPUI window, browser scheduler, or renderer, including explicit reset reasons when
  an adapter starts a new local motion epoch.
- Neutral logical-pixel geometry plus projection, reveal, and clip helpers for final-size content.
- Fallible `MotionProjectionTransformSample` values for scale/translation projection without a
  dependency on GPUI or a renderer.

Adapters keep authority over rendering, input, focus, accessibility, frame scheduling, and clock
sampling. A Splitter, docking host, canvas, or application passes explicit elapsed time into motion
executions, decides when to request a GPUI frame from returned `MotionFrameDemand`, and maps samples
into painted elements. This keeps motion time from owning or competing with the UI runtime clock.

## First-Party Proof Scope

The current first-party proof is intentionally small:

- `Splitter` consumes scalar controller samples for programmatic panel layout transitions.
- `VirtualizedList` consumes scalar controller samples for an active-descendant indicator that
  moves paint-only chrome by stable row key.
- Docking consumes neutral motion geometry and projection helpers for presentation and affordance
  evidence.

These consumers exercise adapter clocks, normalized progress, reduced-motion final state and frame
demand. Controller retargeting, freeze-at-cancel, terminal pruning and sequence composition also have
crate-level tests. Existing consumers may replace a run with zero initial velocity; the existence
of velocity-preserving controller APIs does not prove that every adapter preserves velocity. The
current proof does not cover row enter/exit animation, public presence, keyframes,
repeat/reverse/speed controls, full shared-layout orchestration, WAAPI, or a global scheduler.

## Shared Spring Solver And Domain Policy

The pure solver in `src/spring_solver.rs` is imported from Zed
`635026fabc38440fce5e1767d125026a17b12012`, `crates/gpui/src/spring.rs`. It retains the official
signatures and numerical mechanism: canonical parameters, the three damping regimes, the exact
propagator, fixed-target `step`, moving-target `step_ramp`, natural-frequency settling thresholds,
conservative `settle_time` and duration-based `sampled_easing`. All eight official pure-math
regression scenarios live in `src/spring_solver/tests.rs`. Typed targets, interpolation, playback
and Element-ID lifecycle belong to GPUI; they are not duplicated in this renderer-neutral crate.

One reproduced numerical correction differs from the pinned implementation: canonical parameter
ratios and products use `f64` intermediates so common finite parameter scaling does not underflow
or overflow the `f32` product. Returned parameters and subsequent solver arithmetic remain `f32`;
this does not guarantee arbitrary-range finite output or bitwise parity with the prior arithmetic.
The conservative settling-time envelope also respects representable target±epsilon endpoints;
rounding an endpoint outside the requested tolerance moves it inward before deriving the
position threshold. This may wait for a zero envelope when epsilon is below the target's spacing.
Fixed-target position sampling also accumulates the target and opposing matrix contributions in
`f64` before conversion, avoiding intermediate target rounding before cancellation. The official
matrix/decay formulas, velocity and moving-target ramp path remain unchanged. These corrections
do not establish measured performance or general numerical accuracy outside the tested range.

`SpringConfig::new(stiffness, damping, mass)` follows upstream argument order. Its constructor does
not sanitize physical inputs: stiffness and mass must be finite and positive, and damping must be
finite and non-negative. `MotionSpringOptions::new(config)` retains that exact configuration. Its
positional epsilon defaults to `0.001`; `with_epsilon` accepts finite non-negative values and resets
negative or non-finite values to the default. Settling compares position with epsilon and velocity
with `epsilon * natural_frequency`. There is no independently configurable velocity threshold.
Domain samples retain the existing fallback for non-finite scalar inputs: source becomes zero,
target becomes the sanitized source, and initial velocity becomes zero. Solver output is retained
while active; a settled domain sample publishes the exact target with zero velocity.

`MotionSpringPolicyHints` holds review duration and reported bounce. Finite reported bounce is
clamped to `0..=MAX_REVIEWABLE_BOUNCE`, and non-finite bounce becomes zero. Review duration is retained
for policy validation and sequence planning. Neither field changes the solver, measures actual
overshoot, or establishes physical settling time. Domain sampling retains raw solver overshoot;
only an explicitly normalized progress or geometry projection clamps its own output.

```rust
use open_gpui_motion::{
    MotionIntent, MotionPreference, MotionSpringOptions, MotionSpringPolicyHints, MotionTransition,
    SpringConfig,
};
use std::time::Duration;

let options = MotionSpringOptions::new(SpringConfig::new(260.0, 28.0, 1.0))
    .with_epsilon(0.001)
    .with_policy_hints(MotionSpringPolicyHints::new(Duration::from_millis(180), 0.12));
let transition = MotionTransition::spring(
    MotionIntent::CommittedLayout,
    MotionPreference::Animated,
    options,
);
let sample = transition.sample_scalar_elapsed(0.0, 1.0, 0.0, Duration::from_millis(90));
assert!(sample.is_active());
```

The previous `MotionSpringPhysics` facade and its independent `rest_delta`/`rest_speed` controls are
removed. Pass `MotionSpringOptions` to `MotionTransition::spring`; advanced consumers use
`MotionSpringSpec::from_options` and `options()` instead of `from_physics` and `physics()`. Convert
old physical arguments explicitly from mass/stiffness/damping to the official
stiffness/damping/mass order. Replace positional rest tolerance with `with_epsilon`, then qualify the
new completion timing; the previous independent velocity tolerance has no lossless mapping.

Domain controller cancellation continues to freeze its sampled position and velocity at the cancel
time. It does not adopt GPUI `SpringPlayback::Cancelled`, which restores the element's initial
coordinate and discards velocity. A property has one state owner: a component's domain run and a
GPUI element Spring must not both animate the same value. Adapters pass the App reduced-motion
preference into domain policy and continue routing all frame demand through the existing Window.

## Where To See It

Run the component gallery to inspect Splitter and VirtualizedList motion in a normal checkout:

```sh
cargo run -p open-gpui-ui-foundation-gallery
```

Run the minimal docking example for the common single-window path, or the native dogfood example to
inspect layout, affordance, and viewport-runtime motion through the docking host:

```sh
cargo run -p open-gpui-docking-minimal
cargo run -p open-gpui-docking-native
```

Both examples keep frame scheduling in their GPUI adapters. `open-gpui-motion` only publishes
deterministic samples and frame demand.

For an interactive GPUI subtree, sample `MotionProjection::try_transform_sample` and convert it in
a crate that depends on both Motion and GPUI. The first-party UI Components adapter is
`gpui_adapter::subtree_transform_from_motion_projection`. Motion does not construct
`open_gpui::SubtreeTransform`, and invalid or unrepresentable projection geometry returns
`MotionProjectionError` instead of silently becoming identity. Final and reduced-motion samples
resolve to the exact final endpoint.

## Boundaries

This crate deliberately does not provide React hooks, CSS parsing, DOM measurement, WAAPI behavior,
browser-native acceleration, global animation loops, drag-and-drop policy, asset animation, or full
shared-layout orchestration. `MotionProgressRun` only owns a local 0..1 run lifecycle, and
`MotionProgressSequence` only owns deterministic keyed timing and sampling; neither mutates
properties or schedules frames. Presence, keyframes, repeat/reverse/speed controls, public value
subscriptions, and high-level property builders are deferred until a first-party Open GPUI adapter
proves the shape.

`open-gpui-motion` must stay below `open-gpui-ui-core`, `open-gpui-ui-components`,
`open-gpui-docking`, `open-gpui-platform`, `open-gpui-web`, and renderer crates. Use conversion
helpers in adapter crates to map `MotionRect` to renderer-specific geometry.

## Example

```rust
use open_gpui_motion::{
    MotionDuration, MotionEasing, MotionFrameDriver, MotionIntent, MotionPreference,
    MotionTransition,
};
use std::time::Duration;

let transition = MotionTransition::duration(
    MotionIntent::CommittedLayout,
    MotionPreference::Animated,
    MotionDuration::Custom(Duration::from_millis(180)),
    MotionEasing::EaseOutStrong,
);
let execution = transition.scalar_run(0.0, 1.0, 0.0, Duration::ZERO);
let mut frame_driver = MotionFrameDriver::new();
let sample = frame_driver.sample_elapsed(Duration::from_millis(90), |clock| {
    let sample = execution.sample_clock(clock);
    (sample.value(), sample.frame_demand())
});

if sample.should_request_frame() {
    // Ask the owning adapter to request a GPUI frame.
}
```

Reduced motion uses the same APIs and publishes the final semantic state immediately:

```rust
use open_gpui_motion::{MotionPreference, MotionTransition};

let transition = MotionTransition::committed_layout(MotionPreference::Reduced);
assert!(transition.is_immediate());
```

Lifecycle ordering is intentionally small: start or retarget creates active sampled state, each sample returns a frame demand, `cancel` freezes the sampled value and goes idle without reaching the semantic final state, `finish` publishes the target value as completed, reduced motion publishes the final state immediately, and adapters may prune terminal tracks after observing idle demand. When an adapter retargets, cancels, finishes, prunes terminal state, or changes motion identity, call `MotionFrameDriver::reset(MotionFrameResetReason::...)` before observing the next epoch's demand so stale elapsed time and requested-frame diagnostics do not leak into the new run.

## Testing

For focused changes in this crate, run:

```sh
cargo check -p open-gpui-motion --tests --locked
cargo nextest run -p open-gpui-motion --no-fail-fast
cargo test -p open-gpui-motion --doc
```

When changing geometry or policy used by first-party adapters, also run the focused Splitter and
docking gates documented in `docs/verification.md`.
