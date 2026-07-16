//! Tabs component.

mod render;

use std::collections::BTreeMap;
use std::rc::Rc;

use open_gpui::{AnyElement, App, ElementId, IntoElement, SharedString, Window};
use open_gpui_ui_core::{Orientation, Sizable, Size, ThemeTokens, UiPx, ui_px};

use crate::activation::ActivationHandle;
use crate::choice::{
    ChoiceActivationMode, ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection,
};
use crate::color::{ColorIntent, ColorState};
pub use crate::roving_focus::{
    active_index_from_str_keys, first_enabled, last_enabled, next_enabled,
};

const DEFAULT_SURFACE: u32 = 0xffffff;
const DEFAULT_GHOST_SURFACE: u32 = 0xf1f5ee;
const DEFAULT_BORDER: u32 = 0xcfd5cc;
const DEFAULT_TEXT: u32 = 0x18202a;
const DEFAULT_TEXT_MUTED: u32 = 0x5a6472;
const DEFAULT_ACCENT: u32 = 0x1f7a66;
const DEFAULT_FOCUS_RING: u32 = 0x2f80ed;

/// Tabs activation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabsActivationMode {
    /// Arrow navigation moves focus and selection together.
    #[default]
    Automatic,
    /// Arrow navigation only moves focus; Enter or Space activates the focused tab.
    Manual,
}

impl TabsActivationMode {
    /// Returns the stable activation mode label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
        }
    }
}

/// Pure descriptor for one tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsItemDescriptor {
    value: String,
    label: String,
    disabled: bool,
}

impl TabsItemDescriptor {
    /// Creates a new descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Marks the tab as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns the stable tab value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether the tab is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }
}

/// Resolved tab colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabsColors {
    shell_background: ColorIntent,
    shell_border: ColorIntent,
    tab_background: ColorIntent,
    tab_background_selected: ColorIntent,
    tab_hover_background: ColorIntent,
    tab_text: ColorIntent,
    tab_text_muted: ColorIntent,
    tab_border: ColorIntent,
    tab_border_selected: ColorIntent,
    panel_background: ColorIntent,
    focus_ring: ColorIntent,
}

impl TabsColors {
    /// Resolves colors from the shared token bundle.
    pub const fn from_tokens(tokens: ThemeTokens) -> Self {
        Self {
            shell_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            shell_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            tab_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            tab_background_selected: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Selected,
                DEFAULT_GHOST_SURFACE,
            ),
            tab_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                DEFAULT_GHOST_SURFACE,
            ),
            tab_text: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            tab_text_muted: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            tab_border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            tab_border_selected: ColorIntent::with_state(
                tokens.accent,
                ColorState::Selected,
                DEFAULT_ACCENT,
            ),
            panel_background: ColorIntent::new(tokens.surface, DEFAULT_SURFACE),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                DEFAULT_FOCUS_RING,
            ),
        }
    }

    /// Returns the shell background color intent.
    pub const fn shell_background(self) -> ColorIntent {
        self.shell_background
    }

    /// Returns the shell border color intent.
    pub const fn shell_border(self) -> ColorIntent {
        self.shell_border
    }

    /// Returns the unselected tab background color intent.
    pub const fn tab_background(self) -> ColorIntent {
        self.tab_background
    }

    /// Returns the selected tab background color intent.
    pub const fn tab_background_selected(self) -> ColorIntent {
        self.tab_background_selected
    }

    /// Returns the hover background color intent.
    pub const fn tab_hover_background(self) -> ColorIntent {
        self.tab_hover_background
    }

    /// Returns the default tab text color intent.
    pub const fn tab_text(self) -> ColorIntent {
        self.tab_text
    }

    /// Returns the muted tab text color intent.
    pub const fn tab_text_muted(self) -> ColorIntent {
        self.tab_text_muted
    }

    /// Returns the default tab border color intent.
    pub const fn tab_border(self) -> ColorIntent {
        self.tab_border
    }

    /// Returns the selected tab border color intent.
    pub const fn tab_border_selected(self) -> ColorIntent {
        self.tab_border_selected
    }

    /// Returns the panel background color intent.
    pub const fn panel_background(self) -> ColorIntent {
        self.panel_background
    }

    /// Returns the focus ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved tab metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabsMetrics {
    tab_min_height: UiPx,
    tab_padding_x: UiPx,
    tab_padding_y: UiPx,
    tab_gap: UiPx,
    panel_padding: UiPx,
    radius: UiPx,
    text_size: UiPx,
}

impl TabsMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            tab_min_height: size.button_h(),
            tab_padding_x: size.button_px(),
            tab_padding_y: size.button_py(),
            tab_gap: ui_px(4.0),
            panel_padding: ui_px(12.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the minimum tab height.
    pub const fn tab_min_height(self) -> UiPx {
        self.tab_min_height
    }

    /// Returns the horizontal tab padding.
    pub const fn tab_padding_x(self) -> UiPx {
        self.tab_padding_x
    }

    /// Returns the vertical tab padding.
    pub const fn tab_padding_y(self) -> UiPx {
        self.tab_padding_y
    }

    /// Returns the gap between tabs.
    pub const fn tab_gap(self) -> UiPx {
        self.tab_gap
    }

    /// Returns the panel padding.
    pub const fn panel_padding(self) -> UiPx {
        self.panel_padding
    }

    /// Returns the corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns the text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }
}

/// Resolved tab item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsItemState {
    index: usize,
    value: String,
    label: String,
    disabled: bool,
    selected: bool,
    focused: bool,
}

impl TabsItemState {
    /// Returns the zero-based index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable tab value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether the tab is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the tab is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the tab currently has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }
}

/// Resolved selection change payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabsSelection {
    index: usize,
    value: String,
    label: String,
}

impl TabsSelection {
    /// Creates a selection payload from a descriptor.
    fn from_descriptor(index: usize, descriptor: &TabsItemDescriptor) -> Self {
        Self {
            index,
            value: descriptor.value.clone(),
            label: descriptor.label.clone(),
        }
    }

    /// Returns the zero-based index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the selected tab value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the selected tab label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved tabs state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct TabsState {
    orientation: Orientation,
    activation_mode: TabsActivationMode,
    size: Size,
    metrics: TabsMetrics,
    colors: TabsColors,
    items: Vec<TabsItemState>,
    selected_index: Option<usize>,
    focused_index: Option<usize>,
}

/// Selection ownership passed to [`TabsState::resolve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabsSelectionAuthority<'a> {
    /// Preserve the caller-owned value exactly, including no or unavailable selection.
    Controlled(Option<&'a str>),
    /// Resolve an adapter-owned default, falling back according to the tabs policy.
    Uncontrolled(Option<&'a str>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum TabsSelectionControl {
    #[default]
    Uncontrolled,
    Controlled(Option<String>),
}

impl TabsSelectionControl {
    fn authority<'a>(
        &'a self,
        default_selected_value: Option<&'a str>,
    ) -> TabsSelectionAuthority<'a> {
        match self {
            Self::Uncontrolled => TabsSelectionAuthority::Uncontrolled(default_selected_value),
            Self::Controlled(value) => TabsSelectionAuthority::Controlled(value.as_deref()),
        }
    }

    const fn controlled(&self) -> bool {
        matches!(self, Self::Controlled(_))
    }

    fn initial_value(&self, default_selected_value: Option<&String>) -> Option<String> {
        match self {
            Self::Uncontrolled => default_selected_value.cloned(),
            Self::Controlled(value) => value.clone(),
        }
    }
}

impl TabsState {
    /// Resolves the public state for tabs.
    pub fn resolve(
        orientation: Orientation,
        activation_mode: TabsActivationMode,
        size: Size,
        selection: TabsSelectionAuthority<'_>,
        focused_value: Option<&str>,
        items: impl IntoIterator<Item = TabsItemDescriptor>,
        tokens: ThemeTokens,
    ) -> Self {
        let descriptors: Vec<TabsItemDescriptor> = items.into_iter().collect();
        let policy = tabs_choice_policy(orientation, activation_mode);
        let selected_value = match selection {
            TabsSelectionAuthority::Controlled(value)
            | TabsSelectionAuthority::Uncontrolled(value) => value,
        };
        let selected_fallback_value = match selection {
            TabsSelectionAuthority::Controlled(_) => None,
            TabsSelectionAuthority::Uncontrolled(_) => focused_value,
        };
        let collection = ChoiceCollection::resolve_with_selected_fallback(
            false,
            tabs_choice_items(&descriptors),
            selected_value,
            selected_fallback_value,
            focused_value,
            policy,
        );
        let selected_index = match selection {
            TabsSelectionAuthority::Controlled(Some(value)) => descriptors
                .iter()
                .position(|descriptor| descriptor.value() == value),
            TabsSelectionAuthority::Controlled(None) => None,
            TabsSelectionAuthority::Uncontrolled(_) => collection.selected_index(),
        };
        let focused_index = collection.active_index();
        let metrics = TabsMetrics::from_size(size);
        let colors = TabsColors::from_tokens(tokens);

        let items = descriptors
            .into_iter()
            .enumerate()
            .map(|(index, descriptor)| {
                let selected = Some(index) == selected_index;
                let focused = Some(index) == focused_index;

                TabsItemState {
                    index,
                    value: descriptor.value,
                    label: descriptor.label,
                    disabled: descriptor.disabled,
                    selected,
                    focused,
                }
            })
            .collect();

        Self {
            orientation,
            activation_mode,
            size,
            metrics,
            colors,
            items,
            selected_index,
            focused_index,
        }
    }

    /// Returns the tab orientation.
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Returns the activation mode.
    pub const fn activation_mode(&self) -> TabsActivationMode {
        self.activation_mode
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the resolved metrics.
    pub const fn metrics(&self) -> TabsMetrics {
        self.metrics
    }

    /// Returns the resolved color intents.
    pub const fn colors(&self) -> TabsColors {
        self.colors
    }

    /// Returns all resolved tab items.
    pub fn items(&self) -> &[TabsItemState] {
        &self.items
    }

    /// Returns the selected tab index.
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Returns the selected tab value.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected_index
            .and_then(|index| self.items.get(index))
            .map(|item| item.value())
    }

    /// Returns the selected tab item.
    pub fn selected_item(&self) -> Option<&TabsItemState> {
        self.selected_index.and_then(|index| self.items.get(index))
    }

    /// Returns the focused tab index.
    pub const fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns the focused tab value.
    pub fn focused_value(&self) -> Option<&str> {
        self.focused_index
            .and_then(|index| self.items.get(index))
            .map(|item| item.value())
    }

    /// Returns the focused tab item.
    pub fn focused_item(&self) -> Option<&TabsItemState> {
        self.focused_index.and_then(|index| self.items.get(index))
    }

    /// Returns the current tab stop index.
    pub const fn tab_stop_index(&self) -> Option<usize> {
        if self.focused_index.is_some() {
            self.focused_index
        } else {
            self.selected_index
        }
    }

    /// Returns the current tab stop item.
    pub fn tab_stop_item(&self) -> Option<&TabsItemState> {
        self.tab_stop_index()
            .and_then(|index| self.items.get(index))
    }

    /// Returns the item at the given index.
    pub fn item(&self, index: usize) -> Option<&TabsItemState> {
        self.items.get(index)
    }

    /// Returns whether the state has no tabs.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

fn tabs_choice_policy(
    orientation: Orientation,
    activation_mode: TabsActivationMode,
) -> ChoiceInteractionPolicy {
    let activation_mode = match activation_mode {
        TabsActivationMode::Automatic => ChoiceActivationMode::Automatic,
        TabsActivationMode::Manual => ChoiceActivationMode::Manual,
    };

    ChoiceInteractionPolicy::single_required(orientation).with_activation_mode(activation_mode)
}

fn tabs_choice_items(items: &[TabsItemDescriptor]) -> Vec<ChoiceItemProjection<()>> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let label = item.label().to_owned();
            ChoiceItemProjection::new(index, None, item.value(), label.clone(), item.disabled, ())
                .text_value(label)
        })
        .collect()
}

/// A concrete GPUI tab item.
pub struct TabsItem {
    value: String,
    label: SharedString,
    disabled: bool,
    panel: AnyElement,
}

impl TabsItem {
    /// Creates a new tab item with panel content.
    pub fn new(
        value: impl Into<String>,
        label: impl Into<SharedString>,
        panel: impl IntoElement,
    ) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
            panel: panel.into_any_element(),
        }
    }

    /// Marks the tab item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn descriptor(&self) -> TabsItemDescriptor {
        TabsItemDescriptor {
            value: self.value.clone(),
            label: self.label.to_string(),
            disabled: self.disabled,
        }
    }
}

/// A concrete GPUI tabs component.
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    orientation: Orientation,
    activation_mode: TabsActivationMode,
    selection: TabsSelectionControl,
    default_selected_value: Option<String>,
    size: Size,
    tokens: ThemeTokens,
    items: Vec<TabsItem>,
    on_selection_change: Option<Rc<dyn Fn(TabsSelection, &mut Window, &mut App)>>,
    activation_handles: BTreeMap<String, ActivationHandle>,
}

impl Tabs {
    /// Creates a new tabs component.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            orientation: Orientation::Horizontal,
            activation_mode: TabsActivationMode::Automatic,
            selection: TabsSelectionControl::default(),
            default_selected_value: None,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            items: Vec::new(),
            on_selection_change: None,
            activation_handles: BTreeMap::new(),
        }
    }

    /// Sets the tab orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Sets the activation mode.
    pub fn activation_mode(mut self, activation_mode: TabsActivationMode) -> Self {
        self.activation_mode = activation_mode;
        self
    }

    /// Applies the caller-owned selected tab value.
    pub fn selected(mut self, value: Option<String>) -> Self {
        self.selection = TabsSelectionControl::Controlled(value);
        self
    }

    /// Applies the default selected tab value for adapter-owned runtime state.
    pub fn default_selected(mut self, value: impl Into<String>) -> Self {
        self.default_selected_value = Some(value.into());
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Adds a tab item.
    pub fn item(mut self, item: TabsItem) -> Self {
        self.items.push(item);
        self
    }

    /// Registers a selection change handler.
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(TabsSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Rc::new(handler));
        self
    }

    /// Binds an application-owned activation handle to one stable tab value.
    pub fn activation_handle(
        mut self,
        value: impl Into<String>,
        handle: &ActivationHandle,
    ) -> Self {
        self.activation_handles.insert(value.into(), handle.clone());
        self
    }

    /// Returns the resolved state.
    pub fn state(&self) -> TabsState {
        TabsState::resolve(
            self.orientation,
            self.activation_mode,
            self.size,
            self.selection
                .authority(self.default_selected_value.as_deref()),
            None,
            self.items.iter().map(TabsItem::descriptor),
            self.tokens,
        )
    }
}

impl Sizable for Tabs {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roving_focus_helpers_skip_disabled_items_and_wrap() {
        let disabled = [false, true, false];

        assert_eq!(first_enabled(&disabled), Some(0));
        assert_eq!(last_enabled(&disabled), Some(2));
        assert_eq!(next_enabled(&disabled, 0, true, true), Some(2));
        assert_eq!(next_enabled(&disabled, 2, true, true), Some(0));
        assert_eq!(next_enabled(&disabled, 2, false, true), Some(0));
    }

    #[test]
    fn tabs_state_prefers_selected_value_and_tracks_focus() {
        let state = TabsState::resolve(
            Orientation::Horizontal,
            TabsActivationMode::Manual,
            Size::Medium,
            TabsSelectionAuthority::Uncontrolled(Some("details")),
            Some("history"),
            [
                TabsItemDescriptor::new("overview", "Overview"),
                TabsItemDescriptor::new("details", "Details"),
                TabsItemDescriptor::new("history", "History").disabled(true),
            ],
            ThemeTokens::default(),
        );

        assert_eq!(state.orientation(), Orientation::Horizontal);
        assert_eq!(state.activation_mode(), TabsActivationMode::Manual);
        assert_eq!(state.selected_value(), Some("details"));
        assert_eq!(state.focused_value(), Some("details"));
        assert!(state.items()[1].selected());
        assert!(state.items()[1].focused());
        assert_eq!(state.tab_stop_index(), Some(1));
    }

    #[test]
    fn controlled_tabs_preserve_empty_or_unavailable_selection() {
        let items = || {
            [
                TabsItemDescriptor::new("overview", "Overview"),
                TabsItemDescriptor::new("managed", "Managed").disabled(true),
            ]
        };

        for selection in [
            TabsSelectionAuthority::Controlled(None),
            TabsSelectionAuthority::Controlled(Some("missing")),
        ] {
            let state = TabsState::resolve(
                Orientation::Horizontal,
                TabsActivationMode::Manual,
                Size::Medium,
                selection,
                None,
                items(),
                ThemeTokens::default(),
            );

            assert_eq!(state.selected_value(), None);
            assert_eq!(state.focused_value(), Some("overview"));
            assert_eq!(state.tab_stop_index(), Some(0));
            assert!(state.items().iter().all(|item| !item.selected()));
        }
    }
}
