//! Listbox component and shared collection navigation state.

use crate::geometry::gpui_px_from_ui;
use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, Context, ElementId, Entity, FocusHandle, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{
    AccessibleAction, Role, SemanticDescriptor, Sizable, Size, ThemeTokens, UiPx, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::activation::{ActivationBinding, ActivationHandle, ActivationKeyPolicy};
use crate::choice::{
    self, ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection,
    SingleChoiceSelectionControl,
};
use crate::color::ColorIntent;
use crate::debug_selector::{
    AuthoredSnapshot, StableValueItemRenderIdentity, StableValueItemRenderIdentityInput,
    debug_selector_element_id,
};
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::theme::{ThemeContext, ThemeResolver};

type ListboxSelectHandler = Rc<dyn Fn(ListboxSelection, &mut Window, &mut App)>;
type ListboxSelectionTransaction = Rc<dyn Fn(ListboxSelectionIntent, &mut Window, &mut App)>;

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
    ambiguous_value: bool,
    selected: bool,
    active: bool,
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

pub(crate) struct ListboxSelectionIntent {
    selection: ListboxSelection,
    handler: Option<ListboxSelectHandler>,
}

impl ListboxSelectionIntent {
    fn new(selection: ListboxSelection, handler: Option<ListboxSelectHandler>) -> Self {
        Self { selection, handler }
    }

    pub(crate) const fn selection(&self) -> &ListboxSelection {
        &self.selection
    }

    pub(crate) fn deliver(self, window: &mut Window, cx: &mut App) {
        if let Some(handler) = self.handler.as_ref() {
            handler(self.selection, window, cx);
        }
    }
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

const DEFAULT_SCROLLABLE_OPTION_COUNT_THRESHOLD: usize = 6;

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
        let typeahead_query = typeahead_query
            .map(choice::normalize_query)
            .filter(|query| !query.is_empty());
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
        let collection = ChoiceCollection::resolve_unique(
            disabled,
            flatten_listbox_options(&group_descriptors, standalone_options),
            selected_value,
            active_value,
            ChoiceInteractionPolicy::listbox(),
        );
        let selected_index = collection.selected_index();
        let active_index = collection.active_index();
        let selected_value = collection.selected_value().map(str::to_owned);
        let active_value = collection.active_value().map(str::to_owned);
        let selectable_count = collection
            .items()
            .iter()
            .filter(|option| option.item().kind() == ListboxOptionKind::Option)
            .count();
        let mut position = 0usize;
        let options = collection
            .into_items()
            .into_iter()
            .enumerate()
            .map(|(index, option)| {
                let group_index = option.group_index();
                let disabled = disabled || !option.enabled();
                let ambiguous_value = option.ambiguous_value();
                let descriptor = option.into_item();
                let kind = descriptor.kind();
                let position_in_set = if kind == ListboxOptionKind::Option {
                    position += 1;
                    Some(position)
                } else {
                    None
                };
                let selected = selected_index == Some(index);
                let active = active_index == Some(index);

                ListboxOptionState {
                    index,
                    group_index,
                    value: descriptor.value,
                    label: descriptor.label,
                    kind,
                    disabled,
                    ambiguous_value,
                    selected,
                    active,
                    position_in_set,
                    size_of_set: selectable_count,
                }
            })
            .collect::<Vec<_>>();
        let colors = ThemeResolver::listbox_colors(tokens);

        Self {
            size,
            disabled,
            label,
            selected_value,
            active_value,
            typeahead_query,
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

    /// Returns standalone option states.
    pub fn standalone_options(&self) -> impl Iterator<Item = &ListboxOptionState> + '_ {
        self.options
            .iter()
            .filter(|option| option.group_index().is_none())
    }

    /// Returns option states owned by the given group index.
    pub fn group_options(
        &self,
        group_index: usize,
    ) -> impl Iterator<Item = &ListboxOptionState> + '_ {
        self.options
            .iter()
            .filter(move |option| option.group_index() == Some(group_index))
    }

    /// Returns whether the listbox content should use a scroll viewport.
    pub const fn scrollable_content(&self) -> bool {
        self.options.len() > DEFAULT_SCROLLABLE_OPTION_COUNT_THRESHOLD
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

    /// Returns whether the listbox has no selectable option rows.
    pub fn empty(&self) -> bool {
        self.options
            .iter()
            .all(|option| option.kind() != ListboxOptionKind::Option)
    }

    /// Resolves a navigation target for an APG-style listbox key.
    pub fn navigation_target(&self, key: &str) -> Option<&ListboxOptionState> {
        self.choice_collection()
            .navigation_target(key)
            .and_then(|target| self.options.get(target.source_index()))
    }

    /// Resolves a typeahead target for a query.
    pub fn typeahead_target(&self, query: &str) -> Option<&ListboxOptionState> {
        self.choice_collection()
            .typeahead_target(query)
            .and_then(|target| self.options.get(target.source_index()))
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

    /// Returns the selected option state.
    pub fn selected_option(&self) -> Option<&ListboxOptionState> {
        self.selected_index
            .and_then(|index| self.options.get(index))
    }

    /// Returns the active option state.
    pub fn active_option(&self) -> Option<&ListboxOptionState> {
        self.active_index.and_then(|index| self.options.get(index))
    }

    fn choice_collection(&self) -> ChoiceCollection<()> {
        ChoiceCollection::from_resolved(
            self.disabled,
            self.options
                .iter()
                .map(|option| {
                    let text_value = option.label().to_owned();
                    ChoiceItemProjection::new(
                        option.index(),
                        option.group_index(),
                        option.value(),
                        text_value.clone(),
                        !option.focusable(),
                        (),
                    )
                    .text_value(text_value)
                })
                .collect(),
            choice::ChoiceSelectionResolution::new(self.selected_index, self.active_index),
            ChoiceInteractionPolicy::listbox(),
        )
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

impl ListboxRuntime {
    fn sync(&mut self, selection: &SingleChoiceSelectionControl, state: &ListboxState) {
        if selection.is_controlled() {
            let selected_value = selection.value().as_deref();
            if self.selected_value.as_deref() != selected_value {
                self.selected_value = selected_value.map(str::to_owned);
            }
        }

        if !state.disabled() {
            let active_value = state.active_value();
            if self.active_value.as_deref() != active_value {
                self.active_value = active_value.map(str::to_owned);
            }
        }
    }

    fn set_active(&mut self, value: String, cx: &mut Context<Self>) {
        if self.active_value.as_ref() != Some(&value) {
            self.active_value = Some(value);
            cx.notify();
        }
    }

    fn activate(&mut self, selection: &ListboxSelection, controlled: bool, cx: &mut Context<Self>) {
        let value = selection.value().to_owned();
        let selected_changed = self.selected_value.as_ref() != Some(&value);
        let active_changed = self.active_value.as_ref() != Some(&value);
        if selected_changed && !controlled {
            self.selected_value = Some(value.clone());
        }
        if active_changed {
            self.active_value = Some(value);
        }
        if (selected_changed && !controlled) || active_changed {
            cx.notify();
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ListboxFocusOwner {
    #[default]
    Listbox,
    Editor,
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
    focus_owner: ListboxFocusOwner,
    active_focus: Option<FocusHandle>,
    selection: SingleChoiceSelectionControl,
    active_value: Option<String>,
    typeahead_query: Option<String>,
    empty_label: SharedString,
    tokens: ThemeTokens,
    on_select: Option<ListboxSelectHandler>,
    selection_transaction: Option<ListboxSelectionTransaction>,
    activation_handles: BTreeMap<String, ActivationHandle>,
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
            focus_owner: ListboxFocusOwner::Listbox,
            active_focus: None,
            selection: SingleChoiceSelectionControl::uncontrolled(None),
            active_value: None,
            typeahead_query: None,
            empty_label: "No options".into(),
            tokens: ThemeTokens::default(),
            on_select: None,
            selection_transaction: None,
            activation_handles: BTreeMap::new(),
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

    pub(crate) fn active_focus_handle(mut self, focus: FocusHandle) -> Self {
        self.active_focus = Some(focus);
        self
    }

    pub(crate) fn editor_owned_focus(mut self) -> Self {
        self.focus_owner = ListboxFocusOwner::Editor;
        self.active_focus = None;
        self
    }

    /// Applies the caller-owned selected option value.
    pub fn selected(mut self, value: Option<String>) -> Self {
        self.selection = SingleChoiceSelectionControl::controlled(value);
        self
    }

    /// Applies the default selected option value for adapter-owned runtime state.
    pub fn default_selected(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        if !self.selection.is_controlled() {
            self.selection = SingleChoiceSelectionControl::uncontrolled(Some(value));
        }
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

    pub(crate) fn selection_transaction(
        mut self,
        transaction: impl Fn(ListboxSelectionIntent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.selection_transaction = Some(Rc::new(transaction));
        self
    }

    /// Binds an application-owned activation handle to one stable option value.
    pub fn activation_handle(
        mut self,
        value: impl Into<String>,
        handle: &ActivationHandle,
    ) -> Self {
        self.activation_handles.insert(value.into(), handle.clone());
        self
    }

    /// Returns resolved listbox state.
    pub fn state(&self) -> ListboxState {
        ListboxState::resolve(
            self.size,
            self.disabled,
            self.label.to_string(),
            self.selection.value().as_deref(),
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
        let scope_id = self.id.clone();
        window.with_id(scope_id, |window| {
            let theme = ThemeResolver::current(window, cx);
            let selection_controlled = self.selection.is_controlled();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| ListboxRuntime {
                active_value: self.active_value.clone(),
                selected_value: self.selection.value().clone(),
            });
            let runtime_state = runtime.read(cx).clone();
            let selected_value = if selection_controlled {
                self.selection.value().clone()
            } else {
                runtime_state.selected_value
            };
            let active_value = self
                .active_value
                .as_deref()
                .or(runtime_state.active_value.as_deref());
            let state = ListboxState::resolve(
                self.size,
                self.disabled,
                self.label.to_string(),
                selected_value.as_deref(),
                active_value,
                self.typeahead_query.as_deref(),
                self.empty_label.to_string(),
                self.groups.iter().map(ListboxGroup::descriptor),
                self.options.iter().map(ListboxOption::descriptor),
                self.tokens,
            );
            runtime.update(cx, |runtime, _| runtime.sync(&self.selection, &state));
            let id = self.id;
            let debug_id = debug_selector_element_id(&id);
            let colors = state.colors();
            let metrics = state.metrics();
            let focus_ring = state.focus_ring();
            let focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
            let option_handlers = Rc::new(
                self.options
                    .iter()
                    .map(ListboxOption::select_handler)
                    .chain(self.groups.iter().flat_map(|group| {
                        group
                            .options_ref()
                            .iter()
                            .map(ListboxOption::select_handler)
                    }))
                    .collect::<Vec<_>>(),
            );
            let render_identities = Rc::new(listbox_render_identities(&debug_id, &state));
            let activation_bindings = Rc::new(listbox_activation_bindings(
                window,
                cx,
                &state,
                runtime.clone(),
                option_handlers,
                self.on_select.clone(),
                self.selection_transaction.clone(),
                &self.activation_handles,
                &render_identities,
                selection_controlled,
            ));
            let focus_owner = self.focus_owner;
            let active_focus = self.active_focus.clone();
            let root_focus = active_focus
                .clone()
                .filter(|_| state.active_value().is_none());
            let root_label = state.label().to_owned();
            let root_actions: &[AccessibleAction] =
                if focus_owner == ListboxFocusOwner::Listbox && !state.empty() {
                    &[AccessibleAction::Focus]
                } else {
                    &[]
                };
            let root_semantics = SemanticDescriptor::new(state.role())
                .with_label(&root_label)
                .with_disabled(state.disabled())
                .with_actions(root_actions);
            let active_keyboard_binding = (focus_owner == ListboxFocusOwner::Listbox)
                .then(|| state.active_index())
                .flatten()
                .and_then(|index| activation_bindings.get(index))
                .cloned()
                .flatten();
            let key_state = state.clone();
            let key_runtime = runtime.clone();

            div()
                .id(id.clone())
                .debug_selector({
                    let debug_id = debug_id.clone();
                    move || format!("listbox:{debug_id}")
                })
                .min_w(gpui_px_from_ui(metrics.min_width()))
                .when(!self.embedded, |this| {
                    this.max_h(gpui_px_from_ui(metrics.max_height()))
                        .overflow_y_scroll()
                        .scrollbar_width(gpui_px_from_ui(ui_px(8.0)))
                })
                .when(self.embedded, |this| this.w_full())
                .p(gpui_px_from_ui(metrics.surface_padding()))
                .flex()
                .flex_col()
                .gap_1()
                .rounded(gpui_px_from_ui(metrics.radius()))
                .when(!self.embedded, |this| {
                    this.border_1()
                        .border_color(theme.resolve(colors.border()))
                        .bg(theme.resolve(colors.surface()))
                })
                .text_color(theme.resolve(colors.foreground()))
                .text_size(gpui_px_from_ui(metrics.text_size()))
                .line_height(gpui_px_from_ui(metrics.text_size()))
                .ui_semantics(&root_semantics)
                .when(focus_owner == ListboxFocusOwner::Listbox, |this| {
                    this.focusable()
                        .tab_group()
                        .tab_stop(!state.disabled() && !state.empty())
                        .focus_visible(move |style| style.shadow(focus_shadow.clone()))
                        .on_key_down(move |event: &KeyDownEvent, window, cx| {
                            handle_listbox_navigation_key_down(
                                &key_state,
                                key_runtime.clone(),
                                event,
                                window,
                                cx,
                            );
                        })
                })
                .when_some(active_keyboard_binding, |this, activation| {
                    activation.bind_keyboard(this)
                })
                .when_some(root_focus, |this, focus| this.track_focus(&focus))
                .children(listbox_children(
                    debug_id,
                    state,
                    render_identities,
                    activation_bindings,
                    focus_owner,
                    active_focus,
                    &theme,
                ))
        })
    }
}

fn listbox_render_identities(
    debug_id: &str,
    state: &ListboxState,
) -> Vec<Option<StableValueItemRenderIdentity>> {
    let option_states = state
        .options()
        .iter()
        .filter(|option| option.kind() == ListboxOptionKind::Option)
        .collect::<Vec<_>>();
    let resolved = StableValueItemRenderIdentity::resolve_known_ambiguity(
        "listbox",
        debug_id,
        "option",
        "select",
        option_states.iter().map(|option| {
            let group_value = option
                .group_index()
                .and_then(|index| state.groups().get(index))
                .map(ListboxGroupState::value);
            if option.ambiguous_value {
                StableValueItemRenderIdentityInput::ambiguous(
                    option.value(),
                    AuthoredSnapshot::new()
                        .field(option.kind().as_str())
                        .field(option.label())
                        .field(option.disabled().to_string())
                        .optional_field(group_value)
                        .finish(),
                )
            } else {
                StableValueItemRenderIdentityInput::unique(option.value())
            }
        }),
    );
    let mut identities = vec![None; state.options().len()];
    for (option, identity) in option_states.into_iter().zip(resolved) {
        identities[option.index()] = Some(identity);
    }
    identities
}

#[allow(clippy::too_many_arguments)]
fn listbox_activation_bindings(
    window: &mut Window,
    cx: &mut App,
    state: &ListboxState,
    runtime: Entity<ListboxRuntime>,
    option_handlers: Rc<Vec<Option<ListboxSelectHandler>>>,
    on_select: Option<ListboxSelectHandler>,
    selection_transaction: Option<ListboxSelectionTransaction>,
    activation_handles: &BTreeMap<String, ActivationHandle>,
    render_identities: &[Option<StableValueItemRenderIdentity>],
    selection_controlled: bool,
) -> Vec<Option<ActivationBinding>> {
    state
        .options()
        .iter()
        .enumerate()
        .map(|(index, option)| {
            if option.kind() != ListboxOptionKind::Option {
                return None;
            }

            let selection = ListboxSelection::from_option(option);
            let activation_runtime = runtime.clone();
            let handler = option_handlers
                .get(index)
                .cloned()
                .flatten()
                .or_else(|| on_select.clone());
            let identity = render_identities[index]
                .as_ref()
                .expect("selectable listbox options must have render identity");
            let activation_handle = activation_handles.get(option.value()).cloned();
            let selection_transaction = selection_transaction.clone();

            Some(
                ActivationBinding::new(
                    window,
                    cx,
                    identity.activation_state_key.clone(),
                    option.activation_enabled(),
                    ActivationKeyPolicy::EnterOrSpace,
                    move |_, window, cx| {
                        let Some(selection) = selection.clone() else {
                            return;
                        };
                        activation_runtime.update(cx, |runtime, cx| {
                            runtime.activate(&selection, selection_controlled, cx);
                        });
                        let intent = ListboxSelectionIntent::new(selection, handler.clone());
                        if let Some(selection_transaction) = selection_transaction.as_ref() {
                            selection_transaction(intent, window, cx);
                        } else {
                            intent.deliver(window, cx);
                        }
                    },
                )
                .with_programmatic_handle(activation_handle),
            )
        })
        .collect()
}

fn handle_listbox_navigation_key_down(
    state: &ListboxState,
    runtime: Entity<ListboxRuntime>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if event.keystroke.modifiers.modified() || window.default_prevented() {
        return;
    }

    let key = event.keystroke.key.as_str();
    if let Some(target) = state.navigation_target(key) {
        cx.stop_propagation();
        window.prevent_default();
        let value = target.value().to_owned();
        runtime.update(cx, |runtime, cx| runtime.set_active(value, cx));
    }
}

fn listbox_children(
    debug_id: String,
    state: ListboxState,
    render_identities: Rc<Vec<Option<StableValueItemRenderIdentity>>>,
    activation_bindings: Rc<Vec<Option<ActivationBinding>>>,
    focus_owner: ListboxFocusOwner,
    active_focus: Option<FocusHandle>,
    theme: &ThemeContext,
) -> Vec<AnyElement> {
    if state.empty() {
        return vec![
            div()
                .debug_selector({
                    let debug_id = debug_id.clone();
                    move || format!("listbox:{debug_id}:empty")
                })
                .px(gpui_px_from_ui(state.metrics().option_padding_x()))
                .py(gpui_px_from_ui(state.metrics().option_padding_y()))
                .text_color(theme.resolve(state.colors().muted_foreground()))
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
            debug_id.clone(),
            standalone_states,
            state.metrics(),
            state.colors(),
            render_identities.clone(),
            activation_bindings.clone(),
            focus_owner,
            active_focus.clone(),
            theme,
        ));
    }

    for group_state in state.groups() {
        let group_label = group_state.label().to_owned();
        let group_semantics = SemanticDescriptor::new(group_state.role()).with_label(&group_label);
        children.push(
            div()
                .id(format!("listbox-group-label:{}", group_state.value()))
                .debug_selector({
                    let debug_id = debug_id.clone();
                    let group_value = group_state.value().to_owned();
                    move || format!("listbox:{debug_id}:group:{group_value}")
                })
                .px(gpui_px_from_ui(state.metrics().group_padding_x()))
                .pt_2()
                .pb_1()
                .text_xs()
                .font_weight(open_gpui::FontWeight::BOLD)
                .text_color(theme.resolve(state.colors().muted_foreground()))
                .ui_semantics(&group_semantics)
                .child(group_label.clone())
                .into_any_element(),
        );

        let states = state
            .options()
            .iter()
            .filter(|option| option.group_index() == Some(group_state.index()))
            .cloned()
            .collect::<Vec<_>>();
        children.extend(listbox_option_elements(
            debug_id.clone(),
            states,
            state.metrics(),
            state.colors(),
            render_identities.clone(),
            activation_bindings.clone(),
            focus_owner,
            active_focus.clone(),
            theme,
        ));
    }

    children
}

fn listbox_option_elements(
    debug_id: String,
    states: Vec<ListboxOptionState>,
    metrics: ListboxMetrics,
    colors: ListboxColors,
    render_identities: Rc<Vec<Option<StableValueItemRenderIdentity>>>,
    activation_bindings: Rc<Vec<Option<ActivationBinding>>>,
    focus_owner: ListboxFocusOwner,
    active_focus: Option<FocusHandle>,
    theme: &ThemeContext,
) -> Vec<AnyElement> {
    states
        .into_iter()
        .map(|state| match state.kind() {
            ListboxOptionKind::Separator => {
                let option_value = state.value().to_owned();
                let semantics = SemanticDescriptor::new(Role::Separator);

                div()
                    .id(format!("listbox-separator:{}", state.index()))
                    .debug_selector({
                        let debug_id = debug_id.clone();
                        let option_value = option_value.clone();
                        move || format!("listbox:{debug_id}:separator:{option_value}")
                    })
                    .h(gpui_px_from_ui(metrics.separator_height()))
                    .my_1()
                    .bg(theme.resolve(colors.separator()))
                    .ui_semantics(&semantics)
                    .into_any_element()
            }
            ListboxOptionKind::Option => {
                let disabled = state.disabled();
                let active = state.active();
                let option_label = state.label().to_owned();
                let identity = render_identities[state.index()]
                    .clone()
                    .expect("selectable listbox options must have render identity");
                let activation = activation_bindings[state.index()]
                    .clone()
                    .expect("selectable listbox options must have activation binding");
                let option_element_id = identity.element_id;
                let option_debug_selector = identity.debug_selector;
                let option_focus = active_focus
                    .clone()
                    .filter(|_| focus_owner == ListboxFocusOwner::Listbox && active);
                let option_background_color =
                    theme.resolve(option_background(state.clone(), colors));
                let option_foreground = theme.resolve(if disabled {
                    colors.option_disabled_foreground()
                } else {
                    colors.foreground()
                });
                let option_hover_background = theme.resolve(colors.option_hover_background());
                let option_actions: &[AccessibleAction] = if disabled {
                    &[]
                } else {
                    &[AccessibleAction::Click]
                };
                let mut semantics = SemanticDescriptor::new(Role::ListBoxOption)
                    .with_label(&option_label)
                    .with_selected(state.selected())
                    .with_disabled(disabled)
                    .with_actions(option_actions);
                if let Some(position) = state.position_in_set() {
                    semantics = semantics
                        .with_position_in_set(position)
                        .with_size_of_set(state.size_of_set());
                }

                activation
                    .bind_pointer_and_accessibility(
                        div()
                            .id(option_element_id)
                            .debug_selector(move || option_debug_selector.clone())
                            .min_h(gpui_px_from_ui(metrics.option_height()))
                            .px(gpui_px_from_ui(metrics.option_padding_x()))
                            .py(gpui_px_from_ui(metrics.option_padding_y()))
                            .flex()
                            .items_center()
                            .rounded(gpui_px_from_ui(metrics.radius()))
                            .bg(option_background_color)
                            .text_color(option_foreground)
                            .ui_semantics(&semantics)
                            .when(focus_owner == ListboxFocusOwner::Listbox, |this| {
                                this.focusable()
                                    .tab_stop(active)
                                    .when_some(option_focus, |this, focus| this.track_focus(&focus))
                            })
                            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                            .when(!disabled, |this| {
                                this.cursor_pointer()
                                    .hover(move |style| style.bg(option_hover_background))
                            })
                            .child(option_label.clone()),
                    )
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
) -> Vec<ChoiceItemProjection<ListboxOptionDescriptor>> {
    let mut flattened = standalone_options
        .into_iter()
        .enumerate()
        .map(|(source_index, descriptor)| listbox_choice_projection(source_index, None, descriptor))
        .collect::<Vec<_>>();

    for (group_index, group) in groups.iter().enumerate() {
        flattened.extend(group.options.iter().cloned().enumerate().map(
            |(source_index, descriptor)| {
                listbox_choice_projection(source_index, Some(group_index), descriptor)
            },
        ));
    }

    flattened
}

fn listbox_choice_projection(
    source_index: usize,
    group_index: Option<usize>,
    descriptor: ListboxOptionDescriptor,
) -> ChoiceItemProjection<ListboxOptionDescriptor> {
    let text_value = descriptor.label.clone();
    let structural = descriptor.kind() == ListboxOptionKind::Separator;
    let projection = ChoiceItemProjection::new(
        source_index,
        group_index,
        descriptor.value.clone(),
        text_value.clone(),
        !descriptor.focusable(),
        descriptor,
    )
    .text_value(text_value);

    if structural {
        projection.structural()
    } else {
        projection
    }
}
