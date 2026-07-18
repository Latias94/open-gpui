use std::time::Duration;

use open_gpui::KeyDownEvent;
use web_time::Instant;

const COLLECTION_TYPEAHEAD_RESET: Duration = Duration::from_millis(700);

fn prefers_committed_character_input(event: &KeyDownEvent) -> bool {
    event.prefer_character_input && event.keystroke.key_char.is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionTypeaheadInput {
    text: String,
    disallowed_modifier: bool,
    ime_in_progress: bool,
}

impl CollectionTypeaheadInput {
    pub(crate) fn from_key_down(event: &KeyDownEvent) -> Self {
        let modifiers = event.keystroke.modifiers;
        let text_modifiers_are_character_input = prefers_committed_character_input(event);
        Self {
            text: event
                .keystroke
                .key_char
                .as_deref()
                .unwrap_or(event.keystroke.key.as_str())
                .to_owned(),
            disallowed_modifier: modifiers.platform
                || modifiers.function
                || ((modifiers.control || modifiers.alt) && !text_modifiers_are_character_input),
            ime_in_progress: event.keystroke.is_ime_in_progress(),
        }
    }

    fn normalized_key(self) -> Option<String> {
        if self.disallowed_modifier || self.ime_in_progress {
            return None;
        }

        let mut chars = self.text.chars();
        let character = chars.next()?;
        if chars.next().is_some() || character.is_control() || character.is_whitespace() {
            return None;
        }

        Some(character.to_lowercase().collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionTypeaheadUpdate {
    match_query: String,
    search_after_current: bool,
}

impl CollectionTypeaheadUpdate {
    pub(crate) fn match_query(&self) -> &str {
        &self.match_query
    }

    pub(crate) const fn searches_after_current(&self) -> bool {
        self.search_after_current
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CollectionTypeaheadSession {
    reset_after: Duration,
    buffer: String,
    last_input_at: Option<Instant>,
    repeated_key: Option<String>,
}

impl CollectionTypeaheadSession {
    #[cfg(test)]
    fn new(reset_after: Duration) -> Self {
        Self {
            reset_after,
            buffer: String::new(),
            last_input_at: None,
            repeated_key: None,
        }
    }

    pub(crate) fn push(
        &mut self,
        input: CollectionTypeaheadInput,
        now: Instant,
    ) -> Option<CollectionTypeaheadUpdate> {
        let key = input.normalized_key()?;
        let reset =
            self.last_input_at
                .map_or(true, |last| match now.checked_duration_since(last) {
                    Some(elapsed) => elapsed > self.reset_after,
                    None => true,
                });

        let repeated_character = if reset {
            self.buffer.clear();
            self.repeated_key = Some(key.clone());
            false
        } else {
            let repeated = self.repeated_key.as_deref() == Some(key.as_str());
            if !repeated {
                self.repeated_key = None;
            }
            repeated
        };

        if reset {
            self.buffer.push_str(&key);
        } else if !repeated_character {
            self.buffer.push_str(&key);
        }
        self.last_input_at = Some(now);

        Some(CollectionTypeaheadUpdate {
            match_query: self.buffer.clone(),
            search_after_current: reset || repeated_character,
        })
    }

    pub(crate) fn reset(&mut self) {
        self.buffer.clear();
        self.last_input_at = None;
        self.repeated_key = None;
    }

    #[cfg(test)]
    fn buffer(&self) -> &str {
        &self.buffer
    }
}

impl Default for CollectionTypeaheadSession {
    fn default() -> Self {
        Self {
            reset_after: COLLECTION_TYPEAHEAD_RESET,
            buffer: String::new(),
            last_input_at: None,
            repeated_key: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use open_gpui::{KeyDownEvent, Keystroke, Modifiers};

    use super::*;

    fn input(text: &str) -> CollectionTypeaheadInput {
        CollectionTypeaheadInput {
            text: text.to_owned(),
            disallowed_modifier: false,
            ime_in_progress: false,
        }
    }

    fn key_event(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> KeyDownEvent {
        KeyDownEvent {
            keystroke: Keystroke {
                modifiers,
                key: key.to_owned(),
                key_char: key_char.map(str::to_owned),
            },
            is_held: false,
            prefer_character_input: false,
        }
    }

    #[test]
    fn fake_clock_accumulates_until_timeout_and_resets_without_sleeping() {
        let start = Instant::now();
        let mut session = CollectionTypeaheadSession::new(Duration::from_millis(700));

        let first = session
            .push(input("N"), start)
            .expect("printable input should be accepted");
        assert_eq!(first.match_query(), "n");
        assert!(first.searches_after_current());

        let second = session
            .push(input("o"), start + Duration::from_millis(700))
            .expect("the timeout boundary should retain the buffer");
        assert_eq!(second.match_query(), "no");
        assert!(!second.searches_after_current());
        assert_eq!(session.buffer(), "no");

        let reset = session
            .push(input("t"), start + Duration::from_millis(1_401))
            .expect("the first key after timeout should start a new buffer");
        assert_eq!(reset.match_query(), "t");
        assert!(reset.searches_after_current());
        assert_eq!(session.buffer(), "t");
    }

    #[test]
    fn repeated_character_updates_cycle_with_one_character_match_query() {
        let start = Instant::now();
        let mut session = CollectionTypeaheadSession::default();

        let first = session.push(input("A"), start).unwrap();
        let second = session
            .push(input("a"), start + Duration::from_millis(1))
            .unwrap();
        let third = session
            .push(input("a"), start + Duration::from_millis(2))
            .unwrap();

        assert_eq!(first.match_query(), "a");
        assert!(first.searches_after_current());
        assert_eq!(second.match_query(), "a");
        assert!(second.searches_after_current());
        assert_eq!(third.match_query(), "a");
        assert!(third.searches_after_current());
        assert_eq!(session.buffer(), "a");

        let mixed = session
            .push(input("l"), start + Duration::from_millis(3))
            .unwrap();
        assert_eq!(mixed.match_query(), "al");
        assert!(!mixed.searches_after_current());
    }

    #[test]
    fn key_adapter_accepts_committed_characters_and_filters_modifiers_and_ime() {
        let start = Instant::now();
        let mut session = CollectionTypeaheadSession::default();

        let shifted = key_event(
            "a",
            Some("A"),
            Modifiers {
                shift: true,
                ..Modifiers::none()
            },
        );
        assert_eq!(
            session
                .push(CollectionTypeaheadInput::from_key_down(&shifted), start)
                .unwrap()
                .match_query(),
            "a"
        );

        let committed_ime = key_event("process", Some("文"), Modifiers::none());
        assert!(
            session
                .push(
                    CollectionTypeaheadInput::from_key_down(&committed_ime),
                    start + Duration::from_secs(1),
                )
                .is_some()
        );

        let accepted_buffer = session.buffer().to_owned();
        for event in [
            key_event("a", None, Modifiers::none()),
            key_event("left", None, Modifiers::none()),
            key_event("space", Some(" "), Modifiers::none()),
            key_event(
                "a",
                Some("a"),
                Modifiers {
                    control: true,
                    ..Modifiers::none()
                },
            ),
            key_event(
                "a",
                Some("a"),
                Modifiers {
                    alt: true,
                    ..Modifiers::none()
                },
            ),
            key_event(
                "a",
                Some("a"),
                Modifiers {
                    platform: true,
                    ..Modifiers::none()
                },
            ),
            key_event(
                "a",
                Some("a"),
                Modifiers {
                    function: true,
                    ..Modifiers::none()
                },
            ),
        ] {
            assert!(
                session
                    .push(
                        CollectionTypeaheadInput::from_key_down(&event),
                        start + Duration::from_secs(2),
                    )
                    .is_none(),
                "event should not enter the typeahead buffer: {event:?}"
            );
            assert_eq!(
                session.buffer(),
                accepted_buffer,
                "rejected input must not mutate the session"
            );
        }

        let held = KeyDownEvent {
            is_held: true,
            ..key_event("b", Some("b"), Modifiers::none())
        };
        assert!(
            session
                .push(
                    CollectionTypeaheadInput::from_key_down(&held),
                    start + Duration::from_secs(3),
                )
                .is_some(),
            "held printable input should retain the existing cycling behavior"
        );

        let alt_gr = KeyDownEvent {
            prefer_character_input: true,
            ..key_event(
                "q",
                Some("@"),
                Modifiers {
                    control: true,
                    alt: true,
                    ..Modifiers::none()
                },
            )
        };
        assert!(
            session
                .push(
                    CollectionTypeaheadInput::from_key_down(&alt_gr),
                    start + Duration::from_secs(4),
                )
                .is_some(),
            "committed AltGr character input should be accepted"
        );
    }

    #[test]
    fn clock_rollback_starts_a_new_session_buffer() {
        let start = Instant::now();
        let mut session = CollectionTypeaheadSession::default();
        session
            .push(input("a"), start + Duration::from_secs(1))
            .unwrap();

        let update = session.push(input("b"), start).unwrap();
        assert_eq!(update.match_query(), "b");
        assert_eq!(session.buffer(), "b");
    }

    #[test]
    fn reset_clears_buffer_and_deadline() {
        let start = Instant::now();
        let mut session = CollectionTypeaheadSession::default();
        session.push(input("a"), start).unwrap();

        session.reset();

        assert_eq!(session.buffer(), "");
        let update = session
            .push(input("b"), start + Duration::from_millis(1))
            .unwrap();
        assert_eq!(update.match_query(), "b");
        assert!(update.searches_after_current());
    }

    #[test]
    fn rejected_input_does_not_refresh_the_deadline() {
        let start = Instant::now();
        let mut session = CollectionTypeaheadSession::default();
        session.push(input("n"), start).unwrap();

        let rejected = CollectionTypeaheadInput {
            text: "o".to_owned(),
            disallowed_modifier: true,
            ime_in_progress: false,
        };
        assert!(
            session
                .push(rejected, start + Duration::from_millis(600))
                .is_none()
        );

        let update = session
            .push(input("o"), start + Duration::from_millis(701))
            .unwrap();
        assert_eq!(update.match_query(), "o");
        assert!(update.searches_after_current());
    }
}
