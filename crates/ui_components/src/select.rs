//! Select component built from a trigger, overlay, and listbox state.

use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, KeyDownEvent, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point, px, size,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayAnchorInput,
    OverlayLayerKind, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    OverlayPresence, Role, Sizable, Size, ThemeTokens, rect,
};

use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::listbox::{
    Listbox, ListboxGroup, ListboxGroupDescriptor, ListboxOption, ListboxOptionDescriptor,
    ListboxState,
};
use crate::overlay::{
    DEFAULT_OVERLAY_SAFE_MARGIN, GpuiOverlayAdapterConfig, GpuiOverlayPlacement, GpuiOverlayState,
    outside_press_open_change,
};
use crate::scroll_area::{ScrollArea, ScrollAreaAxis, ScrollAreaState};
use crate::theme::ThemeResolver;

type SelectOpenChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type SelectSelectionHandler = Rc<dyn Fn(SelectSelection, &mut Window, &mut App)>;

/// Select open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

/// Resolved select color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectColors {
    trigger_background: ColorIntent,
    trigger_hover_background: ColorIntent,
    trigger_foreground: ColorIntent,
    trigger_placeholder_foreground: ColorIntent,
    trigger_border: ColorIntent,
    content_background: ColorIntent,
    content_foreground: ColorIntent,
    content_border: ColorIntent,
    focus_ring: ColorIntent,
}

impl SelectColors {
    /// Returns trigger background color intent.
    pub const fn trigger_background(self) -> ColorIntent {
        self.trigger_background
    }

    /// Returns trigger hover background color intent.
    pub const fn trigger_hover_background(self) -> ColorIntent {
        self.trigger_hover_background
    }

    /// Returns trigger foreground color intent.
    pub const fn trigger_foreground(self) -> ColorIntent {
        self.trigger_foreground
    }

    /// Returns placeholder foreground color intent.
    pub const fn trigger_placeholder_foreground(self) -> ColorIntent {
        self.trigger_placeholder_foreground
    }

    /// Returns trigger border color intent.
    pub const fn trigger_border(self) -> ColorIntent {
        self.trigger_border
    }

    /// Returns content background color intent.
    pub const fn content_background(self) -> ColorIntent {
        self.content_background
    }

    /// Returns content foreground color intent.
    pub const fn content_foreground(self) -> ColorIntent {
        self.content_foreground
    }

    /// Returns content border color intent.
    pub const fn content_border(self) -> ColorIntent {
        self.content_border
    }

    /// Returns trigger focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved select metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectMetrics {
    trigger_height: open_gpui::Pixels,
    trigger_padding_x: open_gpui::Pixels,
    trigger_padding_y: open_gpui::Pixels,
    content_padding: open_gpui::Pixels,
    radius: open_gpui::Pixels,
    text_size: open_gpui::Pixels,
    min_width: open_gpui::Pixels,
    max_width: open_gpui::Pixels,
    max_height: open_gpui::Pixels,
}

impl SelectMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            content_padding: px(4.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
            min_width: px(220.0),
            max_width: px(360.0),
            max_height: match size {
                Size::XSmall => px(180.0),
                Size::Small => px(220.0),
                Size::Medium => px(260.0),
                Size::Large => px(320.0),
            },
        }
    }

    /// Returns trigger height.
    pub const fn trigger_height(self) -> open_gpui::Pixels {
        self.trigger_height
    }

    /// Returns trigger horizontal padding.
    pub const fn trigger_padding_x(self) -> open_gpui::Pixels {
        self.trigger_padding_x
    }

    /// Returns trigger vertical padding.
    pub const fn trigger_padding_y(self) -> open_gpui::Pixels {
        self.trigger_padding_y
    }

    /// Returns content padding.
    pub const fn content_padding(self) -> open_gpui::Pixels {
        self.content_padding
    }

    /// Returns corner radius.
    pub const fn radius(self) -> open_gpui::Pixels {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> open_gpui::Pixels {
        self.text_size
    }

    /// Returns minimum content width.
    pub const fn min_width(self) -> open_gpui::Pixels {
        self.min_width
    }

    /// Returns maximum content width.
    pub const fn max_width(self) -> open_gpui::Pixels {
        self.max_width
    }

    /// Returns maximum content height.
    pub const fn max_height(self) -> open_gpui::Pixels {
        self.max_height
    }
}

/// Selection payload emitted by a select.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectSelection {
    index: usize,
    value: String,
    label: String,
}

impl SelectSelection {
    /// Creates a select selection payload.
    pub fn new(index: usize, value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            index,
            value: value.into(),
            label: label.into(),
        }
    }

    /// Returns the flattened option index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns selected option value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns selected option label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl From<crate::listbox::ListboxSelection> for SelectSelection {
    fn from(selection: crate::listbox::ListboxSelection) -> Self {
        Self {
            index: selection.index(),
            value: selection.value().to_owned(),
            label: selection.label().to_owned(),
        }
    }
}

/// Resolved select state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectState {
    size: Size,
    disabled: bool,
    open: bool,
    default_open: bool,
    open_mode: SelectOpenMode,
    label: String,
    placeholder: String,
    trigger_label: String,
    selected_value: Option<String>,
    active_value: Option<String>,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    metrics: SelectMetrics,
    colors: SelectColors,
    focus_ring: FocusRing,
    listbox: ListboxState,
    scroll_area: ScrollAreaState,
    overlay: GpuiOverlayState,
}

impl SelectState {
    /// Resolves public state for a select.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        label: impl Into<String>,
        placeholder: impl Into<String>,
        selected_value: Option<&str>,
        active_value: Option<&str>,
        groups: impl IntoIterator<Item = ListboxGroupDescriptor>,
        options: impl IntoIterator<Item = ListboxOptionDescriptor>,
        placement_side: OverlayPlacementSide,
        placement_alignment: OverlayPlacementAlignment,
        outside_press_policy: OutsidePressPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let label = label.into();
        let placeholder = placeholder.into();
        let open_mode = if open.is_some() {
            SelectOpenMode::Controlled
        } else {
            SelectOpenMode::Uncontrolled
        };
        let open = open.unwrap_or(default_open) && !disabled;
        let group_descriptors = groups.into_iter().collect::<Vec<_>>();
        let option_descriptors = options.into_iter().collect::<Vec<_>>();
        let listbox = ListboxState::resolve(
            size,
            disabled,
            label.clone(),
            selected_value,
            active_value,
            None,
            "No options",
            group_descriptors.clone(),
            option_descriptors.clone(),
            tokens,
        );
        let trigger_label = selected_value
            .and_then(|value| {
                listbox
                    .options()
                    .iter()
                    .find(|option| option.value() == value && option.focusable())
            })
            .map_or_else(|| placeholder.clone(), |option| option.label().to_owned());
        let selected_value = listbox.selected_value().map(str::to_owned);
        let active_value = listbox.active_value().map(str::to_owned);
        let presence = if open {
            OverlayPresence::open()
        } else {
            OverlayPresence::hidden()
        };
        let overlay =
            GpuiOverlayAdapterConfig::new(OverlayLayerKind::NonModalDismissible, presence)
                .outside_press_policy(outside_press_policy)
                .initial_focus_intent(initial_focus_intent.clone())
                .focus_restore_intent(focus_restore_intent.clone())
                .snap_margin(DEFAULT_OVERLAY_SAFE_MARGIN)
                .state();
        let scroll_area = ScrollAreaState::resolve(
            format!("{label}:select-content-scroll"),
            ScrollAreaAxis::Vertical,
            size,
            crate::scroll_area::ScrollResetPolicy::Preserve,
            None,
        );
        let colors = ThemeResolver::select_colors(tokens, open);

        Self {
            size,
            disabled,
            open,
            default_open,
            open_mode,
            label,
            placeholder,
            trigger_label,
            selected_value,
            active_value,
            placement_side,
            placement_alignment,
            outside_press_policy,
            initial_focus_intent,
            focus_restore_intent,
            metrics: SelectMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
            listbox,
            scroll_area,
            overlay,
        }
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the trigger is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the popup is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> SelectOpenMode {
        self.open_mode
    }

    /// Returns accessible select label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns visible trigger label.
    pub fn trigger_label(&self) -> &str {
        &self.trigger_label
    }

    /// Returns selected option value.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected_value.as_deref()
    }

    /// Returns active option value.
    pub fn active_value(&self) -> Option<&str> {
        self.active_value.as_deref()
    }

    /// Returns preferred placement side.
    pub const fn placement_side(&self) -> OverlayPlacementSide {
        self.placement_side
    }

    /// Returns preferred placement alignment.
    pub const fn placement_alignment(&self) -> OverlayPlacementAlignment {
        self.placement_alignment
    }

    /// Returns outside-press policy.
    pub const fn outside_press_policy(&self) -> OutsidePressPolicy {
        self.outside_press_policy
    }

    /// Returns initial focus intent.
    pub fn initial_focus_intent(&self) -> &InitialFocusIntent {
        &self.initial_focus_intent
    }

    /// Returns focus restore intent.
    pub fn focus_restore_intent(&self) -> &FocusRestoreIntent {
        &self.focus_restore_intent
    }

    /// Returns trigger accessibility role.
    pub const fn trigger_role(&self) -> Role {
        Role::Button
    }

    /// Returns content accessibility role.
    pub const fn content_role(&self) -> Role {
        Role::ListBox
    }

    /// Returns whether the trigger is visually selected.
    pub const fn trigger_selected(&self) -> bool {
        self.open
    }

    /// Returns whether content should use a scroll viewport.
    pub fn scrollable_content(&self) -> bool {
        self.listbox.options().len() > 6
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> SelectMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> SelectColors {
        self.colors
    }

    /// Returns focus-ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Returns nested listbox state.
    pub const fn listbox(&self) -> &ListboxState {
        &self.listbox
    }

    /// Returns nested scroll area state.
    pub const fn scroll_area(&self) -> &ScrollAreaState {
        &self.scroll_area
    }

    /// Returns renderer-facing overlay state.
    pub const fn overlay(&self) -> &GpuiOverlayState {
        &self.overlay
    }
}

#[derive(Debug, Clone)]
struct SelectRuntime {
    open: bool,
    active_value: Option<String>,
    selected_value: Option<String>,
}

/// A concrete GPUI select component.
#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    label: SharedString,
    placeholder: SharedString,
    options: Vec<ListboxOption>,
    groups: Vec<ListboxGroup>,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    selected_value: Option<String>,
    active_value: Option<String>,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<SelectOpenChangeHandler>,
    on_select: Option<SelectSelectionHandler>,
}

impl Select {
    /// Creates an empty select.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            placeholder: "Select an option".into(),
            options: Vec::new(),
            groups: Vec::new(),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            selected_value: None,
            active_value: None,
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_open_change: None,
            on_select: None,
        }
    }

    /// Applies placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Adds one standalone option.
    pub fn option(mut self, option: ListboxOption) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many standalone options.
    pub fn options(mut self, options: impl IntoIterator<Item = ListboxOption>) -> Self {
        self.options.extend(options);
        self
    }

    /// Adds one option group.
    pub fn group(mut self, group: ListboxGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds many option groups.
    pub fn groups(mut self, groups: impl IntoIterator<Item = ListboxGroup>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Marks the select as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies controlled open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Applies uncontrolled initial open state.
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Applies selected option value.
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected_value = Some(value.into());
        self
    }

    /// Applies active option value.
    pub fn active(mut self, value: impl Into<String>) -> Self {
        self.active_value = Some(value.into());
        self
    }

    /// Applies preferred placement.
    pub fn placement(
        mut self,
        side: OverlayPlacementSide,
        alignment: OverlayPlacementAlignment,
    ) -> Self {
        self.placement_side = side;
        self.placement_alignment = alignment;
        self
    }

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies initial focus intent.
    pub fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus_intent = intent;
        self
    }

    /// Applies focus restore intent.
    pub fn focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore_intent = intent;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an open-change handler with the next open value.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Registers a select selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(SelectSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns resolved select state.
    pub fn state(&self) -> SelectState {
        SelectState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.label.to_string(),
            self.placeholder.to_string(),
            self.selected_value.as_deref(),
            self.active_value.as_deref(),
            self.groups.iter().map(ListboxGroup::descriptor),
            self.options.iter().map(ListboxOption::descriptor),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for Select {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Select {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| SelectRuntime {
            open: self.default_open,
            active_value: self.active_value.clone(),
            selected_value: self.selected_value.clone(),
        });
        let runtime_state = runtime.read(cx).clone();
        let controlled_open = self.open;
        let resolved_open = controlled_open.unwrap_or(runtime_state.open);

        if controlled_open.is_some() && runtime_state.open != resolved_open {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let selected_value = self
            .selected_value
            .as_deref()
            .or(runtime_state.selected_value.as_deref());
        let active_value = self
            .active_value
            .as_deref()
            .or(runtime_state.active_value.as_deref())
            .or(selected_value);
        let state = SelectState::resolve(
            self.size,
            self.disabled,
            Some(resolved_open),
            self.default_open,
            self.label.to_string(),
            self.placeholder.to_string(),
            selected_value,
            active_value,
            self.groups.iter().map(ListboxGroup::descriptor),
            self.options.iter().map(ListboxOption::descriptor),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let id = self.id;
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let listbox_id: ElementId = (id.clone(), "listbox").into();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let open = state.open();
        let selected = state.selected_value().is_some();
        let trigger_label = state.trigger_label().to_owned();
        let placement = GpuiOverlayPlacement::resolve(
            OverlayPlacementInput::new(
                OverlayAnchorInput::from_layout_bounds(rect(
                    point(px(0.0), px(0.0)),
                    size(metrics.min_width(), metrics.trigger_height()),
                )),
                open_gpui_ui_core::OverlaySize::new(metrics.min_width(), metrics.trigger_height()),
            )
            .with_side(state.placement_side())
            .with_alignment(state.placement_alignment())
            .with_offset(px(4.0)),
            state.overlay().snap_margin(),
        );

        div()
            .id(id)
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div()
                    .id(trigger_id)
                    .min_w(metrics.min_width())
                    .max_w(metrics.max_width())
                    .min_h(metrics.trigger_height())
                    .px(metrics.trigger_padding_x())
                    .py(metrics.trigger_padding_y())
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .rounded(metrics.radius())
                    .border_1()
                    .border_color(ThemeResolver::resolve(colors.trigger_border()))
                    .bg(ThemeResolver::resolve(colors.trigger_background()))
                    .text_color(ThemeResolver::resolve(if selected {
                        colors.trigger_foreground()
                    } else {
                        colors.trigger_placeholder_foreground()
                    }))
                    .text_size(metrics.text_size())
                    .line_height(metrics.text_size())
                    .focusable()
                    .tab_stop(!disabled)
                    .role(state.trigger_role())
                    .aria_label(state.label().to_owned())
                    .aria_selected(state.trigger_selected())
                    .aria_expanded(open)
                    .aria_disabled(disabled)
                    .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
                    .on_key_down({
                        let runtime = runtime.clone();
                        let on_open_change = self.on_open_change.clone();
                        let initial_active = state.active_value().map(str::to_owned);
                        move |event: &KeyDownEvent, window, cx| {
                            let key = event.keystroke.key.as_str();
                            if matches!(key, "enter" | "space" | "down" | "up") {
                                cx.stop_propagation();
                                window.prevent_default();
                                runtime.update(cx, |runtime, _| {
                                    runtime.open = true;
                                    if runtime.active_value.is_none() {
                                        runtime.active_value = initial_active.clone();
                                    }
                                });
                                if let Some(on_open_change) = on_open_change.as_ref() {
                                    on_open_change(true, window, cx);
                                }
                            } else if key == "escape" {
                                cx.stop_propagation();
                                window.prevent_default();
                                close_select(runtime.clone(), on_open_change.clone(), window, cx);
                            }
                        }
                    })
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = self.on_open_change.clone();
                        let initial_active = state.active_value().map(str::to_owned);
                        this.cursor_pointer()
                            .hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.trigger_hover_background()))
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                let next_open = !open;
                                runtime.update(cx, |runtime, _| {
                                    runtime.open = next_open;
                                    if next_open && runtime.active_value.is_none() {
                                        runtime.active_value = initial_active.clone();
                                    }
                                });
                                if let Some(on_open_change) = on_open_change.as_ref() {
                                    on_open_change(next_open, window, cx);
                                }
                            })
                    })
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .truncate()
                            .child(trigger_label),
                    )
                    .child(div().child(if open { "^" } else { "v" })),
            )
            .when(open, |this| {
                this.child(
                    deferred(
                        anchored()
                            .anchor(placement.anchor())
                            .offset(placement.offset())
                            .snap_to_window_with_margin(placement.snap_margin())
                            .child(select_content_element(
                                content_id.clone(),
                                listbox_id.clone(),
                                state.clone(),
                                self.options,
                                self.groups,
                                runtime.clone(),
                                self.on_open_change.clone(),
                                self.on_select.clone(),
                                self.tokens,
                            )),
                    )
                    .priority(state.overlay().deferred_priority()),
                )
            })
    }
}

#[allow(clippy::too_many_arguments)]
fn select_content_element(
    content_id: ElementId,
    listbox_id: ElementId,
    state: SelectState,
    options: Vec<ListboxOption>,
    groups: Vec<ListboxGroup>,
    runtime: open_gpui::Entity<SelectRuntime>,
    on_open_change: Option<SelectOpenChangeHandler>,
    on_select: Option<SelectSelectionHandler>,
    tokens: ThemeTokens,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let escape_runtime = runtime.clone();
    let escape_open_change = on_open_change.clone();
    let listbox_runtime = runtime.clone();
    let listbox_open_change = on_open_change.clone();
    let listbox_select = on_select.clone();
    let selected_value = state.selected_value().map(str::to_owned);
    let active_value = state.active_value().map(str::to_owned);
    let label = state.label().to_owned();
    let listbox = options
        .into_iter()
        .fold(
            Listbox::new(listbox_id, label.clone()),
            |listbox, option| listbox.option(option),
        )
        .groups(groups)
        .tokens(tokens)
        .with_size(state.size())
        .embedded(true)
        .on_select(move |selection, window, cx| {
            let selection = SelectSelection::from(selection);
            listbox_runtime.update(cx, |runtime, _| {
                runtime.selected_value = Some(selection.value().to_owned());
                runtime.active_value = Some(selection.value().to_owned());
                runtime.open = false;
            });
            if let Some(on_select) = listbox_select.as_ref() {
                on_select(selection, window, cx);
            }
            if let Some(on_open_change) = listbox_open_change.as_ref() {
                on_open_change(false, window, cx);
            }
        });
    let listbox = apply_optional_values(listbox, selected_value, active_value);

    let scroll_viewport_id = state.scroll_area().viewport_id().to_owned();

    div()
        .id(content_id)
        .min_w(metrics.min_width())
        .max_w(metrics.max_width())
        .p(metrics.content_padding())
        .h(metrics.max_height())
        .flex()
        .flex_col()
        .rounded(metrics.radius())
        .border_1()
        .border_color(ThemeResolver::resolve(colors.content_border()))
        .bg(ThemeResolver::resolve(colors.content_background()))
        .text_color(ThemeResolver::resolve(colors.content_foreground()))
        .text_size(metrics.text_size())
        .line_height(metrics.text_size())
        .shadow_lg()
        .occlude()
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key.as_str() == "escape" {
                cx.stop_propagation();
                window.prevent_default();
                close_select(
                    escape_runtime.clone(),
                    escape_open_change.clone(),
                    window,
                    cx,
                );
            }
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            this.on_mouse_down_out(move |_, window, cx| {
                close_select(runtime.clone(), on_open_change.clone(), window, cx);
            })
        })
        .child(
            ScrollArea::new(scroll_viewport_id, listbox)
                .vertical()
                .preserve_scroll()
                .with_size(state.size()),
        )
}

fn apply_optional_values(
    mut listbox: Listbox,
    selected_value: Option<String>,
    active_value: Option<String>,
) -> Listbox {
    if let Some(selected_value) = selected_value {
        listbox = listbox.selected(selected_value);
    }
    if let Some(active_value) = active_value {
        listbox = listbox.active(active_value);
    }
    listbox
}

fn close_select(
    runtime: open_gpui::Entity<SelectRuntime>,
    on_open_change: Option<SelectOpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        runtime.open = false;
    });
    if let Some(on_open_change) = on_open_change.as_ref() {
        on_open_change(false, window, cx);
    }
}

impl ThemeResolver {
    pub(crate) const fn select_colors(tokens: ThemeTokens, open: bool) -> SelectColors {
        let trigger_state = if open {
            ColorState::Selected
        } else {
            ColorState::Default
        };

        SelectColors {
            trigger_background: ColorIntent::with_state(
                tokens.surface_muted,
                trigger_state,
                0xf6f7f2,
            ),
            trigger_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            trigger_foreground: ColorIntent::new(tokens.text, 0x18202a),
            trigger_placeholder_foreground: ColorIntent::new(tokens.text_muted, 0x5a6472),
            trigger_border: ColorIntent::new(tokens.border, 0xcfd5cc),
            content_background: ColorIntent::new(tokens.surface, 0xffffff),
            content_foreground: ColorIntent::new(tokens.text, 0x18202a),
            content_border: ColorIntent::new(tokens.border, 0xcfd5cc),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                0x2f80ed,
            ),
        }
    }
}
