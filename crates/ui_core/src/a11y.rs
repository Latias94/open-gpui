//! Renderer-neutral accessibility vocabulary used by the Open GPUI component ecosystem.

/// The semantic role of a component or component part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// A title or descriptive text node.
    Label,
    /// An image or image-like identity primitive.
    Image,
    /// A push button.
    Button,
    /// A link or link-like navigation target.
    Link,
    /// A two-state or tri-state checkbox.
    CheckBox,
    /// A switch control.
    Switch,
    /// A radio button item.
    RadioButton,
    /// A radio group container.
    RadioGroup,
    /// A toolbar container.
    Toolbar,
    /// Navigation landmark content.
    Navigation,
    /// A section or group landmark.
    Section,
    /// A generic grouped collection.
    Group,
    /// A hierarchical tree container.
    Tree,
    /// A tree item in a hierarchical collection.
    TreeItem,
    /// A table container.
    Table,
    /// A table row.
    Row,
    /// A table header cell.
    ColumnHeader,
    /// A table cell.
    Cell,
    /// A listbox popup or collection.
    ListBox,
    /// A listbox option item.
    ListBoxOption,
    /// A menu popup or collection.
    Menu,
    /// A menu action item.
    MenuItem,
    /// A text input field.
    TextInput,
    /// An editable combobox input.
    EditableComboBox,
    /// A dialog surface.
    Dialog,
    /// An alert dialog surface.
    AlertDialog,
    /// A generic window-like overlay surface.
    Window,
    /// A progress indicator.
    ProgressIndicator,
    /// A separator between sections or groups.
    Separator,
    /// A numeric spin button.
    SpinButton,
    /// A numeric slider.
    Slider,
    /// A split view resize handle.
    Splitter,
    /// A tab list container.
    TabList,
    /// A tab item.
    Tab,
    /// A tab panel.
    TabPanel,
}

/// Pressed or checked state for controls with toggle semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Toggled {
    /// The control is not toggled.
    False,
    /// The control is toggled.
    True,
    /// The control has a mixed or indeterminate state.
    Mixed,
}

impl From<bool> for Toggled {
    fn from(value: bool) -> Self {
        if value { Self::True } else { Self::False }
    }
}

/// Semantic orientation for composite widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Orientation {
    /// Items are laid out horizontally.
    Horizontal,
    /// Items are laid out vertically.
    Vertical,
}

/// Semantic sort direction exposed by sortable collection headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SortDirection {
    /// Values are sorted in ascending order.
    Ascending,
    /// Values are sorted in descending order.
    Descending,
    /// Values use an application-defined ordering.
    Other,
}

/// Accessibility action requested by assistive technology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccessibleAction {
    /// Activate the target.
    Click,
    /// Move focus to the target.
    Focus,
    /// Move focus away from the target.
    Blur,
    /// Collapse the target.
    Collapse,
    /// Expand the target.
    Expand,
    /// Dispatch a custom action.
    CustomAction,
    /// Decrement a numeric value.
    Decrement,
    /// Increment a numeric value.
    Increment,
    /// Hide tooltip content.
    HideTooltip,
    /// Show tooltip content.
    ShowTooltip,
    /// Replace the selected text.
    ReplaceSelectedText,
    /// Scroll down.
    ScrollDown,
    /// Scroll left.
    ScrollLeft,
    /// Scroll right.
    ScrollRight,
    /// Scroll up.
    ScrollUp,
    /// Scroll the target into view.
    ScrollIntoView,
    /// Scroll the target to a point.
    ScrollToPoint,
    /// Set the scroll offset.
    SetScrollOffset,
    /// Set the text selection.
    SetTextSelection,
    /// Set the starting point for sequential focus navigation.
    SetSequentialFocusNavigationStartingPoint,
    /// Set a value.
    SetValue,
    /// Show the context menu.
    ShowContextMenu,
}

impl AccessibleAction {
    /// Returns whether this action can mutate the node's value.
    pub const fn mutates_value(self) -> bool {
        matches!(
            self,
            Self::Decrement | Self::Increment | Self::ReplaceSelectedText | Self::SetValue
        )
    }
}

/// Ephemeral accessibility semantics derived from a component's resolved state.
///
/// Components construct this value while rendering. It is a projection, not stored component
/// state, and renderer adapters consume it to build their platform accessibility nodes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticDescriptor<'a, NodeId = std::convert::Infallible> {
    role: Role,
    label: Option<&'a str>,
    description: Option<&'a str>,
    value: Option<&'a str>,
    controls: &'a [NodeId],
    labelled_by: &'a [NodeId],
    described_by: &'a [NodeId],
    selected: Option<bool>,
    required: Option<bool>,
    invalid: Option<bool>,
    busy: Option<bool>,
    read_only: Option<bool>,
    hidden: Option<bool>,
    modal: Option<bool>,
    disabled: Option<bool>,
    expanded: Option<bool>,
    toggled: Option<Toggled>,
    numeric_value: Option<f64>,
    min_numeric_value: Option<f64>,
    max_numeric_value: Option<f64>,
    orientation: Option<Orientation>,
    level: Option<usize>,
    position_in_set: Option<usize>,
    size_of_set: Option<usize>,
    row_index: Option<usize>,
    column_index: Option<usize>,
    row_span: Option<usize>,
    column_span: Option<usize>,
    row_count: Option<usize>,
    column_count: Option<usize>,
    sort_direction: Option<SortDirection>,
    actions: &'a [AccessibleAction],
}

impl<'a, NodeId> SemanticDescriptor<'a, NodeId> {
    /// Creates an empty semantic projection for a role.
    pub const fn new(role: Role) -> Self {
        Self {
            role,
            label: None,
            description: None,
            value: None,
            controls: &[],
            labelled_by: &[],
            described_by: &[],
            selected: None,
            required: None,
            invalid: None,
            busy: None,
            read_only: None,
            hidden: None,
            modal: None,
            disabled: None,
            expanded: None,
            toggled: None,
            numeric_value: None,
            min_numeric_value: None,
            max_numeric_value: None,
            orientation: None,
            level: None,
            position_in_set: None,
            size_of_set: None,
            row_index: None,
            column_index: None,
            row_span: None,
            column_span: None,
            row_count: None,
            column_count: None,
            sort_direction: None,
            actions: &[],
        }
    }

    /// Returns the semantic role.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns the accessible label.
    pub const fn label(&self) -> Option<&'a str> {
        self.label
    }

    /// Returns the accessible description.
    pub const fn description(&self) -> Option<&'a str> {
        self.description
    }

    /// Returns the text value.
    pub const fn value(&self) -> Option<&'a str> {
        self.value
    }

    /// Returns controlled semantic node identities.
    pub const fn controls(&self) -> &[NodeId] {
        self.controls
    }

    /// Returns labelling semantic node identities.
    pub const fn labelled_by(&self) -> &[NodeId] {
        self.labelled_by
    }

    /// Returns describing semantic node identities.
    pub const fn described_by(&self) -> &[NodeId] {
        self.described_by
    }

    /// Returns selected state when applicable.
    pub const fn selected(&self) -> Option<bool> {
        self.selected
    }

    /// Returns required state when applicable.
    pub const fn required(&self) -> Option<bool> {
        self.required
    }

    /// Returns invalid state when applicable.
    pub const fn invalid(&self) -> Option<bool> {
        self.invalid
    }

    /// Returns busy state when applicable.
    pub const fn busy(&self) -> Option<bool> {
        self.busy
    }

    /// Returns read-only state when applicable.
    pub const fn read_only(&self) -> Option<bool> {
        self.read_only
    }

    /// Returns hidden state when applicable.
    pub const fn hidden(&self) -> Option<bool> {
        self.hidden
    }

    /// Returns modal state when applicable.
    pub const fn modal(&self) -> Option<bool> {
        self.modal
    }

    /// Returns disabled state when applicable.
    pub const fn disabled(&self) -> Option<bool> {
        self.disabled
    }

    /// Returns expanded state when applicable.
    pub const fn expanded(&self) -> Option<bool> {
        self.expanded
    }

    /// Returns toggled state when applicable.
    pub const fn toggled(&self) -> Option<Toggled> {
        self.toggled
    }

    /// Returns the numeric value when applicable.
    pub const fn numeric_value(&self) -> Option<f64> {
        self.numeric_value
    }

    /// Returns the minimum numeric value when applicable.
    pub const fn min_numeric_value(&self) -> Option<f64> {
        self.min_numeric_value
    }

    /// Returns the maximum numeric value when applicable.
    pub const fn max_numeric_value(&self) -> Option<f64> {
        self.max_numeric_value
    }

    /// Returns orientation when applicable.
    pub const fn orientation(&self) -> Option<Orientation> {
        self.orientation
    }

    /// Returns hierarchy level when applicable.
    pub const fn level(&self) -> Option<usize> {
        self.level
    }

    /// Returns collection position when applicable.
    pub const fn position_in_set(&self) -> Option<usize> {
        self.position_in_set
    }

    /// Returns collection size when applicable.
    pub const fn size_of_set(&self) -> Option<usize> {
        self.size_of_set
    }

    /// Returns the 1-based table row index when applicable.
    pub const fn row_index(&self) -> Option<usize> {
        self.row_index
    }

    /// Returns the 1-based table column index when applicable.
    pub const fn column_index(&self) -> Option<usize> {
        self.column_index
    }

    /// Returns the number of table rows spanned by this node.
    pub const fn row_span(&self) -> Option<usize> {
        self.row_span
    }

    /// Returns the number of table columns spanned by this node.
    pub const fn column_span(&self) -> Option<usize> {
        self.column_span
    }

    /// Returns total table row count when applicable.
    pub const fn row_count(&self) -> Option<usize> {
        self.row_count
    }

    /// Returns total table column count when applicable.
    pub const fn column_count(&self) -> Option<usize> {
        self.column_count
    }

    /// Returns the semantic sort direction when applicable.
    pub const fn sort_direction(&self) -> Option<SortDirection> {
        self.sort_direction
    }

    /// Returns actions available after applying disabled and read-only state.
    pub fn available_actions(&self) -> impl Iterator<Item = AccessibleAction> + '_ {
        self.actions.iter().copied().filter(|action| {
            self.disabled != Some(true) && (self.read_only != Some(true) || !action.mutates_value())
        })
    }

    /// Returns whether an action is available after applying resolved state.
    pub fn supports_action(&self, action: AccessibleAction) -> bool {
        self.available_actions()
            .any(|available| available == action)
    }

    /// Applies an accessible label.
    pub const fn with_label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Applies an accessible description.
    pub const fn with_description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Applies a text value.
    pub const fn with_value(mut self, value: &'a str) -> Self {
        self.value = Some(value);
        self
    }

    /// Applies controlled node relations.
    pub const fn with_controls(mut self, controls: &'a [NodeId]) -> Self {
        self.controls = controls;
        self
    }

    /// Applies labelling node relations.
    pub const fn with_labelled_by(mut self, labelled_by: &'a [NodeId]) -> Self {
        self.labelled_by = labelled_by;
        self
    }

    /// Applies describing node relations.
    pub const fn with_described_by(mut self, described_by: &'a [NodeId]) -> Self {
        self.described_by = described_by;
        self
    }

    /// Applies selected state.
    pub const fn with_selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Applies required state.
    pub const fn with_required(mut self, required: bool) -> Self {
        self.required = Some(required);
        self
    }

    /// Applies invalid state.
    pub const fn with_invalid(mut self, invalid: bool) -> Self {
        self.invalid = Some(invalid);
        self
    }

    /// Applies busy state.
    pub const fn with_busy(mut self, busy: bool) -> Self {
        self.busy = Some(busy);
        self
    }

    /// Applies read-only state.
    pub const fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = Some(read_only);
        self
    }

    /// Applies hidden state.
    pub const fn with_hidden(mut self, hidden: bool) -> Self {
        self.hidden = Some(hidden);
        self
    }

    /// Applies modal state.
    pub const fn with_modal(mut self, modal: bool) -> Self {
        self.modal = Some(modal);
        self
    }

    /// Applies disabled state.
    pub const fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// Applies expanded state.
    pub const fn with_expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Applies toggled state.
    pub const fn with_toggled(mut self, toggled: Toggled) -> Self {
        self.toggled = Some(toggled);
        self
    }

    /// Applies a numeric value.
    pub const fn with_numeric_value(mut self, value: f64) -> Self {
        self.numeric_value = Some(value);
        self
    }

    /// Applies a minimum numeric value.
    pub const fn with_min_numeric_value(mut self, value: f64) -> Self {
        self.min_numeric_value = Some(value);
        self
    }

    /// Applies a maximum numeric value.
    pub const fn with_max_numeric_value(mut self, value: f64) -> Self {
        self.max_numeric_value = Some(value);
        self
    }

    /// Applies orientation.
    pub const fn with_orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// Applies hierarchy level.
    pub const fn with_level(mut self, level: usize) -> Self {
        self.level = Some(level);
        self
    }

    /// Applies collection position.
    pub const fn with_position_in_set(mut self, position: usize) -> Self {
        self.position_in_set = Some(position);
        self
    }

    /// Applies collection size.
    pub const fn with_size_of_set(mut self, size: usize) -> Self {
        self.size_of_set = Some(size);
        self
    }

    /// Applies a 1-based table row index.
    pub const fn with_row_index(mut self, index: usize) -> Self {
        self.row_index = Some(index);
        self
    }

    /// Applies a 1-based table column index.
    pub const fn with_column_index(mut self, index: usize) -> Self {
        self.column_index = Some(index);
        self
    }

    /// Applies the number of table rows spanned by this node.
    pub const fn with_row_span(mut self, span: usize) -> Self {
        self.row_span = Some(span);
        self
    }

    /// Applies the number of table columns spanned by this node.
    pub const fn with_column_span(mut self, span: usize) -> Self {
        self.column_span = Some(span);
        self
    }

    /// Applies total table row count.
    pub const fn with_row_count(mut self, count: usize) -> Self {
        self.row_count = Some(count);
        self
    }

    /// Applies total table column count.
    pub const fn with_column_count(mut self, count: usize) -> Self {
        self.column_count = Some(count);
        self
    }

    /// Applies the semantic sort direction.
    pub const fn with_sort_direction(mut self, direction: SortDirection) -> Self {
        self.sort_direction = Some(direction);
        self
    }

    /// Applies actions available on the resolved node.
    pub const fn with_actions(mut self, actions: &'a [AccessibleAction]) -> Self {
        self.actions = actions;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn tab_and_splitter_roles_are_renderer_neutral_vocabulary() {
        let roles = BTreeSet::from([Role::TabList, Role::Tab, Role::TabPanel, Role::Splitter]);

        assert_eq!(roles.len(), 4);
        assert!(roles.contains(&Role::TabList));
        assert!(roles.contains(&Role::Tab));
        assert!(roles.contains(&Role::TabPanel));
        assert!(roles.contains(&Role::Splitter));
    }

    #[test]
    fn docking_actions_are_renderer_neutral_vocabulary() {
        let actions = BTreeSet::from([
            AccessibleAction::Click,
            AccessibleAction::Focus,
            AccessibleAction::Increment,
            AccessibleAction::Decrement,
        ]);

        assert_eq!(actions.len(), 4);
        assert!(actions.contains(&AccessibleAction::Click));
        assert!(actions.contains(&AccessibleAction::Focus));
        assert!(actions.contains(&AccessibleAction::Increment));
        assert!(actions.contains(&AccessibleAction::Decrement));
    }

    #[test]
    fn semantic_descriptor_borrows_text_and_keeps_relation_ids_generic() {
        let label = String::from("Save");
        let description = String::from("Writes the document");
        let controls = [7_u16];
        let descriptor = SemanticDescriptor::<u16>::new(Role::Button)
            .with_label(&label)
            .with_description(&description)
            .with_controls(&controls)
            .with_row_span(2)
            .with_column_span(3)
            .with_sort_direction(SortDirection::Ascending);

        assert_eq!(descriptor.label(), Some("Save"));
        assert_eq!(descriptor.description(), Some("Writes the document"));
        assert_eq!(descriptor.controls(), &[7]);
        assert_eq!(descriptor.row_span(), Some(2));
        assert_eq!(descriptor.column_span(), Some(3));
        assert_eq!(descriptor.sort_direction(), Some(SortDirection::Ascending));
    }

    #[test]
    fn semantic_descriptor_resolves_available_actions_from_state() {
        let actions = [
            AccessibleAction::Click,
            AccessibleAction::Focus,
            AccessibleAction::SetValue,
        ];
        let read_only: SemanticDescriptor<'_> = SemanticDescriptor::new(Role::Slider)
            .with_read_only(true)
            .with_actions(&actions);

        assert!(read_only.supports_action(AccessibleAction::Click));
        assert!(read_only.supports_action(AccessibleAction::Focus));
        assert!(!read_only.supports_action(AccessibleAction::SetValue));

        let disabled = read_only.with_disabled(true);
        assert_eq!(disabled.available_actions().count(), 0);
    }
}
