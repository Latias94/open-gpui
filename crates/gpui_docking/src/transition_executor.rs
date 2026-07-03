use crate::{
    DockNodeId, DropZone, SplitAxis,
    geometry::{bounds_from_ui_rect, ui_rect_from_bounds},
    presentation_scene::DockPresentationScene,
    transition_geometry::{
        DockDividerTransition, DockDividerTransitionKind, DockPaneTransition,
        DockPaneTransitionKind, DockSlideTransition, DockTransitionEdge, DockTransitionPlan,
        DockVisualAffordanceTransition, DockVisualAffordanceTransitionKind,
    },
    visual_affordance_scene::DockVisualAffordanceId,
};
use open_gpui::{Bounds, Pixels, Window, point, size};
use open_gpui_ui_core::{
    MotionModel, MotionPolicyContext, MotionPolicyInput, MotionPolicyReport, MotionProjection,
    MotionScalarSample, MotionScalarTrack, MotionSnapshot, MotionSpec, motion_source_rect,
    preferred_motion_edge, retarget_motion_snapshots, reveal_rect_from_edge,
    validate_motion_policy,
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockTransitionExecution {
    pub(crate) plan: DockTransitionPlan,
    pub(crate) model: MotionModel,
    pub(crate) policy_report: MotionPolicyReport,
    pub(crate) state: DockTransitionExecutionState,
    track: MotionScalarTrack,
    started_at: Instant,
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
    pub(crate) visual_affordances: Vec<DockVisualAffordanceSample>,
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

/// Sampled visual affordance transition geometry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockVisualAffordanceSample {
    pub(crate) motion_key: DockVisualAffordanceId,
    pub(crate) kind: DockVisualAffordanceTransitionKind,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) target_node: Option<DockNodeId>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) payload_index: Option<usize>,
    pub(crate) progress: f32,
}

impl DockTransitionExecutor {
    pub(crate) fn current_state_for_debug(&self) -> Option<DockTransitionExecutionState> {
        self.current.as_ref().map(|execution| execution.state)
    }

    pub(crate) fn execute(
        &mut self,
        plan: DockTransitionPlan,
        spec: MotionSpec,
        window: Option<&Window>,
    ) -> &DockTransitionExecution {
        self.execute_model(plan, MotionModel::timeline(spec), window)
    }

    pub(crate) fn execute_model(
        &mut self,
        plan: DockTransitionPlan,
        model: MotionModel,
        _window: Option<&Window>,
    ) -> &DockTransitionExecution {
        let policy_report = validate_motion_policy(
            MotionPolicyInput::new(MotionPolicyContext::Continuity, model)
                .with_spatial_motion(!plan.is_immediate() && !model.is_immediate())
                .with_reduced_motion_final_state(true),
        );
        let model = if policy_report.is_ok() {
            model
        } else {
            MotionModel::timeline(MotionSpec::immediate())
        };
        let state = if plan.is_immediate() || model.is_immediate() {
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
            model,
            policy_report,
            state,
            track: MotionScalarTrack::start(model, 0.0, 1.0, 0.0, Duration::ZERO),
            started_at: Instant::now(),
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

        let sample = sample_execution(
            execution,
            execution
                .track
                .sample_at(Instant::now().saturating_duration_since(execution.started_at)),
        );
        execution.last_sample = Some(sample.clone());
        (!sample.complete).then_some(sample)
    }

    pub(crate) fn sample(&mut self, window: Option<&Window>) -> Option<DockTransitionSample> {
        let sample = self.sample_motion(|execution| {
            execution
                .track
                .sample_at(Instant::now().saturating_duration_since(execution.started_at))
        })?;
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
        self.sample_motion(|execution| {
            let started_at = *execution.test_started_at.get_or_insert(now);
            execution.track.sample_at(now.saturating_sub(started_at))
        })
    }

    fn sample_motion(
        &mut self,
        motion_sample_for: impl FnOnce(&mut DockTransitionExecution) -> MotionScalarSample,
    ) -> Option<DockTransitionSample> {
        let execution = self.current.as_mut()?;
        let motion_sample = motion_sample_for(execution);
        let sample = sample_execution(execution, motion_sample);
        execution.last_sample = Some(sample.clone());
        if sample.complete {
            self.current = None;
        }
        Some(sample)
    }
}

fn sample_execution(
    execution: &DockTransitionExecution,
    motion_sample: MotionScalarSample,
) -> DockTransitionSample {
    let progress = motion_sample.value().clamp(0.0, 1.0);
    let complete = execution.state == DockTransitionExecutionState::Immediate
        || motion_sample.reached_final_state();
    DockTransitionSample {
        final_scene: execution.plan.final_scene.clone(),
        progress,
        complete,
        needs_frame: motion_sample.is_active() && !complete,
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
        visual_affordances: execution
            .plan
            .visual_affordance_transitions
            .iter()
            .map(|transition| visual_affordance_sample(transition, progress))
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
            (Some(from), Some(to)) => Some(projected_visual_bounds(from, to, progress)),
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
    let edge = preferred_motion_edge(
        ui_rect_from_bounds(final_bounds),
        ui_rect_from_bounds(scene_bounds),
    );
    DockSlideTransition {
        edge,
        source_bounds: bounds_from_ui_rect(motion_source_rect(
            edge,
            ui_rect_from_bounds(final_bounds),
            ui_rect_from_bounds(scene_bounds),
        )),
        final_bounds,
        occlusion_bounds: final_bounds,
    }
}

fn pane_clip_sample(transition: &DockPaneTransition, progress: f32) -> Option<DockPaneClipSample> {
    match transition.kind {
        DockPaneTransitionKind::Entering => {
            let slide = transition.slide.as_ref()?;
            Some(DockPaneClipSample {
                node: transition.node,
                content_bounds: slide.final_bounds,
                visible_bounds: reveal_bounds(slide.final_bounds, slide.edge, progress),
                occlusion_bounds: slide.occlusion_bounds,
                progress,
            })
        }
        DockPaneTransitionKind::Leaving => {
            let slide = transition.slide.as_ref()?;
            Some(DockPaneClipSample {
                node: transition.node,
                content_bounds: slide.final_bounds,
                visible_bounds: reveal_bounds(slide.final_bounds, slide.edge, 1.0 - progress),
                occlusion_bounds: slide.occlusion_bounds,
                progress,
            })
        }
        DockPaneTransitionKind::Moving | DockPaneTransitionKind::Resizing => {
            let from = transition.from?;
            let to = transition.to?;
            Some(DockPaneClipSample {
                node: transition.node,
                content_bounds: to,
                visible_bounds: projected_visual_bounds(from, to, progress),
                occlusion_bounds: to,
                progress,
            })
        }
        DockPaneTransitionKind::Unchanged => None,
    }
}

fn divider_sample(transition: &DockDividerTransition, progress: f32) -> DockDividerSample {
    let bounds = match (transition.kind, transition.from) {
        (DockDividerTransitionKind::Appearing, None) => {
            appearing_divider_bounds(transition.to, transition.axis, progress)
        }
        (_, Some(from)) => projected_visual_bounds(from, transition.to, progress),
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

fn visual_affordance_sample(
    transition: &DockVisualAffordanceTransition,
    progress: f32,
) -> DockVisualAffordanceSample {
    DockVisualAffordanceSample {
        motion_key: transition.motion_key.clone(),
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
    bounds_from_ui_rect(reveal_rect_from_edge(
        ui_rect_from_bounds(final_bounds),
        edge,
        progress,
    ))
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

fn projected_visual_bounds(
    from: Bounds<Pixels>,
    to: Bounds<Pixels>,
    progress: f32,
) -> Bounds<Pixels> {
    bounds_from_ui_rect(
        MotionProjection::between(ui_rect_from_bounds(from), ui_rect_from_bounds(to))
            .sample(progress)
            .visual_bounds(),
    )
}
