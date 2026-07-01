use super::*;

/// One label sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved label state.
    pub state: LabelState,
}

/// One text input sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TextInputSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample label.
    pub label: &'static str,
    /// Resolved state.
    pub state: TextInputState,
}

/// One textarea sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TextareaSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample label.
    pub label: &'static str,
    /// Resolved state.
    pub state: TextareaState,
}

/// One field sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved field state.
    pub state: FieldState,
    /// Resolved control state.
    pub input_state: TextInputState,
}

/// One field sample that composes a textarea control.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldTextareaSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved field state.
    pub state: FieldState,
    /// Resolved textarea control state.
    pub textarea_state: TextareaState,
}

/// Returns label samples backed by real component state.
pub fn label_samples(tokens: ThemeTokens) -> [LabelSample; 4] {
    [
        (
            "email",
            "Email",
            Some("email-input"),
            false,
            false,
            Size::Medium,
        ),
        (
            "terms",
            "Terms",
            Some("terms-checkbox"),
            true,
            false,
            Size::Medium,
        ),
        (
            "disabled",
            "Disabled",
            Some("disabled-control"),
            false,
            true,
            Size::Medium,
        ),
        ("standalone", "Standalone", None, false, false, Size::Small),
    ]
    .map(|(id, text, control_id, required, disabled, size)| {
        let label = Label::new(id, text)
            .required(required)
            .disabled(disabled)
            .with_size(size)
            .tokens(tokens);
        let label = if let Some(control_id) = control_id {
            label.for_control(control_id)
        } else {
            label
        };

        LabelSample {
            id,
            state: label.state(),
        }
    })
}

/// Returns text input samples backed by real component state.
pub fn text_input_samples(tokens: ThemeTokens) -> [TextInputSample; 6] {
    [
        (
            "default",
            "Default",
            "",
            "Email address",
            false,
            false,
            false,
            false,
            Size::Medium,
            true,
            TextInputDisplayMode::Plain,
        ),
        (
            "filled",
            "Filled",
            "hello@example.com",
            "Email address",
            false,
            false,
            false,
            false,
            Size::Medium,
            false,
            TextInputDisplayMode::Plain,
        ),
        (
            "invalid",
            "Invalid",
            "not-an-email",
            "Email address",
            false,
            false,
            true,
            true,
            Size::Medium,
            false,
            TextInputDisplayMode::Plain,
        ),
        (
            "read-only",
            "Read only",
            "profile@example.com",
            "Email address",
            false,
            true,
            false,
            false,
            Size::Medium,
            false,
            TextInputDisplayMode::Plain,
        ),
        (
            "disabled",
            "Disabled",
            "",
            "Unavailable",
            true,
            false,
            false,
            false,
            Size::Medium,
            false,
            TextInputDisplayMode::Plain,
        ),
        (
            "password",
            "Password",
            "a🙂中",
            "Password",
            false,
            false,
            false,
            false,
            Size::Medium,
            false,
            TextInputDisplayMode::Password,
        ),
    ]
    .map(
        |(
            id,
            label,
            value,
            placeholder,
            disabled,
            read_only,
            required,
            invalid,
            size,
            controller_driven,
            display_mode,
        )| {
            let state = TextInputState::resolve_with_display_mode(
                value,
                Some(placeholder),
                size,
                disabled,
                read_only,
                invalid,
                required,
                controller_driven,
                display_mode,
                tokens,
            );

            TextInputSample { id, label, state }
        },
    )
}

/// Returns textarea samples backed by real component state.
pub fn textarea_samples(tokens: ThemeTokens) -> [TextareaSample; 4] {
    [
        (
            "default",
            "Default",
            "",
            "Write release notes...",
            3,
            false,
            false,
            false,
            false,
            Size::Medium,
            false,
        ),
        (
            "filled",
            "Filled",
            "Line 1\nLine 2",
            "Write release notes...",
            4,
            false,
            false,
            false,
            false,
            Size::Medium,
            false,
        ),
        (
            "overflow",
            "Overflow",
            "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8",
            "Write release notes...",
            3,
            false,
            false,
            false,
            false,
            Size::Medium,
            false,
        ),
        (
            "invalid",
            "Invalid",
            "Needs a rollback note.",
            "Write release notes...",
            3,
            false,
            false,
            true,
            true,
            Size::Medium,
            false,
        ),
    ]
    .map(
        |(
            id,
            label,
            value,
            placeholder,
            rows,
            disabled,
            read_only,
            required,
            invalid,
            size,
            controller_driven,
        )| {
            let state = TextareaState::resolve(
                value,
                Some(placeholder),
                size,
                rows,
                disabled,
                read_only,
                invalid,
                required,
                controller_driven,
                tokens,
            );

            TextareaSample { id, label, state }
        },
    )
}

/// Returns field samples backed by real component state.
pub fn field_samples(tokens: ThemeTokens) -> [FieldSample; 3] {
    [
        (
            "email",
            "Email",
            "Use a work address.",
            None,
            "",
            "you@example.com",
            true,
            false,
            false,
        ),
        (
            "invalid",
            "Email",
            "Use a work address.",
            Some("Enter a valid email."),
            "not-an-email",
            "you@example.com",
            true,
            false,
            true,
        ),
        (
            "disabled",
            "Workspace",
            "Managed by your organization.",
            None,
            "",
            "Workspace name",
            false,
            true,
            false,
        ),
    ]
    .map(
        |(id, label, help, error, value, placeholder, required, disabled, invalid)| {
            let input = TextInput::new(format!("{id}-input"), label)
                .value(value)
                .placeholder(placeholder)
                .required(required)
                .disabled(disabled)
                .invalid(invalid)
                .tokens(tokens);
            let field = Field::new(id, format!("{id}-input"), label)
                .help(help)
                .required(required)
                .disabled(disabled)
                .invalid(invalid)
                .tokens(tokens);
            let field = if let Some(error) = error {
                field.error(error)
            } else {
                field
            };

            FieldSample {
                id,
                state: field.state(),
                input_state: input.state(),
            }
        },
    )
}

/// Returns field samples that compose a textarea control.
pub fn field_textarea_samples(tokens: ThemeTokens) -> [FieldTextareaSample; 1] {
    [(
        "release-notes",
        "Release notes",
        "Summarize user-visible changes.",
        Some("Add a concise release note."),
        "",
        "Write release notes...",
        4,
        true,
        false,
        true,
    )]
    .map(
        |(id, label, help, error, value, placeholder, rows, required, disabled, invalid)| {
            let textarea = Textarea::new(format!("{id}-textarea"), label)
                .value(value)
                .placeholder(placeholder)
                .rows(rows)
                .required(required)
                .disabled(disabled)
                .invalid(invalid)
                .tokens(tokens);
            let field = Field::new(id, format!("{id}-textarea"), label)
                .help(help)
                .required(required)
                .disabled(disabled)
                .invalid(invalid)
                .tokens(tokens);
            let field = if let Some(error) = error {
                field.error(error)
            } else {
                field
            };

            FieldTextareaSample {
                id,
                state: field.state(),
                textarea_state: textarea.state(),
            }
        },
    )
}
