# Open GPUI Motion

`open-gpui-motion` contains renderer-neutral motion primitives for Open GPUI components and domain
crates. It is a deterministic Rust foundation for layout-like UI motion; it is not a DOM animation
runtime.

## What This Crate Owns

- Motion preferences, duration tokens, easing tokens, and immediate reduced-motion semantics.
- `MotionTransition` as the root facade for duration, spring, immediate, and reduced-motion
  transitions selected by product intent.
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

These consumers prove deterministic clocks, normalized progress runs, retargeting, reduced-motion
final state, cancellation, terminal pruning, sequence composition, and `MotionFrameDemand`
aggregation. They do not prove row enter/exit animation, public presence, keyframes,
repeat/reverse/speed controls, full shared-layout orchestration, WAAPI, or a global scheduler.

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

Lifecycle ordering is intentionally small: start or retarget creates active sampled state, each sample returns a frame demand, `cancel` freezes the sampled value and goes idle without reaching the semantic final state, `finish` publishes the target value as completed, reduced motion publishes the final state immediately, and adapters may prune terminal tracks after observing idle demand. When a host retargets, cancels, finishes, prunes terminal state, or changes motion identity, call `MotionFrameDriver::reset(MotionFrameHostResetReason::...)` before observing the next epoch's demand so stale elapsed time and requested-frame diagnostics do not leak into the new run.

## Testing

For focused changes in this crate, run:

```sh
cargo check -p open-gpui-motion --tests --locked
cargo nextest run -p open-gpui-motion --no-fail-fast
cargo test -p open-gpui-motion --doc
```

When changing geometry or policy used by first-party adapters, also run the focused Splitter and
docking gates documented in `docs/verification.md`.
