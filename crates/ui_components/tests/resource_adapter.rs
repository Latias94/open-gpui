use open_gpui_resource::{
    MutationStatus, QueryKey, RedactedResourceValue, ResourceClient, ResourceRedactionPolicy,
    ResourceSnapshot, ResourceStatus,
};
use open_gpui_ui_components::{
    FeedbackIntent, ResourceAdapterLabels, ResourceAdapterNamespace, ResourceAdapterNamespaceError,
    ResourceCollectionProjection, ResourceMutationProjection, TreeChildrenLoadState,
    resource_query_key_label,
};
use open_gpui_ui_core::{TableRowChildrenLoadState, ThemeTokens};

#[test]
fn resource_projection_maps_loading_error_empty_and_refreshing_states() {
    let mut client = ResourceClient::default();
    let key = QueryKey::new(["projects"]).unwrap();
    let labels = ResourceAdapterLabels::new("projects");

    let loading_ticket = client.begin_fetch(&key).unwrap();
    let loading = ResourceCollectionProjection::resolve(
        &client
            .snapshot(&key, ResourceRedactionPolicy::RedactAll)
            .unwrap(),
        ResourceAdapterNamespace::new("tests-project-collection").unwrap(),
        0,
        labels.clone(),
    );

    assert!(loading.loading());
    assert!(loading.interaction_disabled());
    assert_eq!(loading.status_message(), Some("Loading projects"));
    assert!(
        loading
            .status_cue_state(ThemeTokens::default())
            .unwrap()
            .busy()
    );
    assert_eq!(
        loading
            .command_loading_state()
            .map(|state| state.message().to_owned()),
        Some("Loading projects".to_owned())
    );
    assert_eq!(
        loading
            .virtualized_status_row("resource-status")
            .unwrap()
            .status_kind()
            .unwrap()
            .as_str(),
        "initial-loading"
    );

    assert!(
        client.complete_fetch_success(loading_ticket, serde_json::json!([{"id": 1}, {"id": 2}]))
    );
    let _observer = client.subscribe(key.clone()).unwrap();
    assert!(client.invalidate(&key).unwrap().refetch_requested);
    let refreshing = ResourceCollectionProjection::resolve(
        &client
            .snapshot(&key, ResourceRedactionPolicy::Summarize)
            .unwrap(),
        ResourceAdapterNamespace::new("tests-project-collection").unwrap(),
        2,
        labels.clone(),
    );

    assert!(refreshing.refreshing());
    assert!(!refreshing.interaction_disabled());
    assert_eq!(
        refreshing
            .status_cue_state(ThemeTokens::default())
            .unwrap()
            .intent(),
        FeedbackIntent::Info
    );
    assert!(
        refreshing
            .status_cue_state(ThemeTokens::default())
            .unwrap()
            .busy()
    );
    assert!(
        refreshing
            .virtualized_status_row("resource-status")
            .is_none()
    );

    let refresh_ticket = client.begin_fetch(&key).unwrap();
    assert!(client.complete_fetch_error(refresh_ticket, "backend unavailable"));
    let stale = ResourceCollectionProjection::resolve(
        &client
            .snapshot(&key, ResourceRedactionPolicy::Summarize)
            .unwrap(),
        ResourceAdapterNamespace::new("tests-project-collection").unwrap(),
        2,
        labels.clone(),
    );

    assert!(stale.stale());
    assert_eq!(stale.status_message(), Some("projects is stale"));
    assert_eq!(
        stale
            .status_cue_state(ThemeTokens::default())
            .unwrap()
            .intent(),
        FeedbackIntent::Warning
    );
    assert!(
        !stale
            .status_cue_state(ThemeTokens::default())
            .unwrap()
            .busy()
    );

    let fresh_ticket = client.begin_fetch(&key).unwrap();
    assert!(client.complete_fetch_success(fresh_ticket, serde_json::json!([])));
    let empty = ResourceCollectionProjection::resolve(
        &client
            .snapshot(&key, ResourceRedactionPolicy::Expose)
            .unwrap(),
        ResourceAdapterNamespace::new("tests-project-collection").unwrap(),
        0,
        labels.clone(),
    );

    assert!(empty.empty());
    assert_eq!(
        empty.empty_state(ThemeTokens::default()).unwrap().title(),
        "No projects"
    );
    assert_eq!(
        empty
            .virtualized_status_row("resource-status")
            .unwrap()
            .status_kind()
            .unwrap()
            .as_str(),
        "empty"
    );

    let failing_key = QueryKey::new(["projects", "archived"]).unwrap();
    let failing_ticket = client.begin_fetch(&failing_key).unwrap();
    assert!(client.complete_fetch_error(failing_ticket, "timeout"));
    let error = ResourceCollectionProjection::resolve(
        &client
            .snapshot(&failing_key, ResourceRedactionPolicy::RedactAll)
            .unwrap(),
        ResourceAdapterNamespace::new("tests-project-archived-collection").unwrap(),
        0,
        labels,
    );

    assert!(error.error());
    assert!(error.retryable());
    assert_eq!(error.status_message(), Some("timeout"));
    assert_eq!(
        error.command_status_item().map(|item| (
            item.id().to_owned(),
            item.intent(),
            item.message().to_owned()
        )),
        Some((
            "resource:tests-project-archived-collection".to_owned(),
            open_gpui_ui_components::CommandStatusIntent::Error,
            "timeout".to_owned()
        ))
    );
    assert_eq!(
        error
            .virtualized_status_row("resource-status")
            .unwrap()
            .retry_action_label_ref(),
        Some("Retry")
    );
}

#[test]
fn resource_projection_maps_tree_and_table_child_load_states() {
    let key = QueryKey::new(["workspace", "members"]).unwrap();
    let labels = ResourceAdapterLabels::new("members");

    let loading = ResourceCollectionProjection::resolve(
        &resource_snapshot(key.clone(), ResourceStatus::Loading, None, None),
        ResourceAdapterNamespace::new("tests-members-collection").unwrap(),
        0,
        labels.clone(),
    );
    assert!(matches!(
        loading.table_children_load_state(),
        TableRowChildrenLoadState::Loading { .. }
    ));
    assert!(loading.tree_children_load_state().is_loading());

    let failed = ResourceCollectionProjection::resolve(
        &resource_snapshot(
            key.clone(),
            ResourceStatus::Error,
            None,
            Some("permission denied"),
        ),
        ResourceAdapterNamespace::new("tests-members-collection").unwrap(),
        0,
        labels.clone(),
    );
    assert!(matches!(
        failed.table_children_load_state(),
        TableRowChildrenLoadState::Failed { .. }
    ));
    assert_eq!(
        failed.tree_children_load_state(),
        TreeChildrenLoadState::failed("permission denied")
    );

    let loaded = ResourceCollectionProjection::resolve(
        &resource_snapshot(
            key,
            ResourceStatus::Success,
            Some(RedactedResourceValue::Summary("array:2 items".to_owned())),
            None,
        ),
        ResourceAdapterNamespace::new("tests-members-collection").unwrap(),
        2,
        labels,
    );
    assert_eq!(
        loaded.table_children_load_state(),
        TableRowChildrenLoadState::Idle
    );
    assert_eq!(
        loaded.tree_children_load_state(),
        TreeChildrenLoadState::loaded()
    );
}

#[test]
fn mutation_projection_maps_pending_success_and_error_feedback() {
    let mut client = ResourceClient::default();
    let labels = ResourceAdapterLabels::new("project");
    let ticket = client.begin_mutation("save-project").unwrap();
    let pending = ResourceMutationProjection::resolve(
        &client
            .mutation_snapshot("save-project", ResourceRedactionPolicy::RedactAll)
            .unwrap(),
        ResourceAdapterNamespace::new("tests-save-project").unwrap(),
        labels.clone(),
    );

    assert!(pending.pending());
    assert!(pending.disables_actions());
    assert_eq!(pending.status_message(), Some("Saving project"));
    assert_eq!(
        pending
            .status_cue_state(ThemeTokens::default())
            .unwrap()
            .intent(),
        FeedbackIntent::Info
    );
    assert!(
        pending
            .status_cue_state(ThemeTokens::default())
            .unwrap()
            .busy()
    );

    assert!(client.complete_mutation_success(ticket, None, []));
    let success = ResourceMutationProjection::resolve(
        &client
            .mutation_snapshot("save-project", ResourceRedactionPolicy::RedactAll)
            .unwrap(),
        ResourceAdapterNamespace::new("tests-save-project").unwrap(),
        labels.clone(),
    );
    assert_eq!(success.status(), &MutationStatus::Success);
    assert!(!success.disables_actions());
    assert_eq!(
        success
            .status_cue_state(ThemeTokens::default())
            .unwrap()
            .intent(),
        FeedbackIntent::Success
    );
    assert!(
        !success
            .status_cue_state(ThemeTokens::default())
            .unwrap()
            .busy()
    );

    let ticket = client.begin_mutation("save-project").unwrap();
    assert!(client.complete_mutation_error(ticket, "write failed"));
    let failed = ResourceMutationProjection::resolve(
        &client
            .mutation_snapshot("save-project", ResourceRedactionPolicy::RedactAll)
            .unwrap(),
        ResourceAdapterNamespace::new("tests-save-project").unwrap(),
        labels,
    );

    assert!(failed.error());
    assert_eq!(failed.status_message(), Some("write failed"));
    assert_eq!(
        failed.command_status_item().map(|item| (
            item.id().to_owned(),
            item.intent(),
            item.message().to_owned()
        )),
        Some((
            "mutation:tests-save-project".to_owned(),
            open_gpui_ui_components::CommandStatusIntent::Error,
            "write failed".to_owned()
        ))
    );
}

#[test]
fn resource_status_identity_uses_only_the_explicit_safe_namespace() {
    const SENSITIVE_CANARY: &str = "resource-secret-token-019f4ad7";

    let key = QueryKey::new([SENSITIVE_CANARY]).unwrap();
    let namespace = ResourceAdapterNamespace::new("public.projects").unwrap();
    let labels = ResourceAdapterLabels::new("projects");
    let stale = ResourceCollectionProjection::resolve(
        &resource_snapshot(
            key.clone(),
            ResourceStatus::Stale,
            Some(RedactedResourceValue::Summary("array:1 item".to_owned())),
            None,
        ),
        namespace.clone(),
        1,
        labels.clone(),
    );
    let failed = ResourceCollectionProjection::resolve(
        &resource_snapshot(key, ResourceStatus::Error, None, Some("failed")),
        namespace.clone(),
        0,
        labels.clone(),
    );
    let stale_id = stale.command_status_item().unwrap().id().to_owned();
    let failed_id = failed.command_status_item().unwrap().id().to_owned();
    assert_eq!(stale_id, "resource:public.projects");
    assert_eq!(failed_id, stale_id);
    assert!(!failed_id.contains(SENSITIVE_CANARY));

    let other = ResourceCollectionProjection::resolve(
        &resource_snapshot(
            QueryKey::new(["other"]).unwrap(),
            ResourceStatus::Stale,
            None,
            None,
        ),
        ResourceAdapterNamespace::new("public.archived").unwrap(),
        1,
        labels.clone(),
    );
    assert_ne!(other.command_status_item().unwrap().id(), failed_id);

    let mut client = ResourceClient::default();
    let ticket = client.begin_mutation(SENSITIVE_CANARY).unwrap();
    let pending = ResourceMutationProjection::resolve(
        &client
            .mutation_snapshot(SENSITIVE_CANARY, ResourceRedactionPolicy::RedactAll)
            .unwrap(),
        namespace.clone(),
        labels.clone(),
    );
    let pending_id = pending.command_status_item().unwrap().id().to_owned();
    assert_eq!(pending_id, "mutation:public.projects");
    assert_ne!(pending_id, failed_id);
    assert!(!pending_id.contains(SENSITIVE_CANARY));

    assert!(client.complete_mutation_error(ticket, "failed"));
    let mutation_failed = ResourceMutationProjection::resolve(
        &client
            .mutation_snapshot(SENSITIVE_CANARY, ResourceRedactionPolicy::RedactAll)
            .unwrap(),
        namespace,
        labels,
    );
    assert_eq!(
        mutation_failed.command_status_item().unwrap().id(),
        pending_id
    );

    assert_eq!(
        ResourceAdapterNamespace::new("   "),
        Err(ResourceAdapterNamespaceError::Empty)
    );
    assert_eq!(
        ResourceAdapterNamespace::new("unsafe\nnamespace"),
        Err(ResourceAdapterNamespaceError::ControlCharacter)
    );
}

#[test]
fn query_key_label_is_stable_for_mixed_segments() {
    let key = QueryKey::new([
        open_gpui_resource::QueryKeySegment::from("workspace"),
        open_gpui_resource::QueryKeySegment::from(42_i64),
        open_gpui_resource::QueryKeySegment::from(true),
    ])
    .unwrap();

    assert_eq!(resource_query_key_label(&key), "workspace/42/true");
}

fn resource_snapshot(
    key: QueryKey,
    status: ResourceStatus,
    data: Option<RedactedResourceValue>,
    error: Option<&str>,
) -> ResourceSnapshot {
    ResourceSnapshot {
        key,
        status,
        data,
        error: error.map(ToOwned::to_owned),
        observer_count: 0,
        fetch_attempts: 0,
    }
}
