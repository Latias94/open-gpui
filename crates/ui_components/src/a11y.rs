//! GPUI accessibility adapter utilities.

use open_gpui::{
    AccessibleAction as GpuiAccessibleAction, Orientation as GpuiOrientation, Role as GpuiRole,
    StatefulInteractiveElement, Toggled as GpuiToggled,
};
use open_gpui_ui_core::{AccessibleAction, Orientation, Role, Toggled};

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
        Role::EditableComboBox => GpuiRole::EditableComboBox,
        Role::Dialog => GpuiRole::Dialog,
        Role::AlertDialog => GpuiRole::AlertDialog,
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

/// Applies renderer-neutral accessibility vocabulary to GPUI elements.
pub trait UiA11yElementExt: StatefulInteractiveElement + Sized {
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
    fn gpui_adapter_maps_splitter_role() {
        assert_eq!(gpui_role_from_ui(Role::Splitter), GpuiRole::Splitter);
    }

    #[test]
    fn gpui_adapter_maps_tab_roles() {
        assert_eq!(gpui_role_from_ui(Role::TabList), GpuiRole::TabList);
        assert_eq!(gpui_role_from_ui(Role::Tab), GpuiRole::Tab);
        assert_eq!(gpui_role_from_ui(Role::TabPanel), GpuiRole::TabPanel);
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
