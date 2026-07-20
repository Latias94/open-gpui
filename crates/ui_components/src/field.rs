//! Field component.

use crate::geometry::gpui_px_from_ui;
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div,
};
use open_gpui_ui_core::{Role, SemanticDescriptor, Sizable, Size, ThemeTokens, UiPx, ui_px};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::form_control::FormControlState;
use crate::theme::ThemeResolver;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct FormControlOverrides {
    required: Option<bool>,
    disabled: Option<bool>,
    invalid: Option<bool>,
    busy: Option<bool>,
}

impl FormControlOverrides {
    const fn with_required(mut self, required: bool) -> Self {
        self.required = Some(required);
        self
    }

    const fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    const fn with_invalid(mut self, invalid: bool) -> Self {
        self.invalid = Some(invalid);
        self
    }

    const fn with_busy(mut self, busy: bool) -> Self {
        self.busy = Some(busy);
        self
    }

    const fn apply_to(self, mut state: FormControlState) -> FormControlState {
        if let Some(required) = self.required {
            state = state.with_required(required);
        }
        if let Some(disabled) = self.disabled {
            state = state.with_disabled(disabled);
        }
        if let Some(invalid) = self.invalid {
            state = state.with_invalid(invalid);
        }
        if let Some(busy) = self.busy {
            state = state.with_busy(busy);
        }
        state
    }

    const fn inherit_unset_from(
        self,
        mut state: FormControlState,
        baseline: FormControlState,
    ) -> FormControlState {
        if self.required.is_none() {
            state = state.with_required(baseline.required());
        }
        if self.disabled.is_none() {
            state = state.with_disabled(baseline.disabled());
        }
        if self.invalid.is_none() {
            state = state.with_invalid(baseline.invalid());
        }
        if self.busy.is_none() {
            state = state.with_busy(baseline.busy());
        }
        state
    }
}

pub(crate) mod adapter {
    use super::FormControlOverrides;
    use crate::form_control::FormControlState;
    use open_gpui::{IntoElement, accesskit::NodeId};
    use open_gpui_ui_core::SemanticDescriptor;

    /// Renderer-scoped relations that a field projects onto its control child.
    ///
    /// The projection is created during rendering from the field's actual GPUI element path. It is
    /// adapter metadata, not independently stored component state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FieldControlSemantics {
        labelled_by: open_gpui::accesskit::NodeId,
        described_by: Option<open_gpui::accesskit::NodeId>,
        error_message: Option<open_gpui::accesskit::NodeId>,
        overrides: FormControlOverrides,
    }

    impl FieldControlSemantics {
        pub(super) const fn new(
            labelled_by: open_gpui::accesskit::NodeId,
            described_by: Option<open_gpui::accesskit::NodeId>,
            error_message: Option<open_gpui::accesskit::NodeId>,
            overrides: FormControlOverrides,
        ) -> Self {
            Self {
                labelled_by,
                described_by,
                error_message,
                overrides,
            }
        }

        /// Applies the field's explicit state overrides to a control baseline.
        ///
        /// Custom controls should call this while resolving their semantic state so the field
        /// remains authoritative for explicitly configured required, disabled, invalid, and busy
        /// values.
        pub const fn apply_control_state(&self, state: FormControlState) -> FormControlState {
            self.overrides.apply_to(state)
        }

        /// Projects field-owned relations or the fallback standalone label onto a text control.
        pub(crate) fn project_text_control_descriptor<'a>(
            field_semantics: Option<&'a Self>,
            standalone_label: &'a str,
            mut descriptor: SemanticDescriptor<'a, NodeId>,
        ) -> SemanticDescriptor<'a, NodeId> {
            let Some(field_semantics) = field_semantics else {
                return descriptor.with_label(standalone_label);
            };

            descriptor = descriptor
                .with_labelled_by(std::slice::from_ref(&field_semantics.labelled_by))
                .with_described_by(field_semantics.described_by.as_slice());
            if let Some(error_message) = field_semantics.error_message.as_ref() {
                descriptor = descriptor.with_error_message(error_message);
            }
            descriptor
        }

        /// Returns the node that labels the control.
        pub const fn labelled_by(&self) -> open_gpui::accesskit::NodeId {
            self.labelled_by
        }

        /// Returns the node that describes the control, when present.
        pub const fn described_by(&self) -> Option<open_gpui::accesskit::NodeId> {
            self.described_by
        }

        /// Returns the node that contains the validation error, when present.
        pub const fn error_message(&self) -> Option<open_gpui::accesskit::NodeId> {
            self.error_message
        }

        /// Returns the field's explicit required override, when configured.
        pub const fn required(&self) -> Option<bool> {
            self.overrides.required
        }

        /// Returns the field's explicit disabled override, when configured.
        pub const fn disabled(&self) -> Option<bool> {
            self.overrides.disabled
        }

        /// Returns the field's explicit invalid override, when configured.
        pub const fn invalid(&self) -> Option<bool> {
            self.overrides.invalid
        }

        /// Returns the field's explicit busy override, when configured.
        pub const fn busy(&self) -> Option<bool> {
            self.overrides.busy
        }
    }

    /// A control that can receive semantic relations from a composing field.
    pub trait FieldControl: IntoElement + Sized + 'static {
        /// Returns the control state used as the field's shared-state baseline.
        fn field_control_state(&self) -> FormControlState;

        /// Returns this control with the field's renderer-scoped relations applied.
        fn with_field_semantics(self, semantics: FieldControlSemantics) -> Self;
    }
}

use adapter::{FieldControl, FieldControlSemantics};

type FieldControlFactory = Box<dyn FnOnce(FieldControlSemantics) -> AnyElement>;

/// The resolved field message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldMessage {
    /// Help text shown below a valid field.
    Help(String),
    /// Error text shown below an invalid field.
    Error(String),
}

impl FieldMessage {
    /// Returns the message text.
    pub fn text(&self) -> &str {
        match self {
            Self::Help(text) | Self::Error(text) => text,
        }
    }

    /// Returns whether this is an error message.
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// Resolved field color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldColors {
    pub(crate) label: ColorIntent,
    pub(crate) message: ColorIntent,
    pub(crate) required_marker: ColorIntent,
}

impl FieldColors {
    /// Returns the label color intent.
    pub const fn label(self) -> ColorIntent {
        self.label
    }

    /// Returns the message color intent.
    pub const fn message(self) -> ColorIntent {
        self.message
    }

    /// Returns the required marker color intent.
    pub const fn required_marker(self) -> ColorIntent {
        self.required_marker
    }
}

/// Resolved field metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldMetrics {
    label_text_size: UiPx,
    message_text_size: UiPx,
    gap: UiPx,
}

impl FieldMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            label_text_size: size.control_text_px(),
            message_text_size: ui_px(12.0),
            gap: ui_px(6.0),
        }
    }

    /// Returns the label text size.
    pub const fn label_text_size(self) -> UiPx {
        self.label_text_size
    }

    /// Returns the message text size.
    pub const fn message_text_size(self) -> UiPx {
        self.message_text_size
    }

    /// Returns the vertical gap.
    pub const fn gap(self) -> UiPx {
        self.gap
    }
}

/// Resolved field state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldState {
    label: String,
    help_text: Option<String>,
    error_text: Option<String>,
    control: FormControlState,
    metrics: FieldMetrics,
    colors: FieldColors,
}

impl FieldState {
    /// Resolves the public state for a field.
    pub fn resolve(
        label: impl Into<String>,
        help_text: Option<impl Into<String>>,
        error_text: Option<impl Into<String>>,
        size: Size,
        required: bool,
        disabled: bool,
        invalid: bool,
        tokens: ThemeTokens,
    ) -> Self {
        let control = FormControlState::resolve(size, disabled, false, invalid, required, false);
        Self {
            label: label.into(),
            help_text: help_text.map(Into::into),
            error_text: error_text.map(Into::into),
            control,
            metrics: FieldMetrics::from_size(size),
            colors: ThemeResolver::field_colors(tokens, disabled, invalid),
        }
    }

    /// Returns this state with asynchronous activity updated.
    pub const fn with_busy(mut self, busy: bool) -> Self {
        self.control = self.control.with_busy(busy);
        self
    }

    /// Returns the visible label text.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the configured help text.
    pub fn help_text(&self) -> Option<&str> {
        self.help_text.as_deref()
    }

    /// Returns the configured help text.
    pub fn help(&self) -> Option<&str> {
        self.help_text()
    }

    /// Returns the configured error text.
    pub fn error_text(&self) -> Option<&str> {
        self.error_text.as_deref()
    }

    /// Returns the configured error text.
    pub fn error(&self) -> Option<&str> {
        self.error_text()
    }

    /// Returns the message that should be rendered.
    pub fn message(&self) -> Option<FieldMessage> {
        if self.invalid() {
            if let Some(error) = &self.error_text {
                return Some(FieldMessage::Error(error.clone()));
            }
        }

        self.help_text.clone().map(FieldMessage::Help)
    }

    /// Returns the support text that should be rendered.
    pub fn support_text(&self) -> Option<&str> {
        if self.invalid() {
            self.error_text().or(self.help_text())
        } else {
            self.help_text()
        }
    }

    /// Returns whether the rendered support text is an error.
    pub fn support_is_error(&self) -> bool {
        self.invalid() && self.error_text.is_some()
    }

    /// Returns the shared form-control state.
    pub const fn control_state(&self) -> FormControlState {
        self.control
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.control.size()
    }

    /// Returns whether the field is required.
    pub const fn required(&self) -> bool {
        self.control.required()
    }

    /// Returns whether the field is disabled.
    pub const fn disabled(&self) -> bool {
        self.control.disabled()
    }

    /// Returns whether the field is invalid.
    pub const fn invalid(&self) -> bool {
        self.control.invalid()
    }

    /// Returns whether asynchronous work is pending for this field.
    pub const fn busy(&self) -> bool {
        self.control.busy()
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> FieldMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> FieldColors {
        self.colors
    }
}

/// A concrete GPUI field composition component.
#[derive(IntoElement)]
pub struct Field {
    id: ElementId,
    label: SharedString,
    help_text: Option<SharedString>,
    error_text: Option<SharedString>,
    control_state: FormControlState,
    overrides: FormControlOverrides,
    tokens: ThemeTokens,
    control: Option<FieldControlFactory>,
}

impl Field {
    /// Creates a new field with an id and visible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            help_text: None,
            error_text: None,
            control_state: FormControlState::default(),
            overrides: FormControlOverrides::default(),
            tokens: ThemeTokens::default(),
            control: None,
        }
    }

    /// Sets help text.
    pub fn help_text(mut self, help_text: impl Into<SharedString>) -> Self {
        self.help_text = Some(help_text.into());
        self
    }

    /// Sets help text.
    pub fn help(self, help_text: impl Into<SharedString>) -> Self {
        self.help_text(help_text)
    }

    /// Sets error text.
    pub fn error_text(mut self, error_text: impl Into<SharedString>) -> Self {
        self.error_text = Some(error_text.into());
        self
    }

    /// Sets error text.
    pub fn error(self, error_text: impl Into<SharedString>) -> Self {
        self.error_text(error_text)
    }

    /// Marks the field as required.
    pub fn required(mut self, required: bool) -> Self {
        self.overrides = self.overrides.with_required(required);
        self.control_state = self.control_state.with_required(required);
        self
    }

    /// Marks the field as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.overrides = self.overrides.with_disabled(disabled);
        self.control_state = self.control_state.with_disabled(disabled);
        self
    }

    /// Marks the field as invalid.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.overrides = self.overrides.with_invalid(invalid);
        self.control_state = self.control_state.with_invalid(invalid);
        self
    }

    /// Marks the field as having pending asynchronous work.
    pub fn busy(mut self, busy: bool) -> Self {
        self.overrides = self.overrides.with_busy(busy);
        self.control_state = self.control_state.with_busy(busy);
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Sets the field control child.
    pub fn control(mut self, control: impl FieldControl) -> Self {
        let child_state = control.field_control_state();
        self.control_state = self
            .overrides
            .inherit_unset_from(self.control_state, child_state);
        self.control = Some(Box::new(move |semantics| {
            control.with_field_semantics(semantics).into_any_element()
        }));
        self
    }

    /// Returns the resolved field state.
    pub fn state(&self) -> FieldState {
        FieldState::resolve(
            self.label.to_string(),
            self.help_text.as_ref().map(ToString::to_string),
            self.error_text.as_ref().map(ToString::to_string),
            self.control_state.size(),
            self.control_state.required(),
            self.control_state.disabled(),
            self.control_state.invalid(),
            self.tokens,
        )
        .with_busy(self.control_state.busy())
    }
}

impl Sizable for Field {
    fn with_size(mut self, size: Size) -> Self {
        self.control_state = self.control_state.with_size(size);
        self
    }
}

impl RenderOnce for Field {
    fn render(self, window: &mut Window, cx: &mut open_gpui::App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let message = state.message();
        let support_is_error = message.as_ref().is_some_and(FieldMessage::is_error);
        let id = self.id;
        let label_id: ElementId = (id.clone(), "label").into();
        let help_id: ElementId = (id.clone(), "help").into();
        let error_id: ElementId = (id.clone(), "error").into();
        let label = self.label;
        let overrides = self.overrides;

        window.with_id(id.clone(), |window| {
            let labelled_by = window.with_global_id(label_id.clone(), |global_id, _| {
                global_id.accesskit_node_id()
            });
            let described_by = (message.is_some() && !support_is_error).then(|| {
                window.with_global_id(help_id.clone(), |global_id, _| {
                    global_id.accesskit_node_id()
                })
            });
            let error_message = support_is_error.then(|| {
                window.with_global_id(error_id.clone(), |global_id, _| {
                    global_id.accesskit_node_id()
                })
            });
            let control_semantics =
                FieldControlSemantics::new(labelled_by, described_by, error_message, overrides);
            let control = self.control.map(|factory| factory(control_semantics));
            let label_semantics = SemanticDescriptor::new(Role::Label).with_label(label.as_ref());

            div()
                .id(id)
                .flex()
                .flex_col()
                .gap(gpui_px_from_ui(metrics.gap()))
                .when(state.disabled(), |this| this.opacity(0.64))
                .child(
                    div()
                        .id(label_id)
                        .flex()
                        .items_center()
                        .gap_1()
                        .ui_semantics(&label_semantics)
                        .text_size(gpui_px_from_ui(metrics.label_text_size()))
                        .line_height(gpui_px_from_ui(metrics.label_text_size()))
                        .text_color(theme.resolve(colors.label()))
                        .child(label)
                        .when(state.required(), |this| {
                            this.child(
                                div()
                                    .text_color(theme.resolve(colors.required_marker()))
                                    .child("*"),
                            )
                        }),
                )
                .when_some(control, |this, control| this.child(control))
                .when_some(message, |this, message| {
                    let message_id = if message.is_error() {
                        error_id
                    } else {
                        help_id
                    };
                    let message_semantics = if message.is_error() {
                        SemanticDescriptor::new(Role::Alert).with_live_text(message.text())
                    } else {
                        SemanticDescriptor::new(Role::Label).with_label(message.text())
                    };
                    this.child(
                        div()
                            .id(message_id)
                            .ui_semantics(&message_semantics)
                            .text_size(gpui_px_from_ui(metrics.message_text_size()))
                            .line_height(open_gpui::px(18.0))
                            .text_color(theme.resolve(colors.message()))
                            .child(message.text().to_string()),
                    )
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FormControlOverrides, adapter::FieldControlSemantics};
    use open_gpui::accesskit::NodeId;
    use open_gpui_ui_core::{Role, SemanticDescriptor};

    #[test]
    fn text_control_descriptor_projection_has_one_field_relation_authority() {
        let standalone = FieldControlSemantics::project_text_control_descriptor(
            None,
            "Standalone label",
            SemanticDescriptor::new(Role::TextInput).with_placeholder("Placeholder"),
        );
        assert_eq!(standalone.label(), Some("Standalone label"));
        assert_eq!(standalone.placeholder(), Some("Placeholder"));
        assert!(standalone.labelled_by().is_empty());
        assert!(standalone.described_by().is_empty());
        assert_eq!(standalone.error_message(), None);

        let with_help = FieldControlSemantics::new(
            NodeId(11),
            Some(NodeId(12)),
            None,
            FormControlOverrides::default(),
        );
        let help_descriptor = FieldControlSemantics::project_text_control_descriptor(
            Some(&with_help),
            "Ignored standalone label",
            SemanticDescriptor::new(Role::TextInput).with_placeholder("Placeholder"),
        );
        assert_eq!(help_descriptor.label(), None);
        assert_eq!(help_descriptor.placeholder(), Some("Placeholder"));
        assert_eq!(help_descriptor.labelled_by(), &[NodeId(11)]);
        assert_eq!(help_descriptor.described_by(), &[NodeId(12)]);
        assert_eq!(help_descriptor.error_message(), None);

        let with_error = FieldControlSemantics::new(
            NodeId(21),
            None,
            Some(NodeId(23)),
            FormControlOverrides::default(),
        );
        let error_descriptor = FieldControlSemantics::project_text_control_descriptor(
            Some(&with_error),
            "Ignored standalone label",
            SemanticDescriptor::new(Role::MultilineTextInput).with_placeholder("Placeholder"),
        );
        assert_eq!(error_descriptor.label(), None);
        assert_eq!(error_descriptor.placeholder(), Some("Placeholder"));
        assert_eq!(error_descriptor.labelled_by(), &[NodeId(21)]);
        assert!(error_descriptor.described_by().is_empty());
        assert_eq!(error_descriptor.error_message(), Some(&NodeId(23)));
    }
}
