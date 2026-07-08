//! Shared field-control state contracts.

use open_gpui_ui_core::Size;

/// Renderer-neutral state shared by field-like controls.
///
/// This type is the common contract for form controls that need to answer whether editing,
/// activation, focus traversal, validation, and required metadata are enabled. Concrete controls
/// still own their value-specific state, metrics, colors, and GPUI adapter behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormControlState {
    size: Size,
    disabled: bool,
    read_only: bool,
    invalid: bool,
    required: bool,
    controller_driven: bool,
}

impl Default for FormControlState {
    fn default() -> Self {
        Self::new(Size::Medium)
    }
}

impl FormControlState {
    /// Creates enabled form-control state for a foundation size.
    pub const fn new(size: Size) -> Self {
        Self {
            size,
            disabled: false,
            read_only: false,
            invalid: false,
            required: false,
            controller_driven: false,
        }
    }

    /// Resolves the full form-control state.
    pub const fn resolve(
        size: Size,
        disabled: bool,
        read_only: bool,
        invalid: bool,
        required: bool,
        controller_driven: bool,
    ) -> Self {
        Self {
            size,
            disabled,
            read_only,
            invalid,
            required,
            controller_driven,
        }
    }

    /// Returns a copy with a different foundation size.
    pub const fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Returns a copy with disabled state updated.
    pub const fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns a copy with read-only state updated.
    pub const fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Returns a copy with validation state updated.
    pub const fn with_invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Returns a copy with required metadata updated.
    pub const fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Returns a copy with adapter-controller ownership updated.
    pub const fn with_controller_driven(mut self, controller_driven: bool) -> Self {
        self.controller_driven = controller_driven;
        self
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns whether the control is disabled.
    pub const fn disabled(self) -> bool {
        self.disabled
    }

    /// Returns whether the control is read-only.
    pub const fn read_only(self) -> bool {
        self.read_only
    }

    /// Returns whether the control is invalid.
    pub const fn invalid(self) -> bool {
        self.invalid
    }

    /// Returns whether the control is required.
    pub const fn required(self) -> bool {
        self.required
    }

    /// Returns whether this state is backed by an editable adapter controller.
    pub const fn controller_driven(self) -> bool {
        self.controller_driven
    }

    /// Returns whether value editing should be accepted.
    pub const fn input_enabled(self) -> bool {
        !self.disabled && !self.read_only
    }

    /// Returns whether value editing should be accepted.
    pub const fn editable(self) -> bool {
        self.input_enabled()
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(self) -> bool {
        self.input_enabled()
    }

    /// Returns whether the control should participate in tab traversal.
    pub const fn tab_stop_enabled(self) -> bool {
        !self.disabled
    }
}
