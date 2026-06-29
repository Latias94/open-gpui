use open_gpui::Modifiers;

/// Renderer-neutral modifier-key snapshot carried by table row callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableInputModifiers {
    control: bool,
    alt: bool,
    shift: bool,
    platform: bool,
    function: bool,
}

impl TableInputModifiers {
    pub(in crate::table) fn from_gpui(modifiers: Modifiers) -> Self {
        Self {
            control: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
            platform: modifiers.platform,
            function: modifiers.function,
        }
    }

    /// Returns whether the control key was pressed.
    pub const fn control(self) -> bool {
        self.control
    }

    /// Returns whether the alt key was pressed.
    pub const fn alt(self) -> bool {
        self.alt
    }

    /// Returns whether the shift key was pressed.
    pub const fn shift(self) -> bool {
        self.shift
    }

    /// Returns whether the platform command key was pressed.
    pub const fn platform(self) -> bool {
        self.platform
    }

    /// Returns whether the function key was pressed.
    pub const fn function(self) -> bool {
        self.function
    }

    /// Returns whether any modifier key was pressed.
    pub const fn modified(self) -> bool {
        self.control || self.alt || self.shift || self.platform || self.function
    }
}
