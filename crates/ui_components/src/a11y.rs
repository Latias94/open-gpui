//! GPUI accessibility adapter utilities.

use open_gpui::{
    AccessibleAction as GpuiAccessibleAction, Orientation as GpuiOrientation, Role as GpuiRole,
    StatefulInteractiveElement, Toggled as GpuiToggled,
};
use open_gpui_ui_core::{AccessibleAction, Orientation, Role, Toggled};

/// Converts a renderer-neutral role into GPUI's AccessKit role.
pub fn gpui_role_from_ui(role: Role) -> GpuiRole {
    match role {
        Role::Label => GpuiRole::Label,
        Role::Button => GpuiRole::Button,
        Role::CheckBox => GpuiRole::CheckBox,
        Role::Switch => GpuiRole::Switch,
        Role::RadioButton => GpuiRole::RadioButton,
        Role::RadioGroup => GpuiRole::RadioGroup,
        Role::Toolbar => GpuiRole::Toolbar,
        Role::Navigation => GpuiRole::Navigation,
        Role::Section => GpuiRole::Section,
        Role::Group => GpuiRole::Group,
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
        Role::SpinButton => GpuiRole::SpinButton,
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
