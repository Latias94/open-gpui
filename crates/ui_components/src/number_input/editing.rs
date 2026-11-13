/// The interpretation of a numeric draft before normalization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumberInputDraftValue {
    /// No number has been entered.
    Empty,
    /// A sign, trailing decimal point, or unfinished exponent is still being edited.
    Incomplete,
    /// The draft is not a finite `f32` number.
    Invalid,
    /// A complete, finite number, before clamping or step snapping.
    Value(f32),
}

impl NumberInputDraftValue {
    /// Parses decimal and scientific notation without manufacturing an accepted value.
    pub fn parse(text: &str) -> Self {
        let text = text.trim();
        if text.is_empty() {
            return Self::Empty;
        }
        if matches!(text, "+" | "-" | "." | "+." | "-.") {
            return Self::Incomplete;
        }
        if let Some(mantissa) = text.strip_suffix('.')
            && mantissa.parse::<f32>().is_ok_and(f32::is_finite)
        {
            return Self::Incomplete;
        }
        if let Some((mantissa, exponent)) = text.split_once(['e', 'E'])
            && matches!(exponent, "" | "+" | "-")
            && mantissa.parse::<f32>().is_ok_and(f32::is_finite)
        {
            return Self::Incomplete;
        }
        match text.parse::<f32>() {
            Ok(value) if value.is_finite() => Self::Value(value),
            _ => Self::Invalid,
        }
    }

    /// Returns a complete, finite preview value, if available.
    pub const fn value(self) -> Option<f32> {
        match self {
            Self::Value(value) => Some(value),
            _ => None,
        }
    }
}

/// A local text draft, separate from the owner's accepted numeric value.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberInputDraft {
    pub(super) value: f32,
    pub(super) text: String,
    pub(super) dirty: bool,
}

impl NumberInputDraft {
    pub(super) fn new(value: f32) -> Self {
        Self {
            value,
            text: value.to_string(),
            dirty: false,
        }
    }

    /// Returns the owner's most recently accepted value.
    pub const fn value(&self) -> f32 {
        self.value
    }

    /// Returns the exact local text, including incomplete or invalid input.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the draft's interpretation without clamping or step snapping.
    pub fn parsed(&self) -> NumberInputDraftValue {
        NumberInputDraftValue::parse(&self.text)
    }

    pub(super) fn submitted_value(&self) -> Option<f32> {
        self.text
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
    }

    /// Returns whether an edit is awaiting submission or cancellation.
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }
}

/// The native interaction that produced a preview or commit request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberInputEditSource {
    /// Text entry, including clipboard and platform input.
    Text,
    /// An arrow key step.
    Keyboard,
    /// A step button.
    Stepper,
    /// An accessibility numeric action.
    Accessibility,
    /// A captured horizontal pointer gesture.
    Scrub,
}

/// An observational preview; it does not change the accepted value.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberInputPreview {
    pub(super) source: NumberInputEditSource,
    pub(super) draft: NumberInputDraft,
}

impl NumberInputPreview {
    /// Returns the interaction that produced this preview.
    pub const fn source(&self) -> NumberInputEditSource {
        self.source
    }

    /// Returns the complete local draft, including its accepted baseline.
    pub fn draft(&self) -> &NumberInputDraft {
        &self.draft
    }
}

/// The result of reconciling a normalized proposal with the value owner.
///
/// Controlled inputs resolve this result on the next render. The owner accepts a proposal by
/// supplying it through `NumberInput::value`; retaining the previous value rejects it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberInputCommit {
    pub(super) source: NumberInputEditSource,
    pub(super) previous_value: f32,
    pub(super) proposed_value: f32,
    pub(super) value: f32,
}

impl NumberInputCommit {
    /// Returns the interaction that submitted this proposal.
    pub const fn source(self) -> NumberInputEditSource {
        self.source
    }

    /// Returns the accepted value before the proposal.
    pub const fn previous_value(self) -> f32 {
        self.previous_value
    }

    /// Returns the normalized proposed value.
    pub const fn proposed_value(self) -> f32 {
        self.proposed_value
    }

    /// Returns the owner's value after reconciliation.
    pub const fn value(self) -> f32 {
        self.value
    }

    /// Returns whether the owner accepted the normalized proposal unchanged.
    pub fn accepted(self) -> bool {
        self.value == self.proposed_value
    }
}

/// Why an edit ended without a commit request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberInputCancelReason {
    /// The user pressed Escape or explicitly canceled the edit.
    Escape,
    /// Submission encountered empty, incomplete, or invalid text.
    InvalidDraft,
    /// A different accepted value or numeric constraint replaced the local draft.
    ExternalValue,
    /// The input became read-only or disabled.
    EditingDisabled,
    /// The pointer session was canceled, including capture loss and unmount.
    PointerCancelled,
    /// Focus left an active composition or scrub gesture.
    FocusLost,
}

/// A canceled draft; cancellation never proposes a numeric change to the owner.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberInputCancel {
    pub(super) reason: NumberInputCancelReason,
    pub(super) draft: NumberInputDraft,
}

impl NumberInputCancel {
    /// Returns the cause of cancellation.
    pub const fn reason(&self) -> NumberInputCancelReason {
        self.reason
    }

    /// Returns the discarded draft and its accepted baseline.
    pub fn draft(&self) -> &NumberInputDraft {
        &self.draft
    }
}
