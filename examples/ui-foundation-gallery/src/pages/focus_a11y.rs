//! Focus and accessibility foundation page metadata.

use open_gpui_ui_components::{FieldState, TextInputDisplayMode, TextInputState, TextareaState};
use open_gpui_ui_core::{LivePoliteness, Role, Size, ThemeTokens, Toggled};

use crate::story::{StoryContract, StoryProbeContract, StoryProbeOperation};

/// Page title.
pub const TITLE: &str = "Focus & A11y";
/// Page summary.
pub const SUMMARY: &str = "Focus handles and accessibility roles exposed at the foundation layer.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "FocusHandle",
    "Focusable",
    "Role::Button",
    "AccessibleAction",
    "Toggled",
];

/// Initial fake secret used to prove password accessibility redaction.
pub const PASSWORD_REDACTION_CANARY: &str = "gallery-password-redaction-canary";
/// Accessible label rendered by the editable account-name input.
pub const TEXT_INPUT_LABEL: &str = "Editable account name";
/// Initial controlled account-name value.
pub const TEXT_INPUT_INITIAL_VALUE: &str = "gallery account";
/// Deterministic changed account-name value used by headless history.
pub const TEXT_INPUT_CHANGED_VALUE: &str = "gallery account updated";
/// Placeholder rendered by the editable account-name input.
pub const TEXT_INPUT_PLACEHOLDER: &str = "Account name";
/// Label shared by the release-notes Field and Textarea.
pub const TEXTAREA_FIELD_LABEL: &str = "Release notes";
/// Initial controlled release-notes value.
pub const TEXTAREA_INITIAL_VALUE: &str = "Release note draft";
/// Placeholder rendered by the release-notes Textarea.
pub const TEXTAREA_PLACEHOLDER: &str = "Write release notes";
/// Help relation rendered by the valid release-notes Field.
pub const TEXTAREA_FIELD_HELP: &str = "Summarize user-visible changes.";
/// Error relation rendered by the invalid release-notes Field.
pub const TEXTAREA_FIELD_ERROR: &str = "Add a concise release note.";
/// Accessible label rendered by the password input.
pub const PASSWORD_LABEL: &str = "Gallery password";
/// Placeholder rendered by the password input.
pub const PASSWORD_PLACEHOLDER: &str = "Password";
/// Initial visible copy for the declarative status region before it becomes live.
pub const LIVE_STATUS_IDLE_TEXT: &str = "Waiting for a status update.";
/// First deterministic update copy for the declarative status region.
pub const LIVE_STATUS_UPDATE_ONE_TEXT: &str = "Background synchronization update 1.";
/// Second deterministic update copy for the declarative status region.
pub const LIVE_STATUS_UPDATE_TWO_TEXT: &str = "Background synchronization update 2.";
/// Assertive message rendered by the live-region scenario.
pub const LIVE_ALERT_TEXT: &str = "Background synchronization failed.";
/// Repeated application-global message submitted by the transient announcement control.
pub const WINDOW_ANNOUNCEMENT_TEXT: &str = "Background synchronization completed.";

/// Raw Focus/A11y story text that must never cross a DevTools artifact boundary.
pub const FOCUS_A11Y_SENSITIVE_TEXT: &[&str] = &[
    TEXT_INPUT_LABEL,
    TEXT_INPUT_INITIAL_VALUE,
    TEXT_INPUT_CHANGED_VALUE,
    TEXT_INPUT_PLACEHOLDER,
    TEXTAREA_FIELD_LABEL,
    TEXTAREA_INITIAL_VALUE,
    TEXTAREA_PLACEHOLDER,
    TEXTAREA_FIELD_HELP,
    TEXTAREA_FIELD_ERROR,
    PASSWORD_LABEL,
    PASSWORD_REDACTION_CANARY,
    PASSWORD_PLACEHOLDER,
    LIVE_STATUS_IDLE_TEXT,
    LIVE_STATUS_UPDATE_ONE_TEXT,
    LIVE_STATUS_UPDATE_TWO_TEXT,
    LIVE_ALERT_TEXT,
    WINDOW_ANNOUNCEMENT_TEXT,
];

const TEXT_INPUT_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(
        StoryProbeOperation::Edit,
        "TextInput",
        "final accessible value and selection",
    ),
    StoryProbeContract::new(StoryProbeOperation::Focus, "TextInput", "input focus"),
    StoryProbeContract::new(
        StoryProbeOperation::ReadPublicPayload,
        "final tree",
        "resolved TextInput semantics",
    ),
];

const TEXTAREA_FIELD_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(
        StoryProbeOperation::Activate,
        "relation toggle",
        "help and error relation transition",
    ),
    StoryProbeContract::new(StoryProbeOperation::Edit, "Textarea", "multiline value"),
    StoryProbeContract::new(
        StoryProbeOperation::ReadPublicPayload,
        "final tree",
        "Field-owned label, description, and error relations",
    ),
];

const PASSWORD_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(
        StoryProbeOperation::Edit,
        "PasswordInput",
        "masked accessible value",
    ),
    StoryProbeContract::new(StoryProbeOperation::Focus, "PasswordInput", "input focus"),
    StoryProbeContract::new(
        StoryProbeOperation::ReadPublicPayload,
        "final tree",
        "redacted password semantics",
    ),
];

const LIVE_REGION_PROBES: &[StoryProbeContract] = &[
    StoryProbeContract::new(
        StoryProbeOperation::Activate,
        "status controls",
        "polite, busy, and assertive live-region transitions",
    ),
    StoryProbeContract::new(
        StoryProbeOperation::Focus,
        "announcement control",
        "focus stability across semantic commits",
    ),
    StoryProbeContract::new(
        StoryProbeOperation::ReadPublicPayload,
        "final tree",
        "live priority, atomicity, busy state, and transient generations",
    ),
];

/// Component id for the editable account-name input scenario.
pub const TEXT_INPUT_COMPONENT_ID: &str = "focus-a11y-text-input";
/// Component id for the Field that owns the release-notes relations.
pub const TEXTAREA_FIELD_COMPONENT_ID: &str = "focus-a11y-textarea-field";
/// Component id for the release-notes Textarea control.
pub const TEXTAREA_COMPONENT_ID: &str = "focus-a11y-field-textarea";
/// Component id for the password-redaction input scenario.
pub const PASSWORD_COMPONENT_ID: &str = "focus-a11y-password-input";
/// Component id for the declarative polite status region.
pub const LIVE_STATUS_COMPONENT_ID: &str = "focus-a11y-live-status";
/// Component id for the conditional assertive alert region.
pub const LIVE_ALERT_COMPONENT_ID: &str = "focus-a11y-live-alert";
/// Component id for the polite status update control.
pub const LIVE_STATUS_UPDATE_CONTROL_ID: &str = "focus-a11y-live-update";
/// Component id for the status busy-state control.
pub const LIVE_BUSY_TOGGLE_CONTROL_ID: &str = "focus-a11y-live-busy";
/// Component id for the assertive alert control.
pub const LIVE_ALERT_TOGGLE_CONTROL_ID: &str = "focus-a11y-live-alert-toggle";
/// Component id for the application-global announcement control.
pub const WINDOW_ANNOUNCEMENT_CONTROL_ID: &str = "focus-a11y-window-announce";
/// Stable selector for the Textarea Field relation transition control.
pub const TEXTAREA_FIELD_ERROR_TOGGLE_SELECTOR: &str = "gallery:focus-a11y-field-error-toggle";
/// Stable selector for the polite status update control.
pub const LIVE_STATUS_UPDATE_SELECTOR: &str = "button:focus-a11y-live-update:root";
/// Stable selector for the status busy-state control.
pub const LIVE_BUSY_TOGGLE_SELECTOR: &str = "button:focus-a11y-live-busy:root";
/// Stable selector for the assertive alert control.
pub const LIVE_ALERT_TOGGLE_SELECTOR: &str = "button:focus-a11y-live-alert-toggle:root";
/// Stable selector for the application-global announcement control.
pub const WINDOW_ANNOUNCEMENT_SELECTOR: &str = "button:focus-a11y-window-announce:root";

const TEXT_INPUT_COMPONENT_IDS: &[&str] = &[TEXT_INPUT_COMPONENT_ID];
const TEXTAREA_FIELD_COMPONENT_IDS: &[&str] = &[TEXTAREA_FIELD_COMPONENT_ID, TEXTAREA_COMPONENT_ID];
const PASSWORD_COMPONENT_IDS: &[&str] = &[PASSWORD_COMPONENT_ID];
const LIVE_REGION_COMPONENT_IDS: &[&str] = &[LIVE_STATUS_COMPONENT_ID, LIVE_ALERT_COMPONENT_ID];

/// Typed identity for one real Focus/A11y Text/Form story.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FocusA11yScenarioId {
    /// Editable TextInput value-and-selection story.
    TextInputValueSelection,
    /// Textarea Field help-and-error relation story.
    TextareaFieldRelations,
    /// Password free-text redaction story.
    PasswordFreeTextRedaction,
    /// Declarative live regions and transient window announcement story.
    LiveRegionsAndAnnouncements,
}

impl FocusA11yScenarioId {
    /// All real Focus/A11y Text/Form stories in canonical page order.
    pub const ALL: [Self; 4] = [
        Self::TextInputValueSelection,
        Self::TextareaFieldRelations,
        Self::PasswordFreeTextRedaction,
        Self::LiveRegionsAndAnnouncements,
    ];

    /// Returns the stable scenario id used by story contracts and artifacts.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TextInputValueSelection => "text-input-value-selection",
            Self::TextareaFieldRelations => "textarea-field-relations",
            Self::PasswordFreeTextRedaction => "password-free-text-redaction",
            Self::LiveRegionsAndAnnouncements => "live-regions-and-announcements",
        }
    }
}

/// One executable Focus/A11y scenario and the concrete component instances it owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusA11yScenarioSpec {
    /// Typed scenario identity.
    pub scenario_id: FocusA11yScenarioId,
    /// Stable scenario id.
    pub id: &'static str,
    /// Concrete component ids rendered only by this scenario.
    pub component_ids: &'static [&'static str],
    /// Stable selector for the scenario's primary component.
    pub sample_selector: &'static str,
    /// Optional stable selector for the scenario transition control.
    pub control_selector: Option<&'static str>,
    state: &'static str,
    probes: &'static [StoryProbeContract],
}

impl FocusA11yScenarioSpec {
    /// Returns the federated Gallery story contract for this scenario.
    pub fn story_contract(self) -> StoryContract {
        StoryContract::focus_accessibility(
            self.id,
            "text-form",
            self.state,
            self.sample_selector,
            self.control_selector,
            self.probes,
        )
    }
}

/// Editable TextInput value-and-selection scenario.
pub const TEXT_INPUT_VALUE_SELECTION_SCENARIO: FocusA11yScenarioSpec = FocusA11yScenarioSpec {
    scenario_id: FocusA11yScenarioId::TextInputValueSelection,
    id: FocusA11yScenarioId::TextInputValueSelection.as_str(),
    component_ids: TEXT_INPUT_COMPONENT_IDS,
    sample_selector: "text-input:focus-a11y-text-input:root",
    control_selector: None,
    state: "TextInputState",
    probes: TEXT_INPUT_PROBES,
};

/// Textarea Field help-and-error relation scenario.
pub const TEXTAREA_FIELD_RELATIONS_SCENARIO: FocusA11yScenarioSpec = FocusA11yScenarioSpec {
    scenario_id: FocusA11yScenarioId::TextareaFieldRelations,
    id: FocusA11yScenarioId::TextareaFieldRelations.as_str(),
    component_ids: TEXTAREA_FIELD_COMPONENT_IDS,
    sample_selector: "textarea:focus-a11y-field-textarea:root",
    control_selector: Some(TEXTAREA_FIELD_ERROR_TOGGLE_SELECTOR),
    state: "FieldState + TextareaState",
    probes: TEXTAREA_FIELD_PROBES,
};

/// Password free-text redaction scenario.
pub const PASSWORD_FREE_TEXT_REDACTION_SCENARIO: FocusA11yScenarioSpec = FocusA11yScenarioSpec {
    scenario_id: FocusA11yScenarioId::PasswordFreeTextRedaction,
    id: FocusA11yScenarioId::PasswordFreeTextRedaction.as_str(),
    component_ids: PASSWORD_COMPONENT_IDS,
    sample_selector: "text-input:focus-a11y-password-input:root",
    control_selector: None,
    state: "TextInputState::Password",
    probes: PASSWORD_PROBES,
};

/// Declarative live-region and transient window announcement scenario.
pub const LIVE_REGIONS_AND_ANNOUNCEMENTS_SCENARIO: FocusA11yScenarioSpec = FocusA11yScenarioSpec {
    scenario_id: FocusA11yScenarioId::LiveRegionsAndAnnouncements,
    id: FocusA11yScenarioId::LiveRegionsAndAnnouncements.as_str(),
    component_ids: LIVE_REGION_COMPONENT_IDS,
    sample_selector: "status-cue:focus-a11y-live-status:root",
    control_selector: Some(LIVE_STATUS_UPDATE_SELECTOR),
    state: "StatusCueState + window announcement queue",
    probes: LIVE_REGION_PROBES,
};

/// Real Text/Form scenarios rendered by the Focus/A11y page.
pub const FOCUS_A11Y_SCENARIOS: &[FocusA11yScenarioSpec] = &[
    TEXT_INPUT_VALUE_SELECTION_SCENARIO,
    TEXTAREA_FIELD_RELATIONS_SCENARIO,
    PASSWORD_FREE_TEXT_REDACTION_SCENARIO,
    LIVE_REGIONS_AND_ANNOUNCEMENTS_SCENARIO,
];

/// Returns executable story contracts for the Focus/A11y Text/Form scenarios.
pub fn focus_a11y_story_contracts() -> Vec<StoryContract> {
    FOCUS_A11Y_SCENARIOS
        .iter()
        .copied()
        .map(FocusA11yScenarioSpec::story_contract)
        .collect()
}

/// One focusable control row in the gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusControlSpec {
    /// Stable control id.
    pub id: &'static str,
    /// User-facing label.
    pub label: &'static str,
    /// Tab index used by the focus handle.
    pub tab_index: isize,
    /// Accessibility role used by the rendered control.
    pub role: Role,
}

impl FocusControlSpec {
    const fn new(id: &'static str, label: &'static str, tab_index: isize, role: Role) -> Self {
        Self {
            id,
            label,
            tab_index,
            role,
        }
    }
}

/// Primary action in the Focus/A11y keyboard order.
pub const PRIMARY_FOCUS_CONTROL: FocusControlSpec =
    FocusControlSpec::new("focus-primary", "Primary action", 1, Role::Button);
/// Counter action in the Focus/A11y keyboard order.
pub const COUNTER_FOCUS_CONTROL: FocusControlSpec =
    FocusControlSpec::new("focus-counter", "Counter", 2, Role::SpinButton);
/// Switch action in the Focus/A11y keyboard order.
pub const SWITCH_FOCUS_CONTROL: FocusControlSpec =
    FocusControlSpec::new("focus-switch", "Feature switch", 3, Role::Switch);

/// Canonical focusable controls used by the demo.
pub const FOCUS_CONTROLS: [FocusControlSpec; 3] = [
    PRIMARY_FOCUS_CONTROL,
    COUNTER_FOCUS_CONTROL,
    SWITCH_FOCUS_CONTROL,
];

/// Mutable shell state for the focus and accessibility page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FocusA11yPageState {
    counter: i32,
    enabled: bool,
    focus_message: &'static str,
    text_input_value: String,
    textarea_value: String,
    field_invalid: bool,
    password_value: String,
    live_status_revision: u64,
    live_busy: bool,
    live_alert_visible: bool,
}

impl Default for FocusA11yPageState {
    fn default() -> Self {
        Self {
            counter: 0,
            enabled: false,
            focus_message: "Ready for keyboard focus.",
            text_input_value: TEXT_INPUT_INITIAL_VALUE.to_owned(),
            textarea_value: TEXTAREA_INITIAL_VALUE.to_owned(),
            field_invalid: false,
            password_value: PASSWORD_REDACTION_CANARY.to_owned(),
            live_status_revision: 0,
            live_busy: false,
            live_alert_visible: false,
        }
    }
}

impl FocusA11yPageState {
    /// Builds the resolved Text/Form story state consumed by rendering and DevTools projection.
    pub(crate) fn text_form_story_state(&self, tokens: ThemeTokens) -> FocusA11yTextFormStoryState {
        FocusA11yTextFormStoryState {
            text_input: TextInputState::resolve(
                self.text_input_value.clone(),
                Some(TEXT_INPUT_PLACEHOLDER),
                Size::Medium,
                false,
                false,
                false,
                false,
                true,
                tokens,
            ),
            field: FieldState::resolve(
                TEXTAREA_FIELD_LABEL,
                Some(TEXTAREA_FIELD_HELP),
                Some(TEXTAREA_FIELD_ERROR),
                Size::Medium,
                true,
                false,
                self.field_invalid,
                tokens,
            ),
            textarea: TextareaState::resolve(
                self.textarea_value.clone(),
                Some(TEXTAREA_PLACEHOLDER),
                Size::Medium,
                3,
                false,
                false,
                self.field_invalid,
                true,
                true,
                tokens,
            ),
            password: TextInputState::resolve_with_display_mode(
                self.password_value.clone(),
                Some(PASSWORD_PLACEHOLDER),
                Size::Medium,
                false,
                false,
                false,
                false,
                true,
                TextInputDisplayMode::Password,
                tokens,
            ),
        }
    }

    /// Returns the current demo counter.
    pub(crate) fn counter(&self) -> i32 {
        self.counter
    }

    /// Returns the derived accessibility state used by the page renderer.
    pub(crate) fn demo_state(&self) -> A11yDemoState {
        a11y_demo_state(self.counter, self.enabled)
    }

    /// Returns the current user-facing focus message.
    pub(crate) fn focus_message(&self) -> &'static str {
        self.focus_message
    }

    /// Returns whether the demo switch is enabled.
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    /// Updates the focus message and returns whether the state changed.
    pub(crate) fn set_focus_message(&mut self, message: &'static str) -> bool {
        if self.focus_message == message {
            return false;
        }

        self.focus_message = message;
        true
    }

    /// Increments the demo counter and returns whether the state changed.
    pub(crate) fn increment_counter(&mut self) -> bool {
        self.counter += 1;
        true
    }

    /// Decrements the demo counter and returns whether the state changed.
    pub(crate) fn decrement_counter(&mut self) -> bool {
        let next = (self.counter - 1).max(0);
        if self.counter == next {
            return false;
        }

        self.counter = next;
        true
    }

    /// Resets the demo counter and returns whether the state changed.
    pub(crate) fn reset_counter(&mut self) -> bool {
        if self.counter == 0 {
            return false;
        }

        self.counter = 0;
        true
    }

    /// Toggles the demo switch and returns whether the state changed.
    pub(crate) fn toggle_enabled(&mut self) -> bool {
        self.enabled = !self.enabled;
        true
    }

    /// Updates the controlled TextInput value.
    pub(crate) fn set_text_input_value(&mut self, value: String) -> bool {
        if self.text_input_value == value {
            return false;
        }
        self.text_input_value = value;
        true
    }

    /// Updates the controlled Textarea value.
    pub(crate) fn set_textarea_value(&mut self, value: String) -> bool {
        if self.textarea_value == value {
            return false;
        }
        self.textarea_value = value;
        true
    }

    /// Switches between the Field help and error relation states.
    pub(crate) fn toggle_field_invalid(&mut self) -> bool {
        self.field_invalid = !self.field_invalid;
        true
    }

    /// Updates the controlled password value.
    pub(crate) fn set_password_value(&mut self, value: String) -> bool {
        if self.password_value == value {
            return false;
        }
        self.password_value = value;
        true
    }

    /// Returns the visible and accessible text for the stable status region.
    pub(crate) fn live_status_text(&self) -> String {
        if self.live_status_revision == 0 {
            LIVE_STATUS_IDLE_TEXT.to_owned()
        } else if self.live_status_revision == 1 {
            LIVE_STATUS_UPDATE_ONE_TEXT.to_owned()
        } else if self.live_status_revision == 2 {
            LIVE_STATUS_UPDATE_TWO_TEXT.to_owned()
        } else {
            format!(
                "Background synchronization update {}.",
                self.live_status_revision
            )
        }
    }

    /// Returns the live priority for the stable status region.
    pub(crate) const fn live_status_priority(&self) -> LivePoliteness {
        if self.live_status_revision == 0 {
            LivePoliteness::Off
        } else {
            LivePoliteness::Polite
        }
    }

    /// Returns whether the stable status region is busy.
    pub(crate) const fn live_busy(&self) -> bool {
        self.live_busy
    }

    /// Returns whether the assertive alert region is mounted.
    pub(crate) const fn live_alert_visible(&self) -> bool {
        self.live_alert_visible
    }

    /// Publishes the next declarative status value.
    pub(crate) fn update_live_status(&mut self) -> bool {
        let next_revision = self.live_status_revision.saturating_add(1);
        if next_revision == self.live_status_revision {
            return false;
        }
        self.live_status_revision = next_revision;
        true
    }

    /// Toggles the declarative status busy state.
    pub(crate) fn toggle_live_busy(&mut self) -> bool {
        self.live_busy = !self.live_busy;
        true
    }

    /// Toggles the assertive alert region.
    pub(crate) fn toggle_live_alert(&mut self) -> bool {
        self.live_alert_visible = !self.live_alert_visible;
        true
    }
}

/// Resolved component states shared by the Focus/A11y renderer and DevTools projection.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FocusA11yTextFormStoryState {
    text_input: TextInputState,
    field: FieldState,
    textarea: TextareaState,
    password: TextInputState,
}

impl FocusA11yTextFormStoryState {
    /// Returns the editable account-name state.
    pub(crate) fn text_input(&self) -> &TextInputState {
        &self.text_input
    }

    /// Returns the release-notes Field state.
    pub(crate) fn field(&self) -> &FieldState {
        &self.field
    }

    /// Returns the release-notes Textarea state.
    pub(crate) fn textarea(&self) -> &TextareaState {
        &self.textarea
    }

    /// Returns the password input state.
    pub(crate) fn password(&self) -> &TextInputState {
        &self.password
    }
}

/// Accessibility state surfaced by the demo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A11yDemoState {
    /// Counter value exposed by the spin button.
    pub counter: i32,
    /// Toggle state exposed by the switch.
    pub toggled: Toggled,
    /// Role used for the counter control.
    pub counter_role: Role,
    /// Role used for the toggle control.
    pub toggle_role: Role,
}

/// Builds the accessibility state summary from plain view state.
pub const fn a11y_demo_state(counter: i32, enabled: bool) -> A11yDemoState {
    A11yDemoState {
        counter,
        toggled: if enabled {
            Toggled::True
        } else {
            Toggled::False
        },
        counter_role: Role::SpinButton,
        toggle_role: Role::Switch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_a11y_page_state_tracks_message_counter_and_switch_state() {
        let mut state = FocusA11yPageState::default();

        assert_eq!(state.focus_message(), "Ready for keyboard focus.");
        assert_eq!(state.demo_state(), a11y_demo_state(0, false));
        assert!(!state.enabled());
        assert!(!state.decrement_counter());
        assert!(!state.reset_counter());

        assert!(state.increment_counter());
        assert_eq!(state.demo_state(), a11y_demo_state(1, false));
        assert!(state.toggle_enabled());
        assert_eq!(state.demo_state(), a11y_demo_state(1, true));
        assert!(state.set_focus_message("Focus moved."));
        assert_eq!(state.focus_message(), "Focus moved.");
        assert!(state.reset_counter());
        assert_eq!(state.demo_state(), a11y_demo_state(0, true));

        let initial_story = state.text_form_story_state(ThemeTokens::default());
        assert_eq!(initial_story.text_input().value(), TEXT_INPUT_INITIAL_VALUE);
        assert_eq!(initial_story.textarea().value(), TEXTAREA_INITIAL_VALUE);
        assert!(!initial_story.field().invalid());
        assert_eq!(initial_story.password().value(), PASSWORD_REDACTION_CANARY);

        assert!(state.set_text_input_value("updated account".to_owned()));
        assert!(state.set_textarea_value("Updated note".to_owned()));
        assert!(state.toggle_field_invalid());
        assert!(state.set_password_value("updated secret".to_owned()));
        let updated_story = state.text_form_story_state(ThemeTokens::default());
        assert_eq!(updated_story.text_input().value(), "updated account");
        assert_eq!(updated_story.textarea().value(), "Updated note");
        assert!(updated_story.field().invalid());
        assert_eq!(updated_story.password().value(), "updated secret");

        assert_eq!(state.live_status_text(), LIVE_STATUS_IDLE_TEXT);
        assert_eq!(state.live_status_priority(), LivePoliteness::Off);
        assert!(!state.live_busy());
        assert!(!state.live_alert_visible());
        assert!(state.update_live_status());
        assert_eq!(
            state.live_status_text(),
            "Background synchronization update 1."
        );
        assert_eq!(state.live_status_priority(), LivePoliteness::Polite);
        assert!(state.toggle_live_busy());
        assert!(state.live_busy());
        assert!(state.toggle_live_alert());
        assert!(state.live_alert_visible());
    }
}
