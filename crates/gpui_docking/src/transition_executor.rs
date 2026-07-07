use crate::{
    DockNodeId, DropZone, SplitAxis,
    geometry::{bounds_from_motion_rect, motion_rect_from_bounds},
    presentation_scene::DockPresentationScene,
    transition_geometry::{
        DockDividerTransition, DockDividerTransitionKind, DockPaneTransition,
        DockPaneTransitionKind, DockSlideTransition, DockTransitionEdge, DockTransitionPlan,
        DockVisualAffordanceTransition, DockVisualAffordanceTransitionKind,
    },
    visual_affordance_scene::DockVisualAffordanceId,
};
use open_gpui::{Bounds, Pixels, point, size};
use open_gpui_motion::{
    MotionExecutionPlan, MotionExecutionState, MotionFrameDemand, MotionModel, MotionPolicyContext,
    MotionPolicyInput, MotionPolicyReport, MotionProgressExecution, MotionProgressSample,
    MotionProjection, MotionProjectionClip, MotionSnapshot, MotionSpec, motion_source_rect,
    preferred_motion_edge, retarget_motion_snapshots, reveal_rect_from_edge,
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockTransitionExecution {
    pub(crate) plan: DockTransitionPlan,
    pub(crate) model: MotionModel,
    pub(crate) policy_report: MotionPolicyReport,
    pub(crate) state: DockTransitionExecutionState,
    progress: MotionProgressExecution,
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

impl From<MotionExecutionState> for DockTransitionExecutionState {
    fn from(state: MotionExecutionState) -> Self {
        match state {
            MotionExecutionState::Immediate => Self::Immediate,
            MotionExecutionState::Scheduled => Self::Scheduled,
        }
    }
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
    pub(crate) frame_demand: MotionFrameDemand,
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
    ) -> &DockTransitionExecution {
        self.execute_model(plan, MotionModel::timeline(spec))
    }

    pub(crate) fn execute_model(
        &mut self,
        plan: DockTransitionPlan,
        model: MotionModel,
    ) -> &DockTransitionExecution {
        let mut motion = MotionExecutionPlan::resolve(
            MotionPolicyInput::new(MotionPolicyContext::Continuity, model)
                .with_spatial_motion(!plan.is_immediate() && !model.is_immediate())
                .with_reduced_motion_final_state(true),
        );
        if plan.is_immediate() {
            motion = MotionExecutionPlan::resolve(
                MotionPolicyInput::new(
                    MotionPolicyContext::Continuity,
                    MotionModel::timeline(MotionSpec::immediate()),
                )
                .with_reduced_motion_final_state(true),
            );
        }
        let state = if plan.is_immediate() {
            DockTransitionExecutionState::Immediate
        } else {
            DockTransitionExecutionState::from(motion.state())
        };
        let model = motion.model();
        let policy_report = motion.policy_report().clone();

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
            progress: MotionProgressExecution::start(motion, Instant::now()),
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

        let sample = sample_execution(execution, execution.progress.sample_since(Instant::now()));
        execution.last_sample = Some(sample.clone());
        (!sample.complete).then_some(sample)
    }

    pub(crate) fn sample(&mut self) -> Option<DockTransitionSample> {
        self.sample_motion(|execution| execution.progress.sample_since(Instant::now()))
    }

    pub(crate) fn clear(&mut self) -> Option<DockTransitionExecution> {
        self.current.take()
    }

    #[cfg(test)]
    pub(crate) fn sample_for_test(&mut self, now: Duration) -> Option<DockTransitionSample> {
        self.sample_motion(|execution| {
            let started_at = *execution.test_started_at.get_or_insert(now);
            execution.progress.sample_at(now.saturating_sub(started_at))
        })
    }

    fn sample_motion(
        &mut self,
        motion_sample_for: impl FnOnce(&mut DockTransitionExecution) -> MotionProgressSample,
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
    motion_sample: MotionProgressSample,
) -> DockTransitionSample {
    let progress = motion_sample.progress();
    let complete =
        execution.state == DockTransitionExecutionState::Immediate || motion_sample.complete();
    let frame_demand = motion_sample.frame_demand();
    DockTransitionSample {
        final_scene: execution.plan.final_scene.clone(),
        progress,
        complete,
        frame_demand,
        needs_frame: frame_demand.needs_frame(),
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
        motion_rect_from_bounds(final_bounds),
        motion_rect_from_bounds(scene_bounds),
    );
    DockSlideTransition {
        edge,
        source_bounds: bounds_from_motion_rect(motion_source_rect(
            edge,
            motion_rect_from_bounds(final_bounds),
            motion_rect_from_bounds(scene_bounds),
        )),
        final_bounds,
        occlusion_bounds: final_bounds,
    }
}

fn pane_clip_sample(transition: &DockPaneTransition, progress: f32) -> Option<DockPaneClipSample> {
    match transition.kind {
        DockPaneTransitionKind::Entering => {
            let slide = transition.slide.as_ref()?;
            Some(dock_pane_clip_sample(
                transition.node,
                MotionProjectionClip::new(
                    motion_rect_from_bounds(slide.final_bounds),
                    motion_rect_from_bounds(reveal_bounds(
                        slide.final_bounds,
                        slide.edge,
                        progress,
                    )),
                    motion_rect_from_bounds(slide.occlusion_bounds),
                    progress,
                ),
            ))
        }
        DockPaneTransitionKind::Leaving => {
            let slide = transition.slide.as_ref()?;
            Some(dock_pane_clip_sample(
                transition.node,
                MotionProjectionClip::new(
                    motion_rect_from_bounds(slide.final_bounds),
                    motion_rect_from_bounds(reveal_bounds(
                        slide.final_bounds,
                        slide.edge,
                        1.0 - progress,
                    )),
                    motion_rect_from_bounds(slide.occlusion_bounds),
                    progress,
                ),
            ))
        }
        DockPaneTransitionKind::Moving | DockPaneTransitionKind::Resizing => {
            let from = transition.from?;
            let to = transition.to?;
            Some(dock_pane_clip_sample(
                transition.node,
                MotionProjectionClip::from_projection(
                    MotionProjection::between(
                        motion_rect_from_bounds(from),
                        motion_rect_from_bounds(to),
                    ),
                    progress,
                ),
            ))
        }
        DockPaneTransitionKind::Unchanged => None,
    }
}

fn dock_pane_clip_sample(node: DockNodeId, clip: MotionProjectionClip) -> DockPaneClipSample {
    DockPaneClipSample {
        node,
        content_bounds: bounds_from_motion_rect(clip.content_bounds()),
        visible_bounds: bounds_from_motion_rect(clip.visible_bounds()),
        occlusion_bounds: bounds_from_motion_rect(clip.occlusion_bounds()),
        progress: clip.progress(),
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
    bounds_from_motion_rect(reveal_rect_from_edge(
        motion_rect_from_bounds(final_bounds),
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
    bounds_from_motion_rect(
        MotionProjection::between(motion_rect_from_bounds(from), motion_rect_from_bounds(to))
            .visual_bounds(progress),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockSpaceId,
        presentation_scene::{DockPresentationPane, DockPresentationPaneKind},
    };
    use open_gpui::{point, px, size};
    use open_gpui_motion::{
        MotionDuration, MotionEasing, MotionFrameReason, MotionPreference, MotionSpec,
    };

    fn node(id: u64) -> DockNodeId {
        DockNodeId::from(slotmap::KeyData::from_ffi(id))
    }

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    fn scene(node: DockNodeId, bounds: Bounds<Pixels>) -> DockPresentationScene {
        DockPresentationScene {
            space: DockSpaceId::from("main"),
            bounds,
            root: Some(node),
            panes: vec![DockPresentationPane {
                node: Some(node),
                kind: DockPresentationPaneKind::Tabs,
                bounds,
                floating: None,
                is_central: false,
            }],
            tab_bars: Vec::new(),
            tab_labels: Vec::new(),
            splitters: Vec::new(),
            floating_containers: Vec::new(),
            focus_regions: Vec::new(),
            overlay_anchors: Vec::new(),
        }
    }

    fn resizing_plan(preference: MotionPreference) -> DockTransitionPlan {
        let node = node(1);
        let previous = scene(node, bounds(0.0, 0.0, 100.0, 100.0));
        let next = scene(node, bounds(0.0, 0.0, 200.0, 100.0));
        DockTransitionPlan::between(&previous, &next, preference)
    }

    #[test]
    fn animated_samples_publish_frame_demand_until_terminal() {
        let mut executor = DockTransitionExecutor::default();
        let spec = MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(200)),
            MotionEasing::Linear,
        );

        assert_eq!(
            executor
                .execute(resizing_plan(MotionPreference::Animated), spec)
                .state,
            DockTransitionExecutionState::Scheduled
        );

        let start = executor
            .sample_for_test(Duration::ZERO)
            .expect("scheduled transition should expose a start sample");
        assert_eq!(
            start.frame_demand,
            MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender)
        );
        assert!(start.needs_frame);
        assert!(!start.complete);

        let midpoint = executor
            .sample_for_test(Duration::from_millis(100))
            .expect("scheduled transition should expose a midpoint sample");
        assert_eq!(
            midpoint.frame_demand,
            MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender)
        );
        assert!(midpoint.needs_frame);
        assert!(!midpoint.complete);

        let terminal = executor
            .sample_for_test(Duration::from_millis(250))
            .expect("scheduled transition should expose one terminal sample");
        assert_eq!(terminal.frame_demand, MotionFrameDemand::Idle);
        assert!(!terminal.needs_frame);
        assert!(terminal.complete);
        assert!(
            executor
                .sample_for_test(Duration::from_millis(260))
                .is_none(),
            "completed transition should clear after the terminal sample"
        );
    }

    #[test]
    fn immediate_samples_publish_idle_frame_demand() {
        let mut executor = DockTransitionExecutor::default();

        assert_eq!(
            executor
                .execute(
                    resizing_plan(MotionPreference::Reduced),
                    MotionSpec::immediate()
                )
                .state,
            DockTransitionExecutionState::Immediate
        );

        let sample = executor
            .sample_for_test(Duration::ZERO)
            .expect("immediate transition should expose one final sample");
        assert_eq!(sample.frame_demand, MotionFrameDemand::Idle);
        assert!(!sample.needs_frame);
        assert!(sample.complete);
        assert_eq!(sample.progress, 1.0);
        assert!(
            executor.sample_for_test(Duration::from_millis(1)).is_none(),
            "immediate transition should clear after the final sample"
        );
    }
}
