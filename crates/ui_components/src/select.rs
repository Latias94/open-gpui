//! Select component built from a trigger, overlay, and listbox state.
use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ElementId, IntoElement, KeyDownEvent, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayAnchorInput,
    OverlayLayerKind, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide, Role,
    Sizable, Size, ThemeTokens, UiPx, rect, ui_point, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::listbox::{
    Listbox, ListboxGroup, ListboxGroupDescriptor, ListboxOption, ListboxOptionDescriptor,
    ListboxState,
};
use crate::overlay::{
    GpuiOverlayPlacement, OverlayDisclosureConfig, OverlayDisclosureOpenMode, OverlayResolvedState,
    consume_overlay_event, emit_overlay_open_change, gpui_overlay_state,
    gpui_relative_overlay_layer, outside_press_open_change, resolve_overlay_open_state,
    set_overlay_open,
};
use crate::scroll_area::{ScrollArea, ScrollAreaAxis, ScrollAreaState};
use crate::theme::{ThemeContext, ThemeResolver};

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

const fn select_open_mode_from_disclosure(mode: OverlayDisclosureOpenMode) -> SelectOpenMode {
    match mode {
        OverlayDisclosureOpenMode::Uncontrolled => SelectOpenMode::Uncontrolled,
        OverlayDisclosureOpenMode::Controlled => SelectOpenMode::Controlled,
    }
}

/// Resolved select color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectColors {
    pub(crate) trigger_background: ColorIntent,
    pub(crate) trigger_hover_background: ColorIntent,
    pub(crate) trigger_foreground: ColorIntent,
    pub(crate) trigger_placeholder_foreground: ColorIntent,
    pub(crate) trigger_border: ColorIntent,
    pub(crate) content_background: ColorIntent,
    pub(crate) content_foreground: ColorIntent,
    pub(crate) content_border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
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
    trigger_height: UiPx,
    trigger_padding_x: UiPx,
    trigger_padding_y: UiPx,
    content_padding: UiPx,
    radius: UiPx,
    text_size: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    max_height: UiPx,
}

impl SelectMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            content_padding: ui_px(4.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
            min_width: ui_px(220.0),
            max_width: ui_px(360.0),
            max_height: match size {
                Size::XSmall => ui_px(180.0),
                Size::Small => ui_px(220.0),
                Size::Medium => ui_px(260.0),
                Size::Large => ui_px(320.0),
            },
        }
    }

    /// Returns trigger height.
    pub const fn trigger_height(self) -> UiPx {
        self.trigger_height
    }

    /// Returns trigger horizontal padding.
    pub const fn trigger_padding_x(self) -> UiPx {
        self.trigger_padding_x
    }

    /// Returns trigger vertical padding.
    pub const fn trigger_padding_y(self) -> UiPx {
        self.trigger_padding_y
    }

    /// Returns content padding.
    pub const fn content_padding(self) -> UiPx {
        self.content_padding
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns minimum content width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns maximum content width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }

    /// Returns maximum content height.
    pub const fn max_height(self) -> UiPx {
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
    overlay: OverlayResolvedState,
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
        let disclosure = OverlayDisclosureConfig::new(OverlayLayerKind::NonModalDismissible)
            .controlled_open(open)
            .default_open(default_open)
            .disabled(disabled)
            .outside_press_policy(outside_press_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .resolve();
        let open = disclosure.open();
        let open_mode = select_open_mode_from_disclosure(disclosure.open_mode());
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
        let overlay = disclosure.overlay().clone();
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
        self.listbox
            .selected_option()
            .filter(|option| option.focusable())
            .map(|option| option.label())
            .unwrap_or(self.placeholder.as_str())
    }

    /// Returns selected option value.
    pub fn selected_value(&self) -> Option<&str> {
        self.listbox.selected_value()
    }

    /// Returns active option value.
    pub fn active_value(&self) -> Option<&str> {
        self.listbox.active_value()
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
    pub const fn scrollable_content(&self) -> bool {
        self.listbox.scrollable_content()
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

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
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
    full_width: bool,
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
            full_width: false,
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

    /// Makes the trigger expand to the full width of its parent.
    pub fn full_width(mut self, full_width: bool) -> Self {
        self.full_width = full_width;
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
        let theme = ThemeResolver::current(cx);
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| SelectRuntime {
            open: self.default_open,
            active_value: self.active_value.clone(),
            selected_value: self.selected_value.clone(),
        });
        let runtime_state = runtime.read(cx).clone();
        let open_state = resolve_overlay_open_state(self.open, runtime_state.open);
        let resolved_open = open_state.open();

        if open_state.runtime_changed() {
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
        let explicit_active_value = self.active_value.clone();
        let id = self.id;
        let debug_id = id.to_string();
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let listbox_id: ElementId = (id.clone(), "listbox").into();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let open = state.open();
        let selected = state.selected_value().is_some();
        let trigger_border = theme.resolve(colors.trigger_border());
        let trigger_background = theme.resolve(colors.trigger_background());
        let trigger_foreground = theme.resolve(if selected {
            colors.trigger_foreground()
        } else {
            colors.trigger_placeholder_foreground()
        });
        let trigger_hover_background = theme.resolve(colors.trigger_hover_background());
        let trigger_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
        let trigger_label = state.trigger_label().to_owned();
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement = GpuiOverlayPlacement::resolve(
            OverlayPlacementInput::new(
                OverlayAnchorInput::from_layout_bounds(rect(
                    ui_point(ui_px(0.0), ui_px(0.0)),
                    ui_size(metrics.min_width(), metrics.trigger_height()),
                )),
                ui_size(metrics.min_width(), metrics.trigger_height()),
            )
            .with_side(state.placement_side())
            .with_alignment(state.placement_alignment())
            .with_offset(ui_px(4.0)),
            overlay_adapter.snap_margin(),
        );

        div()
            .id(id.clone())
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("select:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .when(self.full_width, |this| this.w_full().items_stretch())
            .when(!self.full_width, |this| this.items_start())
            .when(self.full_width, |this| this.occlude())
            .child(
                div()
                    .id(trigger_id)
                    .debug_selector({
                        let debug_id = debug_id.clone();
                        move || format!("select:{debug_id}:trigger")
                    })
                    .when(self.full_width, |this| this.w_full())
                    .when(!self.full_width, |this| {
                        this.min_w(gpui_px_from_ui(metrics.min_width()))
                            .max_w(gpui_px_from_ui(metrics.max_width()))
                    })
                    .min_h(gpui_px_from_ui(metrics.trigger_height()))
                    .px(gpui_px_from_ui(metrics.trigger_padding_x()))
                    .py(gpui_px_from_ui(metrics.trigger_padding_y()))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .rounded(gpui_px_from_ui(metrics.radius()))
                    .border_1()
                    .border_color(trigger_border)
                    .bg(trigger_background)
                    .text_color(trigger_foreground)
                    .text_size(gpui_px_from_ui(metrics.text_size()))
                    .line_height(gpui_px_from_ui(metrics.text_size()))
                    .focusable()
                    .tab_stop(!disabled)
                    .ui_role(state.trigger_role())
                    .aria_label(state.label().to_owned())
                    .aria_selected(state.trigger_selected())
                    .aria_expanded(open)
                    .aria_disabled(disabled)
                    .focus_visible(move |style| style.shadow(trigger_focus_shadow.clone()))
                    .on_key_down({
                        let runtime = runtime.clone();
                        let on_open_change = self.on_open_change.clone();
                        move |event: &KeyDownEvent, window, cx| {
                            let key = event.keystroke.key.as_str();
                            if matches!(key, "enter" | "space" | "down" | "up") {
                                consume_overlay_event(window, cx);
                                runtime.update(cx, |runtime, _| {
                                    set_overlay_open(&mut runtime.open, true);
                                });
                                emit_overlay_open_change(
                                    true,
                                    on_open_change.as_deref(),
                                    window,
                                    cx,
                                );
                            } else if key == "escape" {
                                consume_overlay_event(window, cx);
                                close_select(runtime.clone(), on_open_change.clone(), window, cx);
                            }
                        }
                    })
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = self.on_open_change.clone();
                        this.cursor_pointer()
                            .hover(move |style| style.bg(trigger_hover_background))
                            .capture_any_mouse_up(move |_, window, cx| {
                                consume_overlay_event(window, cx);
                                let next_open = !open;
                                runtime.update(cx, |runtime, _| {
                                    set_overlay_open(&mut runtime.open, next_open);
                                });
                                emit_overlay_open_change(
                                    next_open,
                                    on_open_change.as_deref(),
                                    window,
                                    cx,
                                );
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
                this.child(gpui_relative_overlay_layer(
                    &overlay_adapter,
                    &placement,
                    select_content_element(
                        content_id.clone(),
                        listbox_id.clone(),
                        state.clone(),
                        explicit_active_value.clone(),
                        self.options,
                        self.groups,
                        runtime.clone(),
                        self.on_open_change.clone(),
                        self.on_select.clone(),
                        self.tokens,
                        &theme,
                    ),
                ))
            })
    }
}

#[allow(clippy::too_many_arguments)]
fn select_content_element(
    content_id: ElementId,
    listbox_id: ElementId,
    state: SelectState,
    explicit_active_value: Option<String>,
    options: Vec<ListboxOption>,
    groups: Vec<ListboxGroup>,
    runtime: open_gpui::Entity<SelectRuntime>,
    on_open_change: Option<SelectOpenChangeHandler>,
    on_select: Option<SelectSelectionHandler>,
    tokens: ThemeTokens,
    theme: &ThemeContext,
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
                set_overlay_open(&mut runtime.open, false);
            });
            if let Some(on_select) = listbox_select.as_ref() {
                on_select(selection, window, cx);
            }
            emit_overlay_open_change(false, listbox_open_change.as_deref(), window, cx);
        });
    let mut listbox = listbox;
    if let Some(selected_value) = selected_value {
        listbox = listbox.selected(selected_value);
    }
    if let Some(active_value) = explicit_active_value {
        listbox = listbox.active(active_value);
    }

    let scroll_viewport_id = state.scroll_area().viewport_id().to_owned();

    div()
        .id(content_id)
        .debug_selector({
            let viewport_id = scroll_viewport_id.clone();
            move || format!("select:{viewport_id}:content")
        })
        .min_w(gpui_px_from_ui(metrics.min_width()))
        .max_w(gpui_px_from_ui(metrics.max_width()))
        .p(gpui_px_from_ui(metrics.content_padding()))
        .h(gpui_px_from_ui(metrics.max_height()))
        .flex()
        .flex_col()
        .rounded(gpui_px_from_ui(metrics.radius()))
        .border_1()
        .border_color(theme.resolve(colors.content_border()))
        .bg(theme.resolve(colors.content_background()))
        .text_color(theme.resolve(colors.content_foreground()))
        .text_size(gpui_px_from_ui(metrics.text_size()))
        .line_height(gpui_px_from_ui(metrics.text_size()))
        .shadow_lg()
        .occlude()
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key.as_str() == "escape" {
                consume_overlay_event(window, cx);
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

fn close_select(
    runtime: open_gpui::Entity<SelectRuntime>,
    on_open_change: Option<SelectOpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    runtime.update(cx, |runtime, _| {
        set_overlay_open(&mut runtime.open, false);
    });
    emit_overlay_open_change(false, on_open_change.as_deref(), window, cx);
}
