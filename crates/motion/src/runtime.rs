//! Renderer-neutral runtime helpers for deterministic UI motion.

use crate::{MotionPx, MotionRect, motion::MotionSpec, motion_point, motion_rect, motion_size};
use std::{collections::HashMap, hash::Hash, time::Duration};

/// Runtime state for sampled motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionRunState {
    /// The motion spec completed immediately.
    Immediate,
    /// The motion run is still active.
    Active,
    /// The motion run reached its final state.
    Completed,
    /// The motion run was cancelled before reaching its final state.
    Cancelled,
}

impl MotionRunState {
    /// Returns whether callers should continue requesting animation frames.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Returns whether the timeline no longer needs animation frames.
    pub const fn is_terminal(self) -> bool {
        !self.is_active()
    }

    /// Returns whether the semantic final state has been reached.
    pub const fn reached_final_state(self) -> bool {
        matches!(self, Self::Immediate | Self::Completed)
    }
}

/// A sampled point on a motion timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MotionTimelineSample {
    state: MotionRunState,
    elapsed: Duration,
    raw_progress: f32,
    progress: f32,
}

impl MotionTimelineSample {
    /// Creates a sample from explicit values.
    pub(crate) const fn new(
        state: MotionRunState,
        elapsed: Duration,
        raw_progress: f32,
        progress: f32,
    ) -> Self {
        Self {
            state,
            elapsed,
            raw_progress,
            progress,
        }
    }

    /// Returns the sampled timeline state.
    pub(crate) const fn state(self) -> MotionRunState {
        self.state
    }

    /// Returns the elapsed time used for this sample.
    pub(crate) const fn elapsed(self) -> Duration {
        self.elapsed
    }

    /// Returns the unclamped easing input after duration normalization.
    #[cfg(test)]
    pub(crate) const fn raw_progress(self) -> f32 {
        self.raw_progress
    }

    /// Returns the eased progress.
    pub(crate) const fn progress(self) -> f32 {
        self.progress
    }

    /// Returns whether the semantic final state has been reached.
    pub(crate) const fn reached_final_state(self) -> bool {
        self.state.reached_final_state()
    }
}

/// A deterministic timeline for one UI motion transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MotionTimeline;

impl MotionTimeline {
    /// Samples a motion spec using an explicit elapsed duration.
    pub(crate) fn sample_elapsed(spec: MotionSpec, elapsed: Duration) -> MotionTimelineSample {
        if spec.is_immediate() {
            return MotionTimelineSample::new(MotionRunState::Immediate, elapsed, 1.0, 1.0);
        }

        let duration = spec.duration().as_duration();
        if duration.is_zero() {
            return MotionTimelineSample::new(MotionRunState::Immediate, elapsed, 1.0, 1.0);
        }

        let raw_progress = (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
        let progress = spec.easing().sample(raw_progress);
        let state = if raw_progress >= 1.0 {
            MotionRunState::Completed
        } else {
            MotionRunState::Active
        };
        MotionTimelineSample::new(state, elapsed, raw_progress, progress)
    }
}

/// A stable-id value captured from a motion sample or target state.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionSnapshot<K, V> {
    id: K,
    value: V,
}

impl<K, V> MotionSnapshot<K, V> {
    /// Creates a stable-id snapshot.
    pub const fn new(id: K, value: V) -> Self {
        Self { id, value }
    }

    /// Returns the stable identity.
    pub const fn id(&self) -> &K {
        &self.id
    }

    /// Returns the captured value.
    pub const fn value(&self) -> &V {
        &self.value
    }

    /// Consumes the snapshot and returns its parts.
    pub fn into_parts(self) -> (K, V) {
        (self.id, self.value)
    }
}

/// A target item paired with the currently sampled value for the same identity.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionRetargetItem<K, S, T = S> {
    id: K,
    sampled: Option<S>,
    target: T,
}

impl<K, S, T> MotionRetargetItem<K, S, T> {
    /// Creates a retargeted item.
    pub const fn new(id: K, sampled: Option<S>, target: T) -> Self {
        Self {
            id,
            sampled,
            target,
        }
    }

    /// Returns the stable identity.
    pub const fn id(&self) -> &K {
        &self.id
    }

    /// Returns the sampled value when the identity existed in the interrupted transition.
    pub const fn sampled(&self) -> Option<&S> {
        self.sampled.as_ref()
    }

    /// Returns the target value.
    pub const fn target(&self) -> &T {
        &self.target
    }

    /// Consumes the item and returns its parts.
    pub fn into_parts(self) -> (K, Option<S>, T) {
        (self.id, self.sampled, self.target)
    }
}

/// Stable-id retargeting result for a new target set.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionRetargetSet<K, S, T = S> {
    targets: Vec<MotionRetargetItem<K, S, T>>,
    leaving: Vec<MotionSnapshot<K, S>>,
}

impl<K, S, T> MotionRetargetSet<K, S, T> {
    /// Creates a retarget set.
    pub fn new(
        targets: Vec<MotionRetargetItem<K, S, T>>,
        leaving: Vec<MotionSnapshot<K, S>>,
    ) -> Self {
        Self { targets, leaving }
    }

    /// Returns the target items in target order.
    pub fn targets(&self) -> &[MotionRetargetItem<K, S, T>] {
        &self.targets
    }

    /// Returns sampled items that were not present in the target set.
    pub fn leaving(&self) -> &[MotionSnapshot<K, S>] {
        &self.leaving
    }

    /// Consumes the set and returns its parts.
    pub fn into_parts(self) -> (Vec<MotionRetargetItem<K, S, T>>, Vec<MotionSnapshot<K, S>>) {
        (self.targets, self.leaving)
    }
}

/// Matches sampled values to a new target set by stable identity.
///
/// The returned targets preserve target order. The returned leaving snapshots preserve sampled
/// order for identities that are absent from the target set.
pub fn retarget_motion_snapshots<K, S, T>(
    sampled: impl IntoIterator<Item = MotionSnapshot<K, S>>,
    targets: impl IntoIterator<Item = MotionSnapshot<K, T>>,
) -> MotionRetargetSet<K, S, T>
where
    K: Clone + Eq + Hash,
{
    let mut sampled = sampled.into_iter().map(Some).collect::<Vec<_>>();
    let mut sampled_indices = HashMap::new();
    for (index, snapshot) in sampled.iter().enumerate() {
        if let Some(snapshot) = snapshot {
            sampled_indices.entry(snapshot.id.clone()).or_insert(index);
        }
    }

    let targets = targets
        .into_iter()
        .map(|target| {
            let (id, target_value) = target.into_parts();
            let sampled_value = sampled_indices
                .get(&id)
                .and_then(|index| sampled[*index].take())
                .map(|snapshot| snapshot.value);
            MotionRetargetItem::new(id, sampled_value, target_value)
        })
        .collect::<Vec<_>>();
    let leaving = sampled.into_iter().flatten().collect::<Vec<_>>();

    MotionRetargetSet::new(targets, leaving)
}

/// Logical edge used by renderer-neutral rect motion helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionEdge {
    /// The left edge.
    Left,
    /// The right edge.
    Right,
    /// The top edge.
    Top,
    /// The bottom edge.
    Bottom,
}

/// Chooses the best edge to move a rect out of a containing rect.
///
/// A rect touching an edge prefers that edge before distance comparisons. This keeps retargeted
/// split and zoom panes moving toward the window edge they visually belong to.
pub fn preferred_motion_edge(bounds: MotionRect, container: MotionRect) -> MotionEdge {
    let left = (bounds.origin.x - container.origin.x).as_f32().abs();
    let right = (rect_right(container) - rect_right(bounds)).abs();
    let top = (bounds.origin.y - container.origin.y).as_f32().abs();
    let bottom = (rect_bottom(container) - rect_bottom(bounds)).abs();
    let touching_epsilon = 0.5_f32;

    if left <= touching_epsilon {
        return MotionEdge::Left;
    }
    if right <= touching_epsilon {
        return MotionEdge::Right;
    }
    if top <= touching_epsilon {
        return MotionEdge::Top;
    }
    if bottom <= touching_epsilon {
        return MotionEdge::Bottom;
    }

    [
        (MotionEdge::Left, left),
        (MotionEdge::Right, right),
        (MotionEdge::Top, top),
        (MotionEdge::Bottom, bottom),
    ]
    .into_iter()
    .min_by(|(_, a), (_, b)| a.total_cmp(b))
    .map(|(edge, _)| edge)
    .unwrap_or(MotionEdge::Left)
}

/// Returns bounds for the rect just outside the container on the chosen edge.
pub fn motion_source_rect(
    edge: MotionEdge,
    final_bounds: MotionRect,
    container: MotionRect,
) -> MotionRect {
    let origin = match edge {
        MotionEdge::Left => motion_point(
            container.origin.x - final_bounds.size.width,
            final_bounds.origin.y,
        ),
        MotionEdge::Right => {
            motion_point(MotionPx::new(rect_right(container)), final_bounds.origin.y)
        }
        MotionEdge::Top => motion_point(
            final_bounds.origin.x,
            container.origin.y - final_bounds.size.height,
        ),
        MotionEdge::Bottom => {
            motion_point(final_bounds.origin.x, MotionPx::new(rect_bottom(container)))
        }
    };
    motion_rect(origin, final_bounds.size)
}

/// Samples the visible sub-rect revealed from an edge at unit progress.
pub fn reveal_rect_from_edge(
    final_bounds: MotionRect,
    edge: MotionEdge,
    progress: f32,
) -> MotionRect {
    let progress = progress.clamp(0.0, 1.0);
    match edge {
        MotionEdge::Left => {
            let width = final_bounds.size.width * progress;
            motion_rect(
                final_bounds.origin,
                motion_size(width, final_bounds.size.height),
            )
        }
        MotionEdge::Right => {
            let width = final_bounds.size.width * progress;
            motion_rect(
                motion_point(
                    MotionPx::new(rect_right(final_bounds)) - width,
                    final_bounds.origin.y,
                ),
                motion_size(width, final_bounds.size.height),
            )
        }
        MotionEdge::Top => {
            let height = final_bounds.size.height * progress;
            motion_rect(
                final_bounds.origin,
                motion_size(final_bounds.size.width, height),
            )
        }
        MotionEdge::Bottom => {
            let height = final_bounds.size.height * progress;
            motion_rect(
                motion_point(
                    final_bounds.origin.x,
                    MotionPx::new(rect_bottom(final_bounds)) - height,
                ),
                motion_size(final_bounds.size.width, height),
            )
        }
    }
}

/// Samples a rect between two rects at unit progress.
pub fn lerp_rect(from: MotionRect, to: MotionRect, progress: f32) -> MotionRect {
    let progress = progress.clamp(0.0, 1.0);
    motion_rect(
        motion_point(
            lerp_px(from.origin.x, to.origin.x, progress),
            lerp_px(from.origin.y, to.origin.y, progress),
        ),
        motion_size(
            lerp_px(from.size.width, to.size.width, progress),
            lerp_px(from.size.height, to.size.height, progress),
        ),
    )
}

fn lerp_px(from: MotionPx, to: MotionPx, progress: f32) -> MotionPx {
    MotionPx::new(from.as_f32() + (to.as_f32() - from.as_f32()) * progress)
}

fn rect_right(bounds: MotionRect) -> f32 {
    bounds.origin.x.as_f32() + bounds.size.width.as_f32()
}

fn rect_bottom(bounds: MotionRect) -> f32 {
    bounds.origin.y.as_f32() + bounds.size.height.as_f32()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MotionDuration, MotionEasing, MotionPreference};

    #[test]
    fn timeline_samples_start_midpoint_and_completion() {
        let spec = MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(200)),
            MotionEasing::Linear,
        );

        let start = MotionTimeline::sample_elapsed(spec, Duration::ZERO);
        assert_eq!(start.state(), MotionRunState::Active);
        assert_eq!(start.elapsed(), Duration::from_millis(0));
        assert_eq!(start.raw_progress(), 0.0);
        assert_eq!(start.progress(), 0.0);

        let midpoint = MotionTimeline::sample_elapsed(spec, Duration::from_millis(100));
        assert_eq!(midpoint.state(), MotionRunState::Active);
        assert_eq!(midpoint.raw_progress(), 0.5);
        assert_eq!(midpoint.progress(), 0.5);

        let complete = MotionTimeline::sample_elapsed(spec, Duration::from_millis(250));
        assert_eq!(complete.state(), MotionRunState::Completed);
        assert_eq!(complete.raw_progress(), 1.0);
        assert_eq!(complete.progress(), 1.0);
        assert!(complete.reached_final_state());
    }

    #[test]
    fn reduced_motion_samples_as_immediate() {
        let sample = MotionTimeline::sample_elapsed(
            MotionSpec::layout(MotionPreference::Reduced),
            Duration::from_millis(0),
        );

        assert_eq!(sample.state(), MotionRunState::Immediate);
        assert_eq!(sample.raw_progress(), 1.0);
        assert_eq!(sample.progress(), 1.0);
        assert!(sample.reached_final_state());
    }

    #[test]
    fn retarget_snapshots_match_by_identity_and_report_missing_items() {
        let retarget = retarget_motion_snapshots(
            [
                MotionSnapshot::new("left", 0.25),
                MotionSnapshot::new("center", 0.5),
                MotionSnapshot::new("right", 0.25),
            ],
            [
                MotionSnapshot::new("center", 0.7),
                MotionSnapshot::new("inspector", 0.3),
            ],
        );

        assert_eq!(retarget.targets().len(), 2);
        assert_eq!(retarget.targets()[0].id(), &"center");
        assert_eq!(retarget.targets()[0].sampled(), Some(&0.5));
        assert_eq!(retarget.targets()[0].target(), &0.7);
        assert_eq!(retarget.targets()[1].id(), &"inspector");
        assert_eq!(retarget.targets()[1].sampled(), None);
        assert_eq!(retarget.targets()[1].target(), &0.3);

        let leaving_ids = retarget
            .leaving()
            .iter()
            .map(MotionSnapshot::id)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(leaving_ids, ["left", "right"]);
    }

    #[test]
    fn preferred_motion_edge_prefers_touching_edge_before_distance() {
        let container = motion_rect(
            motion_point(MotionPx::ZERO, MotionPx::ZERO),
            motion_size(MotionPx::new(400.0), MotionPx::new(240.0)),
        );
        let touching_top_but_closer_left = motion_rect(
            motion_point(MotionPx::new(20.0), MotionPx::ZERO),
            motion_size(MotionPx::new(80.0), MotionPx::new(80.0)),
        );

        assert_eq!(
            preferred_motion_edge(touching_top_but_closer_left, container),
            MotionEdge::Top
        );
    }

    #[test]
    fn motion_source_rect_places_rect_outside_container_edge() {
        let container = motion_rect(
            motion_point(MotionPx::ZERO, MotionPx::ZERO),
            motion_size(MotionPx::new(400.0), MotionPx::new(240.0)),
        );
        let final_bounds = motion_rect(
            motion_point(MotionPx::new(40.0), MotionPx::new(20.0)),
            motion_size(MotionPx::new(80.0), MotionPx::new(60.0)),
        );

        assert_eq!(
            motion_source_rect(MotionEdge::Left, final_bounds, container),
            motion_rect(
                motion_point(MotionPx::new(-80.0), MotionPx::new(20.0)),
                final_bounds.size
            )
        );
        assert_eq!(
            motion_source_rect(MotionEdge::Bottom, final_bounds, container),
            motion_rect(
                motion_point(MotionPx::new(40.0), MotionPx::new(240.0)),
                final_bounds.size
            )
        );
    }

    #[test]
    fn reveal_and_lerp_rect_clamp_progress() {
        let rect = motion_rect(
            motion_point(MotionPx::new(10.0), MotionPx::new(20.0)),
            motion_size(MotionPx::new(100.0), MotionPx::new(80.0)),
        );

        assert_eq!(
            reveal_rect_from_edge(rect, MotionEdge::Right, 0.25),
            motion_rect(
                motion_point(MotionPx::new(85.0), MotionPx::new(20.0)),
                motion_size(MotionPx::new(25.0), MotionPx::new(80.0))
            )
        );
        assert_eq!(lerp_rect(rect, rect, 2.0), rect);
    }
}
