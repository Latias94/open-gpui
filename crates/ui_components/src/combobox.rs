//! Combobox component built from editable text input, overlay, and listbox state.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, Entity, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, anchored,
    deferred, div,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayAnchorInput,
    OverlayLayerKind, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    OverlayPresence, Role, Sizable, Size, ThemeTokens, UiPx, rect, ui_point, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::listbox::{
    Listbox, ListboxGroup, ListboxGroupDescriptor, ListboxOption, ListboxOptionDescriptor,
    ListboxState,
};
use crate::overlay::{
    GpuiOverlayAdapterConfig, GpuiOverlayPlacement, OverlayResolvedState, gpui_overlay_state,
    outside_press_open_change,
};
use crate::scroll_area::{ScrollArea, ScrollAreaAxis, ScrollAreaState};
use crate::text_input::adapter::TextInputController;
use crate::text_input::{TextInput, TextInputState};
use crate::theme::ThemeResolver;

type ComboboxOpenChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type ComboboxSelectionHandler = Rc<dyn Fn(ComboboxSelection, &mut Window, &mut App)>;

/// Combobox open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComboboxOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

/// Pure descriptor for one combobox option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboboxOptionDescriptor {
    value: String,
    label: String,
    keywords: Vec<String>,
    disabled: bool,
}

impl ComboboxOptionDescriptor {
    /// Creates a selectable combobox option descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            keywords: Vec::new(),
            disabled: false,
        }
    }

    /// Adds one filtering keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keywords.push(keyword.into());
        self
    }

    /// Adds many filtering keywords.
    pub fn keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords.extend(keywords.into_iter().map(Into::into));
        self
    }

    /// Marks the option as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns stable option value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible option label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns filtering keywords.
    pub fn keywords_ref(&self) -> &[String] {
        &self.keywords
    }

    /// Returns whether the option is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    fn matches_query(&self, query: &str) -> bool {
        let query = normalize_query(query);
        if query.is_empty() {
            return true;
        }

        self.value.to_lowercase().contains(query.as_str())
            || self.label.to_lowercase().contains(query.as_str())
            || self
                .keywords
                .iter()
                .any(|keyword| keyword.to_lowercase().contains(query.as_str()))
    }

    fn to_listbox_descriptor(&self) -> ListboxOptionDescriptor {
        ListboxOptionDescriptor::option(self.value.clone(), self.label.clone())
            .disabled(self.disabled)
    }
}

/// Pure descriptor for one combobox option group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboboxGroupDescriptor {
    value: String,
    label: String,
    options: Vec<ComboboxOptionDescriptor>,
}

impl ComboboxGroupDescriptor {
    /// Creates an empty combobox group descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            options: Vec::new(),
        }
    }

    /// Adds one option descriptor.
    pub fn option(mut self, option: ComboboxOptionDescriptor) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many option descriptors.
    pub fn options(mut self, options: impl IntoIterator<Item = ComboboxOptionDescriptor>) -> Self {
        self.options.extend(options);
        self
    }

    /// Returns stable group value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible group label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns group options.
    pub fn options_ref(&self) -> &[ComboboxOptionDescriptor] {
        &self.options
    }
}

/// Resolved combobox color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComboboxColors {
    popup_background: ColorIntent,
    popup_foreground: ColorIntent,
    popup_border: ColorIntent,
    focus_ring: ColorIntent,
}

impl ComboboxColors {
    /// Returns popup background color intent.
    pub const fn popup_background(self) -> ColorIntent {
        self.popup_background
    }

    /// Returns popup foreground color intent.
    pub const fn popup_foreground(self) -> ColorIntent {
        self.popup_foreground
    }

    /// Returns popup border color intent.
    pub const fn popup_border(self) -> ColorIntent {
        self.popup_border
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved combobox metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComboboxMetrics {
    popup_padding: UiPx,
    popup_radius: UiPx,
    popup_min_width: UiPx,
    popup_max_width: UiPx,
    popup_max_height: UiPx,
}

impl ComboboxMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            popup_padding: ui_px(4.0),
            popup_radius: size.control_radius(),
            popup_min_width: ui_px(260.0),
            popup_max_width: ui_px(420.0),
            popup_max_height: match size {
                Size::XSmall => ui_px(180.0),
                Size::Small => ui_px(220.0),
                Size::Medium => ui_px(280.0),
                Size::Large => ui_px(340.0),
            },
        }
    }

    /// Returns popup padding.
    pub const fn popup_padding(self) -> UiPx {
        self.popup_padding
    }

    /// Returns popup corner radius.
    pub const fn popup_radius(self) -> UiPx {
        self.popup_radius
    }

    /// Returns popup minimum width.
    pub const fn popup_min_width(self) -> UiPx {
        self.popup_min_width
    }

    /// Returns popup maximum width.
    pub const fn popup_max_width(self) -> UiPx {
        self.popup_max_width
    }

    /// Returns popup maximum height.
    pub const fn popup_max_height(self) -> UiPx {
        self.popup_max_height
    }
}

/// Selection payload emitted by a combobox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboboxSelection {
    value: String,
    label: String,
}

impl ComboboxSelection {
    /// Creates a selection payload.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }

    /// Returns selected value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns selected label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved combobox state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ComboboxState {
    size: Size,
    disabled: bool,
    required: bool,
    open: bool,
    default_open: bool,
    open_mode: ComboboxOpenMode,
    label: String,
    placeholder: String,
    query: String,
    total_option_count: usize,
    filtered_option_count: usize,
    empty_label: String,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    input: TextInputState,
    listbox: ListboxState,
    scroll_area: ScrollAreaState,
    metrics: ComboboxMetrics,
    colors: ComboboxColors,
    focus_ring: FocusRing,
    overlay: OverlayResolvedState,
}

impl ComboboxState {
    /// Resolves public state for a combobox.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        disabled: bool,
        required: bool,
        open: Option<bool>,
        default_open: bool,
        label: impl Into<String>,
        placeholder: impl Into<String>,
        query: impl Into<String>,
        selected_value: Option<&str>,
        active_value: Option<&str>,
        empty_label: impl Into<String>,
        groups: impl IntoIterator<Item = ComboboxGroupDescriptor>,
        options: impl IntoIterator<Item = ComboboxOptionDescriptor>,
        placement_side: OverlayPlacementSide,
        placement_alignment: OverlayPlacementAlignment,
        outside_press_policy: OutsidePressPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let label = label.into();
        let placeholder = placeholder.into();
        let query = query.into();
        let empty_label = empty_label.into();
        let open_mode = if open.is_some() {
            ComboboxOpenMode::Controlled
        } else {
            ComboboxOpenMode::Uncontrolled
        };
        let open = open.unwrap_or(default_open) && !disabled;
        let raw_groups = groups.into_iter().collect::<Vec<_>>();
        let raw_options = options.into_iter().collect::<Vec<_>>();
        let total_option_count = raw_options.len()
            + raw_groups
                .iter()
                .map(|group| group.options_ref().len())
                .sum::<usize>();
        let filtered_options = raw_options
            .iter()
            .filter(|option| option.matches_query(query.as_str()))
            .map(ComboboxOptionDescriptor::to_listbox_descriptor)
            .collect::<Vec<_>>();
        let filtered_groups = raw_groups
            .iter()
            .filter_map(|group| {
                let options = group
                    .options_ref()
                    .iter()
                    .filter(|option| option.matches_query(query.as_str()))
                    .map(ComboboxOptionDescriptor::to_listbox_descriptor)
                    .collect::<Vec<_>>();
                (!options.is_empty()).then(|| {
                    ListboxGroupDescriptor::new(group.value().to_owned(), group.label().to_owned())
                        .options(options)
                })
            })
            .collect::<Vec<_>>();
        let filtered_option_count = filtered_options.len()
            + filtered_groups
                .iter()
                .map(|group| group.options_ref().len())
                .sum::<usize>();
        let selected_option = selected_value
            .and_then(|value| find_combobox_option(&raw_groups, &raw_options, value))
            .filter(|option| !option.disabled_state());
        let selected_value = selected_option.map(|option| option.value().to_owned());
        let listbox = ListboxState::resolve(
            size,
            disabled,
            label.clone(),
            selected_value.as_deref(),
            active_value,
            (!query.is_empty()).then_some(query.as_str()),
            empty_label.clone(),
            filtered_groups,
            filtered_options,
            tokens,
        );
        let input = TextInputState::resolve(
            query.clone(),
            Some(placeholder.clone()),
            size,
            disabled,
            false,
            false,
            required,
            false,
            tokens,
        );
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
                .resolved_state();
        let scroll_area = ScrollAreaState::resolve(
            format!("{label}:combobox-content-scroll"),
            ScrollAreaAxis::Vertical,
            size,
            crate::scroll_area::ScrollResetPolicy::Preserve,
            None,
        );
        let colors = ThemeResolver::combobox_colors(tokens);

        Self {
            size,
            disabled,
            required,
            open,
            default_open,
            open_mode,
            label,
            placeholder,
            query,
            total_option_count,
            filtered_option_count,
            empty_label,
            placement_side,
            placement_alignment,
            outside_press_policy,
            initial_focus_intent,
            focus_restore_intent,
            input,
            listbox,
            scroll_area,
            metrics: ComboboxMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
            overlay,
        }
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the combobox is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether a value is required.
    pub const fn required(&self) -> bool {
        self.required
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
    pub const fn open_mode(&self) -> ComboboxOpenMode {
        self.open_mode
    }

    /// Returns accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns current query text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns selected option value.
    pub fn selected_value(&self) -> Option<&str> {
        self.listbox.selected_value()
    }

    /// Returns active option value.
    pub fn active_value(&self) -> Option<&str> {
        self.listbox.active_value()
    }

    /// Returns unfiltered option count.
    pub const fn total_option_count(&self) -> usize {
        self.total_option_count
    }

    /// Returns filtered option count.
    pub const fn filtered_option_count(&self) -> usize {
        self.filtered_option_count
    }

    /// Returns empty-state label.
    pub fn empty_label(&self) -> &str {
        &self.empty_label
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

    /// Returns input role.
    pub const fn input_role(&self) -> Role {
        Role::EditableComboBox
    }

    /// Returns popup content role.
    pub const fn content_role(&self) -> Role {
        Role::ListBox
    }

    /// Returns whether query filtering removed options.
    pub const fn filtered(&self) -> bool {
        self.filtered_option_count != self.total_option_count
    }

    /// Returns whether the visible option list is empty.
    pub const fn empty(&self) -> bool {
        self.filtered_option_count == 0
    }

    /// Returns whether popup content should use a scroll viewport.
    pub const fn scrollable_content(&self) -> bool {
        self.listbox.scrollable_content()
    }

    /// Returns resolved input state.
    pub const fn input(&self) -> &TextInputState {
        &self.input
    }

    /// Returns nested listbox state.
    pub const fn listbox(&self) -> &ListboxState {
        &self.listbox
    }

    /// Returns nested scroll area state.
    pub const fn scroll_area(&self) -> &ScrollAreaState {
        &self.scroll_area
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> ComboboxMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> ComboboxColors {
        self.colors
    }

    /// Returns focus-ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }
}

#[derive(Debug, Clone)]
struct ComboboxRuntime {
    open: bool,
    active_value: Option<String>,
    selected_value: Option<String>,
}

/// A concrete GPUI combobox component.
#[derive(IntoElement)]
pub struct Combobox {
    id: ElementId,
    label: SharedString,
    placeholder: SharedString,
    options: Vec<ComboboxOption>,
    groups: Vec<ComboboxGroup>,
    size: Size,
    disabled: bool,
    required: bool,
    open: Option<bool>,
    default_open: bool,
    query: String,
    selected_value: Option<String>,
    active_value: Option<String>,
    empty_label: SharedString,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<ComboboxOpenChangeHandler>,
    on_select: Option<ComboboxSelectionHandler>,
}

impl Combobox {
    /// Creates an empty combobox.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            placeholder: "Search".into(),
            options: Vec::new(),
            groups: Vec::new(),
            size: Size::Medium,
            disabled: false,
            required: false,
            open: None,
            default_open: false,
            query: String::new(),
            selected_value: None,
            active_value: None,
            empty_label: "No results".into(),
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            initial_focus_intent: InitialFocusIntent::None,
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
    pub fn option(mut self, option: ComboboxOption) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many standalone options.
    pub fn options(mut self, options: impl IntoIterator<Item = ComboboxOption>) -> Self {
        self.options.extend(options);
        self
    }

    /// Adds one option group.
    pub fn group(mut self, group: ComboboxGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds many option groups.
    pub fn groups(mut self, groups: impl IntoIterator<Item = ComboboxGroup>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Marks the combobox as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the combobox as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
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

    /// Applies initial query.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
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

    /// Applies empty-state label.
    pub fn empty_label(mut self, label: impl Into<SharedString>) -> Self {
        self.empty_label = label.into();
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

    /// Registers a combobox selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(ComboboxSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns resolved combobox state.
    pub fn state(&self) -> ComboboxState {
        ComboboxState::resolve(
            self.size,
            self.disabled,
            self.required,
            self.open,
            self.default_open,
            self.label.to_string(),
            self.placeholder.to_string(),
            self.query.as_str(),
            self.selected_value.as_deref(),
            self.active_value.as_deref(),
            self.empty_label.to_string(),
            self.groups.iter().map(ComboboxGroup::descriptor),
            self.options.iter().map(ComboboxOption::descriptor),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for Combobox {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Combobox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| ComboboxRuntime {
            open: self.default_open,
            active_value: self.active_value.clone(),
            selected_value: self.selected_value.clone(),
        });
        let input_state_key: ElementId = (self.id.clone(), "input-state").into();
        let input_controller = window.use_keyed_state(input_state_key, cx, |_, cx| {
            let mut input = TextInputController::with_value(self.query.clone(), cx);
            input.set_placeholder(self.placeholder.clone(), cx);
            input
        });
        let runtime_state = runtime.read(cx).clone();
        let controlled_open = self.open;
        let resolved_open = controlled_open.unwrap_or(runtime_state.open);

        if controlled_open.is_some() && runtime_state.open != resolved_open {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let query = input_controller.read(cx).value().to_owned();
        let selected_value = self
            .selected_value
            .as_deref()
            .or(runtime_state.selected_value.as_deref());
        let active_value = self
            .active_value
            .as_deref()
            .or(runtime_state.active_value.as_deref())
            .or(selected_value);
        let state = ComboboxState::resolve(
            self.size,
            self.disabled,
            self.required,
            Some(resolved_open),
            self.default_open,
            self.label.to_string(),
            self.placeholder.to_string(),
            query.as_str(),
            selected_value,
            active_value,
            self.empty_label.to_string(),
            self.groups.iter().map(ComboboxGroup::descriptor),
            self.options.iter().map(ComboboxOption::descriptor),
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        input_controller.update(cx, |controller, cx| {
            if controller.placeholder() != self.placeholder.as_ref() {
                controller.set_placeholder(self.placeholder.clone(), cx);
            }
        });
        let id = self.id;
        let debug_id = id.to_string();
        let input_id: ElementId = (id.clone(), "input").into();
        let input_row_id: ElementId = (id.clone(), "input-row").into();
        let toggle_id: ElementId = (id.clone(), "toggle").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let listbox_id: ElementId = (id.clone(), "listbox").into();
        let metrics = state.metrics();
        let open = state.open();
        let disabled = state.disabled();
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement = GpuiOverlayPlacement::resolve(
            OverlayPlacementInput::new(
                OverlayAnchorInput::from_layout_bounds(rect(
                    ui_point(ui_px(0.0), ui_px(0.0)),
                    ui_size(metrics.popup_min_width(), state.input().metrics().height()),
                )),
                ui_size(metrics.popup_min_width(), state.input().metrics().height()),
            )
            .with_side(state.placement_side())
            .with_alignment(state.placement_alignment())
            .with_offset(ui_px(4.0)),
            overlay_adapter.snap_margin(),
        );

        div()
            .id(id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("combobox:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div()
                    .id(input_row_id)
                    .debug_selector({
                        let debug_id = debug_id.clone();
                        move || format!("combobox:{debug_id}:input-row")
                    })
                    .min_w(gpui_px_from_ui(metrics.popup_min_width()))
                    .max_w(gpui_px_from_ui(metrics.popup_max_width()))
                    .flex()
                    .items_center()
                    .gap_1()
                    .focusable()
                    .ui_role(state.input_role())
                    .aria_label(state.label().to_owned())
                    .aria_expanded(open)
                    .aria_disabled(disabled)
                    .on_key_down({
                        let runtime = runtime.clone();
                        let input_controller = input_controller.clone();
                        let on_open_change = self.on_open_change.clone();
                        let on_select = self.on_select.clone();
                        let key_state = state.clone();
                        move |event: &KeyDownEvent, window, cx| match combobox_keyboard_action(
                            &key_state,
                            event.keystroke.key.as_str(),
                        ) {
                            ComboboxKeyboardAction::Navigate(value) => {
                                cx.stop_propagation();
                                window.prevent_default();
                                runtime.update(cx, |runtime, _| {
                                    runtime.open = true;
                                    runtime.active_value = Some(value);
                                });
                                if !key_state.open() {
                                    if let Some(on_open_change) = on_open_change.as_ref() {
                                        on_open_change(true, window, cx);
                                    }
                                }
                            }
                            ComboboxKeyboardAction::Select(selection) => {
                                cx.stop_propagation();
                                window.prevent_default();
                                runtime.update(cx, |runtime, _| {
                                    runtime.selected_value = Some(selection.value().to_owned());
                                    runtime.active_value = Some(selection.value().to_owned());
                                    runtime.open = false;
                                });
                                input_controller.update(cx, |controller, cx| {
                                    controller.set_value(selection.label().to_owned(), cx);
                                });
                                if let Some(on_select) = on_select.as_ref() {
                                    on_select(selection, window, cx);
                                }
                                if let Some(on_open_change) = on_open_change.as_ref() {
                                    on_open_change(false, window, cx);
                                }
                            }
                            ComboboxKeyboardAction::Open => {
                                cx.stop_propagation();
                                window.prevent_default();
                                runtime.update(cx, |runtime, _| {
                                    runtime.open = true;
                                });
                                if let Some(on_open_change) = on_open_change.as_ref() {
                                    on_open_change(true, window, cx);
                                }
                            }
                            ComboboxKeyboardAction::Close => {
                                cx.stop_propagation();
                                window.prevent_default();
                                close_combobox(runtime.clone(), on_open_change.clone(), window, cx);
                            }
                            ComboboxKeyboardAction::Ignore => {}
                        }
                    })
                    .child(
                        TextInput::new(input_id, state.label().to_owned())
                            .controller(input_controller.clone())
                            .placeholder(state.placeholder().to_owned())
                            .value(query)
                            .disabled(state.disabled())
                            .required(state.required())
                            .tokens(self.tokens)
                            .with_size(state.size()),
                    )
                    .child(
                        div()
                            .id(toggle_id)
                            .debug_selector({
                                let debug_id = debug_id.clone();
                                move || format!("combobox:{debug_id}:toggle")
                            })
                            .px_2()
                            .py_1()
                            .rounded(gpui_px_from_ui(state.input().metrics().radius()))
                            .border_1()
                            .border_color(ThemeResolver::resolve(state.colors().popup_border()))
                            .text_color(ThemeResolver::resolve(state.colors().popup_foreground()))
                            .ui_role(Role::Button)
                            .focus_visible({
                                let focus_ring = state.focus_ring();
                                move |style| style.shadow(focus_ring_shadow(focus_ring))
                            })
                            .focusable()
                            .tab_stop(!disabled)
                            .aria_label("Toggle combobox popup")
                            .aria_expanded(open)
                            .aria_disabled(disabled)
                            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                            .when(!disabled, |this| {
                                let runtime = runtime.clone();
                                let on_open_change = self.on_open_change.clone();
                                this.cursor_pointer().on_click(
                                    move |_event: &ClickEvent, window, cx| {
                                        cx.stop_propagation();
                                        let next_open = !open;
                                        runtime.update(cx, |runtime, _| {
                                            runtime.open = next_open;
                                        });
                                        if let Some(on_open_change) = on_open_change.as_ref() {
                                            on_open_change(next_open, window, cx);
                                        }
                                    },
                                )
                            })
                            .child(if open { "^" } else { "v" }),
                    ),
            )
            .when(open, |this| {
                this.child(
                    deferred(
                        anchored()
                            .anchor(placement.anchor())
                            .offset(placement.offset())
                            .snap_to_window_with_margin(placement.snap_margin())
                            .child(combobox_content_element(
                                content_id.clone(),
                                listbox_id.clone(),
                                debug_id.clone(),
                                state.clone(),
                                self.options,
                                self.groups,
                                input_controller.clone(),
                                runtime.clone(),
                                self.on_open_change.clone(),
                                self.on_select.clone(),
                                self.tokens,
                            )),
                    )
                    .priority(overlay_adapter.deferred_priority()),
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ComboboxKeyboardAction {
    Navigate(String),
    Select(ComboboxSelection),
    Open,
    Close,
    Ignore,
}

fn combobox_keyboard_action(state: &ComboboxState, key: &str) -> ComboboxKeyboardAction {
    if state.disabled() {
        return ComboboxKeyboardAction::Ignore;
    }

    if let Some(target) = state.listbox().navigation_target(key) {
        return ComboboxKeyboardAction::Navigate(target.value().to_owned());
    }

    if matches!(key, "down" | "up" | "home" | "end") {
        return ComboboxKeyboardAction::Open;
    }

    if let Some(selection) = state.listbox().activation_for_key(key) {
        return ComboboxKeyboardAction::Select(ComboboxSelection::new(
            selection.value().to_owned(),
            selection.label().to_owned(),
        ));
    }

    if key == "escape" && state.open() {
        return ComboboxKeyboardAction::Close;
    }

    ComboboxKeyboardAction::Ignore
}

#[allow(clippy::too_many_arguments)]
fn combobox_content_element(
    content_id: ElementId,
    listbox_id: ElementId,
    debug_id: String,
    state: ComboboxState,
    options: Vec<ComboboxOption>,
    groups: Vec<ComboboxGroup>,
    input_controller: Entity<TextInputController>,
    runtime: Entity<ComboboxRuntime>,
    on_open_change: Option<ComboboxOpenChangeHandler>,
    on_select: Option<ComboboxSelectionHandler>,
    tokens: ThemeTokens,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let selected_value = state.selected_value().map(str::to_owned);
    let active_value = state.active_value().map(str::to_owned);
    let query = state.query().to_owned();
    let label = state.label().to_owned();
    let listbox = options
        .into_iter()
        .filter(|option| option.descriptor.matches_query(query.as_str()))
        .fold(
            Listbox::new(listbox_id, label.clone()),
            |listbox, option| listbox.option(option.listbox_option()),
        )
        .groups(
            groups
                .into_iter()
                .filter_map(|group| group.filtered_listbox_group(query.as_str())),
        )
        .tokens(tokens)
        .with_size(state.size())
        .empty_label(state.empty_label().to_owned())
        .embedded(true)
        .on_select({
            let input_controller = input_controller.clone();
            let runtime = runtime.clone();
            let on_select = on_select.clone();
            let on_open_change = on_open_change.clone();
            move |selection, window, cx| {
                let payload = ComboboxSelection::new(
                    selection.value().to_owned(),
                    selection.label().to_owned(),
                );
                runtime.update(cx, |runtime, _| {
                    runtime.selected_value = Some(payload.value().to_owned());
                    runtime.active_value = Some(payload.value().to_owned());
                    runtime.open = false;
                });
                input_controller.update(cx, |controller, cx| {
                    controller.set_value(payload.label().to_owned(), cx);
                });
                if let Some(on_select) = on_select.as_ref() {
                    on_select(payload, window, cx);
                }
                if let Some(on_open_change) = on_open_change.as_ref() {
                    on_open_change(false, window, cx);
                }
            }
        });
    let listbox = if let Some(selected_value) = selected_value {
        listbox.selected(selected_value)
    } else {
        listbox
    };
    let listbox = if let Some(active_value) = active_value {
        listbox.active(active_value)
    } else {
        listbox
    };
    let scroll_viewport_id = state.scroll_area().viewport_id().to_owned();
    let escape_runtime = runtime.clone();
    let escape_open_change = on_open_change.clone();

    div()
        .id(content_id)
        .debug_selector(move || format!("combobox:{debug_id}:content"))
        .min_w(gpui_px_from_ui(metrics.popup_min_width()))
        .max_w(gpui_px_from_ui(metrics.popup_max_width()))
        .p(gpui_px_from_ui(metrics.popup_padding()))
        .h(gpui_px_from_ui(metrics.popup_max_height()))
        .flex()
        .flex_col()
        .rounded(gpui_px_from_ui(metrics.popup_radius()))
        .border_1()
        .border_color(ThemeResolver::resolve(colors.popup_border()))
        .bg(ThemeResolver::resolve(colors.popup_background()))
        .text_color(ThemeResolver::resolve(colors.popup_foreground()))
        .shadow_lg()
        .occlude()
        .ui_role(state.content_role())
        .aria_label(label)
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key.as_str() == "escape" {
                cx.stop_propagation();
                window.prevent_default();
                close_combobox(
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
                close_combobox(runtime.clone(), on_open_change.clone(), window, cx);
            })
        })
        .child(
            ScrollArea::new(scroll_viewport_id, listbox)
                .vertical()
                .preserve_scroll()
                .with_size(state.size()),
        )
}

fn close_combobox(
    runtime: Entity<ComboboxRuntime>,
    on_open_change: Option<ComboboxOpenChangeHandler>,
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

/// A concrete GPUI combobox option.
#[derive(Clone)]
pub struct ComboboxOption {
    descriptor: ComboboxOptionDescriptor,
}

impl ComboboxOption {
    /// Creates a selectable combobox option.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ComboboxOptionDescriptor::new(value, label.to_string()),
        }
    }

    /// Adds one filtering keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.keyword(keyword);
        self
    }

    /// Marks the option as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Returns the pure descriptor.
    pub fn descriptor(&self) -> ComboboxOptionDescriptor {
        self.descriptor.clone()
    }

    fn listbox_option(self) -> ListboxOption {
        ListboxOption::new(self.descriptor.value, self.descriptor.label)
            .disabled(self.descriptor.disabled)
    }
}

/// A concrete GPUI combobox group.
#[derive(Clone)]
pub struct ComboboxGroup {
    descriptor: ComboboxGroupDescriptor,
    options: Vec<ComboboxOption>,
}

impl ComboboxGroup {
    /// Creates an empty combobox group.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ComboboxGroupDescriptor::new(value, label.to_string()),
            options: Vec::new(),
        }
    }

    /// Adds one option.
    pub fn option(mut self, option: ComboboxOption) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many options.
    pub fn options(mut self, options: impl IntoIterator<Item = ComboboxOption>) -> Self {
        self.options.extend(options);
        self
    }

    /// Returns the group descriptor.
    pub fn descriptor(&self) -> ComboboxGroupDescriptor {
        self.options
            .iter()
            .fold(self.descriptor.clone(), |descriptor, option| {
                descriptor.option(option.descriptor())
            })
    }

    fn filtered_listbox_group(self, query: &str) -> Option<ListboxGroup> {
        let mut group = ListboxGroup::new(self.descriptor.value, self.descriptor.label);
        let mut has_options = false;
        for option in self.options {
            if option.descriptor.matches_query(query) {
                has_options = true;
                group = group.option(option.listbox_option());
            }
        }
        has_options.then_some(group)
    }
}

fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
}

fn find_combobox_option<'a>(
    groups: &'a [ComboboxGroupDescriptor],
    options: &'a [ComboboxOptionDescriptor],
    value: &str,
) -> Option<&'a ComboboxOptionDescriptor> {
    options
        .iter()
        .find(|option| option.value() == value)
        .or_else(|| {
            groups
                .iter()
                .flat_map(ComboboxGroupDescriptor::options_ref)
                .find(|option| option.value() == value)
        })
}

impl ThemeResolver {
    pub(crate) const fn combobox_colors(tokens: ThemeTokens) -> ComboboxColors {
        ComboboxColors {
            popup_background: ColorIntent::new(tokens.surface, 0xffffff),
            popup_foreground: ColorIntent::new(tokens.text, 0x18202a),
            popup_border: ColorIntent::new(tokens.border, 0xcfd5cc),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                0x2f80ed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyboard_state(disabled: bool) -> ComboboxState {
        Combobox::new("frameworks", "Frameworks")
            .open(true)
            .disabled(disabled)
            .query("re")
            .selected("solid")
            .option(ComboboxOption::new("react", "React"))
            .option(ComboboxOption::new("solid", "Solid"))
            .option(ComboboxOption::new("relay", "Relay"))
            .state()
    }

    #[test]
    fn keyboard_action_moves_and_selects_active_option() {
        let state = keyboard_state(false);

        assert_eq!(
            combobox_keyboard_action(&state, "down"),
            ComboboxKeyboardAction::Navigate("relay".to_string())
        );
        assert_eq!(
            combobox_keyboard_action(&state, "enter"),
            ComboboxKeyboardAction::Select(ComboboxSelection::new(
                "react".to_string(),
                "React".to_string(),
            ))
        );
        assert_eq!(
            combobox_keyboard_action(&state, "escape"),
            ComboboxKeyboardAction::Close
        );
    }

    #[test]
    fn keyboard_action_ignores_disabled_combobox() {
        let state = keyboard_state(true);

        assert_eq!(
            combobox_keyboard_action(&state, "down"),
            ComboboxKeyboardAction::Ignore
        );
        assert_eq!(
            combobox_keyboard_action(&state, "enter"),
            ComboboxKeyboardAction::Ignore
        );
    }
}
