//! GPUI-facing field-state primitive metadata.

/// Shared state that field-like primitives can inherit and query.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FieldState {
    invalid: bool,
    disabled: bool,
}

impl FieldState {
    /// Creates a field-state value from invalid and disabled flags.
    pub const fn new(invalid: bool, disabled: bool) -> Self {
        Self { invalid, disabled }
    }

    /// Returns whether the field is invalid.
    pub const fn invalid(self) -> bool {
        self.invalid
    }

    /// Returns whether the field is disabled.
    pub const fn disabled(self) -> bool {
        self.disabled
    }
}
