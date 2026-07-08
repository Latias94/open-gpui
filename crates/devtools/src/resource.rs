//! DevTools adapters for `open-gpui-resource` snapshots.

use open_gpui_resource::{
    MutationSnapshot, PaginatedResourceSnapshotView, QueryKey, QueryKeySegment,
    RedactedResourceValue, ResourceSnapshot,
};

use crate::{
    ProbeId, ProbeSnapshotError, SnapshotEnvelope, SnapshotKind, SnapshotNode, SnapshotProbe,
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::{sanitize_sensitive_text, snapshot_node_with_payload, summary_payload},
};

/// Converts resource, mutation, and paginated snapshots into one DevTools snapshot.
pub fn resource_probe_snapshot<'a, R, M, P>(
    resources: R,
    mutations: M,
    paginated: P,
) -> SnapshotProbeSnapshot
where
    R: IntoIterator<Item = &'a ResourceSnapshot>,
    M: IntoIterator<Item = &'a MutationSnapshot>,
    P: IntoIterator<Item = &'a PaginatedResourceSnapshotView>,
{
    let (tree, redaction) = resource_tree_and_redaction(resources, mutations, paginated);
    SnapshotProbeSnapshot::new(tree).with_redaction(redaction)
}

/// Converts resource, mutation, and paginated snapshots into a DevTools envelope.
pub fn resource_snapshot_envelope<'a, R, M, P>(
    probe_id: ProbeId,
    resources: R,
    mutations: M,
    paginated: P,
) -> SnapshotEnvelope
where
    R: IntoIterator<Item = &'a ResourceSnapshot>,
    M: IntoIterator<Item = &'a MutationSnapshot>,
    P: IntoIterator<Item = &'a PaginatedResourceSnapshotView>,
{
    let (tree, redaction) = resource_tree_and_redaction(resources, mutations, paginated);
    SnapshotEnvelope::new(probe_id, SnapshotKind::Resource, tree).with_redaction(redaction)
}

/// Builds a closure-backed resource snapshot probe.
pub fn resource_snapshot_probe<R, M, P>(
    id: impl Into<String>,
    resources: R,
    mutations: M,
    paginated: P,
) -> Result<
    SnapshotProbe<impl Fn() -> Result<SnapshotProbeSnapshot, ProbeSnapshotError> + Send + Sync>,
    ProbeSnapshotError,
>
where
    R: Fn() -> Vec<ResourceSnapshot> + Send + Sync + 'static,
    M: Fn() -> Vec<MutationSnapshot> + Send + Sync + 'static,
    P: Fn() -> Vec<PaginatedResourceSnapshotView> + Send + Sync + 'static,
{
    SnapshotProbe::new(id, SnapshotKind::Resource, move || {
        let resources = resources();
        let mutations = mutations();
        let paginated = paginated();
        Ok(resource_probe_snapshot(
            resources.iter(),
            mutations.iter(),
            paginated.iter(),
        ))
    })
}

/// Converts one resource snapshot into a DevTools node.
pub fn resource_snapshot_node(
    snapshot: &ResourceSnapshot,
    redaction: &mut SnapshotRedactionSummary,
) -> SnapshotNode {
    let key_label = query_key_label(&snapshot.key);
    snapshot_node_with_payload(
        ["resource", "query", key_label.as_str()],
        format!("Query {key_label}"),
        serde_json::json!({
            "key": query_key_payload(&snapshot.key),
            "status": &snapshot.status,
            "data": resource_value_payload(
                snapshot.data.as_ref(),
                format!("resource data {key_label}"),
                redaction,
            ),
            "error": snapshot.error.as_deref().map(sanitize_sensitive_text),
            "observer_count": snapshot.observer_count,
            "fetch_attempts": snapshot.fetch_attempts,
        }),
    )
}

/// Converts one mutation snapshot into a DevTools node.
pub fn mutation_snapshot_node(
    snapshot: &MutationSnapshot,
    redaction: &mut SnapshotRedactionSummary,
) -> SnapshotNode {
    let mutation_id = sanitize_sensitive_text(&snapshot.id);
    snapshot_node_with_payload(
        ["resource", "mutation", mutation_id.as_str()],
        format!("Mutation {mutation_id}"),
        serde_json::json!({
            "id": mutation_id,
            "status": &snapshot.status,
            "data": resource_value_payload(
                snapshot.data.as_ref(),
                format!("mutation data {}", snapshot.id),
                redaction,
            ),
            "error": snapshot.error.as_deref().map(sanitize_sensitive_text),
        }),
    )
}

/// Converts a paginated resource snapshot into a DevTools node.
pub fn paginated_resource_snapshot_node(
    snapshot: &PaginatedResourceSnapshotView,
    redaction: &mut SnapshotRedactionSummary,
) -> SnapshotNode {
    let key_label = query_key_label(&snapshot.key);
    let mut node = snapshot_node_with_payload(
        ["resource", "pages", key_label.as_str()],
        format!("Pages {key_label}"),
        serde_json::json!({
            "key": query_key_payload(&snapshot.key),
            "page_count": snapshot.pages.len(),
        }),
    );

    for (page_index, page) in snapshot.pages.iter().enumerate() {
        let mut page_node = snapshot_node_with_payload(
            [
                "resource",
                "pages",
                key_label.as_str(),
                &page_index.to_string(),
            ],
            format!("Page {page_index}"),
            serde_json::json!({
                "page_index": page_index,
                "cursor": page.cursor.as_deref().map(sanitize_sensitive_text),
                "item_count": page.items.len(),
            }),
        );

        for (item_index, item) in page.items.iter().enumerate() {
            page_node = page_node.with_child(snapshot_node_with_payload(
                [
                    "resource",
                    "pages",
                    key_label.as_str(),
                    &page_index.to_string(),
                    &item_index.to_string(),
                ],
                format!("Item {item_index}"),
                resource_value_payload(
                    Some(item),
                    format!("page {page_index} item {item_index} {key_label}"),
                    redaction,
                ),
            ));
        }

        node = node.with_child(page_node);
    }

    node
}

/// Returns a sanitized, stable label for a query key.
pub fn query_key_label(key: &QueryKey) -> String {
    key.segments()
        .iter()
        .map(query_key_segment_label)
        .collect::<Vec<_>>()
        .join("/")
}

fn resource_tree_and_redaction<'a, R, M, P>(
    resources: R,
    mutations: M,
    paginated: P,
) -> (SnapshotTree, SnapshotRedactionSummary)
where
    R: IntoIterator<Item = &'a ResourceSnapshot>,
    M: IntoIterator<Item = &'a MutationSnapshot>,
    P: IntoIterator<Item = &'a PaginatedResourceSnapshotView>,
{
    let resources = resources.into_iter().collect::<Vec<_>>();
    let mutations = mutations.into_iter().collect::<Vec<_>>();
    let paginated = paginated.into_iter().collect::<Vec<_>>();
    let mut redaction = SnapshotRedactionSummary::default();
    let mut root = snapshot_node_with_payload(
        ["resource"],
        "Resources",
        serde_json::json!({
            "resource_count": resources.len(),
            "mutation_count": mutations.len(),
            "paginated_count": paginated.len(),
        }),
    );

    for snapshot in resources {
        root = root.with_child(resource_snapshot_node(snapshot, &mut redaction));
    }
    for snapshot in mutations {
        root = root.with_child(mutation_snapshot_node(snapshot, &mut redaction));
    }
    for snapshot in paginated {
        root = root.with_child(paginated_resource_snapshot_node(snapshot, &mut redaction));
    }

    (SnapshotTree::new([root]), redaction)
}

fn resource_value_payload(
    value: Option<&RedactedResourceValue>,
    note: impl Into<String>,
    redaction: &mut SnapshotRedactionSummary,
) -> serde_json::Value {
    match value {
        Some(RedactedResourceValue::Redacted) => {
            redaction.record_redacted(note.into());
            serde_json::json!({ "kind": "redacted" })
        }
        Some(RedactedResourceValue::Summary(summary)) => summary_payload(serde_json::json!({
            "kind": "summary",
            "summary": summary,
        })),
        Some(RedactedResourceValue::Json(value)) => serde_json::json!({
            "kind": "json",
            "value": value,
        }),
        None => serde_json::json!({ "kind": "none" }),
    }
}

fn query_key_payload(key: &QueryKey) -> Vec<serde_json::Value> {
    key.segments()
        .iter()
        .map(|segment| match segment {
            QueryKeySegment::Text(value) => serde_json::json!({
                "kind": "text",
                "value": sanitize_sensitive_text(value),
            }),
            QueryKeySegment::Integer(value) => serde_json::json!({
                "kind": "integer",
                "value": value,
            }),
            QueryKeySegment::Bool(value) => serde_json::json!({
                "kind": "bool",
                "value": value,
            }),
        })
        .collect()
}

fn query_key_segment_label(segment: &QueryKeySegment) -> String {
    match segment {
        QueryKeySegment::Text(value) => sanitize_sensitive_text(value),
        QueryKeySegment::Integer(value) => value.to_string(),
        QueryKeySegment::Bool(value) => value.to_string(),
    }
}
