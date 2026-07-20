//! GPUI accessibility adapter utilities.

use open_gpui::{
    AccessibleAction as GpuiAccessibleAction, Orientation as GpuiOrientation, Role as GpuiRole,
    StatefulInteractiveElement, Toggled as GpuiToggled, accesskit,
};
use open_gpui_ui_core::{
    AccessibleAction, LivePoliteness, Orientation, Role, SemanticDescriptor, SortDirection, Toggled,
};

mod text_control;

pub use text_control::TextControlSemanticProjection;
#[cfg(test)]
use text_control::text_runs_cover_value;
pub(crate) use text_control::{
    AccessibleTextInputHandler, AccessibleTextReplacementTarget, AccessibleTextRunRange,
    dispatch_accessible_text_replacement, dispatch_accessible_text_selection,
    dispatch_accessible_text_selection_in_runs, project_accessible_text_selection_in_runs,
};

/// Source that provides an accessible name for a component or component part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum A11yLabelSource {
    /// No accessible name is required for this role in the current contract.
    NotRequired,
    /// The visible text label is the accessible name.
    VisibleText,
    /// The caller supplied an explicit accessible label.
    ExplicitLabel,
    /// The component is associated with a separate label element.
    AssociatedLabel,
    /// The component derives its name from placeholder-like text.
    Placeholder,
    /// The component generates a stable name from semantic state.
    Generated,
}

impl A11yLabelSource {
    /// Returns whether this source provides an accessible name.
    pub const fn provides_name(self) -> bool {
        !matches!(self, Self::NotRequired)
    }
}

/// Source that provides an accessible description for a component or component part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum A11yDescriptionSource {
    /// No accessible description is supplied or required.
    None,
    /// The caller supplied an explicit description.
    ExplicitDescription,
    /// Help text describes the control.
    HelpText,
    /// Error text describes the invalid state.
    ErrorText,
    /// The component generates a stable description from semantic state.
    Generated,
}

/// Kind of value metadata exposed by an accessibility contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum A11yValueKind {
    /// Editable or display text value.
    Text,
    /// Numeric scalar value.
    Number,
    /// Percentage progress or slider value.
    Percent,
    /// Count or collection-size value.
    Count,
    /// Selected item or selected-value value.
    Selection,
}

/// Semantic state or focus behavior covered by component accessibility evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum A11yStateEvidence {
    /// The component evidence covers disabled state propagation.
    Disabled,
    /// The component evidence covers selected state propagation.
    Selected,
    /// The component evidence covers checked or toggled state propagation.
    Checked,
    /// The component evidence covers expanded/collapsed state propagation.
    Expanded,
    /// The component evidence covers value metadata propagation.
    Value,
    /// The component evidence covers focusable interactive behavior.
    Focusable,
    /// The component evidence covers a structural row or overlay that must not be interactive.
    NonInteractiveStructural,
}

/// Renderer-neutral value metadata for accessibility assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct A11yValueMetadata {
    kind: A11yValueKind,
    present: bool,
}

impl A11yValueMetadata {
    /// Creates value metadata for a present semantic value.
    pub const fn present(kind: A11yValueKind) -> Self {
        Self {
            kind,
            present: true,
        }
    }

    /// Creates value metadata for a supported value that is currently absent.
    pub const fn absent(kind: A11yValueKind) -> Self {
        Self {
            kind,
            present: false,
        }
    }

    /// Returns the value kind.
    pub const fn kind(self) -> A11yValueKind {
        self.kind
    }

    /// Returns whether the value is currently present.
    pub const fn is_present(self) -> bool {
        self.present
    }
}

/// Validation failure for a component accessibility contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A11yContractError {
    /// The role requires an accessible name but no label source was recorded.
    MissingAccessibleName,
    /// The role requires value metadata but no value contract was recorded.
    MissingValueMetadata,
    /// The role requires at least one supported action but none were recorded.
    MissingSupportedAction,
}

/// One failed accessibility contract check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A11yContractViolation {
    component: &'static str,
    role: Role,
    error: A11yContractError,
}

impl A11yContractViolation {
    /// Returns the component or component part that failed validation.
    pub const fn component(self) -> &'static str {
        self.component
    }

    /// Returns the role that failed validation.
    pub const fn role(self) -> Role {
        self.role
    }

    /// Returns the validation error.
    pub const fn error(self) -> A11yContractError {
        self.error
    }
}

/// Renderer-neutral accessibility contract for one component or component part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentA11yContract {
    component: &'static str,
    role: Role,
    label_source: A11yLabelSource,
    description_source: A11yDescriptionSource,
    selected: Option<bool>,
    checked: Option<Toggled>,
    expanded: Option<bool>,
    disabled: Option<bool>,
    value: Option<A11yValueMetadata>,
    orientation: Option<Orientation>,
    actions: &'static [AccessibleAction],
}

impl ComponentA11yContract {
    /// Creates a contract for a component or component part.
    pub const fn new(component: &'static str, role: Role) -> Self {
        Self {
            component,
            role,
            label_source: A11yLabelSource::NotRequired,
            description_source: A11yDescriptionSource::None,
            selected: None,
            checked: None,
            expanded: None,
            disabled: None,
            value: None,
            orientation: None,
            actions: &[],
        }
    }

    /// Returns the component or component part name.
    pub const fn component(self) -> &'static str {
        self.component
    }

    /// Returns the semantic role.
    pub const fn role(self) -> Role {
        self.role
    }

    /// Returns the accessible-name source.
    pub const fn label_source(self) -> A11yLabelSource {
        self.label_source
    }

    /// Returns the accessible-description source.
    pub const fn description_source(self) -> A11yDescriptionSource {
        self.description_source
    }

    /// Returns selected state metadata.
    pub const fn selected(self) -> Option<bool> {
        self.selected
    }

    /// Returns checked/toggled state metadata.
    pub const fn checked(self) -> Option<Toggled> {
        self.checked
    }

    /// Returns expanded state metadata.
    pub const fn expanded(self) -> Option<bool> {
        self.expanded
    }

    /// Returns disabled state metadata.
    pub const fn disabled(self) -> Option<bool> {
        self.disabled
    }

    /// Returns value metadata.
    pub const fn value(self) -> Option<A11yValueMetadata> {
        self.value
    }

    /// Returns orientation metadata.
    pub const fn orientation(self) -> Option<Orientation> {
        self.orientation
    }

    /// Returns supported accessibility actions.
    pub const fn actions(self) -> &'static [AccessibleAction] {
        self.actions
    }

    /// Applies the accessible-name source.
    pub const fn with_label_source(mut self, source: A11yLabelSource) -> Self {
        self.label_source = source;
        self
    }

    /// Applies the accessible-description source.
    pub const fn with_description_source(mut self, source: A11yDescriptionSource) -> Self {
        self.description_source = source;
        self
    }

    /// Applies selected state metadata.
    pub const fn selected_state(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Applies checked/toggled state metadata.
    pub const fn checked_state(mut self, checked: Toggled) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Applies expanded state metadata.
    pub const fn expanded_state(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Applies disabled state metadata.
    pub const fn disabled_state(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    /// Applies value metadata.
    pub const fn with_value_metadata(mut self, value: A11yValueMetadata) -> Self {
        self.value = Some(value);
        self
    }

    /// Applies orientation metadata.
    pub const fn with_orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// Applies supported accessibility actions.
    pub const fn with_actions(mut self, actions: &'static [AccessibleAction]) -> Self {
        self.actions = actions;
        self
    }

    /// Validates the contract against the current component-library a11y rules.
    pub const fn validate(self) -> Result<(), A11yContractViolation> {
        if role_requires_name(self.role) && !self.label_source.provides_name() {
            return Err(self.violation(A11yContractError::MissingAccessibleName));
        }
        if role_requires_value(self.role) && self.value.is_none() {
            return Err(self.violation(A11yContractError::MissingValueMetadata));
        }
        if role_requires_action(self.role) && self.actions.is_empty() {
            return Err(self.violation(A11yContractError::MissingSupportedAction));
        }

        Ok(())
    }

    const fn violation(self, error: A11yContractError) -> A11yContractViolation {
        A11yContractViolation {
            component: self.component,
            role: self.role,
            error,
        }
    }
}

const fn role_requires_name(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Link
            | Role::CheckBox
            | Role::Switch
            | Role::RadioButton
            | Role::RadioGroup
            | Role::Toolbar
            | Role::Navigation
            | Role::Tree
            | Role::TreeItem
            | Role::Table
            | Role::ColumnHeader
            | Role::ListBox
            | Role::ListBoxOption
            | Role::Menu
            | Role::MenuItem
            | Role::TextInput
            | Role::MultilineTextInput
            | Role::PasswordInput
            | Role::EditableComboBox
            | Role::Dialog
            | Role::AlertDialog
            | Role::Window
            | Role::ProgressIndicator
            | Role::SpinButton
            | Role::Slider
            | Role::Splitter
            | Role::TabList
            | Role::Tab
            | Role::TabPanel
    )
}

const fn role_requires_value(role: Role) -> bool {
    matches!(
        role,
        Role::ProgressIndicator | Role::SpinButton | Role::Slider
    )
}

const fn role_requires_action(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Link
            | Role::CheckBox
            | Role::Switch
            | Role::RadioButton
            | Role::ListBoxOption
            | Role::MenuItem
            | Role::SpinButton
            | Role::Slider
            | Role::Splitter
            | Role::Tab
    )
}

/// Converts a renderer-neutral role into GPUI's AccessKit role.
pub fn gpui_role_from_ui(role: Role) -> GpuiRole {
    match role {
        Role::Label => GpuiRole::Label,
        Role::TextRun => GpuiRole::TextRun,
        Role::Image => GpuiRole::Image,
        Role::Button => GpuiRole::Button,
        Role::Link => GpuiRole::Link,
        Role::CheckBox => GpuiRole::CheckBox,
        Role::Switch => GpuiRole::Switch,
        Role::RadioButton => GpuiRole::RadioButton,
        Role::RadioGroup => GpuiRole::RadioGroup,
        Role::Toolbar => GpuiRole::Toolbar,
        Role::Navigation => GpuiRole::Navigation,
        Role::Section => GpuiRole::Section,
        Role::Group => GpuiRole::Group,
        Role::Tree => GpuiRole::Tree,
        Role::TreeItem => GpuiRole::TreeItem,
        Role::Table => GpuiRole::Table,
        Role::Row => GpuiRole::Row,
        Role::ColumnHeader => GpuiRole::ColumnHeader,
        Role::Cell => GpuiRole::Cell,
        Role::ListBox => GpuiRole::ListBox,
        Role::ListBoxOption => GpuiRole::ListBoxOption,
        Role::Menu => GpuiRole::Menu,
        Role::MenuItem => GpuiRole::MenuItem,
        Role::TextInput => GpuiRole::TextInput,
        Role::MultilineTextInput => GpuiRole::MultilineTextInput,
        Role::PasswordInput => GpuiRole::PasswordInput,
        Role::EditableComboBox => GpuiRole::EditableComboBox,
        Role::Dialog => GpuiRole::Dialog,
        Role::AlertDialog => GpuiRole::AlertDialog,
        Role::Status => GpuiRole::Status,
        Role::Alert => GpuiRole::Alert,
        Role::Window => GpuiRole::Window,
        Role::ProgressIndicator => GpuiRole::ProgressIndicator,
        Role::Separator => GpuiRole::Group,
        Role::SpinButton => GpuiRole::SpinButton,
        Role::Slider => GpuiRole::Slider,
        Role::Splitter => GpuiRole::Splitter,
        Role::TabList => GpuiRole::TabList,
        Role::Tab => GpuiRole::Tab,
        Role::TabPanel => GpuiRole::TabPanel,
    }
}

/// Converts renderer-neutral live-region politeness into GPUI's AccessKit vocabulary.
pub const fn gpui_live_from_ui(live: LivePoliteness) -> accesskit::Live {
    match live {
        LivePoliteness::Off => accesskit::Live::Off,
        LivePoliteness::Polite => accesskit::Live::Polite,
        LivePoliteness::Assertive => accesskit::Live::Assertive,
    }
}

/// Converts renderer-neutral toggled state into GPUI's AccessKit toggled state.
pub fn gpui_toggled_from_ui(toggled: Toggled) -> GpuiToggled {
    match toggled {
        Toggled::False => GpuiToggled::False,
        Toggled::True => GpuiToggled::True,
        Toggled::Mixed => GpuiToggled::Mixed,
    }
}

/// Converts renderer-neutral orientation into GPUI's AccessKit orientation.
pub fn gpui_orientation_from_ui(orientation: Orientation) -> GpuiOrientation {
    match orientation {
        Orientation::Horizontal => GpuiOrientation::Horizontal,
        Orientation::Vertical => GpuiOrientation::Vertical,
    }
}

/// Converts a renderer-neutral accessibility action into GPUI's AccessKit action.
pub fn gpui_accessible_action_from_ui(action: AccessibleAction) -> GpuiAccessibleAction {
    match action {
        AccessibleAction::Click => GpuiAccessibleAction::Click,
        AccessibleAction::Focus => GpuiAccessibleAction::Focus,
        AccessibleAction::Blur => GpuiAccessibleAction::Blur,
        AccessibleAction::Collapse => GpuiAccessibleAction::Collapse,
        AccessibleAction::Expand => GpuiAccessibleAction::Expand,
        AccessibleAction::CustomAction => GpuiAccessibleAction::CustomAction,
        AccessibleAction::Decrement => GpuiAccessibleAction::Decrement,
        AccessibleAction::Increment => GpuiAccessibleAction::Increment,
        AccessibleAction::HideTooltip => GpuiAccessibleAction::HideTooltip,
        AccessibleAction::ShowTooltip => GpuiAccessibleAction::ShowTooltip,
        AccessibleAction::ReplaceSelectedText => GpuiAccessibleAction::ReplaceSelectedText,
        AccessibleAction::ScrollDown => GpuiAccessibleAction::ScrollDown,
        AccessibleAction::ScrollLeft => GpuiAccessibleAction::ScrollLeft,
        AccessibleAction::ScrollRight => GpuiAccessibleAction::ScrollRight,
        AccessibleAction::ScrollUp => GpuiAccessibleAction::ScrollUp,
        AccessibleAction::ScrollIntoView => GpuiAccessibleAction::ScrollIntoView,
        AccessibleAction::ScrollToPoint => GpuiAccessibleAction::ScrollToPoint,
        AccessibleAction::SetScrollOffset => GpuiAccessibleAction::SetScrollOffset,
        AccessibleAction::SetTextSelection => GpuiAccessibleAction::SetTextSelection,
        AccessibleAction::SetSequentialFocusNavigationStartingPoint => {
            GpuiAccessibleAction::SetSequentialFocusNavigationStartingPoint
        }
        AccessibleAction::SetValue => GpuiAccessibleAction::SetValue,
        AccessibleAction::ShowContextMenu => GpuiAccessibleAction::ShowContextMenu,
    }
}

fn gpui_sort_direction_from_ui(direction: SortDirection) -> accesskit::SortDirection {
    match direction {
        SortDirection::Ascending => accesskit::SortDirection::Ascending,
        SortDirection::Descending => accesskit::SortDirection::Descending,
        SortDirection::Other => accesskit::SortDirection::Other,
    }
}

fn apply_ui_semantics<Element, NodeId>(
    mut element: Element,
    descriptor: &SemanticDescriptor<'_, NodeId>,
) -> Element
where
    Element: StatefulInteractiveElement,
{
    element = StatefulInteractiveElement::role(element, gpui_role_from_ui(descriptor.role()));
    if let Some(label) = descriptor.label() {
        element = StatefulInteractiveElement::aria_label(element, label);
    }
    if let Some(description) = descriptor.description() {
        element = StatefulInteractiveElement::aria_description(element, description);
    }
    if let Some(value) = descriptor.value() {
        element = StatefulInteractiveElement::aria_value(element, value);
    }
    if let Some(placeholder) = descriptor.placeholder() {
        element = StatefulInteractiveElement::aria_placeholder(element, placeholder);
    }
    if descriptor.role() == Role::TextRun {
        element = StatefulInteractiveElement::aria_character_lengths(
            element,
            descriptor.character_lengths().iter().copied(),
        );
    }
    if let Some(selected) = descriptor.selected() {
        element = StatefulInteractiveElement::aria_selected(element, selected);
    }
    if let Some(required) = descriptor.required() {
        element = StatefulInteractiveElement::aria_required(element, required);
    }
    if let Some(invalid) = descriptor.invalid() {
        element = StatefulInteractiveElement::aria_invalid(element, invalid);
    }
    if let Some(busy) = descriptor.busy() {
        element = StatefulInteractiveElement::aria_busy(element, busy);
    }
    if let Some(live) = descriptor.live() {
        element = StatefulInteractiveElement::aria_live(element, gpui_live_from_ui(live));
    }
    if let Some(atomic) = descriptor.live_atomic() {
        element = StatefulInteractiveElement::aria_live_atomic(element, atomic);
    }
    if let Some(read_only) = descriptor.read_only() {
        element = StatefulInteractiveElement::aria_read_only(element, read_only);
    }
    if let Some(omitted) = descriptor.omit_accessibility_node() {
        element = StatefulInteractiveElement::omit_accessibility_node(element, omitted);
    }
    if let Some(modal) = descriptor.modal() {
        element = StatefulInteractiveElement::aria_modal(element, modal);
    }
    if let Some(disabled) = descriptor.disabled() {
        element = StatefulInteractiveElement::aria_disabled(element, disabled);
    }
    if let Some(expanded) = descriptor.expanded() {
        element = StatefulInteractiveElement::aria_expanded(element, expanded);
    }
    if let Some(toggled) = descriptor.toggled() {
        element = StatefulInteractiveElement::aria_toggled(element, gpui_toggled_from_ui(toggled));
    }
    if let Some(value) = descriptor.numeric_value() {
        element = StatefulInteractiveElement::aria_numeric_value(element, value);
    }
    if let Some(value) = descriptor.min_numeric_value() {
        element = StatefulInteractiveElement::aria_min_numeric_value(element, value);
    }
    if let Some(value) = descriptor.max_numeric_value() {
        element = StatefulInteractiveElement::aria_max_numeric_value(element, value);
    }
    if let Some(orientation) = descriptor.orientation() {
        element = StatefulInteractiveElement::aria_orientation(
            element,
            gpui_orientation_from_ui(orientation),
        );
    }
    if let Some(level) = descriptor.level() {
        element = StatefulInteractiveElement::aria_level(element, level);
    }
    if let Some(position) = descriptor.position_in_set() {
        element = StatefulInteractiveElement::aria_position_in_set(element, position);
    }
    if let Some(size) = descriptor.size_of_set() {
        element = StatefulInteractiveElement::aria_size_of_set(element, size);
    }
    if let Some(index) = descriptor.row_index() {
        element = StatefulInteractiveElement::aria_row_index(element, index);
    }
    if let Some(index) = descriptor.column_index() {
        element = StatefulInteractiveElement::aria_column_index(element, index);
    }
    if let Some(span) = descriptor.row_span() {
        element = StatefulInteractiveElement::aria_row_span(element, span);
    }
    if let Some(span) = descriptor.column_span() {
        element = StatefulInteractiveElement::aria_column_span(element, span);
    }
    if let Some(count) = descriptor.row_count() {
        element = StatefulInteractiveElement::aria_row_count(element, count);
    }
    if let Some(count) = descriptor.column_count() {
        element = StatefulInteractiveElement::aria_column_count(element, count);
    }
    if let Some(direction) = descriptor.sort_direction() {
        element = StatefulInteractiveElement::aria_sort_direction(
            element,
            gpui_sort_direction_from_ui(direction),
        );
    }

    StatefulInteractiveElement::aria_actions(
        element,
        descriptor
            .available_actions()
            .map(gpui_accessible_action_from_ui),
    )
}

/// Applies renderer-neutral accessibility vocabulary to GPUI elements.
pub trait UiA11yElementExt: StatefulInteractiveElement + Sized {
    /// Projects a relation-free semantic descriptor onto this GPUI element.
    fn ui_semantics(self, descriptor: &SemanticDescriptor<'_>) -> Self {
        apply_ui_semantics(self, descriptor)
    }

    /// Projects a relation-bearing descriptor through a renderer-scoped node resolver.
    fn ui_semantics_with_relations<NodeId>(
        self,
        descriptor: &SemanticDescriptor<'_, NodeId>,
        mut resolve_node_id: impl FnMut(&NodeId) -> accesskit::NodeId,
    ) -> Self {
        let controls = descriptor
            .controls()
            .iter()
            .map(&mut resolve_node_id)
            .collect::<Vec<_>>();
        let labelled_by = descriptor
            .labelled_by()
            .iter()
            .map(&mut resolve_node_id)
            .collect::<Vec<_>>();
        let described_by = descriptor
            .described_by()
            .iter()
            .map(&mut resolve_node_id)
            .collect::<Vec<_>>();
        let error_message = descriptor.error_message().map(&mut resolve_node_id);
        let text_selection =
            descriptor
                .text_selection()
                .map(|selection| accesskit::TextSelection {
                    anchor: accesskit::TextPosition {
                        node: resolve_node_id(selection.anchor().node()),
                        character_index: selection.anchor().character_index(),
                    },
                    focus: accesskit::TextPosition {
                        node: resolve_node_id(selection.focus().node()),
                        character_index: selection.focus().character_index(),
                    },
                });

        let mut element = apply_ui_semantics(self, descriptor);
        if !controls.is_empty() {
            element = StatefulInteractiveElement::aria_controls(element, controls);
        }
        if !labelled_by.is_empty() {
            element = StatefulInteractiveElement::aria_labelled_by(element, labelled_by);
        }
        if !described_by.is_empty() {
            element = StatefulInteractiveElement::aria_described_by(element, described_by);
        }
        if let Some(error_message) = error_message {
            element = StatefulInteractiveElement::aria_error_message(element, error_message);
        }
        if let Some(text_selection) = text_selection {
            element = StatefulInteractiveElement::aria_text_selection(element, text_selection);
        }
        element
    }

    /// Sets a renderer-neutral accessible role.
    fn ui_role(self, role: Role) -> Self {
        StatefulInteractiveElement::role(self, gpui_role_from_ui(role))
    }

    /// Sets renderer-neutral toggled state.
    fn ui_aria_toggled(self, toggled: Toggled) -> Self {
        StatefulInteractiveElement::aria_toggled(self, gpui_toggled_from_ui(toggled))
    }

    /// Sets renderer-neutral selected state.
    fn ui_aria_selected(self, selected: bool) -> Self {
        StatefulInteractiveElement::aria_selected(self, selected)
    }

    /// Sets renderer-neutral disabled state.
    fn ui_aria_disabled(self, disabled: bool) -> Self {
        StatefulInteractiveElement::aria_disabled(self, disabled)
    }

    /// Sets renderer-neutral orientation.
    fn ui_aria_orientation(self, orientation: Orientation) -> Self {
        StatefulInteractiveElement::aria_orientation(self, gpui_orientation_from_ui(orientation))
    }

    /// Registers a renderer-neutral accessibility action listener.
    fn on_ui_a11y_action(
        self,
        action: AccessibleAction,
        listener: impl FnMut(
            Option<&open_gpui::accesskit::ActionData>,
            &mut open_gpui::Window,
            &mut open_gpui::App,
        ) + 'static,
    ) -> Self {
        StatefulInteractiveElement::on_a11y_action(
            self,
            gpui_accessible_action_from_ui(action),
            listener,
        )
    }
}

impl<T> UiA11yElementExt for T where T: StatefulInteractiveElement + Sized {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_run_ranges_require_contiguous_unique_accesskit_nodes() {
        let value = "a\nb";
        let text_runs = [
            AccessibleTextRunRange::from_text(accesskit::NodeId(1), 0..2, value).unwrap(),
            AccessibleTextRunRange::from_text(accesskit::NodeId(2), 2..3, value).unwrap(),
        ];

        assert!(text_runs_cover_value(value, &text_runs));
        assert!(!text_runs_cover_value(
            value,
            &[
                AccessibleTextRunRange::from_text(accesskit::NodeId(1), 0..2, value).unwrap(),
                AccessibleTextRunRange::from_text(accesskit::NodeId(1), 2..3, value).unwrap(),
            ]
        ));
        assert!(!text_runs_cover_value(
            value,
            &[
                AccessibleTextRunRange::from_text(accesskit::NodeId(1), 0..1, value).unwrap(),
                AccessibleTextRunRange::from_text(accesskit::NodeId(2), 2..3, value).unwrap(),
            ]
        ));

        let oversized = format!("a{}", "\u{301}".repeat(128));
        assert!(
            AccessibleTextRunRange::from_text(
                accesskit::NodeId(1),
                0..oversized.len(),
                &oversized,
            )
            .is_none()
        );
    }

    #[test]
    fn text_run_ranges_map_offsets_from_precomputed_character_lengths() {
        let text_run = AccessibleTextRunRange::from_character_lengths(
            accesskit::NodeId(1),
            0..3,
            std::rc::Rc::from([3_u8]),
        )
        .expect("one three-byte grapheme should form valid text-run metadata");

        assert_eq!(text_run.character_index_from_offset(0), Some(0));
        assert_eq!(text_run.character_index_from_offset(3), Some(1));
        assert_eq!(text_run.offset_from_character_index(0), Some(0));
        assert_eq!(text_run.offset_from_character_index(1), Some(3));
        assert_eq!(text_run.offset_from_character_index(2), None);
    }

    #[test]
    fn gpui_adapter_maps_splitter_role() {
        assert_eq!(gpui_role_from_ui(Role::Splitter), GpuiRole::Splitter);
    }

    #[test]
    fn gpui_adapter_maps_text_run_role() {
        assert_eq!(gpui_role_from_ui(Role::TextRun), GpuiRole::TextRun);
    }

    #[test]
    fn gpui_adapter_downgrades_separator_role_to_group() {
        assert_eq!(gpui_role_from_ui(Role::Separator), GpuiRole::Group);
    }

    #[test]
    fn gpui_adapter_maps_tab_roles() {
        assert_eq!(gpui_role_from_ui(Role::TabList), GpuiRole::TabList);
        assert_eq!(gpui_role_from_ui(Role::Tab), GpuiRole::Tab);
        assert_eq!(gpui_role_from_ui(Role::TabPanel), GpuiRole::TabPanel);
    }

    #[test]
    fn gpui_adapter_maps_live_region_vocabulary_exactly() {
        assert_eq!(gpui_role_from_ui(Role::Status), GpuiRole::Status);
        assert_eq!(gpui_role_from_ui(Role::Alert), GpuiRole::Alert);
        assert_eq!(gpui_live_from_ui(LivePoliteness::Off), accesskit::Live::Off);
        assert_eq!(
            gpui_live_from_ui(LivePoliteness::Polite),
            accesskit::Live::Polite
        );
        assert_eq!(
            gpui_live_from_ui(LivePoliteness::Assertive),
            accesskit::Live::Assertive
        );
    }

    #[test]
    fn gpui_adapter_maps_accessibility_actions_used_by_docking() {
        assert_eq!(
            gpui_accessible_action_from_ui(AccessibleAction::Click),
            GpuiAccessibleAction::Click
        );
        assert_eq!(
            gpui_accessible_action_from_ui(AccessibleAction::Focus),
            GpuiAccessibleAction::Focus
        );
        assert_eq!(
            gpui_accessible_action_from_ui(AccessibleAction::Increment),
            GpuiAccessibleAction::Increment
        );
        assert_eq!(
            gpui_accessible_action_from_ui(AccessibleAction::Decrement),
            GpuiAccessibleAction::Decrement
        );
    }

    #[test]
    fn gpui_adapter_maps_horizontal_orientation() {
        assert_eq!(
            gpui_orientation_from_ui(Orientation::Horizontal),
            GpuiOrientation::Horizontal
        );
    }
}
