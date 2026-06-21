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
