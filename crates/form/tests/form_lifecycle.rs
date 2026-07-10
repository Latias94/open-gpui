use std::time::Duration;

use open_gpui_form::{
    DebouncedValidationQueue, FieldLens, FieldPath, FormError, FormStatus, FormStore,
    RedactedValue, RedactionPolicy, SubmitBlockReason, SubmitCompletion, ValidationCompletion,
};

#[test]
fn value_changes_update_dirty_touched_and_visited_meta() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("old@example.com"))
        .unwrap();

    store.visit(&email).unwrap();
    store.touch(&email).unwrap();
    store
        .set_value(&email, serde_json::json!("new@example.com"))
        .unwrap();

    let snapshot = store.snapshot(RedactionPolicy::Expose);
    let field = snapshot.field(&email).unwrap();
    assert!(field.meta.dirty);
    assert!(field.meta.touched);
    assert!(field.meta.visited);
    assert_eq!(
        field.value,
        RedactedValue::Json(serde_json::json!("new@example.com"))
    );
}

#[test]
fn sync_validation_blocks_submit_until_errors_clear() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("bad"))
        .unwrap();

    let outcome = store
        .validate_field(&email, |value| {
            if value.as_str().is_some_and(|value| value.contains('@')) {
                Vec::new()
            } else {
                vec!["Email must contain @".to_owned()]
            }
        })
        .unwrap();

    assert!(!outcome.is_valid());
    assert!(store.begin_submit().is_err());

    store
        .set_value(&email, serde_json::json!("ok@example.com"))
        .unwrap();
    store
        .validate_field(&email, |value| {
            if value.as_str().is_some_and(|value| value.contains('@')) {
                Vec::new()
            } else {
                vec!["Email must contain @".to_owned()]
            }
        })
        .unwrap();

    let _submit = store.begin_submit().unwrap();
    assert_eq!(store.status(), FormStatus::Submitting);
}

#[test]
fn async_validation_ignores_stale_generation() {
    let mut store = FormStore::default();
    let username = FieldPath::new("account.username").unwrap();
    store
        .register_field(username.clone(), serde_json::json!("frank"))
        .unwrap();

    let stale = store.begin_async_validation(&username).unwrap();
    let latest = store.begin_async_validation(&username).unwrap();
    let before_stale_completion = store.snapshot(RedactionPolicy::Expose);

    assert_eq!(
        store.complete_async_validation(stale, vec!["too short".to_owned()]),
        ValidationCompletion::Stale
    );
    assert_eq!(
        store.snapshot(RedactionPolicy::Expose),
        before_stale_completion
    );
    assert_eq!(
        store.complete_async_validation(latest, Vec::new()),
        ValidationCompletion::Applied
    );
    assert!(
        !store
            .snapshot(RedactionPolicy::RedactAll)
            .field(&username)
            .unwrap()
            .meta
            .validating
    );
}

#[test]
fn debounce_queue_keeps_only_latest_field_request() {
    let mut store = FormStore::default();
    let username = FieldPath::new("account.username").unwrap();
    store
        .register_field(username.clone(), serde_json::json!("frank"))
        .unwrap();
    let mut queue = DebouncedValidationQueue::new(Duration::from_millis(150));

    let first = queue.request(&mut store, &username).unwrap();
    let second = queue.request(&mut store, &username).unwrap();

    assert_eq!(queue.pending_len(), 1);
    assert_eq!(queue.take_pending().unwrap(), second);
    assert_eq!(
        store.complete_async_validation(first, vec!["stale".to_owned()]),
        ValidationCompletion::Stale
    );
}

#[test]
fn async_validation_derives_form_status_until_all_current_tickets_complete() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    let username = FieldPath::new("account.username").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("team@example.com"))
        .unwrap();
    store
        .register_field(username.clone(), serde_json::json!("frank"))
        .unwrap();

    let email_ticket = store.begin_async_validation(&email).unwrap();
    let username_ticket = store.begin_async_validation(&username).unwrap();

    assert_eq!(store.status(), FormStatus::Validating);
    assert_eq!(
        store
            .snapshot(RedactionPolicy::RedactAll)
            .validating_field_count(),
        2
    );
    assert_eq!(
        store.complete_async_validation(email_ticket, Vec::new()),
        ValidationCompletion::Applied
    );
    assert_eq!(store.status(), FormStatus::Validating);
    assert_eq!(
        store.complete_async_validation(username_ticket, vec!["taken".to_owned()]),
        ValidationCompletion::Applied
    );
    assert_eq!(store.status(), FormStatus::Idle);
    assert!(!store.can_submit());
}

#[test]
fn value_change_and_reset_cancel_old_value_validation() {
    let mut store = FormStore::default();
    let username = FieldPath::new("account.username").unwrap();
    store
        .register_field(username.clone(), serde_json::json!("old"))
        .unwrap();

    let edited_ticket = store.begin_async_validation(&username).unwrap();
    store
        .set_value(&username, serde_json::json!("new"))
        .unwrap();
    let after_edit = store.snapshot(RedactionPolicy::Expose);

    assert_eq!(store.status(), FormStatus::Idle);
    assert_eq!(
        store.complete_async_validation(edited_ticket, vec!["old value error".to_owned()]),
        ValidationCompletion::Cancelled
    );
    assert_eq!(store.snapshot(RedactionPolicy::Expose), after_edit);

    let reset_ticket = store.begin_async_validation(&username).unwrap();
    store.reset();
    let after_reset = store.snapshot(RedactionPolicy::Expose);
    assert_eq!(
        store.complete_async_validation(reset_ticket, vec!["post-reset error".to_owned()]),
        ValidationCompletion::Cancelled
    );
    assert_eq!(store.snapshot(RedactionPolicy::Expose), after_reset);
    assert_eq!(store.status(), FormStatus::Idle);
}

#[test]
fn same_value_write_preserves_current_validation_ticket() {
    let mut store = FormStore::default();
    let username = FieldPath::new("account.username").unwrap();
    store
        .register_field(username.clone(), serde_json::json!("frank"))
        .unwrap();

    let ticket = store.begin_async_validation(&username).unwrap();
    store
        .set_value(&username, serde_json::json!("frank"))
        .unwrap();

    assert_eq!(store.status(), FormStatus::Validating);
    assert_eq!(
        store.complete_async_validation(ticket, Vec::new()),
        ValidationCompletion::Applied
    );
}

#[test]
fn old_value_validation_is_stale_once_a_new_value_validation_owns_the_field() {
    let mut store = FormStore::default();
    let username = FieldPath::new("account.username").unwrap();
    store
        .register_field(username.clone(), serde_json::json!("first"))
        .unwrap();

    let old_value = store.begin_async_validation(&username).unwrap();
    store
        .set_value(&username, serde_json::json!("second"))
        .unwrap();
    let current = store.begin_async_validation(&username).unwrap();

    assert_eq!(
        store.complete_async_validation(old_value, vec!["obsolete".to_owned()]),
        ValidationCompletion::Stale
    );
    assert_eq!(store.status(), FormStatus::Validating);
    assert_eq!(
        store.complete_async_validation(current, Vec::new()),
        ValidationCompletion::Applied
    );
}

#[test]
fn editing_one_field_preserves_unrelated_current_validation() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    let username = FieldPath::new("account.username").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("old@example.com"))
        .unwrap();
    store
        .register_field(username.clone(), serde_json::json!("frank"))
        .unwrap();

    let email_ticket = store.begin_async_validation(&email).unwrap();
    let username_ticket = store.begin_async_validation(&username).unwrap();
    store
        .set_value(&email, serde_json::json!("new@example.com"))
        .unwrap();

    assert_eq!(store.status(), FormStatus::Validating);
    assert_eq!(
        store.complete_async_validation(email_ticket, vec!["obsolete".to_owned()]),
        ValidationCompletion::Cancelled
    );
    assert_eq!(
        store.complete_async_validation(username_ticket, Vec::new()),
        ValidationCompletion::Applied
    );
    assert_eq!(store.status(), FormStatus::Idle);
}

#[test]
fn synchronous_validation_supersedes_pending_async_validation() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("team@example.com"))
        .unwrap();
    let async_ticket = store.begin_async_validation(&email).unwrap();

    store
        .validate_field(&email, |_| vec!["sync error".to_owned()])
        .unwrap();

    assert_eq!(
        store.complete_async_validation(async_ticket, vec!["async error".to_owned()]),
        ValidationCompletion::Cancelled
    );
    let snapshot = store.snapshot(RedactionPolicy::Expose);
    assert_eq!(snapshot.field(&email).unwrap().meta.errors, ["sync error"]);
    assert_eq!(snapshot.status, FormStatus::Idle);
}

#[test]
fn validation_cannot_start_while_submitting() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("team@example.com"))
        .unwrap();
    let submit = store.begin_submit().unwrap();

    assert_eq!(
        store.begin_async_validation(&email),
        Err(FormError::CannotValidateWhileSubmitting)
    );
    assert_eq!(
        store.validate_field(&email, |_| Vec::new()),
        Err(FormError::CannotValidateWhileSubmitting)
    );
    assert_eq!(
        store.finish_submit_success(submit),
        SubmitCompletion::Applied
    );
}

#[test]
fn reset_restores_initial_values_and_meta() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("old@example.com"))
        .unwrap();
    store.touch(&email).unwrap();
    store
        .set_value(&email, serde_json::json!("new@example.com"))
        .unwrap();

    store.reset();

    let snapshot = store.snapshot(RedactionPolicy::Expose);
    let field = snapshot.field(&email).unwrap();
    assert_eq!(
        field.value,
        RedactedValue::Json(serde_json::json!("old@example.com"))
    );
    assert_eq!(field.meta, Default::default());
}

#[test]
fn submit_lifecycle_tracks_success_and_failure() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email, serde_json::json!("ok@example.com"))
        .unwrap();

    let failed_submit = store.begin_submit().unwrap();
    assert_eq!(
        store.finish_submit_error(failed_submit.clone(), "network unavailable"),
        SubmitCompletion::Applied
    );
    assert_eq!(
        store.finish_submit_success(failed_submit),
        SubmitCompletion::Cancelled
    );
    assert_eq!(store.status(), FormStatus::SubmitFailed);
    assert_eq!(store.snapshot(RedactionPolicy::RedactAll).submit_count, 1);

    let successful_submit = store.begin_submit().unwrap();
    assert_eq!(
        store.finish_submit_success(successful_submit),
        SubmitCompletion::Applied
    );
    assert_eq!(store.status(), FormStatus::Submitted);
    assert_eq!(store.snapshot(RedactionPolicy::RedactAll).submit_count, 2);
}

#[test]
fn submit_rejections_and_completions_are_typed() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("bad"))
        .unwrap();
    store
        .validate_field(&email, |_| vec!["invalid".to_owned()])
        .unwrap();

    assert_eq!(
        store.begin_submit(),
        Err(FormError::CannotSubmit {
            reason: SubmitBlockReason::Invalid,
        })
    );
    assert_eq!(store.snapshot(RedactionPolicy::RedactAll).submit_count, 0);

    store.validate_field(&email, |_| Vec::new()).unwrap();
    let validation = store.begin_async_validation(&email).unwrap();
    assert_eq!(
        store.begin_submit(),
        Err(FormError::CannotSubmit {
            reason: SubmitBlockReason::Validating,
        })
    );
    assert_eq!(
        store.complete_async_validation(validation, Vec::new()),
        ValidationCompletion::Applied
    );

    let submit = store.begin_submit().unwrap();
    assert_eq!(
        store.begin_submit(),
        Err(FormError::CannotSubmit {
            reason: SubmitBlockReason::AlreadySubmitting,
        })
    );
    assert_eq!(store.snapshot(RedactionPolicy::RedactAll).submit_count, 1);

    store
        .set_value(&email, serde_json::json!("edited@example.com"))
        .unwrap();
    assert_eq!(
        store.finish_submit_success(submit),
        SubmitCompletion::Cancelled
    );
    assert_eq!(store.status(), FormStatus::Idle);
}

#[test]
fn stale_submit_completion_cannot_replace_a_newer_submission() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("first@example.com"))
        .unwrap();

    let stale = store.begin_submit().unwrap();
    store
        .set_value(&email, serde_json::json!("second@example.com"))
        .unwrap();
    let current = store.begin_submit().unwrap();
    let before_stale_completion = store.snapshot(RedactionPolicy::RedactAll);

    assert_eq!(
        store.finish_submit_error(stale, "obsolete failure"),
        SubmitCompletion::Stale
    );
    assert_eq!(
        store.snapshot(RedactionPolicy::RedactAll),
        before_stale_completion
    );
    assert_eq!(
        store.finish_submit_success(current),
        SubmitCompletion::Applied
    );
    assert_eq!(store.status(), FormStatus::Submitted);
}

#[test]
fn tickets_are_scoped_to_the_form_store_that_created_them() {
    let path = FieldPath::new("account.email").unwrap();
    let mut validation_owner = FormStore::default();
    let mut other_validation_owner = FormStore::default();
    for store in [&mut validation_owner, &mut other_validation_owner] {
        store
            .register_field(path.clone(), serde_json::json!("team@example.com"))
            .unwrap();
    }

    let foreign_validation = validation_owner.begin_async_validation(&path).unwrap();
    let current_validation = other_validation_owner
        .begin_async_validation(&path)
        .unwrap();
    let before_foreign_validation = other_validation_owner.snapshot(RedactionPolicy::RedactAll);
    assert_eq!(
        other_validation_owner
            .complete_async_validation(foreign_validation, vec!["foreign".to_owned()]),
        ValidationCompletion::Cancelled
    );
    assert_eq!(
        other_validation_owner.snapshot(RedactionPolicy::RedactAll),
        before_foreign_validation
    );
    assert_eq!(
        other_validation_owner.complete_async_validation(current_validation, Vec::new()),
        ValidationCompletion::Applied
    );

    let mut submit_owner = FormStore::default();
    let mut other_submit_owner = FormStore::default();
    for store in [&mut submit_owner, &mut other_submit_owner] {
        store
            .register_field(path.clone(), serde_json::json!("team@example.com"))
            .unwrap();
    }
    let foreign_submit = submit_owner.begin_submit().unwrap();
    let current_submit = other_submit_owner.begin_submit().unwrap();
    let before_foreign_submit = other_submit_owner.snapshot(RedactionPolicy::RedactAll);
    assert_eq!(
        other_submit_owner.finish_submit_error(foreign_submit, "foreign"),
        SubmitCompletion::Cancelled
    );
    assert_eq!(
        other_submit_owner.snapshot(RedactionPolicy::RedactAll),
        before_foreign_submit
    );
    assert_eq!(
        other_submit_owner.finish_submit_success(current_submit),
        SubmitCompletion::Applied
    );
}

#[test]
fn successful_field_registration_invalidates_submit_and_terminal_authority() {
    let mut store = FormStore::default();
    store
        .register_field(
            FieldPath::new("account.email").unwrap(),
            serde_json::json!("team@example.com"),
        )
        .unwrap();

    let obsolete = store.begin_submit().unwrap();
    store
        .register_field(
            FieldPath::new("profile.notes").unwrap(),
            serde_json::json!("new shape"),
        )
        .unwrap();
    assert_eq!(
        store.finish_submit_success(obsolete),
        SubmitCompletion::Cancelled
    );
    assert_eq!(store.status(), FormStatus::Idle);

    let current = store.begin_submit().unwrap();
    assert_eq!(
        store.finish_submit_success(current),
        SubmitCompletion::Applied
    );
    store
        .register_field(
            FieldPath::new("workspace.region").unwrap(),
            serde_json::json!("eu-west"),
        )
        .unwrap();
    assert_eq!(store.status(), FormStatus::Idle);
    assert_eq!(store.snapshot(RedactionPolicy::RedactAll).submit_count, 2);
}

#[test]
fn rejected_duplicate_registration_does_not_cancel_submission() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("team@example.com"))
        .unwrap();
    let submit = store.begin_submit().unwrap();

    assert_eq!(
        store.register_field(email.clone(), serde_json::json!("replacement")),
        Err(FormError::DuplicateField(email))
    );
    assert_eq!(
        store.finish_submit_success(submit),
        SubmitCompletion::Applied
    );
}

#[test]
fn reset_cancels_active_submission_without_rewinding_submit_count() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("first@example.com"))
        .unwrap();
    store
        .set_value(&email, serde_json::json!("second@example.com"))
        .unwrap();

    let submit = store.begin_submit().unwrap();
    store.reset();

    assert_eq!(
        store.finish_submit_error(submit, "obsolete failure"),
        SubmitCompletion::Cancelled
    );
    let snapshot = store.snapshot(RedactionPolicy::Expose);
    assert_eq!(snapshot.status, FormStatus::Idle);
    assert_eq!(snapshot.submit_count, 1);
    assert!(snapshot.errors.is_empty());
    assert_eq!(
        snapshot.field(&email).unwrap().value,
        RedactedValue::Json(serde_json::json!("first@example.com"))
    );
}

#[test]
fn effective_edit_clears_terminal_submission_but_same_value_write_does_not() {
    let mut store = FormStore::default();
    let email = FieldPath::new("account.email").unwrap();
    store
        .register_field(email.clone(), serde_json::json!("first@example.com"))
        .unwrap();
    let submit = store.begin_submit().unwrap();
    assert_eq!(
        store.finish_submit_error(submit, "retry later"),
        SubmitCompletion::Applied
    );

    store
        .set_value(&email, serde_json::json!("first@example.com"))
        .unwrap();
    assert_eq!(store.status(), FormStatus::SubmitFailed);

    store
        .set_value(&email, serde_json::json!("second@example.com"))
        .unwrap();
    let snapshot = store.snapshot(RedactionPolicy::RedactAll);
    assert_eq!(snapshot.status, FormStatus::Idle);
    assert!(snapshot.errors.is_empty());
    assert!(snapshot.can_submit());
}

#[test]
fn snapshots_are_redacted_by_default() {
    let mut store = FormStore::default();
    let password = FieldPath::new("account.password").unwrap();
    store
        .register_field(password.clone(), serde_json::json!("secret"))
        .unwrap();

    let snapshot = store.snapshot(RedactionPolicy::default());
    let field = snapshot.field(&password).unwrap();

    assert_eq!(field.value, RedactedValue::Redacted);
}

#[test]
fn typed_lens_reads_and_writes_app_owned_values() {
    #[derive(Default)]
    struct Account {
        email: String,
    }

    let lens = FieldLens::new(
        FieldPath::new("account.email").unwrap(),
        |account: &Account| serde_json::json!(account.email),
        |account: &mut Account, value| {
            account.email = value.as_str().unwrap_or_default().to_owned();
            Ok(())
        },
    );
    let mut account = Account::default();

    lens.set(&mut account, serde_json::json!("ok@example.com"))
        .unwrap();

    assert_eq!(lens.get(&account), serde_json::json!("ok@example.com"));
}
