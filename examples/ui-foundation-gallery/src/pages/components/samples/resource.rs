use std::sync::Arc;

use super::*;
use open_gpui_resource::{
    MutationSnapshot, QueryKey, ResourceClient, ResourceRedactionPolicy, ResourceSnapshot,
};
use open_gpui_ui_components::{
    ResourceAdapterLabels, ResourceCollectionProjection, ResourceMutationProjection,
    TreeChildrenLoadState,
};
use open_gpui_ui_core::TableRowChildrenLoadState;

/// One resource-adapter integration sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceAdapterSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Short explanation of the adapter slice.
    pub summary: &'static str,
    /// Stable badge label.
    pub badge: &'static str,
    /// Query lifecycle projection.
    pub collection: ResourceCollectionProjection,
    /// Optional mutation lifecycle projection.
    pub mutation: Option<ResourceMutationProjection>,
    /// Optional compact status cue projected from resource state.
    pub status_cue: Option<StatusCueState>,
    /// Optional full-surface empty/error state projected from resource state.
    pub empty_state: Option<EmptyStateState>,
    /// Virtualized-list descriptors projected from resource state and sample rows.
    pub virtualized_items: Arc<[VirtualizedListItemDescriptor]>,
    /// Command loading copy projected for resource-backed providers.
    pub command_loading_message: Option<String>,
    /// Command status copy projected for resource-backed providers.
    pub command_status_message: Option<String>,
    /// Table child-loading state projected from query lifecycle.
    pub table_children_state: TableRowChildrenLoadState,
    /// Tree child-loading state projected from query lifecycle.
    pub tree_children_state: TreeChildrenLoadState,
    /// Query snapshot shared with the DevTools dogfood page.
    pub snapshot: ResourceSnapshot,
    /// Mutation snapshot shared with the DevTools dogfood page.
    pub mutation_snapshot: Option<MutationSnapshot>,
}

/// Deterministic resource snapshots consumed by the DevTools dogfood page.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceDevtoolsDogfoodSnapshots {
    /// Redacted query snapshot after mutation-driven invalidation.
    pub resource: ResourceSnapshot,
    /// Redacted mutation snapshot after success.
    pub mutation: MutationSnapshot,
    /// Whether invalidation requested a refetch for the observed query.
    pub refetch_requested: bool,
}

/// Returns resource-adapter samples backed by deterministic resource snapshots.
pub fn resource_adapter_samples(tokens: ThemeTokens) -> Vec<ResourceAdapterSample> {
    vec![
        loading_resource_sample(tokens),
        empty_resource_sample(tokens),
        retry_resource_sample(tokens),
        refreshing_resource_sample(tokens),
        mutation_resource_sample(tokens),
    ]
}

/// Returns the deterministic resource snapshots consumed by the DevTools dogfood page.
pub fn resource_devtools_dogfood_snapshots() -> ResourceDevtoolsDogfoodSnapshots {
    let mut client = ResourceClient::default();
    let key = projects_key();
    let ticket = client.begin_fetch(&key).unwrap();
    assert!(client.complete_fetch_success(ticket, project_rows_json()));
    let _observer = client.subscribe(key.clone()).unwrap();
    let mutation_ticket = client
        .begin_mutation("save-project-token=gallery-secret")
        .unwrap();
    assert!(client.complete_mutation_success(
        mutation_ticket.clone(),
        Some(serde_json::json!({"saved": true, "token": "gallery-secret"})),
        [key.clone()],
    ));
    let invalidation = client.invalidate(&key).unwrap();

    ResourceDevtoolsDogfoodSnapshots {
        resource: client
            .snapshot(&key, ResourceRedactionPolicy::RedactAll)
            .unwrap(),
        mutation: client
            .mutation_snapshot(&mutation_ticket.id, ResourceRedactionPolicy::RedactAll)
            .unwrap(),
        refetch_requested: invalidation.refetch_requested,
    }
}

fn loading_resource_sample(tokens: ThemeTokens) -> ResourceAdapterSample {
    let mut client = ResourceClient::default();
    let key = projects_key();
    client.begin_fetch(&key).unwrap();
    let snapshot = client
        .snapshot(&key, ResourceRedactionPolicy::RedactAll)
        .unwrap();

    build_resource_adapter_sample(
        "loading",
        "Loading query",
        "Initial query loading projects into a virtualized status row and command loading state.",
        "loading",
        snapshot,
        0,
        Vec::new(),
        None,
        None,
        tokens,
    )
}

fn empty_resource_sample(tokens: ThemeTokens) -> ResourceAdapterSample {
    let mut client = ResourceClient::default();
    let key = projects_key();
    let ticket = client.begin_fetch(&key).unwrap();
    assert!(client.complete_fetch_success(ticket, serde_json::json!([])));
    let snapshot = client
        .snapshot(&key, ResourceRedactionPolicy::Expose)
        .unwrap();

    build_resource_adapter_sample(
        "empty",
        "Empty query",
        "Successful query with no rows maps to EmptyState and a virtualized empty row.",
        "empty",
        snapshot,
        0,
        Vec::new(),
        None,
        None,
        tokens,
    )
}

fn retry_resource_sample(tokens: ThemeTokens) -> ResourceAdapterSample {
    let mut client = ResourceClient::default();
    let key = projects_key();
    let ticket = client.begin_fetch(&key).unwrap();
    assert!(client.complete_fetch_error(ticket, "Gateway timeout"));
    let snapshot = client
        .snapshot(&key, ResourceRedactionPolicy::RedactAll)
        .unwrap();

    build_resource_adapter_sample(
        "retry",
        "Retry query",
        "Failed query projects retry state into VirtualizedList, Command, Table, and Tree adapters.",
        "retry",
        snapshot,
        0,
        Vec::new(),
        None,
        None,
        tokens,
    )
}

fn refreshing_resource_sample(tokens: ThemeTokens) -> ResourceAdapterSample {
    let mut client = ResourceClient::default();
    let key = projects_key();
    let ticket = client.begin_fetch(&key).unwrap();
    assert!(client.complete_fetch_success(ticket, project_rows_json()));
    let _observer = client.subscribe(key.clone()).unwrap();
    assert!(client.invalidate(&key).unwrap().refetch_requested);
    let snapshot = client
        .snapshot(&key, ResourceRedactionPolicy::Summarize)
        .unwrap();

    build_resource_adapter_sample(
        "refreshing",
        "Refreshing query",
        "Observed stale data keeps rows visible while background refetch maps to a quiet status cue.",
        "refreshing",
        snapshot,
        2,
        project_virtual_rows(),
        None,
        None,
        tokens,
    )
}

fn mutation_resource_sample(tokens: ThemeTokens) -> ResourceAdapterSample {
    let mut client = ResourceClient::default();
    let key = projects_key();
    let ticket = client.begin_fetch(&key).unwrap();
    assert!(client.complete_fetch_success(ticket, project_rows_json()));
    let mutation_ticket = client.begin_mutation("save-project").unwrap();
    let snapshot = client
        .snapshot(&key, ResourceRedactionPolicy::Summarize)
        .unwrap();
    let mutation = client
        .mutation_snapshot(&mutation_ticket.id, ResourceRedactionPolicy::RedactAll)
        .unwrap();
    let mutation_snapshot = mutation.clone();
    let mutation = Some(ResourceMutationProjection::resolve(
        &mutation,
        ResourceAdapterLabels::new("project"),
    ));

    build_resource_adapter_sample(
        "mutation",
        "Pending mutation",
        "Mutation pending state disables conflicting actions while query rows remain visible.",
        "mutation",
        snapshot,
        2,
        project_virtual_rows(),
        mutation,
        Some(mutation_snapshot),
        tokens,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_resource_adapter_sample(
    id: &'static str,
    title: &'static str,
    summary: &'static str,
    badge: &'static str,
    snapshot: open_gpui_resource::ResourceSnapshot,
    visible_item_count: usize,
    rows: Vec<VirtualizedListItemDescriptor>,
    mutation: Option<ResourceMutationProjection>,
    mutation_snapshot: Option<MutationSnapshot>,
    tokens: ThemeTokens,
) -> ResourceAdapterSample {
    let collection = ResourceCollectionProjection::resolve(
        &snapshot,
        visible_item_count,
        ResourceAdapterLabels::new("projects"),
    );
    let mut virtualized_items = Vec::new();
    if let Some(status_row) = collection.virtualized_status_row(format!("{id}-status")) {
        virtualized_items.push(status_row);
    } else {
        virtualized_items.extend(rows);
    }
    let status_cue = collection.status_cue_state(tokens);
    let empty_state = collection.empty_state(tokens);
    let command_loading_message = collection
        .command_loading_state()
        .map(|state| state.message().to_owned());
    let command_status_message = collection
        .command_status_item()
        .map(|item| item.message().to_owned())
        .or_else(|| {
            mutation
                .as_ref()
                .and_then(ResourceMutationProjection::command_status_item)
                .map(|item| item.message().to_owned())
        });
    let table_children_state = collection.table_children_load_state();
    let tree_children_state = collection.tree_children_load_state();

    ResourceAdapterSample {
        id,
        title,
        summary,
        badge,
        collection,
        mutation,
        status_cue,
        empty_state,
        virtualized_items: Arc::from(virtualized_items.into_boxed_slice()),
        command_loading_message,
        command_status_message,
        table_children_state,
        tree_children_state,
        snapshot,
        mutation_snapshot,
    }
}

fn projects_key() -> QueryKey {
    QueryKey::new(["projects"]).unwrap()
}

fn project_rows_json() -> serde_json::Value {
    serde_json::json!([
        {"id": "atlas", "name": "Atlas"},
        {"id": "beacon", "name": "Beacon"}
    ])
}

fn project_virtual_rows() -> Vec<VirtualizedListItemDescriptor> {
    vec![
        VirtualizedListItemDescriptor::item("atlas", "Atlas")
            .secondary_text("Ready")
            .badge("active"),
        VirtualizedListItemDescriptor::item("beacon", "Beacon")
            .secondary_text("Refreshing")
            .badge("stale"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_resource::{MutationStatus, ResourceStatus};

    #[test]
    fn resource_adapter_samples_cover_query_and_mutation_states() {
        let samples = resource_adapter_samples(ThemeTokens::default());
        let loading = samples
            .iter()
            .find(|sample| sample.id == "loading")
            .unwrap();
        let retry = samples.iter().find(|sample| sample.id == "retry").unwrap();
        let refreshing = samples
            .iter()
            .find(|sample| sample.id == "refreshing")
            .unwrap();
        let mutation = samples
            .iter()
            .find(|sample| sample.id == "mutation")
            .unwrap();

        assert_eq!(loading.collection.status(), &ResourceStatus::Loading);
        assert_eq!(
            loading.command_loading_message.as_deref(),
            Some("Loading projects")
        );
        assert!(loading.table_children_state.is_loading());

        assert!(retry.collection.retryable());
        assert!(matches!(
            retry.table_children_state,
            TableRowChildrenLoadState::Failed { .. }
        ));
        assert!(retry.tree_children_state.is_failed());
        assert_eq!(
            retry.virtualized_items[0].retry_action_label_ref(),
            Some("Retry")
        );

        assert!(refreshing.collection.refreshing());
        assert_eq!(refreshing.virtualized_items.len(), 2);

        let mutation = mutation.mutation.as_ref().unwrap();
        assert_eq!(mutation.status(), &MutationStatus::Pending);
        assert!(mutation.disables_actions());
    }
}
