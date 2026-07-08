use std::time::Duration;

use open_gpui_resource::{
    MutationStatus, PaginatedResourceSnapshot, QueryKey, ResourceClient, ResourcePage,
    ResourceRedactionPolicy, ResourceStatus, RetryPolicy,
};

fn key(parts: &[&str]) -> QueryKey {
    QueryKey::new(parts.iter().copied()).unwrap()
}

#[test]
fn observed_query_tracks_fetch_success_and_redacted_snapshot() {
    let mut client = ResourceClient::default();
    let key = key(&["workspace", "members"]);
    let observer = client.subscribe(key.clone()).unwrap();

    let fetch = client.begin_fetch(&key).unwrap();
    assert_eq!(
        client
            .snapshot(&key, ResourceRedactionPolicy::RedactAll)
            .unwrap()
            .status,
        ResourceStatus::Loading
    );
    assert!(client.complete_fetch_success(fetch, serde_json::json!(["a", "b"])));

    let snapshot = client
        .snapshot(&key, ResourceRedactionPolicy::RedactAll)
        .unwrap();
    assert_eq!(snapshot.status, ResourceStatus::Success);
    assert_eq!(snapshot.observer_count, 1);
    assert!(snapshot.data.unwrap().is_redacted());

    client.unsubscribe(observer).unwrap();
    assert_eq!(
        client
            .snapshot(&key, ResourceRedactionPolicy::RedactAll)
            .unwrap()
            .observer_count,
        0
    );
}

#[test]
fn stale_fetch_generation_is_ignored() {
    let mut client = ResourceClient::default();
    let key = key(&["workspace", "members"]);
    client.ensure_query(key.clone());

    let stale = client.begin_fetch(&key).unwrap();
    let latest = client.begin_fetch(&key).unwrap();

    assert!(!client.complete_fetch_success(stale, serde_json::json!(["stale"])));
    assert!(client.complete_fetch_success(latest, serde_json::json!(["latest"])));
    assert_eq!(
        client
            .snapshot(&key, ResourceRedactionPolicy::Expose)
            .unwrap()
            .data
            .unwrap()
            .as_json(),
        Some(&serde_json::json!(["latest"]))
    );
}

#[test]
fn invalidation_requests_refetch_when_query_is_observed() {
    let mut client = ResourceClient::default();
    let key = key(&["workspace", "members"]);
    client.subscribe(key.clone()).unwrap();
    let fetch = client.begin_fetch(&key).unwrap();
    assert!(client.complete_fetch_success(fetch, serde_json::json!(["a"])));

    let outcome = client.invalidate(&key).unwrap();

    assert!(outcome.refetch_requested);
    assert_eq!(
        client
            .snapshot(&key, ResourceRedactionPolicy::RedactAll)
            .unwrap()
            .status,
        ResourceStatus::Refetching
    );
}

#[test]
fn retry_policy_stops_after_max_attempts() {
    let policy = RetryPolicy::new(3, Duration::from_millis(50));

    assert!(policy.should_retry(1));
    assert!(policy.should_retry(2));
    assert!(!policy.should_retry(3));
    assert_eq!(policy.next_delay(2), Some(Duration::from_millis(100)));
}

#[test]
fn mutation_success_invalidates_configured_queries() {
    let mut client = ResourceClient::default();
    let key = key(&["workspace", "members"]);
    client.subscribe(key.clone()).unwrap();
    let fetch = client.begin_fetch(&key).unwrap();
    assert!(client.complete_fetch_success(fetch, serde_json::json!(["a"])));

    let mutation = client.begin_mutation("save-member").unwrap();
    assert!(client.complete_mutation_success(
        mutation,
        Some(serde_json::json!({"ok": true})),
        [key.clone()],
    ));

    let mutation_snapshot = client
        .mutation_snapshot("save-member", ResourceRedactionPolicy::RedactAll)
        .unwrap();
    assert_eq!(mutation_snapshot.status, MutationStatus::Success);
    assert_eq!(
        client
            .snapshot(&key, ResourceRedactionPolicy::RedactAll)
            .unwrap()
            .status,
        ResourceStatus::Refetching
    );
}

#[test]
fn paginated_snapshot_preserves_page_order_and_cursors() {
    let key = key(&["workspace", "members"]);
    let mut pages = PaginatedResourceSnapshot::new(key);
    pages.push_page(ResourcePage::new(
        Some("cursor-2".to_owned()),
        [serde_json::json!("a")],
    ));
    pages.push_page(ResourcePage::new(
        Some("cursor-3".to_owned()),
        [serde_json::json!("b")],
    ));

    let snapshot = pages.snapshot(ResourceRedactionPolicy::Summarize);

    assert_eq!(snapshot.pages[0].cursor.as_deref(), Some("cursor-2"));
    assert_eq!(snapshot.pages[1].cursor.as_deref(), Some("cursor-3"));
    assert_eq!(snapshot.pages[0].items.len(), 1);
}
