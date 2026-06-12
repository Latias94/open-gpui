use crate::{
    CanvasDocument, CanvasEndpoint, CanvasGeometryFacts, CanvasRecordId, CanvasSelection,
    CanvasShape, CanvasTransaction, DocumentCommand, ShapeId,
};
use indexmap::IndexSet;
use open_gpui::{Bounds, Pixels, px};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasGroupEdit {
    pub transaction: CanvasTransaction,
    pub selection: CanvasSelection,
}

pub(crate) fn group_selection_edit(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    group_id: ShapeId,
) -> Option<CanvasGroupEdit> {
    if document.contains_shape(&group_id) {
        return None;
    }

    let group_record = CanvasRecordId::Shape(group_id.clone());
    let members = groupable_selection_records(document, selection, &group_record);
    let node_shape_members = members
        .iter()
        .filter(|record_id| is_node_or_shape(record_id))
        .count();
    if node_shape_members < 2 {
        return None;
    }

    let Some(bounds) = group_bounds(
        document,
        members
            .iter()
            .filter(|record_id| is_node_or_shape(record_id)),
    ) else {
        return None;
    };

    let mut group = CanvasShape::new(group_id, bounds);
    group.kind = "group".to_string();
    group.style.fill = Some("#00000000".to_string());
    group.style.stroke = Some("#8c959f".to_string());
    group.style.stroke_width = px(1.0);

    let mut commands = vec![DocumentCommand::InsertShape(group)];
    for member in members {
        commands.push(DocumentCommand::SetRecordParent {
            child: member.clone(),
            parent: group_record.clone(),
        });
        commands.push(DocumentCommand::AddRecordToGroup {
            group: group_record.clone(),
            member,
        });
    }

    let mut next_selection = CanvasSelection::default();
    next_selection.insert_shape(match group_record {
        CanvasRecordId::Shape(id) => id,
        CanvasRecordId::Node(_) | CanvasRecordId::Edge(_) => unreachable!(),
    });

    Some(CanvasGroupEdit {
        transaction: CanvasTransaction::new(commands),
        selection: next_selection,
    })
}

pub(crate) fn ungroup_selection_edit(
    document: &CanvasDocument,
    selection: &CanvasSelection,
) -> Option<CanvasGroupEdit> {
    let group_records = selected_group_records(document, selection);
    if group_records.is_empty() {
        return None;
    }

    let mut commands = Vec::new();
    let mut next_selection = CanvasSelection::default();
    for group in group_records {
        for child in document.relations().children_of(&group) {
            commands.push(DocumentCommand::ClearRecordParent {
                child: child.clone(),
            });
            select_record(&mut next_selection, child);
        }
        for member in document.relations().members_of(&group) {
            commands.push(DocumentCommand::RemoveRecordFromGroup {
                group: group.clone(),
                member: member.clone(),
            });
            select_record(&mut next_selection, member);
        }
        if let CanvasRecordId::Shape(id) = group {
            commands.push(DocumentCommand::RemoveShape(id));
        }
    }

    Some(CanvasGroupEdit {
        transaction: CanvasTransaction::new(commands),
        selection: next_selection,
    })
}

fn groupable_selection_records(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    group_record: &CanvasRecordId,
) -> Vec<CanvasRecordId> {
    let mut records = IndexSet::new();

    for record_id in directly_selected_record_ids(selection) {
        if can_group_record(document, &record_id, group_record) {
            records.insert(record_id);
        }
    }

    let selected_nodes = records
        .iter()
        .filter_map(|record_id| match record_id {
            CanvasRecordId::Node(id) => Some(id.clone()),
            CanvasRecordId::Edge(_) | CanvasRecordId::Shape(_) => None,
        })
        .collect::<IndexSet<_>>();

    for edge in document
        .edges()
        .filter(|edge| endpoint_is_selected(&edge.source, &selected_nodes))
        .filter(|edge| endpoint_is_selected(&edge.target, &selected_nodes))
    {
        let record_id = CanvasRecordId::Edge(edge.id.clone());
        if can_group_record(document, &record_id, group_record) {
            records.insert(record_id);
        }
    }

    records.into_iter().collect()
}

fn directly_selected_record_ids(
    selection: &CanvasSelection,
) -> impl Iterator<Item = CanvasRecordId> + '_ {
    selection
        .selected_nodes()
        .cloned()
        .map(CanvasRecordId::Node)
        .chain(
            selection
                .selected_edges()
                .cloned()
                .map(CanvasRecordId::Edge),
        )
        .chain(
            selection
                .selected_shapes()
                .cloned()
                .map(CanvasRecordId::Shape),
        )
}

fn endpoint_is_selected(
    endpoint: &CanvasEndpoint,
    selected_nodes: &IndexSet<crate::NodeId>,
) -> bool {
    selected_nodes.contains(&endpoint.node_id)
}

fn can_group_record(
    document: &CanvasDocument,
    record_id: &CanvasRecordId,
    group_record: &CanvasRecordId,
) -> bool {
    record_exists(document, record_id)
        && record_id != group_record
        && !record_is_locked(document, record_id)
        && !record_is_hidden(document, record_id)
        && !record_would_contain_group(document, record_id, group_record)
}

fn selected_group_records(
    document: &CanvasDocument,
    selection: &CanvasSelection,
) -> Vec<CanvasRecordId> {
    selection
        .selected_shapes()
        .filter_map(|id| {
            let shape = document.shape(id)?;
            (shape.kind == "group").then(|| CanvasRecordId::Shape(id.clone()))
        })
        .collect()
}

fn group_bounds<'a>(
    document: &CanvasDocument,
    members: impl IntoIterator<Item = &'a CanvasRecordId>,
) -> Option<Bounds<Pixels>> {
    CanvasGeometryFacts::new(document).node_shape_bounds_for_records(members)
}

fn is_node_or_shape(record_id: &CanvasRecordId) -> bool {
    matches!(
        record_id,
        CanvasRecordId::Node(_) | CanvasRecordId::Shape(_)
    )
}

fn select_record(selection: &mut CanvasSelection, record_id: &CanvasRecordId) {
    match record_id {
        CanvasRecordId::Node(id) => {
            selection.insert_node(id.clone());
        }
        CanvasRecordId::Edge(id) => {
            selection.insert_edge(id.clone());
        }
        CanvasRecordId::Shape(id) => {
            selection.insert_shape(id.clone());
        }
    }
}

fn record_exists(document: &CanvasDocument, record_id: &CanvasRecordId) -> bool {
    match record_id {
        CanvasRecordId::Node(id) => document.contains_node(id),
        CanvasRecordId::Edge(id) => document.contains_edge(id),
        CanvasRecordId::Shape(id) => document.contains_shape(id),
    }
}

fn record_is_locked(document: &CanvasDocument, record_id: &CanvasRecordId) -> bool {
    match record_id {
        CanvasRecordId::Node(id) => document.node(id).is_some_and(|node| node.locked),
        CanvasRecordId::Edge(id) => document.edge(id).is_some_and(|edge| edge.locked),
        CanvasRecordId::Shape(id) => document.shape(id).is_some_and(|shape| shape.locked),
    }
}

fn record_is_hidden(document: &CanvasDocument, record_id: &CanvasRecordId) -> bool {
    match record_id {
        CanvasRecordId::Node(id) => document.node(id).is_some_and(|node| node.hidden),
        CanvasRecordId::Edge(id) => document.edge(id).is_some_and(|edge| edge.hidden),
        CanvasRecordId::Shape(id) => document.shape(id).is_some_and(|shape| shape.hidden),
    }
}

fn record_would_contain_group(
    document: &CanvasDocument,
    record_id: &CanvasRecordId,
    group_record: &CanvasRecordId,
) -> bool {
    let mut pending = vec![record_id.clone()];
    let mut visited = IndexSet::new();

    while let Some(current) = pending.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }
        if &current == group_record {
            return true;
        }
        pending.extend(document.relations().children_of(&current).cloned());
        pending.extend(document.relations().members_of(&current).cloned());
    }

    false
}
