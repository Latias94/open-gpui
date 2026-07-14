use super::common::*;

#[open_gpui::test]
fn password_input_redacts_final_tree_value(cx: &mut open_gpui::TestAppContext) {
    const INITIAL_PASSWORD_CANARY: &str = "s3cr3t-accessibility-canary-initial";
    const UPDATED_PASSWORD_CANARY: &str = "updated-secret";

    struct PasswordProbe {
        value: Rc<RefCell<String>>,
        changes: Rc<RefCell<Vec<String>>>,
    }

    impl Render for PasswordProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.borrow().clone();
            let next_value = self.value.clone();
            let changes = self.changes.clone();
            TextInput::new("semantic-password", "Password")
                .value(value)
                .display_mode(TextInputDisplayMode::Password)
                .on_change(move |value, _, _| {
                    *next_value.borrow_mut() = value.clone();
                    changes.borrow_mut().push(value);
                })
        }
    }

    let value = Rc::new(RefCell::new(INITIAL_PASSWORD_CANARY.to_owned()));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let (_, cx) = cx.add_window_view(|_, _| PasswordProbe {
        value: value.clone(),
        changes: changes.clone(),
    });
    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("password input should publish its final accessibility tree");
    let (password_id, password) = a11y_node_with_label(&update, "Password");

    assert_eq!(password.role(), accesskit::Role::PasswordInput);
    assert_exact_actions(
        password,
        &[accesskit::Action::Focus, accesskit::Action::SetValue],
    );
    let masked_value = password
        .value()
        .expect("password should expose a masked value");
    assert!(!masked_value.is_empty());
    assert!(
        masked_value
            .chars()
            .all(|character| character == '\u{2022}')
    );
    assert_eq!(
        masked_value.chars().count(),
        INITIAL_PASSWORD_CANARY.chars().count()
    );
    assert_ne!(masked_value, INITIAL_PASSWORD_CANARY);
    assert_tree_excludes_text(&update, INITIAL_PASSWORD_CANARY);

    assert!(cx.dispatch_accessibility_action(action_request(
        accesskit::Action::SetValue,
        password_id,
        Some(accesskit::ActionData::Value(UPDATED_PASSWORD_CANARY.into())),
    )));
    assert_eq!(changes.borrow().as_slice(), [UPDATED_PASSWORD_CANARY]);
    cx.run_until_parked();

    let updated = cx
        .latest_accessibility_tree_update()
        .expect("updated password should publish its masked final tree");
    let (updated_password_id, updated_password) = a11y_node_with_label(&updated, "Password");
    assert_eq!(updated_password_id, password_id);
    let updated_masked_value = updated_password
        .value()
        .expect("updated password should expose a masked value");
    assert!(!updated_masked_value.is_empty());
    assert!(
        updated_masked_value
            .chars()
            .all(|character| character == '\u{2022}')
    );
    assert_eq!(
        updated_masked_value.chars().count(),
        UPDATED_PASSWORD_CANARY.chars().count()
    );
    assert_ne!(updated_masked_value, masked_value);
    assert_tree_excludes_text(&updated, INITIAL_PASSWORD_CANARY);
    assert_tree_excludes_text(&updated, UPDATED_PASSWORD_CANARY);
}
