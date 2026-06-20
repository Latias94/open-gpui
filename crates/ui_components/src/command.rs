//! Command palette component built from search input, grouped command items, and listbox state.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, Entity, IntoElement, KeyDownEvent, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred, div,
    point, px, rgba,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayLayerKind,
    OverlayPresence, Role, Sizable, Size, ThemeTokens, UiPx, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::listbox::{
    Listbox, ListboxGroup, ListboxGroupDescriptor, ListboxOption, ListboxOptionDescriptor,
    ListboxState,
};
use crate::overlay::{
    GpuiOverlayAdapterConfig, OverlayResolvedState, escape_open_change, gpui_overlay_state,
    outside_press_open_change,
};
use crate::scroll_area::{ScrollArea, ScrollAreaAxis, ScrollAreaState};
use crate::text_input::adapter::TextInputController;
use crate::text_input::{TextInput, TextInputState};
use crate::theme::ThemeResolver;

type CommandOpenChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type CommandSelectionHandler = Rc<dyn Fn(CommandSelection, &mut Window, &mut App)>;

/// Command dialog open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

/// Command loading state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLoadingState {
    message: String,
    progress_percent: Option<u8>,
}

impl CommandLoadingState {
    /// Creates command loading metadata.
    pub fn new(message: impl Into<String>, progress_percent: Option<u8>) -> Self {
        Self {
            message: message.into(),
            progress_percent: progress_percent.map(|progress| progress.min(100)),
        }
    }

    /// Returns loading message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns optional progress percentage.
    pub const fn progress_percent(&self) -> Option<u8> {
        self.progress_percent
    }

    /// Returns loading accessibility role.
    pub const fn role(&self) -> Role {
        Role::ProgressIndicator
    }
}

/// Pure descriptor for one command item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItemDescriptor {
    value: String,
    label: String,
    keywords: Vec<String>,
    shortcut: Option<String>,
    disabled: bool,
}

impl CommandItemDescriptor {
    /// Creates a selectable command item descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            keywords: Vec::new(),
            shortcut: None,
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

    /// Adds a display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Marks the item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns filtering keywords.
    pub fn keywords_ref(&self) -> &[String] {
        &self.keywords
    }

    /// Returns the display shortcut label.
    pub fn shortcut_ref(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns whether the item is disabled.
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
                .shortcut
                .as_ref()
                .is_some_and(|shortcut| shortcut.to_lowercase().contains(query.as_str()))
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

/// Pure descriptor for one command group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandGroupDescriptor {
    value: String,
    label: String,
    items: Vec<CommandItemDescriptor>,
}

impl CommandGroupDescriptor {
    /// Creates an empty command group descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            items: Vec::new(),
        }
    }

    /// Adds one command item.
    pub fn item(mut self, item: CommandItemDescriptor) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many command items.
    pub fn items(mut self, items: impl IntoIterator<Item = CommandItemDescriptor>) -> Self {
        self.items.extend(items);
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

    /// Returns group items.
    pub fn items_ref(&self) -> &[CommandItemDescriptor] {
        &self.items
    }
}

/// Resolved command color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandColors {
    surface: ColorIntent,
    foreground: ColorIntent,
    muted_foreground: ColorIntent,
    border: ColorIntent,
    shortcut_foreground: ColorIntent,
    focus_ring: ColorIntent,
}

impl CommandColors {
    /// Returns surface color intent.
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

    /// Returns shortcut label color intent.
    pub const fn shortcut_foreground(self) -> ColorIntent {
        self.shortcut_foreground
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved command metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandMetrics {
    padding: UiPx,
    radius: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    max_height: UiPx,
    shortcut_min_width: UiPx,
}

impl CommandMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            padding: ui_px(6.0),
            radius: size.control_radius(),
            min_width: ui_px(320.0),
            max_width: ui_px(560.0),
            max_height: match size {
                Size::XSmall => ui_px(220.0),
                Size::Small => ui_px(260.0),
                Size::Medium => ui_px(340.0),
                Size::Large => ui_px(420.0),
            },
            shortcut_min_width: match size {
                Size::XSmall | Size::Small => ui_px(48.0),
                Size::Medium => ui_px(64.0),
                Size::Large => ui_px(76.0),
            },
        }
    }

    /// Returns panel padding.
    pub const fn padding(self) -> UiPx {
        self.padding
    }

    /// Returns panel radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns minimum panel width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns maximum panel width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }

    /// Returns maximum command list height.
    pub const fn max_height(self) -> UiPx {
        self.max_height
    }

    /// Returns minimum shortcut label width.
    pub const fn shortcut_min_width(self) -> UiPx {
        self.shortcut_min_width
    }
}

/// Selection payload emitted by a command surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSelection {
    index: usize,
    value: String,
    label: String,
    shortcut: Option<String>,
}

impl CommandSelection {
    /// Creates a command selection payload.
    pub fn new(
        index: usize,
        value: impl Into<String>,
        label: impl Into<String>,
        shortcut: Option<String>,
    ) -> Self {
        Self {
            index,
            value: value.into(),
            label: label.into(),
            shortcut,
        }
    }

    /// Creates a selection payload from an item state.
    pub fn from_item(item: &CommandItemState) -> Option<Self> {
        item.activation_enabled().then(|| {
            Self::new(
                item.index,
                item.value.clone(),
                item.label.clone(),
                item.shortcut.clone(),
            )
        })
    }

    /// Returns the flattened item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns selected item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns selected item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns optional shortcut label.
    pub fn shortcut(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }
}

/// Resolved command group state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandGroupState {
    index: usize,
    value: String,
    label: String,
    item_count: usize,
    standalone: bool,
}

impl CommandGroupState {
    /// Returns group index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns stable group value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible group label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns visible item count.
    pub const fn item_count(&self) -> usize {
        self.item_count
    }

    /// Returns whether this is the synthetic standalone command group.
    pub const fn standalone(&self) -> bool {
        self.standalone
    }

    /// Returns group accessibility role.
    pub const fn role(&self) -> Role {
        Role::Group
    }
}

/// Resolved command item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItemState {
    index: usize,
    group_index: Option<usize>,
    value: String,
    label: String,
    shortcut: Option<String>,
    disabled: bool,
    selected: bool,
    active: bool,
    position_in_set: Option<usize>,
    size_of_set: usize,
}

impl CommandItemState {
    /// Returns flattened item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns containing group index when grouped.
    pub const fn group_index(&self) -> Option<usize> {
        self.group_index
    }

    /// Returns stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns visible item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns optional shortcut label.
    pub fn shortcut(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }

    /// Returns whether the item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the item can be activated.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns whether the item is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the item is active.
    pub const fn active(&self) -> bool {
        self.active
    }

    /// Returns the item's accessibility role.
    pub const fn role(&self) -> Role {
        Role::ListBoxOption
    }

    /// Returns one-based position among command items.
    pub const fn position_in_set(&self) -> Option<usize> {
        self.position_in_set
    }

    /// Returns total command item count in the visible set.
    pub const fn size_of_set(&self) -> usize {
        self.size_of_set
    }
}

/// Dialog wrapper state for command surfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandDialogState {
    enabled: bool,
    open: bool,
    title: String,
    description: Option<String>,
    overlay: OverlayResolvedState,
}

impl CommandDialogState {
    /// Returns whether this command is presented as a dialog.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns whether the dialog is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns dialog role.
    pub const fn role(&self) -> Role {
        Role::Window
    }

    /// Returns dialog content role.
    pub const fn content_role(&self) -> Role {
        Role::Window
    }

    /// Returns dialog title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns optional dialog description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns renderer-neutral overlay state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }
}

#[derive(Debug, Clone)]
struct FlattenedCommandItem {
    group_index: Option<usize>,
    descriptor: CommandItemDescriptor,
}

/// Resolved command state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandState {
    size: Size,
    disabled: bool,
    label: String,
    placeholder: String,
    query: String,
    open: bool,
    default_open: bool,
    open_mode: CommandOpenMode,
    overlay: OverlayResolvedState,
    dialog: Option<CommandDialogState>,
    loading_state: Option<CommandLoadingState>,
    empty_label: String,
    escape_key_policy: EscapeKeyPolicy,
    focus_restore_intent: FocusRestoreIntent,
    total_item_count: usize,
    filtered_item_count: usize,
    groups: Vec<CommandGroupState>,
    items: Vec<CommandItemState>,
    input: TextInputState,
    listbox: ListboxState,
    scroll_area: ScrollAreaState,
    metrics: CommandMetrics,
    colors: CommandColors,
    focus_ring: FocusRing,
}

impl CommandState {
    /// Resolves public state for a command surface.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        dialog_enabled: bool,
        label: impl Into<String>,
        placeholder: impl Into<String>,
        query: impl Into<String>,
        selected_value: Option<&str>,
        active_value: Option<&str>,
        loading_state: Option<CommandLoadingState>,
        empty_label: impl Into<String>,
        dialog_title: Option<String>,
        dialog_description: Option<String>,
        groups: impl IntoIterator<Item = CommandGroupDescriptor>,
        items: impl IntoIterator<Item = CommandItemDescriptor>,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let label = label.into();
        let placeholder = placeholder.into();
        let query = query.into();
        let empty_label = empty_label.into();
        let open_mode = if open.is_some() {
            CommandOpenMode::Controlled
        } else {
            CommandOpenMode::Uncontrolled
        };
        let open = open.unwrap_or(default_open) && !disabled;
        let raw_groups = groups.into_iter().collect::<Vec<_>>();
        let raw_items = items.into_iter().collect::<Vec<_>>();
        let total_item_count = raw_items.len()
            + raw_groups
                .iter()
                .map(|group| group.items_ref().len())
                .sum::<usize>();
        let selected_item = selected_value
            .and_then(|value| find_command_item(&raw_groups, &raw_items, value))
            .filter(|item| !item.disabled_state());
        let selected_value = selected_item.map(|item| item.value().to_owned());

        let mut filtered_group_descriptors = Vec::new();
        let mut command_groups = Vec::new();
        let mut flattened = raw_items
            .iter()
            .filter(|item| item.matches_query(query.as_str()))
            .cloned()
            .map(|descriptor| FlattenedCommandItem {
                group_index: None,
                descriptor,
            })
            .collect::<Vec<_>>();
        if !flattened.is_empty() {
            let group_index = command_groups.len();
            command_groups.push(CommandGroupState {
                index: group_index,
                value: "commands".to_string(),
                label: "Commands".to_string(),
                item_count: flattened.len(),
                standalone: true,
            });
            for item in &mut flattened {
                item.group_index = Some(group_index);
            }
        }
        let filtered_item_descriptors = flattened
            .iter()
            .map(|item| item.descriptor.to_listbox_descriptor())
            .collect::<Vec<_>>();

        for group in &raw_groups {
            let filtered_items = group
                .items_ref()
                .iter()
                .filter(|item| item.matches_query(query.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            if filtered_items.is_empty() {
                continue;
            }

            let group_index = command_groups.len();
            command_groups.push(CommandGroupState {
                index: group_index,
                value: group.value().to_owned(),
                label: group.label().to_owned(),
                item_count: filtered_items.len(),
                standalone: false,
            });
            filtered_group_descriptors.push(
                ListboxGroupDescriptor::new(group.value().to_owned(), group.label().to_owned())
                    .options(
                        filtered_items
                            .iter()
                            .map(CommandItemDescriptor::to_listbox_descriptor),
                    ),
            );
            flattened.extend(
                filtered_items
                    .into_iter()
                    .map(|descriptor| FlattenedCommandItem {
                        group_index: Some(group_index),
                        descriptor,
                    }),
            );
        }

        let filtered_item_count = flattened.len();
        let listbox = ListboxState::resolve(
            size,
            disabled,
            label.clone(),
            selected_value.as_deref(),
            active_value,
            (!query.is_empty()).then_some(query.as_str()),
            empty_label.clone(),
            filtered_group_descriptors,
            filtered_item_descriptors,
            tokens,
        );
        let items = flattened
            .into_iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let option = listbox.options().get(index)?;
                Some(CommandItemState {
                    index,
                    group_index: item.group_index,
                    value: item.descriptor.value,
                    label: item.descriptor.label,
                    shortcut: item.descriptor.shortcut,
                    disabled: item.descriptor.disabled,
                    selected: option.selected(),
                    active: option.active(),
                    position_in_set: option.position_in_set(),
                    size_of_set: option.size_of_set(),
                })
            })
            .collect::<Vec<_>>();
        let input = TextInputState::resolve(
            query.clone(),
            Some(placeholder.clone()),
            size,
            disabled,
            false,
            false,
            false,
            true,
            tokens,
        );
        let presence = if dialog_enabled && open {
            OverlayPresence::open()
        } else {
            OverlayPresence::hidden()
        };
        let overlay =
            GpuiOverlayAdapterConfig::new(OverlayLayerKind::NonModalDismissible, presence)
                .outside_press_policy(outside_press_policy)
                .escape_key_policy(escape_key_policy)
                .initial_focus_intent(initial_focus_intent.clone())
                .focus_restore_intent(focus_restore_intent.clone())
                .resolved_state();
        let dialog_overlay = GpuiOverlayAdapterConfig::new(OverlayLayerKind::Modal, presence)
            .outside_press_policy(outside_press_policy)
            .escape_key_policy(escape_key_policy)
            .initial_focus_intent(initial_focus_intent)
            .focus_restore_intent(focus_restore_intent.clone())
            .resolved_state();
        let dialog = dialog_enabled.then(|| CommandDialogState {
            enabled: true,
            open,
            title: dialog_title.unwrap_or_else(|| label.clone()),
            description: dialog_description,
            overlay: dialog_overlay,
        });
        let scroll_area = ScrollAreaState::resolve(
            format!("{label}:command-list-scroll"),
            ScrollAreaAxis::Vertical,
            size,
            crate::scroll_area::ScrollResetPolicy::Preserve,
            None,
        );
        let colors = ThemeResolver::command_colors(tokens);

        Self {
            size,
            disabled,
            label,
            placeholder,
            query,
            open,
            default_open,
            open_mode,
            overlay,
            dialog,
            loading_state,
            empty_label,
            escape_key_policy,
            focus_restore_intent,
            total_item_count,
            filtered_item_count,
            groups: command_groups,
            items,
            input,
            listbox,
            scroll_area,
            metrics: CommandMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the command surface is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns current search query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns selected command value.
    pub fn selected_value(&self) -> Option<&str> {
        self.listbox.selected_value()
    }

    /// Returns active command value.
    pub fn active_value(&self) -> Option<&str> {
        self.listbox.active_value()
    }

    /// Returns whether the dialog wrapper is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> CommandOpenMode {
        self.open_mode
    }

    /// Returns dialog wrapper state.
    pub const fn overlay(&self) -> &OverlayResolvedState {
        &self.overlay
    }

    /// Returns optional dialog wrapper state.
    pub const fn dialog(&self) -> Option<&CommandDialogState> {
        self.dialog.as_ref()
    }

    /// Returns loading state.
    pub const fn loading_state(&self) -> Option<&CommandLoadingState> {
        self.loading_state.as_ref()
    }

    /// Returns optional loading metadata.
    pub const fn loading(&self) -> Option<&CommandLoadingState> {
        self.loading_state.as_ref()
    }

    /// Returns empty-state label.
    pub fn empty_label(&self) -> &str {
        &self.empty_label
    }

    /// Returns Escape key policy.
    pub const fn escape_key_policy(&self) -> EscapeKeyPolicy {
        self.escape_key_policy
    }

    /// Returns focus restore intent.
    pub fn focus_restore_intent(&self) -> &FocusRestoreIntent {
        &self.focus_restore_intent
    }

    /// Returns unfiltered command count.
    pub const fn total_item_count(&self) -> usize {
        self.total_item_count
    }

    /// Returns filtered command count.
    pub const fn filtered_item_count(&self) -> usize {
        self.filtered_item_count
    }

    /// Returns whether the visible list is empty.
    pub const fn empty(&self) -> bool {
        self.filtered_item_count == 0
    }

    /// Returns whether query filtering removed commands.
    pub const fn filtered(&self) -> bool {
        self.filtered_item_count != self.total_item_count
    }

    /// Returns whether command content should be rendered.
    pub const fn content_visible(&self) -> bool {
        self.dialog.is_none() || self.open
    }

    /// Returns input role.
    pub const fn input_role(&self) -> Role {
        Role::TextInput
    }

    /// Returns list role.
    pub const fn list_role(&self) -> Role {
        Role::ListBox
    }

    /// Returns list role.
    pub const fn content_role(&self) -> Role {
        self.list_role()
    }

    /// Returns resolved group states.
    pub fn groups(&self) -> &[CommandGroupState] {
        &self.groups
    }

    /// Returns resolved standalone command items.
    pub fn standalone_items(&self) -> impl Iterator<Item = &CommandItemState> + '_ {
        let standalone_group_index = self.groups.iter().find(|group| group.standalone());
        self.items
            .iter()
            .filter(move |item| match standalone_group_index {
                Some(group) => item.group_index() == Some(group.index()),
                None => item.group_index().is_none(),
            })
    }

    /// Returns resolved non-synthetic command groups.
    pub fn grouped_groups(&self) -> impl Iterator<Item = &CommandGroupState> + '_ {
        self.groups.iter().filter(|group| !group.standalone())
    }

    /// Returns resolved items for one command group.
    pub fn group_items(&self, group_index: usize) -> impl Iterator<Item = &CommandItemState> + '_ {
        self.items
            .iter()
            .filter(move |item| item.group_index() == Some(group_index))
    }

    /// Returns resolved item states.
    pub fn items(&self) -> &[CommandItemState] {
        &self.items
    }

    /// Returns resolved input state.
    pub const fn input(&self) -> &TextInputState {
        &self.input
    }

    /// Returns nested listbox state.
    pub const fn listbox(&self) -> &ListboxState {
        &self.listbox
    }

    /// Returns scroll area state.
    pub const fn scroll_area(&self) -> &ScrollAreaState {
        &self.scroll_area
    }

    /// Returns metrics.
    pub const fn metrics(&self) -> CommandMetrics {
        self.metrics
    }

    /// Returns color intents.
    pub const fn colors(&self) -> CommandColors {
        self.colors
    }

    /// Returns focus-ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Resolves an activation payload for an APG-style activation key.
    pub fn activation_for_key(&self, key: &str) -> Option<CommandSelection> {
        if !matches!(key, "enter" | "space") {
            return None;
        }
        self.items
            .iter()
            .find(|item| item.active())
            .and_then(CommandSelection::from_item)
    }
}

#[derive(Debug, Clone)]
struct CommandRuntime {
    open: bool,
    active_value: Option<String>,
    selected_value: Option<String>,
}

/// A concrete GPUI command surface.
#[derive(IntoElement)]
pub struct Command {
    id: ElementId,
    label: SharedString,
    placeholder: SharedString,
    trigger_label: SharedString,
    items: Vec<CommandItem>,
    groups: Vec<CommandGroup>,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    dialog_enabled: bool,
    query: String,
    selected_value: Option<String>,
    active_value: Option<String>,
    loading_state: Option<CommandLoadingState>,
    empty_label: SharedString,
    dialog_title: Option<String>,
    dialog_description: Option<String>,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<CommandOpenChangeHandler>,
    on_select: Option<CommandSelectionHandler>,
}

impl Command {
    /// Creates an inline command surface.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            placeholder: "Search commands".into(),
            trigger_label: "Open command menu".into(),
            items: Vec::new(),
            groups: Vec::new(),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            dialog_enabled: false,
            query: String::new(),
            selected_value: None,
            active_value: None,
            loading_state: None,
            empty_label: "No commands".into(),
            dialog_title: None,
            dialog_description: None,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
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

    /// Applies dialog trigger label.
    pub fn trigger_label(mut self, label: impl Into<SharedString>) -> Self {
        self.trigger_label = label.into();
        self
    }

    /// Adds one standalone command item.
    pub fn item(mut self, item: CommandItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many standalone command items.
    pub fn items(mut self, items: impl IntoIterator<Item = CommandItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Adds one command group.
    pub fn group(mut self, group: CommandGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds many command groups.
    pub fn groups(mut self, groups: impl IntoIterator<Item = CommandGroup>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Marks the command surface as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies controlled dialog open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Applies uncontrolled initial dialog open state.
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Enables dialog presentation with a title.
    pub fn dialog(mut self, title: impl Into<String>) -> Self {
        self.dialog_enabled = true;
        self.dialog_title = Some(title.into());
        self
    }

    /// Enables or disables dialog presentation.
    pub fn dialog_enabled(mut self, enabled: bool) -> Self {
        self.dialog_enabled = enabled;
        if !enabled {
            self.dialog_title = None;
            self.dialog_description = None;
        }
        self
    }

    /// Applies optional dialog description text.
    pub fn dialog_description(mut self, description: impl Into<String>) -> Self {
        self.dialog_description = Some(description.into());
        self
    }

    /// Applies search query text.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    /// Applies selected item value.
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected_value = Some(value.into());
        self
    }

    /// Applies active item value.
    pub fn active(mut self, value: impl Into<String>) -> Self {
        self.active_value = Some(value.into());
        self
    }

    /// Applies loading metadata.
    pub fn loading(mut self, message: impl Into<String>, progress_percent: Option<u8>) -> Self {
        self.loading_state = Some(CommandLoadingState::new(message, progress_percent));
        self
    }

    /// Clears loading metadata.
    pub fn idle(mut self) -> Self {
        self.loading_state = None;
        self
    }

    /// Applies empty-state label.
    pub fn empty_label(mut self, label: impl Into<SharedString>) -> Self {
        self.empty_label = label.into();
        self
    }

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies Escape key policy.
    pub fn escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key_policy = policy;
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

    /// Registers an open-change handler.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Registers a command selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(CommandSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Returns resolved command state.
    pub fn state(&self) -> CommandState {
        CommandState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.dialog_enabled,
            self.label.to_string(),
            self.placeholder.to_string(),
            self.query.as_str(),
            self.selected_value.as_deref(),
            self.active_value.as_deref(),
            self.loading_state.clone(),
            self.empty_label.to_string(),
            self.dialog_title.clone(),
            self.dialog_description.clone(),
            self.groups.iter().map(CommandGroup::descriptor),
            self.items.iter().map(CommandItem::descriptor),
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for Command {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Command {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| CommandRuntime {
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
        let resolved_open = self.open.unwrap_or(runtime_state.open);
        if self.open.is_some() && runtime_state.open != resolved_open {
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
        let state = CommandState::resolve(
            self.size,
            self.disabled,
            Some(resolved_open),
            self.default_open,
            self.dialog_enabled,
            self.label.to_string(),
            self.placeholder.to_string(),
            query.as_str(),
            selected_value,
            active_value,
            self.loading_state,
            self.empty_label.to_string(),
            self.dialog_title.clone(),
            self.dialog_description.clone(),
            self.groups.iter().map(CommandGroup::descriptor),
            self.items.iter().map(CommandItem::descriptor),
            self.outside_press_policy,
            self.escape_key_policy,
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
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let input_id: ElementId = (id.clone(), "input").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let listbox_id: ElementId = (id.clone(), "listbox").into();
        let metrics = state.metrics();
        let colors = state.colors();
        let disabled = state.disabled();
        let focus_ring = state.focus_ring();
        let dialog_state = state.dialog().cloned();
        let dialog_open = dialog_state.clone().filter(|_| state.open());
        let dialog_priority = dialog_state
            .as_ref()
            .map(|dialog| gpui_overlay_state(dialog.overlay()).deferred_priority())
            .unwrap_or_else(|| gpui_overlay_state(state.overlay()).deferred_priority());
        let viewport = window.viewport_size();
        let dialog_enabled = self.dialog_enabled;
        let trigger_label = self.trigger_label;
        let items = self.items;
        let groups = self.groups;
        let on_open_change = self.on_open_change;
        let on_select = self.on_select;
        let tokens = self.tokens;

        div()
            .id(id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("command:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .gap_2()
            .when(dialog_state.is_some(), |this| {
                let runtime = runtime.clone();
                let on_open_change = on_open_change.clone();
                let trigger_label = trigger_label.clone();
                this.child(
                    div()
                        .id(trigger_id)
                        .debug_selector({
                            let debug_id = debug_id.clone();
                            move || format!("command:{debug_id}:trigger")
                        })
                        .min_h(gpui_px_from_ui(state.size().button_h()))
                        .px(gpui_px_from_ui(state.size().button_px()))
                        .py(gpui_px_from_ui(state.size().button_py()))
                        .rounded(gpui_px_from_ui(metrics.radius()))
                        .border_1()
                        .border_color(ThemeResolver::resolve(colors.border()))
                        .bg(ThemeResolver::resolve(colors.surface()))
                        .text_color(ThemeResolver::resolve(colors.foreground()))
                        .focusable()
                        .tab_stop(!disabled)
                        .ui_role(Role::Button)
                        .aria_label(trigger_label.clone())
                        .aria_expanded(state.open())
                        .aria_disabled(disabled)
                        .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
                        .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                        .when(!disabled, |this| {
                            this.cursor_pointer().on_click(
                                move |_event: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    runtime.update(cx, |runtime, _| {
                                        runtime.open = true;
                                    });
                                    if let Some(on_open_change) = on_open_change.as_ref() {
                                        on_open_change(true, window, cx);
                                    }
                                },
                            )
                        })
                        .child(trigger_label),
                )
            })
            .when(!dialog_enabled, |this| {
                this.child(command_content_element(
                    content_id.clone(),
                    input_id.clone(),
                    listbox_id.clone(),
                    debug_id.clone(),
                    state.clone(),
                    items.clone(),
                    groups.clone(),
                    input_controller.clone(),
                    runtime.clone(),
                    on_open_change.clone(),
                    on_select.clone(),
                    tokens,
                ))
            })
            .when_some(dialog_open, |this, dialog_state| {
                this.child(
                    deferred(
                        anchored()
                            .position(point(px(0.0), px(0.0)))
                            .snap_to_window()
                            .child(command_dialog_layer_element(
                                content_id,
                                input_id,
                                listbox_id,
                                debug_id,
                                state,
                                dialog_state,
                                viewport,
                                items,
                                groups,
                                input_controller,
                                runtime,
                                on_open_change,
                                on_select,
                                tokens,
                            )),
                    )
                    .priority(dialog_priority),
                )
            })
    }
}

#[allow(clippy::too_many_arguments)]
fn command_dialog_layer_element(
    content_id: ElementId,
    input_id: ElementId,
    listbox_id: ElementId,
    debug_id: String,
    state: CommandState,
    dialog_state: CommandDialogState,
    viewport: open_gpui::Size<Pixels>,
    items: Vec<CommandItem>,
    groups: Vec<CommandGroup>,
    input_controller: Entity<TextInputController>,
    runtime: Entity<CommandRuntime>,
    on_open_change: Option<CommandOpenChangeHandler>,
    on_select: Option<CommandSelectionHandler>,
    tokens: ThemeTokens,
) -> impl IntoElement {
    let metrics = state.metrics();
    let outside_change = outside_press_open_change(dialog_state.overlay().policy());
    let x = ((viewport.width - gpui_px_from_ui(metrics.max_width())) / 2.0).max(px(12.0));
    let y = (viewport.height / 10.0).max(px(24.0));

    div()
        .id((content_id.clone(), "layer"))
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .w(viewport.width)
        .h(viewport.height)
        .bg(rgba(0x00000033))
        .occlude()
        .on_any_mouse_down(|_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            this.on_click(move |_: &ClickEvent, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
                close_command_dialog(runtime.clone(), on_open_change.clone(), window, cx);
            })
        })
        .child(
            div()
                .absolute()
                .left(x)
                .top(y)
                .on_any_mouse_down(|_, _, cx| {
                    cx.stop_propagation();
                })
                .tab_group()
                .child(command_content_element(
                    content_id,
                    input_id,
                    listbox_id,
                    debug_id,
                    state,
                    items,
                    groups,
                    input_controller,
                    runtime,
                    on_open_change,
                    on_select,
                    tokens,
                )),
        )
}

#[allow(clippy::too_many_arguments)]
fn command_content_element(
    content_id: ElementId,
    input_id: ElementId,
    listbox_id: ElementId,
    debug_id: String,
    state: CommandState,
    items: Vec<CommandItem>,
    groups: Vec<CommandGroup>,
    input_controller: Entity<TextInputController>,
    runtime: Entity<CommandRuntime>,
    on_open_change: Option<CommandOpenChangeHandler>,
    on_select: Option<CommandSelectionHandler>,
    tokens: ThemeTokens,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let query = state.query().to_owned();
    let label = state.label().to_owned();
    let selected_value = state.selected_value().map(str::to_owned);
    let active_value = state.active_value().map(str::to_owned);
    let dialog_state = state.dialog().cloned();
    let shortcut_lookup = command_shortcut_lookup(&items, &groups, query.as_str());
    let outside_change = if let Some(dialog_state) = dialog_state.as_ref() {
        outside_press_open_change(dialog_state.overlay().policy())
    } else {
        None
    };
    let mut listbox = items
        .into_iter()
        .filter(|item| item.descriptor.matches_query(query.as_str()))
        .fold(Listbox::new(listbox_id, label.clone()), |listbox, item| {
            listbox.option(item.listbox_option())
        })
        .groups(
            groups
                .into_iter()
                .filter_map(|group| group.filtered_listbox_group(query.as_str())),
        )
        .tokens(tokens)
        .with_size(state.size())
        .empty_label(state.empty_label().to_owned())
        .disabled(state.disabled())
        .embedded(true)
        .on_select({
            let runtime = runtime.clone();
            let on_select = on_select.clone();
            let on_open_change = on_open_change.clone();
            let dialog_enabled = state.dialog().is_some();
            let shortcut_lookup = shortcut_lookup.clone();
            move |selection, window, cx| {
                let shortcut = command_shortcut_for(selection.value(), &shortcut_lookup);
                let payload = CommandSelection::new(
                    selection.index(),
                    selection.value().to_owned(),
                    selection.label().to_owned(),
                    shortcut,
                );
                runtime.update(cx, |runtime, _| {
                    runtime.selected_value = Some(payload.value().to_owned());
                    runtime.active_value = Some(payload.value().to_owned());
                    if dialog_enabled {
                        runtime.open = false;
                    }
                });
                if let Some(on_select) = on_select.as_ref() {
                    on_select(payload, window, cx);
                }
                if dialog_enabled {
                    if let Some(on_open_change) = on_open_change.as_ref() {
                        on_open_change(false, window, cx);
                    }
                }
            }
        });
    if let Some(selected_value) = selected_value {
        listbox = listbox.selected(selected_value);
    }
    if let Some(active_value) = active_value {
        listbox = listbox.active(active_value);
    }
    let scroll_viewport_id = state.scroll_area().viewport_id().to_owned();
    let loading_id: ElementId = (content_id.clone(), "loading").into();
    let escape_runtime = runtime.clone();
    let on_escape_open_change = on_open_change.clone();
    let key_state = state.clone();
    let key_runtime = runtime.clone();
    let key_on_select = on_select.clone();
    let key_on_open_change = on_open_change.clone();
    let key_dialog_enabled = state.dialog().is_some();
    let escape_change = state
        .dialog()
        .map(|dialog_state| escape_open_change(dialog_state.overlay().policy()))
        .unwrap_or_else(|| escape_open_change(state.overlay().policy()));

    div()
        .id(content_id)
        .debug_selector(move || format!("command:{debug_id}:content"))
        .min_w(gpui_px_from_ui(metrics.min_width()))
        .max_w(gpui_px_from_ui(metrics.max_width()))
        .p(gpui_px_from_ui(metrics.padding()))
        .flex()
        .flex_col()
        .gap_2()
        .rounded(gpui_px_from_ui(metrics.radius()))
        .border_1()
        .border_color(ThemeResolver::resolve(colors.border()))
        .bg(ThemeResolver::resolve(colors.surface()))
        .text_color(ThemeResolver::resolve(colors.foreground()))
        .shadow_lg()
        .when_some(dialog_state.clone(), |this, dialog_state| {
            this.occlude().ui_role(dialog_state.role())
        })
        .when(dialog_state.is_none(), |this| {
            this.ui_role(state.content_role())
        })
        .aria_label(label.clone())
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let key = event.keystroke.key.as_str();
            if key == "escape" && escape_change.is_some() {
                cx.stop_propagation();
                window.prevent_default();
                close_command_dialog(
                    escape_runtime.clone(),
                    on_escape_open_change.clone(),
                    window,
                    cx,
                );
                return;
            }

            match command_keyboard_action(&key_state, key) {
                CommandKeyboardAction::Navigate(value) => {
                    cx.stop_propagation();
                    window.prevent_default();
                    key_runtime.update(cx, |runtime, _| {
                        runtime.active_value = Some(value);
                    });
                }
                CommandKeyboardAction::Select(selection) => {
                    cx.stop_propagation();
                    window.prevent_default();
                    key_runtime.update(cx, |runtime, _| {
                        runtime.selected_value = Some(selection.value().to_owned());
                        runtime.active_value = Some(selection.value().to_owned());
                        if key_dialog_enabled {
                            runtime.open = false;
                        }
                    });
                    if let Some(on_select) = key_on_select.as_ref() {
                        on_select(selection, window, cx);
                    }
                    if key_dialog_enabled {
                        if let Some(on_open_change) = key_on_open_change.as_ref() {
                            on_open_change(false, window, cx);
                        }
                    }
                }
                CommandKeyboardAction::Ignore => {}
            }
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            this.on_mouse_down_out(move |_, window, cx| {
                close_command_dialog(runtime.clone(), on_open_change.clone(), window, cx);
            })
        })
        .child(
            TextInput::new(input_id, state.label().to_owned())
                .controller(input_controller)
                .placeholder(state.placeholder().to_owned())
                .value(query)
                .disabled(state.disabled())
                .tokens(tokens)
                .with_size(state.size()),
        )
        .when_some(state.loading().cloned(), |this, loading| {
            this.child(
                div()
                    .id(loading_id)
                    .text_color(ThemeResolver::resolve(colors.muted_foreground()))
                    .ui_role(loading.role())
                    .aria_label(loading.message().to_owned())
                    .child(loading.message().to_owned()),
            )
        })
        .h(gpui_px_from_ui(metrics.max_height()))
        .child(
            ScrollArea::new(scroll_viewport_id, listbox)
                .vertical()
                .preserve_scroll()
                .with_size(state.size()),
        )
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommandKeyboardAction {
    Navigate(String),
    Select(CommandSelection),
    Ignore,
}

fn command_keyboard_action(state: &CommandState, key: &str) -> CommandKeyboardAction {
    if state.disabled() {
        return CommandKeyboardAction::Ignore;
    }

    if let Some(target) = state.listbox().navigation_target(key) {
        return CommandKeyboardAction::Navigate(target.value().to_owned());
    }

    if let Some(selection) = state.activation_for_key(key) {
        return CommandKeyboardAction::Select(selection);
    }

    CommandKeyboardAction::Ignore
}

fn close_command_dialog(
    runtime: Entity<CommandRuntime>,
    on_open_change: Option<CommandOpenChangeHandler>,
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

fn command_shortcut_lookup(
    items: &[CommandItem],
    groups: &[CommandGroup],
    query: &str,
) -> Vec<(String, Option<String>)> {
    items
        .iter()
        .filter(|item| item.descriptor.matches_query(query))
        .chain(
            groups
                .iter()
                .flat_map(|group| group.items.iter())
                .filter(|item| item.descriptor.matches_query(query)),
        )
        .map(|item| {
            (
                item.descriptor.value().to_owned(),
                item.descriptor.shortcut_ref().map(str::to_owned),
            )
        })
        .collect()
}

fn command_shortcut_for(value: &str, lookup: &[(String, Option<String>)]) -> Option<String> {
    lookup
        .iter()
        .find(|(item_value, _)| item_value == value)
        .and_then(|(_, shortcut)| shortcut.clone())
}

/// A concrete GPUI command item.
#[derive(Clone)]
pub struct CommandItem {
    descriptor: CommandItemDescriptor,
}

impl CommandItem {
    /// Creates a selectable command item.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: CommandItemDescriptor::new(value, label.to_string()),
        }
    }

    /// Adds one filtering keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.keyword(keyword);
        self
    }

    /// Adds a display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.shortcut(shortcut);
        self
    }

    /// Marks the command as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Returns the pure descriptor.
    pub fn descriptor(&self) -> CommandItemDescriptor {
        self.descriptor.clone()
    }

    fn listbox_option(self) -> ListboxOption {
        ListboxOption::new(self.descriptor.value, self.descriptor.label)
            .disabled(self.descriptor.disabled)
    }
}

/// A concrete GPUI command group.
#[derive(Clone)]
pub struct CommandGroup {
    descriptor: CommandGroupDescriptor,
    items: Vec<CommandItem>,
}

impl CommandGroup {
    /// Creates an empty command group.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: CommandGroupDescriptor::new(value, label.to_string()),
            items: Vec::new(),
        }
    }

    /// Adds one command item.
    pub fn item(mut self, item: CommandItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many command items.
    pub fn items(mut self, items: impl IntoIterator<Item = CommandItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Returns the pure descriptor.
    pub fn descriptor(&self) -> CommandGroupDescriptor {
        self.items
            .iter()
            .fold(self.descriptor.clone(), |descriptor, item| {
                descriptor.item(item.descriptor())
            })
    }

    fn filtered_listbox_group(self, query: &str) -> Option<ListboxGroup> {
        let mut group = ListboxGroup::new(self.descriptor.value, self.descriptor.label);
        let mut has_items = false;
        for item in self.items {
            if item.descriptor.matches_query(query) {
                has_items = true;
                group = group.option(item.listbox_option());
            }
        }
        has_items.then_some(group)
    }
}

fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
}

fn find_command_item<'a>(
    groups: &'a [CommandGroupDescriptor],
    items: &'a [CommandItemDescriptor],
    value: &str,
) -> Option<&'a CommandItemDescriptor> {
    items.iter().find(|item| item.value() == value).or_else(|| {
        groups
            .iter()
            .flat_map(CommandGroupDescriptor::items_ref)
            .find(|item| item.value() == value)
    })
}

impl ThemeResolver {
    pub(crate) const fn command_colors(tokens: ThemeTokens) -> CommandColors {
        CommandColors {
            surface: ColorIntent::new(tokens.surface, 0xffffff),
            foreground: ColorIntent::new(tokens.text, 0x18202a),
            muted_foreground: ColorIntent::new(tokens.text_muted, 0x5a6472),
            border: ColorIntent::new(tokens.border, 0xcfd5cc),
            shortcut_foreground: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Message,
                0x5a6472,
            ),
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

    fn keyboard_state(disabled: bool) -> CommandState {
        Command::new("palette", "Command palette")
            .open(true)
            .disabled(disabled)
            .query("file")
            .selected("new-file")
            .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
            .group(
                CommandGroup::new("file", "File")
                    .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                    .item(CommandItem::new("close-window", "Close Window").shortcut("Alt+F4")),
            )
            .state()
    }

    #[test]
    fn keyboard_action_moves_and_selects_active_command() {
        let state = keyboard_state(false);

        assert_eq!(
            command_keyboard_action(&state, "up"),
            CommandKeyboardAction::Navigate("open-file".to_string())
        );
        assert_eq!(
            command_keyboard_action(&state, "enter"),
            CommandKeyboardAction::Select(CommandSelection::new(
                1,
                "new-file".to_string(),
                "New File".to_string(),
                Some("Ctrl+N".to_string()),
            ))
        );
    }

    #[test]
    fn keyboard_action_ignores_disabled_command() {
        let state = keyboard_state(true);

        assert_eq!(
            command_keyboard_action(&state, "down"),
            CommandKeyboardAction::Ignore
        );
        assert_eq!(
            command_keyboard_action(&state, "enter"),
            CommandKeyboardAction::Ignore
        );
    }

    #[test]
    fn command_state_exposes_standalone_and_grouped_views() {
        let state = keyboard_state(false);

        let standalone_values = state
            .standalone_items()
            .map(|item| item.value().to_owned())
            .collect::<Vec<_>>();
        let grouped_values = state
            .grouped_groups()
            .map(|group| group.value().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(standalone_values, vec!["open-file".to_string()]);
        assert_eq!(grouped_values, vec!["file".to_string()]);
        assert_eq!(state.standalone_items().count(), 1);
    }
}
