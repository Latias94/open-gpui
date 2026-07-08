use std::time::Duration;

use open_gpui_form::{
    DebouncedValidationQueue, FieldLens, FieldPath, FormStatus, FormStore, RedactedValue,
    RedactionPolicy,
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

    store.begin_submit().unwrap();
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

    assert!(!store.complete_async_validation(stale, vec!["too short".to_owned()]));
    assert!(store.complete_async_validation(latest, Vec::new()));
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
    assert!(!store.complete_async_validation(first, vec!["stale".to_owned()]));
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

    store.begin_submit().unwrap();
    store.finish_submit_error("network unavailable");
    assert_eq!(store.status(), FormStatus::SubmitFailed);
    assert_eq!(store.snapshot(RedactionPolicy::RedactAll).submit_count, 1);

    store.begin_submit().unwrap();
    store.finish_submit_success();
    assert_eq!(store.status(), FormStatus::Submitted);
    assert_eq!(store.snapshot(RedactionPolicy::RedactAll).submit_count, 2);
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
