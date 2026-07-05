//! Splitter component.

use crate::geometry::gpui_px_from_ui;
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, AnyView, App, Context, CursorStyle, DefiniteLength, DragMoveEvent, ElementId,
    Empty, Entity, IntoElement, ParentElement, Pixels, Point, Render, RenderOnce, Styled, Window,
    div, px, relative, rgb,
};
use open_gpui_ui_core::split::{
    SplitterLayoutTransition, SplitterLayoutTransitionSample, SplitterPanelTransitionSample,
    SplitterTransitionIntent,
};
use open_gpui_ui_core::{
    MotionExecutionPlan, MotionModel, MotionPolicyContext, MotionPolicyInput, MotionPreference,
    MotionPreset, MotionProjectionClip, MotionScalarController, MotionScalarExecution, MotionSpec,
    Orientation, Sizable, Size, UiRect, ui_point, ui_px, ui_rect, ui_size,
};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

const EPSILON: f32 = 0.000_1;
const TRANSITION_SCENE_EXTENT: f32 = 1000.0;

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
    last_state: Option<SplitterState>,
    drag_start: Option<SplitterDragStart>,
    transition: Option<SplitterRuntimeTransition>,
    layout_transition: Option<SplitterRuntimeLayoutTransition>,
    retained_views: HashMap<String, AnyView>,
}

impl SplitterRuntime {
    fn retain_panel_views(&mut self, panel_views: impl IntoIterator<Item = (String, AnyView)>) {
        self.retained_views.extend(panel_views);
    }

    fn prune_retained_views_to_current_panels(&mut self) {
        if self.layout_transition.is_some() {
            return;
        }
        let current = self.panel_ids.iter().collect::<HashSet<_>>();
        self.retained_views.retain(|id, _| current.contains(id));
    }

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

        if self.panel_ids.is_empty() {
            self.sync_immediate(panel_ids, target_fractions);
            self.last_state = Some(state.clone());
            return false;
        }

        if self.layout_transition_should_run(state, &panel_ids, &target_fractions) {
            return self.sync_layout_transition(state, panel_ids, target_fractions, now, model);
        }

        if let Some(transition) = self.transition.as_ref()
            && transition.panel_ids == panel_ids
            && fractions_equal(&transition.to_fractions, &target_fractions)
        {
            let complete = self.sample_transition(now);
            if complete {
                self.last_state = Some(state.clone());
            }
            return !complete;
        }

        if fractions_equal(&self.state_fractions, &target_fractions) {
            self.transition = None;
            self.last_state = Some(state.clone());
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
            self.last_state = Some(state.clone());
            return false;
        }
        let motion = committed_layout_motion_plan(model);
        if motion.is_immediate() {
            self.state_fractions = target_fractions.clone();
            self.panel_fractions = target_fractions;
            self.transition = None;
            self.layout_transition = None;
            self.last_state = Some(state.clone());
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
                motion.model(),
            ),
        });
        self.last_state = Some(state.clone());
        true
    }

    fn sync_immediate(&mut self, panel_ids: Vec<String>, panel_fractions: Vec<f32>) {
        self.panel_ids = panel_ids;
        self.state_fractions = panel_fractions.clone();
        self.panel_fractions = panel_fractions;
        self.drag_start = None;
        self.transition = None;
        self.layout_transition = None;
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
            self.last_state = Some(state.clone());
            return;
        }

        self.transition = None;
        self.layout_transition = None;
        self.last_state = Some(state.clone());
    }

    fn layout_transition_should_run(
        &self,
        state: &SplitterState,
        panel_ids: &[String],
        target_fractions: &[f32],
    ) -> bool {
        if self.layout_transition.is_some() {
            return true;
        }
        if self.panel_ids != panel_ids || self.panel_fractions.len() != target_fractions.len() {
            return true;
        }

        self.last_state
            .as_ref()
            .is_some_and(|previous| collapsed_states_changed(previous, state))
    }

    fn sync_layout_transition(
        &mut self,
        state: &SplitterState,
        panel_ids: Vec<String>,
        target_fractions: Vec<f32>,
        now: Instant,
        model: MotionModel,
    ) -> bool {
        if let Some(transition) = self.layout_transition.as_ref()
            && transition.target_state == *state
        {
            let complete = self.sample_layout_transition(now);
            if complete {
                self.last_state = Some(state.clone());
            }
            return !complete;
        }

        let from_state = self
            .last_state
            .as_ref()
            .map(|state| state.with_panel_fractions(&self.panel_fractions))
            .unwrap_or_else(|| state.clone());
        let intent = transition_intent(&from_state, state);
        let motion = committed_layout_motion_plan(model);
        if motion.is_immediate() {
            self.sync_immediate(panel_ids, target_fractions);
            self.last_state = Some(state.clone());
            return false;
        }

        let transition = SplitterLayoutTransition::between(
            intent,
            SplitterLayoutScene::from_state(&from_state, transition_scene_bounds()),
            SplitterLayoutScene::from_state(state, transition_scene_bounds()),
            MotionSpec::committed_layout(motion.model().preference()),
        );
        self.panel_ids = panel_ids;
        self.state_fractions = target_fractions.clone();
        self.panel_fractions = target_fractions;
        self.drag_start = None;
        self.transition = None;
        self.layout_transition = Some(SplitterRuntimeLayoutTransition {
            target_state: state.clone(),
            transition,
            started_at: now,
            track: MotionScalarExecution::start(motion, 0.0, 1.0, 0.0, Duration::ZERO),
        });
        self.last_state = Some(state.clone());
        true
    }

    fn sample_transition(&mut self, now: Instant) -> bool {
        let Some(transition) = self.transition.as_ref() else {
            return true;
        };
        let sample = transition
            .controller
            .sample_since(transition.started_at, now);
        self.panel_fractions = fraction_samples_for_transition(transition, &sample);
        let complete = sample.complete();
        if complete {
            self.panel_fractions = transition.to_fractions.clone();
            self.transition = None;
        }
        complete
    }

    fn sample_layout_transition(&mut self, now: Instant) -> bool {
        let Some(transition) = self.layout_transition.as_ref() else {
            return true;
        };
        let sample = transition.track.sample_since(transition.started_at, now);
        let complete = sample.complete();
        if complete {
            self.layout_transition = None;
        }
        complete
    }

    fn layout_transition_sample(&self, now: Instant) -> Option<SplitterLayoutTransitionSample> {
        let transition = self.layout_transition.as_ref()?;
        let sample = transition.track.sample_since(transition.started_at, now);
        Some(transition.transition.sample(sample.value()))
    }

    fn sampled_transition_fractions(&self, now: Instant) -> Vec<f32> {
        let Some(transition) = self.transition.as_ref() else {
            return self.panel_fractions.clone();
        };
        let sample = transition
            .controller
            .sample_since(transition.started_at, now);
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

#[derive(Debug, Clone, PartialEq)]
struct SplitterRuntimeLayoutTransition {
    target_state: SplitterState,
    transition: SplitterLayoutTransition,
    started_at: Instant,
    track: MotionScalarExecution,
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

fn committed_layout_motion_plan(model: MotionModel) -> MotionExecutionPlan {
    MotionExecutionPlan::resolve(
        MotionPolicyInput::new(MotionPolicyContext::CommittedLayout, model)
            .with_spatial_motion(!model.is_immediate())
            .with_reduced_motion_final_state(true),
    )
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

fn collapsed_states_changed(previous: &SplitterState, next: &SplitterState) -> bool {
    previous.panels().len() == next.panels().len()
        && previous
            .panels()
            .iter()
            .zip(next.panels())
            .any(|(previous, next)| {
                previous.id() == next.id() && previous.collapsed() != next.collapsed()
            })
}

fn transition_intent(previous: &SplitterState, next: &SplitterState) -> SplitterTransitionIntent {
    let previous_ids = previous
        .panels()
        .iter()
        .map(SplitterPanelState::id)
        .collect::<Vec<_>>();
    let next_ids = next
        .panels()
        .iter()
        .map(SplitterPanelState::id)
        .collect::<Vec<_>>();

    if previous_ids == next_ids {
        if previous
            .panels()
            .iter()
            .zip(next.panels())
            .any(|(previous, next)| !previous.collapsed() && next.collapsed())
        {
            return SplitterTransitionIntent::Collapse;
        }
        if previous
            .panels()
            .iter()
            .zip(next.panels())
            .any(|(previous, next)| previous.collapsed() && !next.collapsed())
        {
            return SplitterTransitionIntent::Expand;
        }
        return SplitterTransitionIntent::Resize;
    }

    match next.panels().len().cmp(&previous.panels().len()) {
        std::cmp::Ordering::Greater => SplitterTransitionIntent::Insert,
        std::cmp::Ordering::Less => SplitterTransitionIntent::Remove,
        std::cmp::Ordering::Equal => SplitterTransitionIntent::Replace,
    }
}

fn transition_scene_bounds() -> UiRect {
    ui_rect(
        ui_point(ui_px(0.0), ui_px(0.0)),
        ui_size(
            ui_px(TRANSITION_SCENE_EXTENT),
            ui_px(TRANSITION_SCENE_EXTENT),
        ),
    )
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
    content: SplitterPanelContent,
}

enum SplitterPanelContent {
    Element(AnyElement),
    View(AnyView),
}

impl SplitterPanelContent {
    fn into_any_element(self) -> AnyElement {
        match self {
            Self::Element(element) => element,
            Self::View(view) => view.into_any_element(),
        }
    }

    fn view(&self) -> Option<AnyView> {
        match self {
            Self::Element(_) => None,
            Self::View(view) => Some(view.clone()),
        }
    }
}

impl SplitterPanel {
    /// Creates a splitter panel.
    pub fn new(descriptor: SplitterPanelDescriptor, content: impl IntoElement) -> Self {
        Self {
            descriptor,
            content: SplitterPanelContent::Element(content.into_any_element()),
        }
    }

    /// Creates a splitter panel from a retained view handle.
    ///
    /// View-backed panels can participate in insert/remove layout transitions because their
    /// content can be rendered again from a stable entity handle. Plain element-backed panels keep
    /// the existing one-shot render behavior.
    pub fn view(descriptor: SplitterPanelDescriptor, view: impl Into<AnyView>) -> Self {
        Self {
            descriptor,
            content: SplitterPanelContent::View(view.into()),
        }
    }

    /// Returns the panel descriptor.
    pub fn descriptor(&self) -> SplitterPanelDescriptor {
        self.descriptor.clone()
    }

    fn retained_view(&self) -> Option<(String, AnyView)> {
        self.content
            .view()
            .map(|view| (self.descriptor.id().to_owned(), view))
    }

    fn into_content(self) -> AnyElement {
        self.content.into_any_element()
    }
}

impl RenderOnce for SplitterPanel {
    fn render(self, _: &mut open_gpui::Window, _: &mut open_gpui::App) -> impl IntoElement {
        self.into_content()
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
        let now = Instant::now();
        let panel_views = self
            .panels
            .iter()
            .filter_map(SplitterPanel::retained_view)
            .collect::<Vec<_>>();
        let runtime =
            window.use_keyed_state(self.id.clone(), cx, |_, _| SplitterRuntime::default());
        let needs_frame = runtime.update(cx, |runtime, _| {
            runtime.retain_panel_views(panel_views);
            let needs_frame = runtime.sync_at_with_model(&base_state, now, motion_model);
            runtime.prune_retained_views_to_current_panels();
            needs_frame
        });
        if needs_frame {
            window.request_animation_frame();
        }
        let runtime_snapshot = runtime.read(cx).clone();
        let layout_sample = runtime_snapshot.layout_transition_sample(now);
        let state = base_state.with_panel_fractions(&runtime_snapshot.panel_fractions);
        let is_vertical = matches!(state.orientation(), Orientation::Vertical);
        let metrics = state.metrics();
        let handles = state.handles().to_vec();
        let panels = self.panels;
        let mut panel_slots = panels.into_iter().map(Some).collect::<Vec<_>>();
        let panel_index_by_id = state
            .panels()
            .iter()
            .enumerate()
            .map(|(index, panel)| (panel.id().to_owned(), index))
            .collect::<HashMap<_, _>>();
        let overlay_panel_ids = transition_overlay_panel_ids(&layout_sample);
        let runtime_for_drag = runtime.clone();
        let drag_state = state.clone();

        let mut children = Vec::new();
        for (index, panel_slot) in panel_slots.iter_mut().enumerate() {
            let panel_state = state.panels()[index].clone();
            if overlay_panel_ids.contains(panel_state.id()) {
                children.push(render_panel_placeholder(panel_state, is_vertical));
            } else if let Some(panel) = panel_slot.take() {
                children.push(render_panel(panel_state, panel, is_vertical));
            }

            if let Some(handle) = handles.get(index) {
                children.push(render_handle(
                    state.clone(),
                    handle.clone(),
                    runtime.clone(),
                    metrics,
                    is_vertical,
                ));
            }
        }

        let mut root = div()
            .id(self.id)
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .relative()
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
            .children(children);

        if let Some(sample) = layout_sample {
            root = root.child(render_layout_transition_overlay(
                sample,
                &mut panel_slots,
                &panel_index_by_id,
                &runtime_snapshot.retained_views,
            ));
        }

        root
    }
}

fn render_panel(state: SplitterPanelState, panel: SplitterPanel, is_vertical: bool) -> AnyElement {
    render_panel_content(state, Some(panel.into_content()), is_vertical)
}

fn render_panel_placeholder(state: SplitterPanelState, is_vertical: bool) -> AnyElement {
    render_panel_content(state, None, is_vertical)
}

fn render_panel_content(
    state: SplitterPanelState,
    content: Option<AnyElement>,
    is_vertical: bool,
) -> AnyElement {
    let panel_selector = format!("splitter-panel:{}", state.id());
    let mut panel = div()
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
        .when(!is_vertical, |this| this.h_full());

    if let Some(content) = content {
        panel = panel.child(content);
    }

    panel.into_any_element()
}

fn transition_overlay_panel_ids(
    sample: &Option<SplitterLayoutTransitionSample>,
) -> HashSet<String> {
    sample
        .as_ref()
        .map(|sample| {
            sample
                .panels()
                .iter()
                .filter(|panel| panel.clip().is_some())
                .map(|panel| panel.id().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn render_layout_transition_overlay(
    sample: SplitterLayoutTransitionSample,
    panel_slots: &mut [Option<SplitterPanel>],
    panel_index_by_id: &HashMap<String, usize>,
    retained_views: &HashMap<String, AnyView>,
) -> AnyElement {
    let mut overlay = div()
        .id("splitter-transition-overlay")
        .debug_selector(|| "splitter:transition-overlay".to_string())
        .absolute()
        .inset_0()
        .overflow_hidden();

    for panel in sample.panels() {
        let Some(clip) = panel.clip() else {
            continue;
        };
        let Some(content) =
            transition_panel_content(panel, panel_slots, panel_index_by_id, retained_views)
        else {
            continue;
        };
        overlay = overlay.child(render_transition_clip(panel.id(), clip, content));
    }

    overlay.into_any_element()
}

fn transition_panel_content(
    panel: &SplitterPanelTransitionSample,
    panel_slots: &mut [Option<SplitterPanel>],
    panel_index_by_id: &HashMap<String, usize>,
    retained_views: &HashMap<String, AnyView>,
) -> Option<AnyElement> {
    if let Some(index) = panel_index_by_id.get(panel.id()).copied()
        && let Some(panel) = panel_slots.get_mut(index).and_then(Option::take)
    {
        return Some(panel.into_content());
    }

    retained_views
        .get(panel.id())
        .cloned()
        .map(IntoElement::into_any_element)
}

fn render_transition_clip(id: &str, clip: MotionProjectionClip, content: AnyElement) -> AnyElement {
    let visible = clip.visible_bounds();
    let content_bounds = clip.content_bounds();
    let content_left =
        relative_fraction(content_bounds.origin.x.as_f32() - visible.origin.x.as_f32());
    let content_top =
        relative_fraction(content_bounds.origin.y.as_f32() - visible.origin.y.as_f32());

    div()
        .id(format!("splitter-transition-panel:{id}"))
        .debug_selector({
            let id = id.to_owned();
            move || format!("splitter:transition-panel:{id}")
        })
        .absolute()
        .left(relative_fraction(visible.origin.x.as_f32()))
        .top(relative_fraction(visible.origin.y.as_f32()))
        .w(relative_fraction(visible.size.width.as_f32()))
        .h(relative_fraction(visible.size.height.as_f32()))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left(content_left)
                .top(content_top)
                .w(relative_fraction(content_bounds.size.width.as_f32()))
                .h(relative_fraction(content_bounds.size.height.as_f32()))
                .child(content),
        )
        .into_any_element()
}

fn relative_fraction(value: f32) -> DefiniteLength {
    relative(value / TRANSITION_SCENE_EXTENT)
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
    fn runtime_panel_identity_changes_create_layout_transition() {
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
        assert!(runtime.sync_at(&replaced, start + Duration::from_millis(16)));

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
        assert!(runtime.layout_transition.is_some());

        let sample = runtime
            .layout_transition_sample(start + Duration::from_millis(16))
            .expect("identity change should expose a layout transition sample");
        let center = sample
            .panel("center")
            .expect("inserted panel should be sampled");
        let center_clip = center
            .clip()
            .expect("inserted panel should expose a reveal clip");
        assert_eq!(center_clip.visible_bounds().size.width, ui_px(0.0));

        assert!(!runtime.sync_at(&replaced, start + Duration::from_millis(900)));
        assert!(runtime.layout_transition.is_none());
    }

    #[test]
    fn runtime_reduced_motion_identity_change_completes_without_layout_transition() {
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
        assert!(!runtime.sync_at_with_model(
            &replaced,
            start + Duration::from_millis(16),
            MotionPreset::committed_layout(MotionPreference::Reduced).resolve_model()
        ));

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
        assert!(runtime.layout_transition.is_none());
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
