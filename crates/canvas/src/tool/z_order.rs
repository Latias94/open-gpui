use crate::layer::{
    CanvasLayerRecord, CanvasZOrderCommand, reorder_layer_z_indices, sort_layer_records,
};
use crate::record_scope::{CanvasRecordScopeOptions, collect_selection_record_scope};
use crate::{CanvasDocument, CanvasRecordId, CanvasSelection, CanvasTransaction, DocumentCommand};
use indexmap::IndexSet;

pub(crate) fn reorder_selection_transaction(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    command: CanvasZOrderCommand,
) -> CanvasTransaction {
    let selection_records = z_order_selection_record_ids(document, selection);
    let mut records = z_order_records(document, &selection_records);
    let commands = reorder_layer_z_indices(&mut records, command)
        .into_iter()
        .filter_map(|(record_id, z_index)| z_order_update_command(document, &record_id, z_index))
        .collect::<Vec<_>>();

    CanvasTransaction::new(commands)
}

fn z_order_selection_record_ids(
    document: &CanvasDocument,
    selection: &CanvasSelection,
) -> IndexSet<CanvasRecordId> {
    collect_selection_record_scope(
        document,
        selection,
        CanvasRecordScopeOptions::structural_with_internal_edges(),
        |record_id| is_reorderable_record(document, record_id),
    )
}

fn is_reorderable_record(document: &CanvasDocument, record_id: &CanvasRecordId) -> bool {
    match record_id {
        CanvasRecordId::Node(id) => document.node(id).is_some_and(|node| !node.locked),
        CanvasRecordId::Edge(id) => document.edge(id).is_some_and(|edge| !edge.locked),
        CanvasRecordId::Shape(id) => document.shape(id).is_some_and(|shape| !shape.locked),
    }
}

fn z_order_records(
    document: &CanvasDocument,
    selection_records: &IndexSet<CanvasRecordId>,
) -> Vec<CanvasLayerRecord> {
    let mut ordinal = 0;
    let mut records = Vec::new();

    records.extend(document.nodes().map(|node| {
        let record = CanvasLayerRecord {
            id: CanvasRecordId::Node(node.id.clone()),
            z_index: node.z_index,
            ordinal,
            selected: selection_records.contains(&CanvasRecordId::Node(node.id.clone())),
        };
        ordinal += 1;
        record
    }));
    records.extend(document.shapes().map(|shape| {
        let record = CanvasLayerRecord {
            id: CanvasRecordId::Shape(shape.id.clone()),
            z_index: shape.z_index,
            ordinal,
            selected: selection_records.contains(&CanvasRecordId::Shape(shape.id.clone())),
        };
        ordinal += 1;
        record
    }));
    records.extend(document.edges().map(|edge| {
        let record = CanvasLayerRecord {
            id: CanvasRecordId::Edge(edge.id.clone()),
            z_index: edge.z_index,
            ordinal,
            selected: selection_records.contains(&CanvasRecordId::Edge(edge.id.clone())),
        };
        ordinal += 1;
        record
    }));

    sort_layer_records(&mut records);
    records
}

fn z_order_update_command(
    document: &CanvasDocument,
    record_id: &CanvasRecordId,
    z_index: i32,
) -> Option<DocumentCommand> {
    match record_id {
        CanvasRecordId::Node(id) => {
            let mut node = document.node(id)?.clone();
            node.z_index = z_index;
            Some(DocumentCommand::UpdateNode(node))
        }
        CanvasRecordId::Edge(id) => {
            let mut edge = document.edge(id)?.clone();
            edge.z_index = z_index;
            Some(DocumentCommand::UpdateEdge(edge))
        }
        CanvasRecordId::Shape(id) => {
            let mut shape = document.shape(id)?.clone();
            shape.z_index = z_index;
            Some(DocumentCommand::UpdateShape(shape))
        }
    }
}
