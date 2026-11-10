use super::*;
use crate::{NoAction, OperatingSystem, PlatformInfo, PlatformTarget, TestAppContext};
use open_gpui_collections::HashMap;
use std::cell::RefCell;

#[derive(Default)]
struct RecordingMapper {
    calls: RefCell<Vec<(Keystroke, bool)>>,
}

impl PlatformKeyboardMapper for RecordingMapper {
    fn map_key_equivalent(
        &self,
        mut keystroke: Keystroke,
        use_key_equivalents: bool,
    ) -> KeybindingKeystroke {
        self.calls
            .borrow_mut()
            .push((keystroke.clone(), use_key_equivalents));
        if use_key_equivalents {
            match keystroke.key.as_str() {
                "a" => keystroke.key = "q".into(),
                "b" => keystroke.key = "x".into(),
                _ => {}
            }
        }
        KeybindingKeystroke::from_keystroke(keystroke)
    }

    fn get_key_equivalents(&self) -> Option<&HashMap<char, char>> {
        None
    }
}

#[test]
#[should_panic(expected = "secondary-a")]
fn static_binding_rejects_secondary() {
    KeyBinding::new("ctrl-a secondary-a", NoAction {}, None);
}

#[crate::test]
fn load_uses_app_host_os_and_actual_mapper(cx: &mut TestAppContext) {
    let mapper = Rc::new(RecordingMapper::default());
    cx.update(|app| app.keyboard_mapper = mapper.clone());

    let static_binding = KeyBinding::new("cmd-a ctrl-b", NoAction {}, Some("Editor"));
    assert!(mapper.calls.borrow().is_empty());
    assert_eq!(
        static_binding.keystrokes()[0].inner(),
        &Keystroke::parse("cmd-a").unwrap()
    );
    assert_eq!(
        static_binding.keystrokes()[1].inner(),
        &Keystroke::parse("ctrl-b").unwrap()
    );

    let source = String::from("secondary-a alt-b");
    for os in [
        OperatingSystem::Mac,
        OperatingSystem::Windows,
        OperatingSystem::Linux,
        OperatingSystem::FreeBsd,
        OperatingSystem::Ios,
        OperatingSystem::Android,
        OperatingSystem::Unknown,
    ] {
        for target in [PlatformTarget::Native, PlatformTarget::Web] {
            cx.set_platform_info(PlatformInfo { os, target });
            for use_key_equivalents in [false, true] {
                mapper.calls.borrow_mut().clear();
                let binding = cx.read(|app| {
                    KeyBinding::load(
                        &source,
                        Box::new(NoAction {}),
                        None,
                        use_key_equivalents,
                        None,
                        app,
                    )
                    .unwrap()
                });
                let first = Keystroke::parse_with_os("secondary-a", os).unwrap();
                let second = Keystroke::parse("alt-b").unwrap();
                assert_eq!(
                    *mapper.calls.borrow(),
                    [
                        (first.clone(), use_key_equivalents),
                        (second.clone(), use_key_equivalents)
                    ],
                    "{os:?} {target:?}"
                );
                let typed = [
                    Keystroke {
                        key: if use_key_equivalents { "q" } else { "a" }.into(),
                        ..first
                    },
                    Keystroke {
                        key: if use_key_equivalents { "x" } else { "b" }.into(),
                        ..second
                    },
                ];
                assert_eq!(binding.keystrokes()[0].inner(), &typed[0]);
                assert_eq!(binding.keystrokes()[1].inner(), &typed[1]);
                assert_eq!(binding.match_keystrokes(&[&typed[0]]), Some(true));
                assert_eq!(
                    binding.match_keystrokes(&[&typed[0], &typed[1]]),
                    Some(false)
                );
                assert_eq!(
                    binding.match_keystrokes(&[&typed[0], &typed[1], &typed[0]]),
                    None
                );
                assert_eq!(
                    binding.match_keystrokes(&[Keystroke::parse("z").unwrap()]),
                    None
                );
            }
        }
    }

    let replacement = Rc::new(RecordingMapper::default());
    cx.update(|app| app.keyboard_mapper = replacement.clone());
    let binding =
        cx.read(|app| KeyBinding::load("a", Box::new(NoAction {}), None, true, None, app).unwrap());
    assert_eq!(replacement.calls.borrow().len(), 1);
    assert_eq!(binding.keystrokes()[0].inner().key, "q");
}

#[crate::test]
fn dynamic_binding_owns_input_and_retains_metadata_when_cloned(cx: &mut TestAppContext) {
    cx.set_platform_info(PlatformInfo {
        os: OperatingSystem::Mac,
        target: PlatformTarget::Web,
    });
    let source = String::from("alt-s->ß secondary-a");
    let predicate = Rc::new(KeyBindingContextPredicate::parse("Editor && os == macos").unwrap());
    let input = SharedString::from(String::from(r#"{"message":"retained"}"#));
    let binding = cx.read(|app| {
        KeyBinding::load(
            &source,
            Box::new(NoAction {}),
            Some(predicate.clone()),
            false,
            Some(input.clone()),
            app,
        )
        .unwrap()
        .with_meta(KeyBindingMetaIndex(7))
    });
    drop(source);

    let mut cloned = binding.clone();
    assert!(cloned.action().as_any().is::<NoAction>());
    assert!(Rc::ptr_eq(&cloned.predicate().unwrap(), &predicate));
    assert_eq!(cloned.meta(), Some(KeyBindingMetaIndex(7)));
    assert_eq!(cloned.action_input(), Some(input));
    assert_eq!(cloned.keystrokes(), binding.keystrokes());
    assert_eq!(
        cloned.keystrokes()[1].inner(),
        &Keystroke::parse("cmd-a").unwrap()
    );

    cloned.keystrokes[0].remove_key_char();
    assert_eq!(cloned.keystrokes()[0].inner().key_char, None);
    assert_eq!(
        binding.keystrokes()[0].inner().key_char.as_deref(),
        Some("ß")
    );
}

#[crate::test]
fn invalid_dynamic_chord_retains_the_failing_keystroke(cx: &mut TestAppContext) {
    for os in [
        OperatingSystem::Mac,
        OperatingSystem::Windows,
        OperatingSystem::Unknown,
    ] {
        cx.set_platform_info(PlatformInfo {
            os,
            target: PlatformTarget::Web,
        });
        let source = String::from("secondary-a ctrl-a-b");
        let error = cx.read(|app| {
            KeyBinding::load(&source, Box::new(NoAction {}), None, false, None, app).unwrap_err()
        });
        drop(source);
        assert_eq!(error.keystroke, "ctrl-a-b");
        assert!(
            error
                .to_string()
                .contains(crate::KEYSTROKE_PARSE_EXPECTED_MESSAGE)
        );
    }
}
