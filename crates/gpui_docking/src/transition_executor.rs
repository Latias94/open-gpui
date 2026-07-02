use crate::{
    DockNodeId, DropZone, SplitAxis,
    presentation_scene::DockPresentationScene,
    transition_geometry::{
        DockDividerTransition, DockDividerTransitionKind, DockOverlayTransition,
        DockOverlayTransitionKind, DockPaneTransition, DockPaneTransitionKind, DockSlideTransition,
        DockTransitionEdge, DockTransitionPlan,
    },
};
use open_gpui::{Bounds, Pixels, Window, point, px, size};
use open_gpui_ui_core::{
    MotionSnapshot, MotionSpec, MotionTimeline, MotionTimelineSample, retarget_motion_snapshots,
};
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockTransitionExecution {
    pub(crate) plan: DockTransitionPlan,
    pub(crate) spec: MotionSpec,
    pub(crate) state: DockTransitionExecutionState,
    timeline: MotionTimeline,
    last_sample: Option<DockTransitionSample>,
    #[cfg(test)]
    test_started_at: Option<Duration>,
}

/// Execution state returned by the docking transition executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockTransitionExecutionState {
    /// Transition reached the final scene immediately.
    Immediate,
    /// Transition requested an animation frame and kept the final scene as the semantic target.
    Scheduled,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct DockTransitionExecutor {
    current: Option<DockTransitionExecution>,
}

/// Render-time sample of the currently active docking transition.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockTransitionSample {
    pub(crate) final_scene: DockPresentationScene,
    pub(crate) progress: f32,
    pub(crate) complete: bool,
    pub(crate) needs_frame: bool,
    pub(crate) pane_bounds: Vec<DockPaneBoundsSample>,
    pub(crate) pane_clips: Vec<DockPaneClipSample>,
    pub(crate) dividers: Vec<DockDividerSample>,
    pub(crate) overlays: Vec<DockOverlaySample>,
}

/// Sampled visual bounds for a pane at the current transition frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPaneBoundsSample {
    pub(crate) node: DockNodeId,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) progress: f32,
}

/// Sampled visible area for a pane whose content is laid out at final size.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPaneClipSample {
    pub(crate) node: DockNodeId,
    pub(crate) content_bounds: Bounds<Pixels>,
    pub(crate) visible_bounds: Bounds<Pixels>,
    pub(crate) occlusion_bounds: Bounds<Pixels>,
    pub(crate) progress: f32,
}

/// Sampled divider geometry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDividerSample {
    pub(crate) split: DockNodeId,
    pub(crate) index: usize,
    pub(crate) axis: SplitAxis,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) progress: f32,
}

/// Sampled overlay transition geometry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockOverlaySample {
    pub(crate) kind: DockOverlayTransitionKind,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) target_node: Option<DockNodeId>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) payload_index: Option<usize>,
    pub(crate) progress: f32,
}

impl DockTransitionExecutor {
    pub(crate) fn execute(
        &mut self,
        plan: DockTransitionPlan,
        spec: MotionSpec,
        _window: Option<&Window>,
    ) -> &DockTransitionExecution {
        let state = if plan.is_immediate() || spec.is_immediate() {
            DockTransitionExecutionState::Immediate
        } else {
            DockTransitionExecutionState::Scheduled
        };

        let plan = if state == DockTransitionExecutionState::Scheduled {
            match self.sample_active_for_retarget() {
                Some(sample) => retarget_plan_from_sample(plan, &sample),
                None => plan,
            }
        } else {
            plan
        };

        self.current = Some(DockTransitionExecution {
            plan,
            spec,
            state,
            timeline: MotionTimeline::new(
                if state == DockTransitionExecutionState::Immediate {
                    MotionSpec::immediate()
                } else {
                    spec
                },
                Instant::now(),
            ),
            last_sample: None,
            #[cfg(test)]
            test_started_at: None,
        });
        self.current.as_ref().expect("execution should be stored")
    }

    fn sample_active_for_retarget(&mut self) -> Option<DockTransitionSample> {
        let execution = self.current.as_mut()?;
        if execution.state == DockTransitionExecutionState::Immediate {
            return None;
        }
        if let Some(sample) = execution
            .last_sample
            .as_ref()
            .filter(|sample| !sample.complete)
        {
            return Some(sample.clone());
        }

        let sample = sample_execution(execution, execution.timeline.sample(Instant::now()));
        execution.last_sample = Some(sample.clone());
        (!sample.complete).then_some(sample)
    }

    pub(crate) fn sample(&mut self, window: Option<&Window>) -> Option<DockTransitionSample> {
        let sample = self.sample_timeline(|execution| execution.timeline.sample(Instant::now()))?;
        if sample.needs_frame
            && let Some(window) = window
        {
            window.request_animation_frame();
        }
        Some(sample)
    }

    pub(crate) fn clear(&mut self) -> Option<DockTransitionExecution> {
        self.current.take()
    }

    #[cfg(test)]
    pub(crate) fn sample_for_test(&mut self, now: Duration) -> Option<DockTransitionSample> {
        self.sample_timeline(|execution| {
            let started_at = *execution.test_started_at.get_or_insert(now);
            MotionTimeline::sample_elapsed(
                execution.timeline.spec(),
                now.saturating_sub(started_at),
            )
        })
    }

    fn sample_timeline(
        &mut self,
        timeline_sample_for: impl FnOnce(&mut DockTransitionExecution) -> MotionTimelineSample,
    ) -> Option<DockTransitionSample> {
        let execution = self.current.as_mut()?;
        let timeline_sample = timeline_sample_for(execution);
        let sample = sample_execution(execution, timeline_sample);
        execution.last_sample = Some(sample.clone());
        if sample.complete {
            self.current = None;
        }
        Some(sample)
    }
}

fn sample_execution(
    execution: &DockTransitionExecution,
    timeline_sample: MotionTimelineSample,
) -> DockTransitionSample {
    let progress = timeline_sample.progress();
    let complete = execution.state == DockTransitionExecutionState::Immediate
        || timeline_sample.reached_final_state();
    DockTransitionSample {
        final_scene: execution.plan.final_scene.clone(),
        progress,
        complete,
        needs_frame: timeline_sample.is_active() && !complete,
        pane_bounds: pane_bounds_samples(&execution.plan, progress),
        pane_clips: execution
            .plan
            .pane_transitions
            .iter()
            .filter_map(|transition| pane_clip_sample(transition, progress))
            .collect(),
        dividers: execution
            .plan
            .divider_transitions
            .iter()
            .map(|transition| divider_sample(transition, progress))
            .collect(),
        overlays: execution
            .plan
            .overlay_transitions
            .iter()
            .map(|transition| overlay_sample(transition, progress))
            .collect(),
    }
}

fn pane_bounds_samples(plan: &DockTransitionPlan, progress: f32) -> Vec<DockPaneBoundsSample> {
    plan.pane_transitions
        .iter()
        .filter_map(|transition| {
            let bounds = pane_visual_bounds(transition, progress)?;
            Some(DockPaneBoundsSample {
                node: transition.node,
                bounds,
                progress,
            })
        })
        .collect()
}

fn pane_visual_bounds(transition: &DockPaneTransition, progress: f32) -> Option<Bounds<Pixels>> {
    match transition.kind {
        DockPaneTransitionKind::Entering => transition
            .slide
            .as_ref()
            .map(|slide| reveal_bounds(slide.final_bounds, slide.edge, progress)),
        DockPaneTransitionKind::Leaving => transition
            .slide
            .as_ref()
            .map(|slide| reveal_bounds(slide.final_bounds, slide.edge, 1.0 - progress)),
        DockPaneTransitionKind::Moving
        | DockPaneTransitionKind::Resizing
        | DockPaneTransitionKind::Unchanged => match (transition.from, transition.to) {
            (Some(from), Some(to)) => Some(lerp_bounds(from, to, progress)),
            (Some(bounds), None) | (None, Some(bounds)) => Some(bounds),
            (None, None) => None,
        },
    }
}

fn retarget_plan_from_sample(
    mut plan: DockTransitionPlan,
    sample: &DockTransitionSample,
) -> DockTransitionPlan {
    let pane_retargets = retarget_motion_snapshots(
        sample
            .pane_bounds
            .iter()
            .map(|pane| MotionSnapshot::new(pane.node, pane.bounds)),
        plan.pane_transitions
            .iter()
            .enumerate()
            .map(|(index, transition)| MotionSnapshot::new(transition.node, index)),
    );
    let scene_bounds = plan.final_scene.bounds;
    for retarget in pane_retargets.targets() {
        if let Some(bounds) = retarget.sampled().copied() {
            retarget_pane_transition(
                &mut plan.pane_transitions[*retarget.target()],
                bounds,
                scene_bounds,
            );
        }
    }

    let divider_retargets = retarget_motion_snapshots(
        sample
            .dividers
            .iter()
            .map(|divider| MotionSnapshot::new((divider.split, divider.index), divider.bounds)),
        plan.divider_transitions
            .iter()
            .enumerate()
            .map(|(index, transition)| {
                MotionSnapshot::new((transition.split, transition.index), index)
            }),
    );
    for retarget in divider_retargets.targets() {
        if let Some(bounds) = retarget.sampled().copied() {
            let transition = &mut plan.divider_transitions[*retarget.target()];
            transition.from = Some(bounds);
            transition.kind = if bounds == transition.to {
                DockDividerTransitionKind::Unchanged
            } else {
                DockDividerTransitionKind::Moving
            };
        }
    }

    let overlay_retargets = retarget_motion_snapshots(
        sample
            .overlays
            .iter()
            .map(|overlay| MotionSnapshot::new(overlay_key(overlay), overlay.bounds)),
        plan.overlay_transitions
            .iter()
            .enumerate()
            .map(|(index, transition)| MotionSnapshot::new(transition_key(transition), index)),
    );
    for retarget in overlay_retargets.targets() {
        let transition = &mut plan.overlay_transitions[*retarget.target()];
        if !transition.kind.animates_from_previous_bounds() {
            continue;
        }
        if let Some(bounds) = retarget.sampled().copied() {
            transition.from_bounds = Some(bounds);
        }
    }

    plan
}

fn retarget_pane_transition(
    transition: &mut DockPaneTransition,
    current_bounds: Bounds<Pixels>,
    scene_bounds: Bounds<Pixels>,
) {
    transition.from = Some(current_bounds);
    if matches!(transition.kind, DockPaneTransitionKind::Leaving) || transition.to.is_none() {
        let slide = slide_transition_from_bounds(current_bounds, scene_bounds);
        transition.to = Some(slide.source_bounds);
        transition.slide = Some(slide);
        transition.kind = DockPaneTransitionKind::Leaving;
        return;
    }

    let to = transition
        .to
        .expect("retargeted transition should have a final bound");
    transition.slide = None;
    transition.kind = if current_bounds == to {
        DockPaneTransitionKind::Unchanged
    } else if current_bounds.size != to.size {
        DockPaneTransitionKind::Resizing
    } else {
        DockPaneTransitionKind::Moving
    };
}

fn slide_transition_from_bounds(
    final_bounds: Bounds<Pixels>,
    scene_bounds: Bounds<Pixels>,
) -> DockSlideTransition {
    let edge = preferred_retarget_edge(final_bounds, scene_bounds);
    DockSlideTransition {
        edge,
        source_bounds: slide_source_bounds(edge, final_bounds, scene_bounds),
        final_bounds,
        occlusion_bounds: final_bounds,
    }
}

fn preferred_retarget_edge(
    bounds: Bounds<Pixels>,
    scene_bounds: Bounds<Pixels>,
) -> DockTransitionEdge {
    let left = f32::from((bounds.origin.x - scene_bounds.origin.x).abs());
    let right = f32::from((scene_bounds.right() - bounds.right()).abs());
    let top = f32::from((bounds.origin.y - scene_bounds.origin.y).abs());
    let bottom = f32::from((scene_bounds.bottom() - bounds.bottom()).abs());
    let touching_epsilon = 0.5_f32;

    if left <= touching_epsilon {
        return DockTransitionEdge::Left;
    }
    if right <= touching_epsilon {
        return DockTransitionEdge::Right;
    }
    if top <= touching_epsilon {
        return DockTransitionEdge::Top;
    }
    if bottom <= touching_epsilon {
        return DockTransitionEdge::Bottom;
    }

    [
        (DockTransitionEdge::Left, left),
        (DockTransitionEdge::Right, right),
        (DockTransitionEdge::Top, top),
        (DockTransitionEdge::Bottom, bottom),
    ]
    .into_iter()
    .min_by(|(_, a), (_, b)| a.total_cmp(b))
    .map(|(edge, _)| edge)
    .unwrap_or(DockTransitionEdge::Left)
}

fn slide_source_bounds(
    edge: DockTransitionEdge,
    final_bounds: Bounds<Pixels>,
    scene_bounds: Bounds<Pixels>,
) -> Bounds<Pixels> {
    let origin = match edge {
        DockTransitionEdge::Left => point(
            scene_bounds.origin.x - final_bounds.size.width,
            final_bounds.origin.y,
        ),
        DockTransitionEdge::Right => point(scene_bounds.right(), final_bounds.origin.y),
        DockTransitionEdge::Top => point(
            final_bounds.origin.x,
            scene_bounds.origin.y - final_bounds.size.height,
        ),
        DockTransitionEdge::Bottom => point(final_bounds.origin.x, scene_bounds.bottom()),
    };
    Bounds::new(origin, final_bounds.size)
}

fn pane_clip_sample(transition: &DockPaneTransition, progress: f32) -> Option<DockPaneClipSample> {
    let slide = transition.slide.as_ref()?;
    match transition.kind {
        DockPaneTransitionKind::Entering => Some(DockPaneClipSample {
            node: transition.node,
            content_bounds: slide.final_bounds,
            visible_bounds: reveal_bounds(slide.final_bounds, slide.edge, progress),
            occlusion_bounds: slide.occlusion_bounds,
            progress,
        }),
        DockPaneTransitionKind::Leaving => Some(DockPaneClipSample {
            node: transition.node,
            content_bounds: slide.final_bounds,
            visible_bounds: reveal_bounds(slide.final_bounds, slide.edge, 1.0 - progress),
            occlusion_bounds: slide.occlusion_bounds,
            progress,
        }),
        DockPaneTransitionKind::Moving
        | DockPaneTransitionKind::Resizing
        | DockPaneTransitionKind::Unchanged => None,
    }
}

fn divider_sample(transition: &DockDividerTransition, progress: f32) -> DockDividerSample {
    let bounds = match (transition.kind, transition.from) {
        (DockDividerTransitionKind::Appearing, None) => {
            appearing_divider_bounds(transition.to, transition.axis, progress)
        }
        (_, Some(from)) => lerp_bounds(from, transition.to, progress),
        _ => transition.to,
    };
    DockDividerSample {
        split: transition.split,
        index: transition.index,
        axis: transition.axis,
        bounds,
        progress,
    }
}

fn overlay_sample(transition: &DockOverlayTransition, progress: f32) -> DockOverlaySample {
    DockOverlaySample {
        kind: transition.kind,
        bounds: transition
            .from_bounds
            .map(|from| lerp_bounds(from, transition.bounds, progress))
            .unwrap_or(transition.bounds),
        target_node: transition.target_node,
        zone: transition.zone,
        payload_index: transition.payload_index,
        progress,
    }
}

fn transition_key(
    transition: &DockOverlayTransition,
) -> (
    DockOverlayTransitionKind,
    Option<DockNodeId>,
    Option<DropZone>,
    Option<usize>,
) {
    (
        transition.kind,
        transition.target_node,
        transition.zone,
        transition.payload_index,
    )
}

fn overlay_key(
    overlay: &DockOverlaySample,
) -> (
    DockOverlayTransitionKind,
    Option<DockNodeId>,
    Option<DropZone>,
    Option<usize>,
) {
    (
        overlay.kind,
        overlay.target_node,
        overlay.zone,
        overlay.payload_index,
    )
}

fn reveal_bounds(
    final_bounds: Bounds<Pixels>,
    edge: DockTransitionEdge,
    progress: f32,
) -> Bounds<Pixels> {
    let progress = progress.clamp(0.0, 1.0);
    match edge {
        DockTransitionEdge::Left => {
            let width = final_bounds.size.width * progress;
            Bounds::new(final_bounds.origin, size(width, final_bounds.size.height))
        }
        DockTransitionEdge::Right => {
            let width = final_bounds.size.width * progress;
            Bounds::new(
                point(final_bounds.right() - width, final_bounds.origin.y),
                size(width, final_bounds.size.height),
            )
        }
        DockTransitionEdge::Top => {
            let height = final_bounds.size.height * progress;
            Bounds::new(final_bounds.origin, size(final_bounds.size.width, height))
        }
        DockTransitionEdge::Bottom => {
            let height = final_bounds.size.height * progress;
            Bounds::new(
                point(final_bounds.origin.x, final_bounds.bottom() - height),
                size(final_bounds.size.width, height),
            )
        }
    }
}

fn appearing_divider_bounds(
    final_bounds: Bounds<Pixels>,
    axis: SplitAxis,
    progress: f32,
) -> Bounds<Pixels> {
    let progress = progress.clamp(0.0, 1.0);
    match axis {
        SplitAxis::Horizontal => {
            let height = final_bounds.size.height * progress;
            Bounds::new(
                point(
                    final_bounds.origin.x,
                    final_bounds.origin.y + (final_bounds.size.height - height) / 2.0,
                ),
                size(final_bounds.size.width, height),
            )
        }
        SplitAxis::Vertical => {
            let width = final_bounds.size.width * progress;
            Bounds::new(
                point(
                    final_bounds.origin.x + (final_bounds.size.width - width) / 2.0,
                    final_bounds.origin.y,
                ),
                size(width, final_bounds.size.height),
            )
        }
    }
}

fn lerp_bounds(from: Bounds<Pixels>, to: Bounds<Pixels>, progress: f32) -> Bounds<Pixels> {
    Bounds::new(
        point(
            lerp_pixels(from.origin.x, to.origin.x, progress),
            lerp_pixels(from.origin.y, to.origin.y, progress),
        ),
        size(
            lerp_pixels(from.size.width, to.size.width, progress),
            lerp_pixels(from.size.height, to.size.height, progress),
        ),
    )
}

fn lerp_pixels(from: Pixels, to: Pixels, progress: f32) -> Pixels {
    px(f32::from(from) + (f32::from(to) - f32::from(from)) * progress.clamp(0.0, 1.0))
}
