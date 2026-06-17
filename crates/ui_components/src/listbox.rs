//! Listbox component and shared collection navigation state.

use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, Entity, IntoElement, KeyDownEvent, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, UiPx, ui_px};

use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::roving_focus::{first_enabled, last_enabled, next_enabled};
use crate::theme::ThemeResolver;

type ListboxSelectHandler = Rc<dyn Fn(ListboxSelection, &mut Window, &mut App)>;

/// Listbox option anatomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListboxOptionKind {
    /// Selectable option.
    Option,
    /// Visual separator. Separators are not focusable or selectable.
    Separator,
}

impl ListboxOptionKind {
    /// Returns a stable kind label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Option => "option",
            Self::Separator => "separator",
        }
    }
}

/// Pure descriptor for a listbox option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListboxOptionDescriptor {
    value: String,
    label: String,
    kind: ListboxOptionKind,
    disabled: bool,
}

impl ListboxOptionDescriptor {
    /// Creates a selectable option descriptor.
    pub fn option(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            kind: ListboxOptionKind::Option,
            disabled: false,
        }
    }

    /// Creates a visual separator descriptor.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: String::new(),
            kind: ListboxOptionKind::Separator,
            disabled: true,
        }
    }

    /// Marks the selectable option as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        if self.kind == ListboxOptionKind::Option {
            self.disabled = disabled;
        }
        self
    }

    /// Returns the stable option value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible option label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the option kind.
    pub const fn kind(&self) -> ListboxOptionKind {
        self.kind
    }

    /// Returns whether the option is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns whether the descriptor participates in active-item navigation.
    pub const fn focusable(&self) -> bool {
        matches!(self.kind, ListboxOptionKind::Option) && !self.disabled
    }
}

/// Pure descriptor for a listbox group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListboxGroupDescriptor {
    value: String,
    label: String,
    options: Vec<ListboxOptionDescriptor>,
}

impl ListboxGroupDescriptor {
    /// Creates an empty group descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            options: Vec::new(),
        }
    }

    /// Adds one option descriptor.
    pub fn option(mut self, option: ListboxOptionDescriptor) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many option descriptors.
    pub fn options(mut self, options: impl IntoIterator<Item = ListboxOptionDescriptor>) -> Self {
        self.options.extend(options);
        self
    }

    /// Returns the stable group value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible group label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns group options.
    pub fn options_ref(&self) -> &[ListboxOptionDescriptor] {
        &self.options
    }
}

/// Resolved listbox color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListboxColors {
    pub(crate) surface: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) muted_foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) option_background: ColorIntent,
    pub(crate) option_hover_background: ColorIntent,
    pub(crate) option_active_background: ColorIntent,
    pub(crate) option_selected_background: ColorIntent,
    pub(crate) option_disabled_foreground: ColorIntent,
    pub(crate) separator: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl ListboxColors {
    /// Returns listbox surface color intent.
    pub const fn surface(self) -> ColorIntent {
        self.surface
    }

    /// Returns foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns muted foreground color intent.
    pub const fn muted_foreground(self) -> ColorIntent {
        self.muted_foreground
    }

    /// Returns border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns default option background color intent.
    pub const fn option_background(self) -> ColorIntent {
        self.option_background
    }

    /// Returns hovered option background color intent.
    pub const fn option_hover_background(self) -> ColorIntent {
        self.option_hover_background
    }

    /// Returns active option background color intent.
    pub const fn option_active_background(self) -> ColorIntent {
        self.option_active_background
    }

    /// Returns selected option background color intent.
    pub const fn option_selected_background(self) -> ColorIntent {
        self.option_selected_background
    }

    /// Returns disabled option foreground color intent.
    pub const fn option_disabled_foreground(self) -> ColorIntent {
        self.option_disabled_foreground
    }

    /// Returns separator color intent.
    pub const fn separator(self) -> ColorIntent {
        self.separator
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved listbox metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListboxMetrics {
    surface_padding: UiPx,
    option_height: UiPx,
    option_padding_x: UiPx,
    option_padding_y: UiPx,
    group_padding_x: UiPx,
    separator_height: UiPx,
    radius: UiPx,
    text_size: UiPx,
    min_width: UiPx,
    max_height: UiPx,
}

impl ListboxMetrics {
    /// Resolves listbox metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            surface_padding: ui_px(6.0),
            option_height: size.button_h(),
            option_padding_x: size.button_px(),
            option_padding_y: ui_px(6.0),
            group_padding_x: size.button_px(),
            separator_height: ui_px(1.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
            min_width: ui_px(220.0),
            max_height: ui_px(240.0),
        }
    }

    /// Returns listbox surface padding.
    pub const fn surface_padding(self) -> UiPx {
        self.surface_padding
    }

    /// Returns option row height.
    pub const fn option_height(self) -> UiPx {
        self.option_height
    }

    /// Returns option horizontal padding.
    pub const fn option_padding_x(self) -> UiPx {
        self.option_padding_x
    }

    /// Returns option vertical padding.
    pub const fn option_padding_y(self) -> UiPx {
        self.option_padding_y
    }

    /// Returns group label horizontal padding.
    pub const fn group_padding_x(self) -> UiPx {
        self.group_padding_x
    }

    /// Returns separator height.
    pub const fn separator_height(self) -> UiPx {
        self.separator_height
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns minimum listbox width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns maximum listbox height.
    pub const fn max_height(self) -> UiPx {
        self.max_height
    }
}

/// Resolved listbox group state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListboxGroupState {
    index: usize,
    value: String,
    label: String,
    option_count: usize,
}

impl ListboxGroupState {
    /// Returns the zero-based group index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable group value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible group label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the count of selectable options in the group.
    pub const fn option_count(&self) -> usize {
        self.option_count
    }

    /// Returns group accessibility role.
    pub const fn role(&self) -> Role {
        Role::Group
    }
}

/// Resolved listbox option state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListboxOptionState {
    index: usize,
    group_index: Option<usize>,
    value: String,
    label: String,
    kind: ListboxOptionKind,
    disabled: bool,
    selected: bool,
    active: bool,
    tab_stop: bool,
    position_in_set: Option<usize>,
    size_of_set: usize,
}

impl ListboxOptionState {
    /// Returns the zero-based flattened option index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the owning group index, if grouped.
    pub const fn group_index(&self) -> Option<usize> {
        self.group_index
    }

    /// Returns the stable option value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible option label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns option anatomy.
    pub const fn kind(&self) -> ListboxOptionKind {
        self.kind
    }

    /// Returns whether the option is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the option can receive active focus.
    pub const fn focusable(&self) -> bool {
        matches!(self.kind, ListboxOptionKind::Option) && !self.disabled
    }

    /// Returns whether the option is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the option is the active descendant.
    pub const fn active(&self) -> bool {
        self.active
    }

    /// Returns whether the option is the current tab stop.
    pub const fn tab_stop(&self) -> bool {
        self.tab_stop
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        self.focusable()
    }

    /// Returns option accessibility role.
    pub const fn role(&self) -> Option<Role> {
        match self.kind {
            ListboxOptionKind::Option => Some(Role::ListBoxOption),
            ListboxOptionKind::Separator => None,
        }
    }

    /// Returns the one-based position in the selectable option set.
    pub const fn position_in_set(&self) -> Option<usize> {
        self.position_in_set
    }

    /// Returns the total selectable option set size.
    pub const fn size_of_set(&self) -> usize {
        self.size_of_set
    }
}

/// Resolved listbox selection payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListboxSelection {
    index: usize,
    value: String,
    label: String,
}

impl ListboxSelection {
    /// Creates a selection payload from an option state.
    pub fn from_option(option: &ListboxOptionState) -> Option<Self> {
        option.activation_enabled().then(|| Self {
            index: option.index,
            value: option.value.clone(),
            label: option.label.clone(),
        })
    }

    /// Returns the selected option index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the selected option value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the selected option label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug, Clone)]
struct FlattenedOption {
    group_index: Option<usize>,
    descriptor: ListboxOptionDescriptor,
}

/// Resolved listbox state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ListboxState {
    size: Size,
    disabled: bool,
    label: String,
    selected_value: Option<String>,
    active_value: Option<String>,
    typeahead_query: Option<String>,
    empty_label: String,
    groups: Vec<ListboxGroupState>,
    options: Vec<ListboxOptionState>,
    active_index: Option<usize>,
    selected_index: Option<usize>,
    metrics: ListboxMetrics,
    colors: ListboxColors,
    focus_ring: FocusRing,
}

impl ListboxState {
    /// Resolves public state for a listbox.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        disabled: bool,
        label: impl Into<String>,
        selected_value: Option<&str>,
        active_value: Option<&str>,
        typeahead_query: Option<&str>,
        empty_label: impl Into<String>,
        groups: impl IntoIterator<Item = ListboxGroupDescriptor>,
        options: impl IntoIterator<Item = ListboxOptionDescriptor>,
        tokens: ThemeTokens,
    ) -> Self {
        let label = label.into();
        let empty_label = empty_label.into();
        let group_descriptors: Vec<ListboxGroupDescriptor> = groups.into_iter().collect();
        let standalone_options: Vec<ListboxOptionDescriptor> = options.into_iter().collect();
        let groups = group_descriptors
            .iter()
            .enumerate()
            .map(|(index, group)| ListboxGroupState {
                index,
                value: group.value.clone(),
                label: group.label.clone(),
                option_count: group
                    .options
                    .iter()
                    .filter(|option| option.kind() == ListboxOptionKind::Option)
                    .count(),
            })
            .collect::<Vec<_>>();
        let flattened = flatten_listbox_options(&group_descriptors, standalone_options);
        let disabled_map = flattened
            .iter()
            .map(|option| !option.descriptor.focusable())
            .collect::<Vec<_>>();
        let selected_index = if disabled {
            None
        } else {
            selected_value.and_then(|value| {
                flattened.iter().position(|option| {
                    option.descriptor.value() == value && option.descriptor.focusable()
                })
            })
        };
        let active_index = if disabled || flattened.is_empty() {
            None
        } else {
            active_value
                .and_then(|value| {
                    flattened.iter().position(|option| {
                        option.descriptor.value() == value && option.descriptor.focusable()
                    })
                })
                .or(selected_index)
                .or_else(|| first_enabled(&disabled_map))
        };
        let selectable_count = flattened
            .iter()
            .filter(|option| option.descriptor.kind() == ListboxOptionKind::Option)
            .count();
        let mut position = 0usize;
        let options = flattened
            .into_iter()
            .enumerate()
            .map(|(index, option)| {
                let kind = option.descriptor.kind();
                let position_in_set = if kind == ListboxOptionKind::Option {
                    position += 1;
                    Some(position)
                } else {
                    None
                };
                let selected = selected_index == Some(index);
                let active = active_index == Some(index);
                let focusable = option.descriptor.focusable();

                ListboxOptionState {
                    index,
                    group_index: option.group_index,
                    value: option.descriptor.value,
                    label: option.descriptor.label,
                    kind,
                    disabled: option.descriptor.disabled,
                    selected,
                    active,
                    tab_stop: active && focusable,
                    position_in_set,
                    size_of_set: selectable_count,
                }
            })
            .collect::<Vec<_>>();
        let selected_value = selected_index
            .and_then(|index| options.get(index).map(|option| option.value().to_owned()));
        let active_value = active_index
            .and_then(|index| options.get(index).map(|option| option.value().to_owned()));
        let colors = ThemeResolver::listbox_colors(tokens);

        Self {
            size,
            disabled,
            label,
            selected_value,
            active_value,
            typeahead_query: typeahead_query.map(str::to_owned),
            empty_label,
            groups,
            options,
            active_index,
            selected_index,
            metrics: ListboxMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the whole listbox is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the accessible listbox label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns selected option value.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected_value.as_deref()
    }

    /// Returns active descendant value.
    pub fn active_value(&self) -> Option<&str> {
        self.active_value.as_deref()
    }

    /// Returns typeahead query metadata.
    pub fn typeahead_query(&self) -> Option<&str> {
        self.typeahead_query.as_deref()
    }

    /// Returns empty-state label.
    pub fn empty_label(&self) -> &str {
        &self.empty_label
    }

    /// Returns resolved group states.
    pub fn groups(&self) -> &[ListboxGroupState] {
        &self.groups
    }

    /// Returns flattened option states.
    pub fn options(&self) -> &[ListboxOptionState] {
        &self.options
    }

    /// Returns active option index.
    pub const fn active_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Returns selected option index.
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Returns current tab-stop value.
    pub fn tab_stop_value(&self) -> Option<&str> {
        self.options
            .iter()
            .find(|option| option.tab_stop())
            .map(ListboxOptionState::value)
    }

    /// Returns whether the listbox has no selectable option rows.
    pub fn empty(&self) -> bool {
        self.options
            .iter()
            .all(|option| option.kind() != ListboxOptionKind::Option)
    }

    /// Resolves a navigation target for an APG-style listbox key.
    pub fn navigation_target(&self, key: &str) -> Option<&ListboxOptionState> {
        let current = self.active_index?;
        let disabled = self.disabled_map();
        listbox_navigation_target(key, current, &disabled).and_then(|index| self.options.get(index))
    }

    /// Resolves a typeahead target for a query.
    pub fn typeahead_target(&self, query: &str) -> Option<&ListboxOptionState> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return None;
        }

        let len = self.options.len();
        if len == 0 {
            return None;
        }

        let start = self.active_index.map_or(0, |index| (index + 1) % len);
        (0..len)
            .map(|step| (start + step) % len)
            .filter_map(|index| self.options.get(index))
            .find(|option| {
                option.focusable() && option.label().to_lowercase().starts_with(query.as_str())
            })
    }

    /// Resolves an activation payload for an APG-style activation key.
    pub fn activation_for_key(&self, key: &str) -> Option<ListboxSelection> {
        if !matches!(key, "enter" | "space") {
            return None;
        }

        self.active_index
            .and_then(|index| self.options.get(index))
            .and_then(ListboxSelection::from_option)
    }

    /// Returns listbox accessibility role.
    pub const fn role(&self) -> Role {
        Role::ListBox
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> ListboxMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> ListboxColors {
        self.colors
    }

    /// Returns focus-ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    fn disabled_map(&self) -> Vec<bool> {
        self.options
            .iter()
            .map(|option| !option.focusable())
            .collect()
    }
}

/// Resolves a listbox active descendant target from an APG-style key name.
pub fn listbox_navigation_target(key: &str, current: usize, disabled: &[bool]) -> Option<usize> {
    match key {
        "home" => first_enabled(disabled),
        "end" => last_enabled(disabled),
        "up" => next_enabled(disabled, current, false, true),
        "down" => next_enabled(disabled, current, true, true),
        _ => None,
    }
}

/// A concrete GPUI listbox option.
#[derive(Clone)]
pub struct ListboxOption {
    descriptor: ListboxOptionDescriptor,
    on_select: Option<ListboxSelectHandler>,
}

impl ListboxOption {
    /// Creates a selectable option.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ListboxOptionDescriptor::option(value, label.to_string()),
            on_select: None,
        }
    }

    /// Creates a separator option.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            descriptor: ListboxOptionDescriptor::separator(value),
            on_select: None,
        }
    }

    /// Marks the option as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Registers an option selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(ListboxSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns the pure descriptor.
    pub fn descriptor(&self) -> ListboxOptionDescriptor {
        self.descriptor.clone()
    }

    fn select_handler(&self) -> Option<ListboxSelectHandler> {
        self.on_select.clone()
    }
}

/// A concrete GPUI listbox group.
#[derive(Clone)]
pub struct ListboxGroup {
    descriptor: ListboxGroupDescriptor,
    options: Vec<ListboxOption>,
}

impl ListboxGroup {
    /// Creates an empty listbox group.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: ListboxGroupDescriptor::new(value, label.to_string()),
            options: Vec::new(),
        }
    }

    /// Adds one option.
    pub fn option(mut self, option: ListboxOption) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many options.
    pub fn options(mut self, options: impl IntoIterator<Item = ListboxOption>) -> Self {
        self.options.extend(options);
        self
    }

    /// Returns the group descriptor.
    pub fn descriptor(&self) -> ListboxGroupDescriptor {
        self.options
            .iter()
            .fold(self.descriptor.clone(), |descriptor, option| {
                descriptor.option(option.descriptor())
            })
    }

    fn options_ref(&self) -> &[ListboxOption] {
        &self.options
    }
}

#[derive(Debug, Clone)]
struct ListboxRuntime {
    active_value: Option<String>,
    selected_value: Option<String>,
}

/// A concrete GPUI listbox component.
#[derive(IntoElement)]
pub struct Listbox {
    id: ElementId,
    label: SharedString,
    options: Vec<ListboxOption>,
    groups: Vec<ListboxGroup>,
    size: Size,
    disabled: bool,
    embedded: bool,
    selected_value: Option<String>,
    active_value: Option<String>,
    typeahead_query: Option<String>,
    empty_label: SharedString,
    tokens: ThemeTokens,
    on_select: Option<ListboxSelectHandler>,
}

impl Listbox {
    /// Creates an empty listbox.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            options: Vec::new(),
            groups: Vec::new(),
            size: Size::Medium,
            disabled: false,
            embedded: false,
            selected_value: None,
            active_value: None,
            typeahead_query: None,
            empty_label: "No options".into(),
            tokens: ThemeTokens::default(),
            on_select: None,
        }
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

    /// Marks the listbox as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Removes the standalone scroll surface when the parent owns clipping and scrolling.
    pub fn embedded(mut self, embedded: bool) -> Self {
        self.embedded = embedded;
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

    /// Applies typeahead query metadata.
    pub fn typeahead_query(mut self, query: impl Into<String>) -> Self {
        self.typeahead_query = Some(query.into());
        self
    }

    /// Applies empty-state label.
    pub fn empty_label(mut self, label: impl Into<SharedString>) -> Self {
        self.empty_label = label.into();
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a listbox selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(ListboxSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns resolved listbox state.
    pub fn state(&self) -> ListboxState {
        ListboxState::resolve(
            self.size,
            self.disabled,
            self.label.to_string(),
            self.selected_value.as_deref(),
            self.active_value.as_deref(),
            self.typeahead_query.as_deref(),
            self.empty_label.to_string(),
            self.groups.iter().map(ListboxGroup::descriptor),
            self.options.iter().map(ListboxOption::descriptor),
            self.tokens,
        )
    }
}

impl Sizable for Listbox {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Listbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| ListboxRuntime {
            active_value: self.active_value.clone(),
            selected_value: self.selected_value.clone(),
        });
        let runtime_state = runtime.read(cx).clone();
        let selected_value = self
            .selected_value
            .as_deref()
            .or(runtime_state.selected_value.as_deref());
        let active_value = self
            .active_value
            .as_deref()
            .or(runtime_state.active_value.as_deref());
        let state = ListboxState::resolve(
            self.size,
            self.disabled,
            self.label.to_string(),
            selected_value,
            active_value,
            self.typeahead_query.as_deref(),
            self.empty_label.to_string(),
            self.groups.iter().map(ListboxGroup::descriptor),
            self.options.iter().map(ListboxOption::descriptor),
            self.tokens,
        );
        let id = self.id;
        let colors = state.colors();
        let metrics = state.metrics();
        let focus_ring = state.focus_ring();
        let key_state = state.clone();
        let key_runtime = runtime.clone();
        let key_select = self.on_select.clone();

        div()
            .id(id)
            .min_w(metrics.min_width())
            .when(!self.embedded, |this| {
                this.max_h(metrics.max_height())
                    .overflow_y_scroll()
                    .scrollbar_width(ui_px(8.0))
            })
            .when(self.embedded, |this| this.w_full())
            .p(metrics.surface_padding())
            .flex()
            .flex_col()
            .gap_1()
            .rounded(metrics.radius())
            .when(!self.embedded, |this| {
                this.border_1()
                    .border_color(ThemeResolver::resolve(colors.border()))
                    .bg(ThemeResolver::resolve(colors.surface()))
            })
            .text_color(ThemeResolver::resolve(colors.foreground()))
            .text_size(metrics.text_size())
            .line_height(metrics.text_size())
            .focusable()
            .tab_group()
            .tab_stop(!state.disabled() && !state.empty())
            .role(state.role())
            .aria_label(state.label().to_owned())
            .aria_disabled(state.disabled())
            .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                handle_listbox_key_down(
                    &key_state,
                    key_runtime.clone(),
                    key_select.clone(),
                    event,
                    window,
                    cx,
                );
            })
            .children(listbox_children(
                self.options,
                self.groups,
                state,
                runtime,
                self.on_select,
            ))
    }
}

fn handle_listbox_key_down(
    state: &ListboxState,
    runtime: Entity<ListboxRuntime>,
    on_select: Option<ListboxSelectHandler>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    let key = event.keystroke.key.as_str();
    if let Some(target) = state.navigation_target(key) {
        cx.stop_propagation();
        window.prevent_default();
        let value = target.value().to_owned();
        runtime.update(cx, |runtime, _| {
            runtime.active_value = Some(value);
        });
        return;
    }

    if let Some(selection) = state.activation_for_key(key) {
        cx.stop_propagation();
        window.prevent_default();
        runtime.update(cx, |runtime, _| {
            runtime.selected_value = Some(selection.value().to_owned());
            runtime.active_value = Some(selection.value().to_owned());
        });
        if let Some(on_select) = on_select.as_ref() {
            on_select(selection, window, cx);
        }
    }
}

fn listbox_children(
    options: Vec<ListboxOption>,
    groups: Vec<ListboxGroup>,
    state: ListboxState,
    runtime: Entity<ListboxRuntime>,
    on_select: Option<ListboxSelectHandler>,
) -> Vec<AnyElement> {
    if state.empty() {
        return vec![
            div()
                .px(state.metrics().option_padding_x())
                .py(state.metrics().option_padding_y())
                .text_color(ThemeResolver::resolve(state.colors().muted_foreground()))
                .child(state.empty_label().to_owned())
                .into_any_element(),
        ];
    }

    let mut children = Vec::new();
    let standalone_states = state
        .options()
        .iter()
        .filter(|option| option.group_index().is_none())
        .cloned()
        .collect::<Vec<_>>();
    if !standalone_states.is_empty() {
        children.extend(listbox_option_elements(
            options,
            standalone_states,
            state.metrics(),
            state.colors(),
            runtime.clone(),
            on_select.clone(),
        ));
    }

    for group_state in state.groups() {
        children.push(
            div()
                .id(format!("listbox-group-label:{}", group_state.value()))
                .px(state.metrics().group_padding_x())
                .pt_2()
                .pb_1()
                .text_xs()
                .font_weight(open_gpui::FontWeight::BOLD)
                .text_color(ThemeResolver::resolve(state.colors().muted_foreground()))
                .role(group_state.role())
                .aria_label(group_state.label().to_owned())
                .child(group_state.label().to_owned())
                .into_any_element(),
        );

        if let Some(group) = groups.get(group_state.index()) {
            let states = state
                .options()
                .iter()
                .filter(|option| option.group_index() == Some(group_state.index()))
                .cloned()
                .collect::<Vec<_>>();
            children.extend(listbox_option_elements(
                group.options_ref().to_vec(),
                states,
                state.metrics(),
                state.colors(),
                runtime.clone(),
                on_select.clone(),
            ));
        }
    }

    children
}

fn listbox_option_elements(
    options: Vec<ListboxOption>,
    states: Vec<ListboxOptionState>,
    metrics: ListboxMetrics,
    colors: ListboxColors,
    runtime: Entity<ListboxRuntime>,
    on_select: Option<ListboxSelectHandler>,
) -> Vec<AnyElement> {
    options
        .into_iter()
        .zip(states)
        .map(|(option, state)| match state.kind() {
            ListboxOptionKind::Separator => div()
                .id(format!("listbox-separator:{}", state.index()))
                .h(metrics.separator_height())
                .my_1()
                .bg(ThemeResolver::resolve(colors.separator()))
                .into_any_element(),
            ListboxOptionKind::Option => {
                let selection = ListboxSelection::from_option(&state);
                let option_handler = option.select_handler();
                let global_handler = on_select.clone();
                let runtime = runtime.clone();
                let disabled = state.disabled();
                div()
                    .id(format!("listbox-option:{}", state.value()))
                    .min_h(metrics.option_height())
                    .px(metrics.option_padding_x())
                    .py(metrics.option_padding_y())
                    .flex()
                    .items_center()
                    .rounded(metrics.radius())
                    .bg(ThemeResolver::resolve(option_background(
                        state.clone(),
                        colors,
                    )))
                    .text_color(ThemeResolver::resolve(if disabled {
                        colors.option_disabled_foreground()
                    } else {
                        colors.foreground()
                    }))
                    .role(state.role().unwrap_or(Role::ListBoxOption))
                    .aria_label(state.label().to_owned())
                    .aria_selected(state.selected())
                    .aria_disabled(disabled)
                    .focusable()
                    .tab_stop(state.tab_stop())
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        this.cursor_pointer()
                            .hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.option_hover_background()))
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                let Some(selection) = selection.clone() else {
                                    return;
                                };
                                runtime.update(cx, |runtime, _| {
                                    runtime.selected_value = Some(selection.value().to_owned());
                                    runtime.active_value = Some(selection.value().to_owned());
                                });
                                if let Some(option_handler) = option_handler.as_ref() {
                                    option_handler(selection.clone(), window, cx);
                                }
                                if let Some(global_handler) = global_handler.as_ref() {
                                    global_handler(selection, window, cx);
                                }
                            })
                    })
                    .child(state.label().to_owned())
                    .into_any_element()
            }
        })
        .collect()
}

fn option_background(state: ListboxOptionState, colors: ListboxColors) -> ColorIntent {
    if state.active() {
        colors.option_active_background()
    } else if state.selected() {
        colors.option_selected_background()
    } else {
        colors.option_background()
    }
}

fn flatten_listbox_options(
    groups: &[ListboxGroupDescriptor],
    standalone_options: Vec<ListboxOptionDescriptor>,
) -> Vec<FlattenedOption> {
    let mut flattened = standalone_options
        .into_iter()
        .map(|descriptor| FlattenedOption {
            group_index: None,
            descriptor,
        })
        .collect::<Vec<_>>();

    for (group_index, group) in groups.iter().enumerate() {
        flattened.extend(
            group
                .options
                .iter()
                .cloned()
                .map(|descriptor| FlattenedOption {
                    group_index: Some(group_index),
                    descriptor,
                }),
        );
    }

    flattened
}

impl ThemeResolver {
    pub(crate) const fn listbox_colors(tokens: ThemeTokens) -> ListboxColors {
        ListboxColors {
            surface: ColorIntent::new(tokens.surface, 0xffffff),
            foreground: ColorIntent::new(tokens.text, 0x18202a),
            muted_foreground: ColorIntent::new(tokens.text_muted, 0x5a6472),
            border: ColorIntent::new(tokens.border, 0xcfd5cc),
            option_background: ColorIntent::new(tokens.surface, 0xffffff),
            option_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xf1f5ee,
            ),
            option_active_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::FocusVisible,
                0xe8ede6,
            ),
            option_selected_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Selected,
                0xe8ede6,
            ),
            option_disabled_foreground: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Disabled,
                0x7a8491,
            ),
            separator: ColorIntent::new(tokens.border, 0xcfd5cc),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                0x2f80ed,
            ),
        }
    }
}
