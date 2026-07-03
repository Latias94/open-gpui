//! Splitter component.

use crate::geometry::gpui_px_from_ui;
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, Context, CursorStyle, DefiniteLength, DragMoveEvent, ElementId, Empty, Entity,
    IntoElement, ParentElement, Pixels, Point, Render, RenderOnce, Styled, Window, div, px,
    relative, rgb,
};
use open_gpui_ui_core::{
    MotionModel, MotionPolicyContext, MotionPolicyInput, MotionPreference, MotionPreset,
    MotionScalarController, Orientation, Sizable, Size, validate_motion_policy,
};
use std::time::{Duration, Instant};

const EPSILON: f32 = 0.000_1;

pub use open_gpui_ui_core::{
    SplitterHandleLayout, SplitterHandleState, SplitterHitMap, SplitterHitTarget,
    SplitterJunctionHitRegion, SplitterLayoutScene, SplitterMetrics, SplitterPanelDescriptor,
    SplitterPanelLayout, SplitterPanelState, SplitterResizeOutcome, SplitterResizeResult,
    SplitterState,
};

#[derive(Debug, Clone, Default)]
struct SplitterRuntime {
    panel_ids: Vec<String>,
    state_fractions: Vec<f32>,
    panel_fractions: Vec<f32>,
    drag_start: Option<SplitterDragStart>,
    transition: Option<SplitterRuntimeTransition>,
}

impl SplitterRuntime {
    #[cfg(test)]
    fn sync_at(&mut self, state: &SplitterState, now: Instant) -> bool {
        self.sync_at_with_model(
            state,
            now,
            MotionPreset::committed_layout(MotionPreference::Animated).resolve_model(),
        )
    }

    fn sync_at_with_model(
        &mut self,
        state: &SplitterState,
        now: Instant,
        model: MotionModel,
    ) -> bool {
        let panel_ids = state
            .panels()
            .iter()
            .map(|panel| panel.id().to_owned())
            .collect::<Vec<_>>();
        let target_fractions = state
            .panels()
            .iter()
            .map(SplitterPanelState::fraction)
            .collect::<Vec<_>>();

        if self.panel_ids.is_empty()
            || self.panel_ids != panel_ids
            || self.panel_fractions.len() != target_fractions.len()
        {
            self.sync_immediate(panel_ids, target_fractions);
            return false;
        }

        if let Some(transition) = self.transition.as_ref()
            && transition.panel_ids == panel_ids
            && fractions_equal(&transition.to_fractions, &target_fractions)
        {
            let complete = self.sample_transition(now);
            return !complete;
        }

        if fractions_equal(&self.state_fractions, &target_fractions) {
            self.transition = None;
            return false;
        }

        let from_fractions = self
            .transition
            .as_ref()
            .map(|_| self.sampled_transition_fractions(now))
            .unwrap_or_else(|| self.panel_fractions.clone());
        if fractions_equal(&from_fractions, &target_fractions) {
            self.state_fractions = target_fractions;
            self.panel_fractions = from_fractions;
            self.transition = None;
            return false;
        }
        let policy_report = validate_motion_policy(
            MotionPolicyInput::new(MotionPolicyContext::CommittedLayout, model)
                .with_spatial_motion(!model.is_immediate())
                .with_reduced_motion_final_state(true),
        );
        if model.is_immediate() || !policy_report.is_ok() {
            self.state_fractions = target_fractions.clone();
            self.panel_fractions = target_fractions;
            self.transition = None;
            return false;
        }
        self.panel_fractions = from_fractions.clone();
        self.state_fractions = target_fractions.clone();
        self.transition = Some(SplitterRuntimeTransition {
            panel_ids: panel_ids.clone(),
            to_fractions: target_fractions.clone(),
            started_at: now,
            controller: scalar_controller_for_fractions(
                panel_ids,
                from_fractions,
                target_fractions,
                model,
            ),
        });
        true
    }

    fn sync_immediate(&mut self, panel_ids: Vec<String>, panel_fractions: Vec<f32>) {
        self.panel_ids = panel_ids;
        self.state_fractions = panel_fractions.clone();
        self.panel_fractions = panel_fractions;
        self.drag_start = None;
        self.transition = None;
    }

    fn sync_drag_state(&mut self, state: &SplitterState) {
        let panel_ids = state
            .panels()
            .iter()
            .map(|panel| panel.id().to_owned())
            .collect::<Vec<_>>();
        let panel_fractions = state
            .panels()
            .iter()
            .map(SplitterPanelState::fraction)
            .collect::<Vec<_>>();

        if self.panel_ids != panel_ids || self.panel_fractions.len() != panel_fractions.len() {
            self.sync_immediate(panel_ids, panel_fractions);
            return;
        }

        self.transition = None;
    }

    fn sample_transition(&mut self, now: Instant) -> bool {
        let Some(transition) = self.transition.as_ref() else {
            return true;
        };
        let sample = transition
            .controller
            .sample_at(now.saturating_duration_since(transition.started_at));
        self.panel_fractions = fraction_samples_for_transition(transition, &sample);
        let complete = !sample.frame_demand().needs_frame();
        if complete {
            self.panel_fractions = transition.to_fractions.clone();
            self.transition = None;
        }
        complete
    }

    fn sampled_transition_fractions(&self, now: Instant) -> Vec<f32> {
        let Some(transition) = self.transition.as_ref() else {
            return self.panel_fractions.clone();
        };
        let sample = transition
            .controller
            .sample_at(now.saturating_duration_since(transition.started_at));
        fraction_samples_for_transition(transition, &sample)
    }
}

#[derive(Debug, Clone)]
struct SplitterRuntimeTransition {
    panel_ids: Vec<String>,
    to_fractions: Vec<f32>,
    started_at: Instant,
    controller: MotionScalarController<String>,
}

fn fractions_equal(left: &[f32], right: &[f32]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| (left - right).abs() <= EPSILON)
}

fn scalar_controller_for_fractions(
    panel_ids: Vec<String>,
    from_fractions: Vec<f32>,
    to_fractions: Vec<f32>,
    model: MotionModel,
) -> MotionScalarController<String> {
    let mut controller = MotionScalarController::new();
    for ((panel_id, from), to) in panel_ids.into_iter().zip(from_fractions).zip(to_fractions) {
        controller.start(panel_id, model, from, to, 0.0, Duration::ZERO);
    }
    controller
}

fn fraction_samples_for_transition(
    transition: &SplitterRuntimeTransition,
    sample: &open_gpui_ui_core::MotionScalarControllerSample<String>,
) -> Vec<f32> {
    transition
        .panel_ids
        .iter()
        .zip(&transition.to_fractions)
        .map(|(panel_id, target)| {
            sample
                .track(panel_id)
                .map(|track| track.sample().value())
                .unwrap_or(*target)
        })
        .collect()
}

#[derive(Debug, Clone)]
struct SplitterDragStart {
    origin: Point<Pixels>,
    origin_fractions: Vec<f32>,
    axis_length: Pixels,
}

#[derive(Clone)]
struct SplitterDrag {
    group_id: String,
    handle_index: usize,
}

#[derive(Clone)]
struct SplitterDragPreview;

impl Render for SplitterDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        Empty
    }
}

/// A concrete GPUI splitter panel.
#[derive(IntoElement)]
pub struct SplitterPanel {
    descriptor: SplitterPanelDescriptor,
    content: AnyElement,
}

impl SplitterPanel {
    /// Creates a splitter panel.
    pub fn new(descriptor: SplitterPanelDescriptor, content: impl IntoElement) -> Self {
        Self {
            descriptor,
            content: content.into_any_element(),
        }
    }

    /// Returns the panel descriptor.
    pub fn descriptor(&self) -> SplitterPanelDescriptor {
        self.descriptor.clone()
    }
}

impl RenderOnce for SplitterPanel {
    fn render(self, _: &mut open_gpui::Window, _: &mut open_gpui::App) -> impl IntoElement {
        self.content
    }
}

/// A concrete GPUI splitter component.
#[derive(IntoElement)]
pub struct Splitter {
    id: ElementId,
    group_id: String,
    orientation: Orientation,
    size: Size,
    disabled: bool,
    motion_preference: MotionPreference,
    panels: Vec<SplitterPanel>,
}

impl Splitter {
    /// Creates a splitter group.
    pub fn new(id: impl Into<String>) -> Self {
        let group_id = id.into();

        Self {
            id: group_id.clone().into(),
            group_id,
            orientation: Orientation::Horizontal,
            size: Size::Medium,
            disabled: false,
            motion_preference: MotionPreference::Animated,
            panels: Vec::new(),
        }
    }

    /// Applies the splitter orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Uses a horizontal row of panels.
    pub fn horizontal(self) -> Self {
        self.orientation(Orientation::Horizontal)
    }

    /// Uses a vertical stack of panels.
    pub fn vertical(self) -> Self {
        self.orientation(Orientation::Vertical)
    }

    /// Disables resize handles.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies the motion preference for programmatic layout changes.
    pub fn motion_preference(mut self, motion_preference: MotionPreference) -> Self {
        self.motion_preference = motion_preference;
        self
    }

    /// Adds a panel to the splitter.
    pub fn panel(mut self, panel: SplitterPanel) -> Self {
        self.panels.push(panel);
        self
    }

    /// Returns the resolved splitter state.
    pub fn state(&self) -> SplitterState {
        SplitterState::resolve(
            self.group_id.clone(),
            self.orientation,
            self.size,
            self.disabled,
            self.panels.iter().map(SplitterPanel::descriptor),
        )
    }
}

impl Sizable for Splitter {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Splitter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base_state = self.state();
        let motion_model = MotionPreset::committed_layout(self.motion_preference).resolve_model();
        let runtime =
            window.use_keyed_state(self.id.clone(), cx, |_, _| SplitterRuntime::default());
        let needs_frame = runtime.update(cx, |runtime, _| {
            runtime.sync_at_with_model(&base_state, Instant::now(), motion_model)
        });
        if needs_frame {
            window.request_animation_frame();
        }
        let runtime_snapshot = runtime.read(cx).clone();
        let state = base_state.with_panel_fractions(&runtime_snapshot.panel_fractions);
        let is_vertical = matches!(state.orientation(), Orientation::Vertical);
        let metrics = state.metrics();
        let handles = state.handles().to_vec();
        let panels = self.panels;
        let runtime_for_drag = runtime.clone();
        let drag_state = state.clone();

        div()
            .id(self.id)
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .overflow_hidden()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .when(is_vertical, |this| this.flex_col())
            .when(!is_vertical, |this| this.flex_row())
            .on_drag_move(move |event: &DragMoveEvent<SplitterDrag>, window, cx| {
                let drag = event.drag(cx).clone();
                if drag.group_id != drag_state.group_id() {
                    return;
                }

                runtime_for_drag.update(cx, |runtime, _| {
                    runtime.sync_drag_state(&drag_state);

                    let axis_length = if is_vertical {
                        event.bounds.size.height
                    } else {
                        event.bounds.size.width
                    };
                    if axis_length.as_f32() <= EPSILON {
                        return;
                    }

                    if runtime.drag_start.is_none() {
                        runtime.drag_start = Some(SplitterDragStart {
                            origin: event.event.position,
                            origin_fractions: runtime.panel_fractions.clone(),
                            axis_length,
                        });
                    }

                    let Some(start) = runtime.drag_start.clone() else {
                        return;
                    };

                    if start.axis_length.as_f32() <= EPSILON {
                        return;
                    }

                    let delta_px = if is_vertical {
                        event.event.position.y - start.origin.y
                    } else {
                        event.event.position.x - start.origin.x
                    };
                    let delta_fraction = delta_px.as_f32() / start.axis_length.as_f32();
                    let origin_state = drag_state.with_panel_fractions(&start.origin_fractions);
                    let resized = origin_state.resized_by(drag.handle_index, delta_fraction);
                    runtime.panel_fractions = resized
                        .panels()
                        .iter()
                        .map(SplitterPanelState::fraction)
                        .collect();
                });
                window.refresh();
            })
            .children(
                panels
                    .into_iter()
                    .enumerate()
                    .flat_map(move |(index, panel)| {
                        let panel_state = state.panels()[index].clone();
                        let mut elements = Vec::with_capacity(2);
                        elements.push(render_panel(panel_state, panel, is_vertical));
                        if let Some(handle) = handles.get(index) {
                            elements.push(render_handle(
                                state.clone(),
                                handle.clone(),
                                runtime.clone(),
                                metrics,
                                is_vertical,
                            ));
                        }
                        elements
                    }),
            )
    }
}

fn render_panel(state: SplitterPanelState, panel: SplitterPanel, is_vertical: bool) -> AnyElement {
    let panel_selector = format!("splitter-panel:{}", state.id());
    div()
        .id(format!("splitter-panel:{}", state.id()))
        .debug_selector(move || panel_selector)
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .flex()
        .flex_col()
        .flex_grow(0.0)
        .flex_shrink(0.0)
        .flex_basis(DefiniteLength::from(relative(state.fraction())))
        .when(state.collapsed(), |this| this.opacity(0.0))
        .when(is_vertical, |this| this.w_full())
        .when(!is_vertical, |this| this.h_full())
        .child(panel)
        .into_any_element()
}

fn render_handle(
    splitter_state: SplitterState,
    state: SplitterHandleState,
    runtime: Entity<SplitterRuntime>,
    metrics: SplitterMetrics,
    is_vertical: bool,
) -> AnyElement {
    let cursor = if state.disabled() {
        CursorStyle::OperationNotAllowed
    } else if is_vertical {
        CursorStyle::ResizeRow
    } else {
        CursorStyle::ResizeColumn
    };

    div()
        .id(format!("splitter-handle:{}", state.index()))
        .debug_selector({
            let group_id = splitter_state.group_id().to_owned();
            let handle_index = state.index();
            move || format!("splitter:{group_id}:handle:{handle_index}")
        })
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor(cursor)
        .when(state.disabled(), |this| this.opacity(0.48))
        .when(!state.disabled(), |this| {
            let drag_runtime = runtime.clone();
            let drag_state = splitter_state.clone();
            let group_id = splitter_state.group_id().to_owned();
            let handle_index = state.index();

            this.on_drag(
                SplitterDrag {
                    group_id,
                    handle_index,
                },
                move |_, _, _, _, cx| {
                    cx.stop_propagation();
                    drag_runtime.update(cx, |runtime, _| {
                        runtime.sync_drag_state(&drag_state);
                        runtime.drag_start = None;
                    });
                    cx.new(|_| SplitterDragPreview)
                },
            )
        })
        .when(is_vertical, |this| {
            this.w_full().h(gpui_px_from_ui(metrics.handle_hit_size()))
        })
        .when(!is_vertical, |this| {
            this.h_full().w(gpui_px_from_ui(metrics.handle_hit_size()))
        })
        .child(
            div()
                .rounded_sm()
                .bg(rgb(0xc8cdc2))
                .when(is_vertical, |this| {
                    this.w_full().h(gpui_px_from_ui(metrics.handle_thickness()))
                })
                .when(!is_vertical, |this| {
                    this.h_full().w(gpui_px_from_ui(metrics.handle_thickness()))
                }),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::{
        MotionDuration, MotionEasing, MotionPolicyContext, MotionPolicyInput, MotionPolicyIssue,
        MotionSpec, validate_motion_policy,
    };
    use std::time::Duration;

    fn state(left: f32, right: f32) -> SplitterState {
        SplitterState::resolve(
            "runtime-motion",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("left", left),
                SplitterPanelDescriptor::new("right", right),
            ],
        )
    }

    #[test]
    fn runtime_animates_programmatic_fraction_changes() {
        let start = Instant::now();
        let from = state(0.3, 0.7);
        let to = state(0.6, 0.4);
        let mut runtime = SplitterRuntime::default();

        assert!(!runtime.sync_at(&from, start));
        assert!(fractions_equal(&runtime.panel_fractions, &[0.3, 0.7]));

        assert!(runtime.sync_at(&to, start));
        assert!(fractions_equal(&runtime.panel_fractions, &[0.3, 0.7]));

        assert!(runtime.sync_at(&to, start + Duration::from_millis(90)));
        assert!(
            runtime.panel_fractions[0] > 0.3 && runtime.panel_fractions[0] < 0.6,
            "programmatic splitter change should sample between old and new fractions"
        );

        assert!(!runtime.sync_at(&to, start + Duration::from_millis(900)));
        assert!(fractions_equal(&runtime.panel_fractions, &[0.6, 0.4]));
        assert!(runtime.transition.is_none());
    }

    #[test]
    fn runtime_retargets_from_sampled_fraction_and_drag_syncs_immediately() {
        let start = Instant::now();
        let from = state(0.3, 0.7);
        let first_target = state(0.6, 0.4);
        let second_target = state(0.2, 0.8);
        let mut runtime = SplitterRuntime::default();

        runtime.sync_at(&from, start);
        runtime.sync_at(&first_target, start);
        runtime.sync_at(&first_target, start + Duration::from_millis(45));
        let sampled_left = runtime.panel_fractions[0];

        assert!(runtime.sync_at(&second_target, start + Duration::from_millis(45)));
        assert!(
            (runtime.panel_fractions[0] - sampled_left).abs() <= EPSILON,
            "retargeting should start from the sampled fraction instead of the original fraction"
        );

        runtime.sync_drag_state(&from);
        assert!(runtime.transition.is_none());
        assert!(
            (runtime.panel_fractions[0] - sampled_left).abs() <= EPSILON,
            "drag sync should cancel animation without discarding the current runtime override"
        );

        let mut empty_runtime = SplitterRuntime::default();
        empty_runtime.sync_drag_state(&from);
        assert!(fractions_equal(&empty_runtime.panel_fractions, &[0.3, 0.7]));
    }

    #[test]
    fn runtime_reduced_motion_completes_without_transition() {
        let start = Instant::now();
        let from = state(0.3, 0.7);
        let to = state(0.6, 0.4);
        let mut runtime = SplitterRuntime::default();

        runtime.sync_at(&from, start);
        assert!(!runtime.sync_at_with_model(
            &to,
            start,
            MotionPreset::committed_layout(MotionPreference::Reduced).resolve_model()
        ));

        assert!(fractions_equal(&runtime.state_fractions, &[0.6, 0.4]));
        assert!(fractions_equal(&runtime.panel_fractions, &[0.6, 0.4]));
        assert!(runtime.transition.is_none());
    }

    #[test]
    fn runtime_panel_identity_changes_sync_immediately() {
        let start = Instant::now();
        let from = state(0.3, 0.7);
        let replaced = SplitterState::resolve(
            "runtime-motion",
            Orientation::Horizontal,
            Size::Medium,
            false,
            [
                SplitterPanelDescriptor::new("left", 0.2),
                SplitterPanelDescriptor::new("center", 0.3),
                SplitterPanelDescriptor::new("right", 0.5),
            ],
        );
        let mut runtime = SplitterRuntime::default();

        runtime.sync_at(&from, start);
        assert!(!runtime.sync_at(&replaced, start + Duration::from_millis(16)));

        assert_eq!(
            runtime.panel_ids,
            [
                "left".to_string(),
                "center".to_string(),
                "right".to_string()
            ]
        );
        assert!(fractions_equal(&runtime.panel_fractions, &[0.2, 0.3, 0.5]));
        assert!(runtime.transition.is_none());
    }

    #[test]
    fn runtime_custom_timeline_model_remains_timeline_backed() {
        let start = Instant::now();
        let from = state(0.3, 0.7);
        let to = state(0.6, 0.4);
        let spec = MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(240)),
            MotionEasing::Linear,
        );
        let mut runtime = SplitterRuntime::default();

        runtime.sync_at(&from, start);
        assert!(runtime.sync_at_with_model(
            &to,
            start,
            MotionPreset::timeline(spec).resolve_model()
        ));

        let transition = runtime
            .transition
            .as_ref()
            .expect("custom timeline should create a transition");
        assert!(matches!(
            transition.controller.tracks()[0].1.model(),
            MotionModel::Timeline(model_spec) if model_spec == spec
        ));
    }

    #[test]
    fn runtime_policy_rejects_over_budget_programmatic_timeline() {
        let start = Instant::now();
        let from = state(0.3, 0.7);
        let to = state(0.6, 0.4);
        let model = MotionModel::timeline(MotionSpec::new(
            MotionPreference::Animated,
            MotionDuration::Custom(Duration::from_millis(420)),
            MotionEasing::Linear,
        ));
        let mut runtime = SplitterRuntime::default();

        runtime.sync_at(&from, start);
        assert!(!runtime.sync_at_with_model(&to, start, model));

        assert!(fractions_equal(&runtime.state_fractions, &[0.6, 0.4]));
        assert!(fractions_equal(&runtime.panel_fractions, &[0.6, 0.4]));
        assert!(runtime.transition.is_none());
    }

    #[test]
    fn splitter_motion_policy_preserves_programmatic_motion_and_drag_bypass() {
        let programmatic = MotionPolicyInput::new(
            MotionPolicyContext::CommittedLayout,
            MotionPreset::committed_layout(MotionPreference::Animated).resolve_model(),
        )
        .with_spatial_motion(true)
        .with_reduced_motion_final_state(true);
        assert!(validate_motion_policy(programmatic).is_ok());

        let pointer_drag = MotionPolicyInput::new(
            MotionPolicyContext::PointerDrag,
            MotionModel::timeline(MotionSpec::committed_layout(MotionPreference::Animated)),
        )
        .with_spatial_motion(true)
        .with_reduced_motion_final_state(true);
        assert!(
            validate_motion_policy(pointer_drag)
                .has_issue(MotionPolicyIssue::SpatialMotionForbidden)
        );
    }
}
