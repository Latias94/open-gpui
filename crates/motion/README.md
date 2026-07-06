# Open GPUI Motion

`open-gpui-motion` contains renderer-neutral motion primitives for Open GPUI components and domain
crates. It is a deterministic Rust foundation for layout-like UI motion; it is not a DOM animation
runtime.

## What This Crate Owns

- Motion preferences, duration tokens, easing tokens, and immediate reduced-motion semantics.
- Timeline and spring scalar sampling from explicit elapsed time.
- Policy-resolved execution plans for committed layout, continuity, affordance previews, and
  input-coupled paths.
- Scalar controllers, retargeting, cancellation, completion, and adapter-owned frame demand.
- Neutral logical-pixel geometry plus projection, reveal, and clip helpers for final-size content.

Adapters keep authority over rendering, input, focus, accessibility, and frame scheduling. A
Splitter, docking host, canvas, or application decides when to request a GPUI frame and how to map a
motion sample into painted elements.

## Boundaries

This crate deliberately does not provide React hooks, CSS parsing, DOM measurement, WAAPI behavior,
browser-native acceleration, global animation loops, drag-and-drop policy, asset animation, or full
shared-layout orchestration. Presence, keyframes, repeat/reverse/speed controls, public value
subscriptions, and high-level builders are deferred until a first-party Open GPUI adapter proves the
shape.

`open-gpui-motion` must stay below `open-gpui-ui-core`, `open-gpui-ui-components`,
`open-gpui-docking`, `open-gpui-platform`, `open-gpui-web`, and renderer crates. Use conversion
helpers in adapter crates to map `MotionRect` to renderer-specific geometry.

## Example

```rust
use open_gpui_motion::{
    MotionDuration, MotionEasing, MotionExecutionPlan, MotionModel, MotionPolicyContext,
    MotionPolicyInput, MotionPreference, MotionScalarExecution, MotionSpec,
};
use std::time::Duration;

let spec = MotionSpec::new(
    MotionPreference::Animated,
    MotionDuration::Custom(Duration::from_millis(180)),
    MotionEasing::EaseOutStrong,
);
let plan = MotionExecutionPlan::resolve(
    MotionPolicyInput::new(MotionPolicyContext::CommittedLayout, MotionModel::timeline(spec))
        .with_spatial_motion(true)
        .with_reduced_motion_final_state(true),
);
let execution = MotionScalarExecution::start(plan, 0.0, 1.0, 0.0, Duration::ZERO);
let sample = execution.sample_at(Duration::from_millis(90));

if sample.frame_demand().needs_frame() {
    // Ask the owning adapter to request a GPUI frame.
}
```

Reduced motion uses the same APIs and publishes the final semantic state immediately:

```rust
use open_gpui_motion::{MotionPreference, MotionSpec};

let spec = MotionSpec::committed_layout(MotionPreference::Reduced);
assert!(spec.is_immediate());
```

## Testing

For focused changes in this crate, run:

```sh
cargo check -p open-gpui-motion --tests --locked
cargo nextest run -p open-gpui-motion --no-fail-fast
cargo test -p open-gpui-motion --doc
```

When changing geometry or policy used by first-party adapters, also run the focused Splitter and
docking gates documented in `docs/verification.md`.
