use crate::overlay::OverlayDisclosureOpenMode;
/// Menu open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

impl MenuOpenMode {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uncontrolled => "uncontrolled",
            Self::Controlled => "controlled",
        }
    }
}

pub(crate) const fn menu_open_mode_from_disclosure(
    mode: OverlayDisclosureOpenMode,
) -> MenuOpenMode {
    match mode {
        OverlayDisclosureOpenMode::Uncontrolled => MenuOpenMode::Uncontrolled,
        OverlayDisclosureOpenMode::Controlled => MenuOpenMode::Controlled,
    }
}

/// Menu item kind for the base menu model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItemKind {
    /// Activatable command item.
    Action,
    /// Checkable menu item. Checked state is caller-owned.
    Checkbox,
    /// Radio-style menu item. Checked state is caller-owned.
    Radio,
    /// Visual separator. Separators are not focusable or activatable.
    Separator,
    /// Submenu trigger item.
    Submenu,
}

impl MenuItemKind {
    /// Returns a stable item-kind label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio",
            Self::Separator => "separator",
            Self::Submenu => "submenu",
        }
    }

    /// Returns whether this kind can be activated when enabled.
    pub const fn activatable(self) -> bool {
        matches!(self, Self::Action | Self::Checkbox | Self::Radio)
    }

    /// Returns whether this kind can receive roving focus when enabled.
    pub const fn focusable(self) -> bool {
        matches!(
            self,
            Self::Action | Self::Checkbox | Self::Radio | Self::Submenu
        )
    }
}

/// Pure descriptor for one menu item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItemDescriptor {
    value: String,
    label: String,
    kind: MenuItemKind,
    disabled: bool,
    checked: bool,
    children: Vec<MenuItemDescriptor>,
}

impl MenuItemDescriptor {
    /// Creates an action item descriptor.
    pub fn action(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            kind: MenuItemKind::Action,
            disabled: false,
            checked: false,
            children: Vec::new(),
        }
    }

    /// Creates a checkbox item descriptor.
    pub fn checkbox(value: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            kind: MenuItemKind::Checkbox,
            disabled: false,
            checked,
            children: Vec::new(),
        }
    }

    /// Creates a radio item descriptor.
    pub fn radio(value: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            kind: MenuItemKind::Radio,
            disabled: false,
            checked,
            children: Vec::new(),
        }
    }

    /// Creates a separator descriptor.
    pub fn separator(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: String::new(),
            kind: MenuItemKind::Separator,
            disabled: true,
            checked: false,
            children: Vec::new(),
        }
    }

    /// Creates a submenu descriptor.
    pub fn submenu(
        value: impl Into<String>,
        label: impl Into<String>,
        children: impl IntoIterator<Item = MenuItemDescriptor>,
    ) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            kind: MenuItemKind::Submenu,
            disabled: false,
            checked: false,
            children: children.into_iter().collect(),
        }
    }

    /// Marks an activatable or submenu item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        if self.kind != MenuItemKind::Separator {
            self.disabled = disabled;
        }
        self
    }

    /// Applies caller-owned checked state to checkbox and radio items.
    pub fn checked(mut self, checked: bool) -> Self {
        if matches!(self.kind, MenuItemKind::Checkbox | MenuItemKind::Radio) {
            self.checked = checked;
        }
        self
    }

    /// Adds one submenu child descriptor.
    pub fn child(mut self, child: MenuItemDescriptor) -> Self {
        if self.kind == MenuItemKind::Submenu {
            self.children.push(child);
        }
        self
    }

    /// Adds many submenu child descriptors.
    pub fn children(mut self, children: impl IntoIterator<Item = MenuItemDescriptor>) -> Self {
        if self.kind == MenuItemKind::Submenu {
            self.children.extend(children);
        }
        self
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the menu item kind.
    pub const fn kind(&self) -> MenuItemKind {
        self.kind
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns caller-owned checked state for checkbox and radio items.
    pub const fn checked_state(&self) -> bool {
        self.checked
    }

    /// Returns submenu child descriptors.
    pub fn children_ref(&self) -> &[MenuItemDescriptor] {
        &self.children
    }

    /// Returns whether the item participates in roving focus.
    pub const fn focusable(&self) -> bool {
        self.kind.focusable()
            && !self.disabled
            && (!matches!(self.kind, MenuItemKind::Submenu) || !self.children.is_empty())
    }
}
