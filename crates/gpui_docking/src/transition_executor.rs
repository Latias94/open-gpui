use crate::{
    DockNodeId, DropZone, SplitAxis,
    presentation_scene::DockPresentationScene,
    transition_geometry::{
        DockDividerTransition, DockDividerTransitionKind, DockOverlayTransition,
        DockOverlayTransitionKind, DockPaneTransition, DockPaneTransitionKind, DockTransitionEdge,
        DockTransitionPlan,
    },
};
use open_gpui::{Bounds, Pixels, Window, point, px, size};
use open_gpui_ui_core::MotionSpec;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockTransitionExecution {
    pub(crate) plan: DockTransitionPlan,
    pub(crate) spec: MotionSpec,
    pub(crate) state: DockTransitionExecutionState,
    retarget_start_progress: f32,
    last_sample: Option<DockTransitionSample>,
    started_at: Option<Instant>,
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
    pub(crate) pane_clips: Vec<DockPaneClipSample>,
    pub(crate) dividers: Vec<DockDividerSample>,
    pub(crate) overlays: Vec<DockOverlaySample>,
}

/// Sampled visible area for a pane whose content is laid out at final size.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPaneClipSample {
    pub(crate) node: DockNodeId,
    pub(crate) content_bounds: Bounds<Pixels>,
    pub(crate) visible_bounds: Bounds<Pixels>,
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
        let retarget_start_progress = if state == DockTransitionExecutionState::Scheduled {
            self.current
                .as_ref()
                .and_then(|execution| execution.last_sample.as_ref())
                .filter(|sample| !sample.complete)
                .map(|sample| sample.progress)
                .unwrap_or(0.0)
        } else {
            0.0
        };

        self.current = Some(DockTransitionExecution {
            plan,
            spec,
            state,
            retarget_start_progress,
            last_sample: None,
            started_at: if state == DockTransitionExecutionState::Scheduled {
                Some(Instant::now())
            } else {
                None
            },
            #[cfg(test)]
            test_started_at: None,
        });
        self.current.as_ref().expect("execution should be stored")
    }

    pub(crate) fn sample(&mut self, window: Option<&Window>) -> Option<DockTransitionSample> {
        let sample = self.sample_elapsed(|execution| {
            execution
                .started_at
                .map(|started_at| started_at.elapsed())
                .unwrap_or(Duration::ZERO)
        })?;
        if sample.needs_frame
            && let Some(window) = window
        {
            window.request_animation_frame();
        }
        Some(sample)
    }

    #[cfg(test)]
    pub(crate) fn clear(&mut self) -> Option<DockTransitionExecution> {
        self.current.take()
    }

    #[cfg(test)]
    pub(crate) fn sample_for_test(&mut self, now: Duration) -> Option<DockTransitionSample> {
        self.sample_elapsed(|execution| {
            let started_at = *execution.test_started_at.get_or_insert(now);
            now.saturating_sub(started_at)
        })
    }

    fn sample_elapsed(
        &mut self,
        elapsed_for: impl FnOnce(&mut DockTransitionExecution) -> Duration,
    ) -> Option<DockTransitionSample> {
        let execution = self.current.as_mut()?;
        let elapsed = elapsed_for(execution);
        let sample = sample_execution(execution, elapsed);
        execution.last_sample = Some(sample.clone());
        if sample.complete {
            self.current = None;
        }
        Some(sample)
    }
}

fn sample_execution(
    execution: &DockTransitionExecution,
    elapsed: Duration,
) -> DockTransitionSample {
    let progress = transition_progress(
        execution.spec,
        execution.state,
        execution.retarget_start_progress,
        elapsed,
    );
    let complete = execution.state == DockTransitionExecutionState::Immediate || progress >= 1.0;
    DockTransitionSample {
        final_scene: execution.plan.final_scene.clone(),
        progress,
        complete,
        needs_frame: !complete,
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

fn transition_progress(
    spec: MotionSpec,
    state: DockTransitionExecutionState,
    retarget_start_progress: f32,
    elapsed: Duration,
) -> f32 {
    if state == DockTransitionExecutionState::Immediate || spec.is_immediate() {
        return 1.0;
    }
    let progress = spec.progress_at(elapsed);
    retarget_start_progress + ((1.0 - retarget_start_progress) * progress)
}

fn pane_clip_sample(transition: &DockPaneTransition, progress: f32) -> Option<DockPaneClipSample> {
    let slide = transition.slide.as_ref()?;
    match transition.kind {
        DockPaneTransitionKind::Entering => Some(DockPaneClipSample {
            node: transition.node,
            content_bounds: slide.final_bounds,
            visible_bounds: reveal_bounds(slide.final_bounds, slide.edge, progress),
            progress,
        }),
        DockPaneTransitionKind::Leaving => Some(DockPaneClipSample {
            node: transition.node,
            content_bounds: slide.final_bounds,
            visible_bounds: reveal_bounds(slide.final_bounds, slide.edge, 1.0 - progress),
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
        bounds: transition.bounds,
        target_node: transition.target_node,
        zone: transition.zone,
        payload_index: transition.payload_index,
        progress,
    }
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
