use crate::a11y::UiA11yElementExt;
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::scroll_area::ScrollArea;
use crate::scroll_surface::{
    ScrollSurfaceRuntime, scroll_surface_handle, set_vertical_scroll_offset_with_source,
    vertical_scroll_offset, vertical_viewport_extent,
};
use crate::theme::ThemeResolver;
use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    AnyElement, App, Context, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, ScrollHandle, ScrollViewportChangeSource,
    ScrollViewportProgrammaticSource, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px,
};
use open_gpui_motion::{MotionFrameHost, MotionPreference, MotionPreset};
use open_gpui_ui_core::{Sizable, Size, ThemeTokens, UiPx, VirtualizerSnapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::descriptor::VirtualizedListItemDescriptor;
use super::model::{
    VirtualizedListActivation, VirtualizedListRevealResult, VirtualizedListScrollStrategy,
    VirtualizedListSelectionChange, VirtualizedListSelectionMode, VirtualizedListState,
    virtualized_list_state_items,
};
use super::motion::VirtualizedListActiveIndicatorRuntime;
use super::render::{render_virtualized_list_body, render_virtualized_list_sticky_overlay};
use super::render_plan::{
    VirtualizedListBehaviorSnapshot, VirtualizedListRenderPlan, VirtualizedListRowMeasureMode,
    VirtualizedListRowRenderContext,
};
use super::style::{
    DEFAULT_VIRTUALIZED_LIST_VIEWPORT_ITEM_COUNT, VirtualizedListMetrics, nonnegative_px,
};

pub(super) type VirtualizedListActivationHandler =
    Rc<dyn Fn(VirtualizedListActivation, &mut Window, &mut App)>;
pub(super) type VirtualizedListSelectionChangeHandler =
    Rc<dyn Fn(VirtualizedListSelectionChange, &mut Window, &mut App)>;
pub(super) type VirtualizedListRowRenderer =
    Rc<dyn Fn(VirtualizedListRowRenderContext, &mut Window, &mut App) -> AnyElement>;

const VIRTUALIZED_LIST_TYPEAHEAD_RESET: Duration = Duration::from_millis(700);

#[derive(Debug, Clone)]
pub(super) struct VirtualizedListRuntime {
    pub(super) scroll_surface: ScrollSurfaceRuntime,
    pub(super) focus_handle: FocusHandle,
    pub(super) active_key: Option<String>,
    pub(super) selected_keys: BTreeSet<String>,
    pub(super) selection_anchor_key: Option<String>,
    pub(super) row_measurements: BTreeMap<String, UiPx>,
    pub(super) pending_scroll_to_active: Option<String>,
    pub(super) typeahead_buffer: String,
    pub(super) last_typeahead_at: Option<Instant>,
    pub(super) active_indicator: VirtualizedListActiveIndicatorRuntime,
    pub(super) active_indicator_frame_host: MotionFrameHost,
}

impl VirtualizedListRuntime {
    pub(super) fn set_row_measurement(
        &mut self,
        render_key: String,
        height: UiPx,
        cx: &mut Context<Self>,
    ) {
        let height = nonnegative_px(height);
        if self.row_measurements.get(&render_key).copied() != Some(height) {
            self.row_measurements.insert(render_key, height);
            cx.notify();
        }
    }

    fn push_typeahead_key(&mut self, key: &str) -> String {
        let now = Instant::now();
        if self.last_typeahead_at.map_or(true, |last| {
            now.duration_since(last) > VIRTUALIZED_LIST_TYPEAHEAD_RESET
        }) {
            self.typeahead_buffer.clear();
        }

        self.typeahead_buffer.push_str(&key.to_lowercase());
        self.last_typeahead_at = Some(now);
        self.typeahead_buffer.clone()
    }
}

/// A concrete GPUI virtualized list renderer.
#[derive(IntoElement)]
pub struct VirtualizedList {
    id: String,
    label: SharedString,
    items: Arc<[VirtualizedListItemDescriptor]>,
    size: Size,
    disabled: bool,
    active_key: Option<String>,
    selected_keys: BTreeSet<String>,
    selection_mode: VirtualizedListSelectionMode,
    tokens: ThemeTokens,
    viewport_item_count: usize,
    metrics: VirtualizedListMetrics,
    row_measure_mode: VirtualizedListRowMeasureMode,
    motion_preference: MotionPreference,
    snapshot: Option<VirtualizerSnapshot>,
    scroll_handle: Option<ScrollHandle>,
    reveal_key: Option<String>,
    reveal_strategy: VirtualizedListScrollStrategy,
    row_renderer: Option<VirtualizedListRowRenderer>,
    on_activate: Option<VirtualizedListActivationHandler>,
    on_selection_change: Option<VirtualizedListSelectionChangeHandler>,
}

impl VirtualizedList {
    /// Creates a new virtualized list renderer.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = VirtualizedListItemDescriptor>,
    ) -> Self {
        Self::from_shared_items(
            id,
            label,
            Arc::from(items.into_iter().collect::<Vec<_>>().into_boxed_slice()),
        )
    }

    /// Creates a new virtualized list renderer from shared item storage.
    pub fn from_shared_items(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        items: Arc<[VirtualizedListItemDescriptor]>,
    ) -> Self {
        let size = Size::Medium;

        Self {
            id: id.into(),
            label: label.into(),
            items,
            size,
            disabled: false,
            active_key: None,
            selected_keys: BTreeSet::new(),
            selection_mode: VirtualizedListSelectionMode::Single,
            tokens: ThemeTokens::default(),
            viewport_item_count: DEFAULT_VIRTUALIZED_LIST_VIEWPORT_ITEM_COUNT,
            metrics: VirtualizedListMetrics::from_size(size),
            row_measure_mode: VirtualizedListRowMeasureMode::default(),
            motion_preference: MotionPreference::Animated,
            snapshot: None,
            scroll_handle: None,
            reveal_key: None,
            reveal_strategy: VirtualizedListScrollStrategy::Nearest,
            row_renderer: None,
            on_activate: None,
            on_selection_change: None,
        }
    }

    /// Marks the list as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies the default active item key for adapter-owned runtime state.
    pub fn default_active_key(mut self, key: impl Into<String>) -> Self {
        self.active_key = Some(key.into());
        self
    }

    /// Applies the default selected item key for adapter-owned runtime state.
    pub fn default_selected_key(mut self, key: impl Into<String>) -> Self {
        self.selected_keys.clear();
        self.selected_keys.insert(key.into());
        self
    }

    /// Applies the default selected item keys for adapter-owned runtime state.
    pub fn default_selected_keys<I, K>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = K>,
        K: Into<String>,
    {
        self.selected_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// Applies the list selection behavior.
    pub fn selection_mode(mut self, selection_mode: VirtualizedListSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// Applies the theme token bundle used by virtualized-list color recipes.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Applies the estimated viewport item count used for keyboard page navigation.
    pub fn viewport_item_count(mut self, count: usize) -> Self {
        self.viewport_item_count = count.max(1);
        self
    }

    /// Applies a fixed row height.
    pub fn row_height(mut self, row_height: UiPx) -> Self {
        self.metrics = self.metrics.with_row_height(row_height);
        self
    }

    /// Applies the body row measurement mode.
    pub fn row_measure_mode(mut self, row_measure_mode: VirtualizedListRowMeasureMode) -> Self {
        self.row_measure_mode = row_measure_mode;
        self
    }

    /// Applies the motion preference for active-descendant chrome.
    pub fn motion_preference(mut self, motion_preference: MotionPreference) -> Self {
        self.motion_preference = motion_preference;
        self
    }

    /// Seeds measured-row virtualizer measurements from a snapshot.
    pub fn virtualizer_snapshot(mut self, snapshot: VirtualizerSnapshot) -> Self {
        self.snapshot = Some(snapshot);
        self
    }

    /// Uses an externally owned GPUI scroll handle for the list viewport.
    ///
    /// The handle remains outside renderer-neutral state so application shells can observe and
    /// control final committed viewport facts without weakening row semantics.
    pub fn scroll_handle(mut self, scroll_handle: &ScrollHandle) -> Self {
        self.scroll_handle = Some(scroll_handle.clone());
        self
    }

    /// Requests a key-based reveal during render using the provided scroll strategy.
    pub fn reveal_key(
        mut self,
        key: impl Into<String>,
        strategy: VirtualizedListScrollStrategy,
    ) -> Self {
        self.reveal_key = Some(key.into());
        self.reveal_strategy = strategy;
        self
    }

    /// Registers a custom row-content renderer.
    ///
    /// The outer row keeps ownership of virtual layout, accessibility, focus, hit testing, and
    /// selection behavior. The renderer replaces only the row content.
    pub fn render_row<E>(
        mut self,
        renderer: impl Fn(VirtualizedListRowRenderContext, &mut Window, &mut App) -> E + 'static,
    ) -> Self
    where
        E: IntoElement + 'static,
    {
        self.row_renderer = Some(Rc::new(move |context, window, cx| {
            renderer(context, window, cx).into_any_element()
        }));
        self
    }

    /// Applies the overscan row budget.
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.metrics = self.metrics.with_overscan_count(overscan);
        self
    }

    /// Registers an activation handler for clicked or keyboard-activated rows.
    pub fn on_activate(
        mut self,
        handler: impl Fn(VirtualizedListActivation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Registers a selection-change handler for controlled selected keys.
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(VirtualizedListSelectionChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved renderer-neutral list state from the builder seed.
    pub fn state(&self) -> VirtualizedListState {
        self.resolved_state(
            self.active_key.as_deref(),
            self.selected_keys.iter().map(String::as_str),
            self.viewport_item_count,
        )
    }

    /// Returns the public behavior snapshot at the default viewport origin.
    pub fn behavior_snapshot(&self) -> VirtualizedListBehaviorSnapshot {
        self.behavior_snapshot_with_viewport(
            UiPx::ZERO,
            self.metrics.row_height() * self.viewport_item_count as f32,
        )
    }

    /// Resolves the public behavior snapshot for a viewport.
    pub fn behavior_snapshot_with_viewport(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> VirtualizedListBehaviorSnapshot {
        let plan = self.render_plan(scroll_offset, viewport_extent);
        VirtualizedListBehaviorSnapshot::from_render_plan(&plan)
    }

    /// Resolves the renderer-neutral state and virtual window for the current list.
    fn render_plan(&self, scroll_offset: UiPx, viewport_extent: UiPx) -> VirtualizedListRenderPlan {
        let state = self.resolved_state(
            self.active_key.as_deref(),
            self.selected_keys.iter().map(String::as_str),
            self.viewport_item_count,
        );
        VirtualizedListRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            state,
            self.items.as_ref(),
            self.row_measure_mode,
            &BTreeMap::new(),
            self.snapshot.as_ref(),
            scroll_offset,
            viewport_extent,
        )
    }

    fn resolved_state<'a, I>(
        &self,
        active_key: Option<&str>,
        selected_keys: I,
        viewport_item_count: usize,
    ) -> VirtualizedListState
    where
        I: IntoIterator<Item = &'a str>,
    {
        VirtualizedListState::resolve(
            self.size,
            self.disabled,
            virtualized_list_state_items(self.items.as_ref()),
            active_key,
            selected_keys,
            self.selection_mode,
            Some(viewport_item_count.max(1)),
        )
        .with_metrics(self.metrics)
    }
}

impl Sizable for VirtualizedList {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self.metrics = VirtualizedListMetrics::from_size(size);
        self
    }
}

impl RenderOnce for VirtualizedList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let colors = ThemeResolver::virtualized_list_colors(self.tokens);
        let focus_shadow = focus_ring_shadow_with_theme(colors.focus_ring_shape(), &theme);
        let runtime_id = format!("virtualized-list:{}:runtime", self.id);
        let debug_id = self.id.to_string();
        let now = Instant::now();
        let active_indicator_model =
            MotionPreset::affordance(self.motion_preference).resolve_model();
        let runtime = window.use_keyed_state(runtime_id, cx, |_, cx| VirtualizedListRuntime {
            scroll_surface: ScrollSurfaceRuntime::new(None),
            focus_handle: cx.focus_handle(),
            active_key: self.active_key.clone(),
            selected_keys: self.selected_keys.clone(),
            selection_anchor_key: self.active_key.clone(),
            row_measurements: BTreeMap::new(),
            pending_scroll_to_active: None,
            typeahead_buffer: String::new(),
            last_typeahead_at: None,
            active_indicator: VirtualizedListActiveIndicatorRuntime::default(),
            active_indicator_frame_host: MotionFrameHost::new(),
        });
        let runtime_state = runtime.read(cx).clone();
        let scroll_handle =
            scroll_surface_handle(&runtime_state.scroll_surface, self.scroll_handle.as_ref());
        let focus_handle = runtime_state.focus_handle.clone();
        let viewport_extent = vertical_viewport_extent(&scroll_handle);
        let viewport_item_count = resolve_viewport_item_count(
            self.metrics.row_height(),
            viewport_extent,
            self.viewport_item_count,
        );
        let state = self.resolved_state(
            runtime_state.active_key.as_deref(),
            runtime_state.selected_keys.iter().map(String::as_str),
            viewport_item_count,
        );
        let scroll_offset = vertical_scroll_offset(&scroll_handle);
        let plan = VirtualizedListRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            state.clone(),
            self.items.as_ref(),
            self.row_measure_mode,
            &runtime_state.row_measurements,
            self.snapshot.as_ref(),
            scroll_offset,
            viewport_extent,
        );
        if let Some(pending_scroll_to_active) = runtime_state.pending_scroll_to_active.as_deref() {
            scroll_active_key(
                &scroll_handle,
                &state,
                pending_scroll_to_active,
                plan.row_measure_mode(),
                plan.virtualizer().snapshot(),
            );
            runtime.update(cx, |runtime, _| {
                runtime.pending_scroll_to_active = None;
            });
        }
        if let Some(reveal_key) = self.reveal_key.as_deref() {
            reveal_virtualized_list_key(
                &scroll_handle,
                &state,
                reveal_key,
                self.reveal_strategy,
                plan.row_measure_mode(),
                plan.virtualizer().snapshot(),
            );
        }
        let on_activate = self.on_activate.clone();
        let on_selection_change = self.on_selection_change.clone();
        let list_state = plan.state().clone();
        let rows = plan.rows().to_vec();
        let sticky_overlay = plan.sticky_overlay().cloned();
        let row_measure_mode = plan.row_measure_mode();
        let estimated_row_height = plan.metrics().row_height();
        let virtualizer_snapshot = plan.virtualizer().snapshot().clone();
        let row_renderer = self.row_renderer.clone();
        let list_id = plan.list_id().to_owned();
        let scroll_viewport_id = format!("virtualized-list:{}:viewport", plan.list_id());
        let root_click_state = list_state.clone();

        let active_indicator_frame = runtime.update(cx, |runtime, _| {
            if runtime.active_key.as_deref() != list_state.active_key() {
                runtime.active_key = list_state.active_key().map(str::to_owned);
                runtime.pending_scroll_to_active = list_state.active_key().map(str::to_owned);
            }
            if &runtime.selected_keys != list_state.selected_key_set() {
                runtime.selected_keys = list_state.selected_key_set().clone();
            }
            let anchor_is_valid = runtime
                .selection_anchor_key
                .as_deref()
                .is_some_and(|key| list_state.selectable_index_for_key(key).is_some());
            if !anchor_is_valid {
                runtime.selection_anchor_key = list_state.active_key().map(str::to_owned);
            }
            let active_indicator_demand =
                runtime
                    .active_indicator
                    .sync(&plan, now, active_indicator_model);
            if let Some(reset_reason) = active_indicator_demand.reset_reason() {
                runtime.active_indicator_frame_host.reset(reset_reason);
            }
            runtime
                .active_indicator_frame_host
                .observe(active_indicator_demand.frame_demand())
        });
        if active_indicator_frame.should_request_frame() {
            window.request_animation_frame();
        }
        let active_indicator = runtime.read(cx).active_indicator.snapshot();

        div()
            .id(self.id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("virtualized-list:{debug_id}:root")
            })
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.resolve(colors.border()))
            .bg(theme.resolve(colors.surface()))
            .text_size(gpui_px_from_ui(self.size.control_text_px()))
            .text_color(theme.resolve(colors.foreground()))
            .focusable()
            .tab_group()
            .tab_stop(!list_state.disabled() && !list_state.visible_empty())
            .track_focus(&focus_handle)
            .focus_visible(move |style| style.shadow(focus_shadow.clone()))
            .ui_role(plan.role())
            .aria_label(plan.label().to_owned())
            .aria_disabled(list_state.disabled())
            .on_click({
                let focus_handle = focus_handle.clone();
                move |_, window, cx| {
                    if !root_click_state.disabled() && !root_click_state.visible_empty() {
                        focus_handle.focus(window, cx);
                    }
                }
            })
            .on_scroll_wheel(|_, _, _| open_gpui::ScrollWheelIntent::handled().stop_propagation())
            .on_key_down({
                let runtime = runtime.clone();
                let scroll_handle = scroll_handle.clone();
                let on_activate = on_activate.clone();
                let on_selection_change = on_selection_change.clone();
                let plan_state = list_state.clone();
                let row_measure_mode = row_measure_mode;
                let virtualizer_snapshot = virtualizer_snapshot.clone();
                move |event: &KeyDownEvent, window, cx| {
                    handle_virtualized_list_key_down(
                        &plan_state,
                        runtime.clone(),
                        scroll_handle.clone(),
                        on_activate.clone(),
                        on_selection_change.clone(),
                        row_measure_mode,
                        &virtualizer_snapshot,
                        event,
                        window,
                        cx,
                    );
                }
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(
                        ScrollArea::new(
                            scroll_viewport_id,
                            render_virtualized_list_body(
                                &list_id,
                                &rows,
                                plan.virtualizer().total_size(),
                                active_indicator,
                                colors,
                                row_measure_mode,
                                estimated_row_height,
                                row_renderer,
                                list_state.clone(),
                                runtime.clone(),
                                focus_handle,
                                on_activate,
                                on_selection_change,
                                window,
                                cx,
                            ),
                        )
                        .vertical()
                        .scroll_handle(&scroll_handle)
                        .with_size(self.size),
                    )
                    .when_some(sticky_overlay, |this, overlay| {
                        this.child(render_virtualized_list_sticky_overlay(
                            list_id,
                            overlay,
                            colors,
                            estimated_row_height,
                            cx,
                        ))
                    }),
            )
    }
}

fn handle_virtualized_list_key_down(
    state: &VirtualizedListState,
    runtime: Entity<VirtualizedListRuntime>,
    scroll_handle: ScrollHandle,
    on_activate: Option<VirtualizedListActivationHandler>,
    on_selection_change: Option<VirtualizedListSelectionChangeHandler>,
    row_measure_mode: VirtualizedListRowMeasureMode,
    virtualizer_snapshot: &VirtualizerSnapshot,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if state.disabled() || state.visible_empty() {
        return;
    }

    let key = event.keystroke.key.as_str();
    let shift_only = virtualized_list_shift_only(event);
    if let Some(target) = state.navigation_target(key) {
        let Some(target) = state.target_at_index(target) else {
            return;
        };
        cx.stop_propagation();
        window.prevent_default();
        let runtime_anchor = runtime.read(cx).selection_anchor_key.clone();
        let anchor_key = shift_only
            .then(|| {
                state
                    .range_anchor_key(runtime_anchor.as_deref(), target.key())
                    .map(str::to_owned)
            })
            .flatten();
        let selection_change = if shift_only {
            state.range_selection_change(anchor_key.as_deref(), target.key())
        } else {
            None
        };
        runtime.update(cx, |runtime, _| {
            runtime.active_key = Some(target.key().to_owned());
            runtime.selection_anchor_key = if shift_only {
                anchor_key.clone().or_else(|| Some(target.key().to_owned()))
            } else {
                Some(target.key().to_owned())
            };
            if let Some(selection_change) = selection_change.as_ref() {
                runtime.selected_keys = selection_change.selected_key_set();
            }
            runtime.pending_scroll_to_active = Some(target.key().to_owned());
        });
        scroll_active_key(
            &scroll_handle,
            state,
            target.key(),
            row_measure_mode,
            virtualizer_snapshot,
        );
        if let (Some(on_selection_change), Some(selection_change)) =
            (on_selection_change.as_ref(), selection_change)
        {
            on_selection_change(selection_change, window, cx);
        }
        return;
    }

    if shift_only
        && key == "space"
        && state.selection_mode() == VirtualizedListSelectionMode::Multiple
    {
        let Some(active_key) = state.active_key() else {
            return;
        };
        cx.stop_propagation();
        window.prevent_default();
        let runtime_anchor = runtime.read(cx).selection_anchor_key.clone();
        let anchor_key = state
            .range_anchor_key(runtime_anchor.as_deref(), active_key)
            .map(str::to_owned);
        let selection_change = state.range_selection_change(anchor_key.as_deref(), active_key);
        runtime.update(cx, |runtime, _| {
            runtime.selection_anchor_key =
                anchor_key.clone().or_else(|| Some(active_key.to_owned()));
            if let Some(selection_change) = selection_change.as_ref() {
                runtime.selected_keys = selection_change.selected_key_set();
            }
        });
        if let (Some(on_selection_change), Some(selection_change)) =
            (on_selection_change.as_ref(), selection_change)
        {
            on_selection_change(selection_change, window, cx);
        }
        return;
    }

    if let Some(activation) = state.activation_for_key(key) {
        cx.stop_propagation();
        window.prevent_default();
        let selection_change = if state.selection_mode() == VirtualizedListSelectionMode::Single {
            state
                .target_at_index(activation.index())
                .and_then(|target| state.selection_change_for_target(&target))
        } else {
            None
        };
        runtime.update(cx, |runtime, _| {
            runtime.active_key = Some(activation.key().to_owned());
            runtime.selection_anchor_key = Some(activation.key().to_owned());
            if let Some(selection_change) = selection_change.as_ref() {
                runtime.selected_keys = selection_change.selected_key_set();
            }
            runtime.pending_scroll_to_active = Some(activation.key().to_owned());
        });
        scroll_active_key(
            &scroll_handle,
            state,
            activation.key(),
            row_measure_mode,
            virtualizer_snapshot,
        );
        if let (Some(on_selection_change), Some(selection_change)) =
            (on_selection_change.as_ref(), selection_change)
        {
            on_selection_change(selection_change, window, cx);
        }
        if let Some(on_activate) = on_activate.as_ref() {
            on_activate(activation, window, cx);
        }
        return;
    }

    if let Some(selection_change) = state.selection_change_for_key(key) {
        cx.stop_propagation();
        window.prevent_default();
        runtime.update(cx, |runtime, _| {
            runtime.selected_keys = selection_change.selected_key_set();
            runtime.selection_anchor_key = Some(selection_change.changed_key().to_owned());
        });
        if let Some(on_selection_change) = on_selection_change.as_ref() {
            on_selection_change(selection_change, window, cx);
        }
        return;
    }

    let Some(typeahead_key) = virtualized_list_typeahead_key(event) else {
        return;
    };

    cx.stop_propagation();
    window.prevent_default();
    let query = runtime.update(cx, |runtime, _| runtime.push_typeahead_key(&typeahead_key));
    if let Some(target) = state.typeahead_target(&query) {
        let target_key = target.key().to_owned();
        runtime.update(cx, |runtime, _| {
            runtime.active_key = Some(target_key.clone());
            runtime.selection_anchor_key = Some(target_key.clone());
            runtime.pending_scroll_to_active = Some(target_key.clone());
        });
        scroll_active_key(
            &scroll_handle,
            state,
            target_key.as_str(),
            row_measure_mode,
            virtualizer_snapshot,
        );
    }
}

fn virtualized_list_shift_only(event: &KeyDownEvent) -> bool {
    let modifiers = event.keystroke.modifiers;
    modifiers.shift
        && !modifiers.control
        && !modifiers.alt
        && !modifiers.platform
        && !modifiers.function
}

fn virtualized_list_typeahead_key(event: &KeyDownEvent) -> Option<String> {
    let modifiers = event.keystroke.modifiers;
    if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
        return None;
    }

    let key = event
        .keystroke
        .key_char
        .as_deref()
        .unwrap_or(event.keystroke.key.as_str());
    let mut chars = key.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || ch.is_control() {
        return None;
    }

    Some(ch.to_string())
}

fn scroll_active_key(
    scroll_handle: &ScrollHandle,
    state: &VirtualizedListState,
    key: &str,
    row_measure_mode: VirtualizedListRowMeasureMode,
    virtualizer_snapshot: &VirtualizerSnapshot,
) {
    let viewport_extent = state.viewport_extent();
    let current_scroll_offset = vertical_scroll_offset(scroll_handle);
    let target = match if row_measure_mode.measured() {
        state.scroll_target_for_key_with_snapshot(
            key,
            VirtualizedListScrollStrategy::Nearest,
            viewport_extent,
            current_scroll_offset,
            virtualizer_snapshot,
        )
    } else {
        state.scroll_target_for_key(
            key,
            VirtualizedListScrollStrategy::Nearest,
            viewport_extent,
            current_scroll_offset,
        )
    } {
        VirtualizedListRevealResult::Revealed(target)
        | VirtualizedListRevealResult::Estimated(target) => target,
        VirtualizedListRevealResult::NotFound(_)
        | VirtualizedListRevealResult::DuplicateKey(_)
        | VirtualizedListRevealResult::Disabled(_)
        | VirtualizedListRevealResult::StatusRow(_)
        | VirtualizedListRevealResult::StructuralRow(_)
        | VirtualizedListRevealResult::NotSelectable(_) => {
            return;
        }
    };

    if target.scroll_offset() != current_scroll_offset {
        set_vertical_scroll_offset_with_source(
            scroll_handle,
            target.scroll_offset(),
            ScrollViewportChangeSource::Programmatic(ScrollViewportProgrammaticSource::Reveal),
        );
    }
}

fn reveal_virtualized_list_key(
    scroll_handle: &ScrollHandle,
    state: &VirtualizedListState,
    key: &str,
    strategy: VirtualizedListScrollStrategy,
    row_measure_mode: VirtualizedListRowMeasureMode,
    virtualizer_snapshot: &VirtualizerSnapshot,
) -> VirtualizedListRevealResult {
    let viewport_extent = state.viewport_extent();
    let current_scroll_offset = vertical_scroll_offset(scroll_handle);
    let result = if row_measure_mode.measured() {
        state.scroll_target_for_key_with_snapshot(
            key,
            strategy,
            viewport_extent,
            current_scroll_offset,
            virtualizer_snapshot,
        )
    } else {
        state.scroll_target_for_key(key, strategy, viewport_extent, current_scroll_offset)
    };

    match &result {
        VirtualizedListRevealResult::Revealed(target)
        | VirtualizedListRevealResult::Estimated(target) => {
            if target.scroll_offset() != current_scroll_offset {
                set_vertical_scroll_offset_with_source(
                    scroll_handle,
                    target.scroll_offset(),
                    ScrollViewportChangeSource::Programmatic(
                        ScrollViewportProgrammaticSource::Reveal,
                    ),
                );
            }
        }
        VirtualizedListRevealResult::NotFound(_)
        | VirtualizedListRevealResult::DuplicateKey(_)
        | VirtualizedListRevealResult::Disabled(_)
        | VirtualizedListRevealResult::StatusRow(_)
        | VirtualizedListRevealResult::StructuralRow(_)
        | VirtualizedListRevealResult::NotSelectable(_) => {}
    }

    result
}

fn resolve_viewport_item_count(row_height: UiPx, viewport_extent: UiPx, fallback: usize) -> usize {
    let row_height = nonnegative_px(row_height);
    let viewport_extent = nonnegative_px(viewport_extent);
    if viewport_extent.as_f32() > 0.0 && row_height.as_f32() > 0.0 {
        (viewport_extent.as_f32() / row_height.as_f32())
            .ceil()
            .max(1.0) as usize
    } else {
        fallback.max(1)
    }
}
