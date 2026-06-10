use crate::document::CanvasRecordId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CanvasZOrderCommand {
    BringToFront,
    BringForward,
    SendBackward,
    SendToBack,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CanvasLayerRecord {
    pub id: CanvasRecordId,
    pub z_index: i32,
    pub ordinal: usize,
    pub selected: bool,
}

pub(crate) fn sort_layer_records(records: &mut [CanvasLayerRecord]) {
    records.sort_by(|left, right| {
        left.z_index
            .cmp(&right.z_index)
            .then_with(|| left.ordinal.cmp(&right.ordinal))
    });
}

pub(crate) fn reorder_layer_z_indices(
    records: &mut Vec<CanvasLayerRecord>,
    command: CanvasZOrderCommand,
) -> Vec<(CanvasRecordId, i32)> {
    if !records.iter().any(|record| record.selected) {
        return Vec::new();
    }

    let original_order = records
        .iter()
        .map(|record| record.id.clone())
        .collect::<Vec<_>>();
    reorder_layer_records(records, command);
    if records
        .iter()
        .map(|record| &record.id)
        .eq(original_order.iter())
    {
        return Vec::new();
    }

    layer_z_index_updates(records)
}

fn reorder_layer_records(records: &mut Vec<CanvasLayerRecord>, command: CanvasZOrderCommand) {
    match command {
        CanvasZOrderCommand::BringToFront => {
            let mut selected = Vec::new();
            records.retain(|record| {
                if record.selected {
                    selected.push(record.clone());
                    false
                } else {
                    true
                }
            });
            records.extend(selected);
        }
        CanvasZOrderCommand::SendToBack => {
            let mut selected = Vec::new();
            records.retain(|record| {
                if record.selected {
                    selected.push(record.clone());
                    false
                } else {
                    true
                }
            });
            selected.extend(records.iter().cloned());
            *records = selected;
        }
        CanvasZOrderCommand::BringForward => {
            let mut index = records.len();
            while index > 0 {
                index -= 1;
                if !records[index].selected {
                    continue;
                }
                let next = index + 1;
                if next < records.len() && !records[next].selected {
                    records.swap(index, next);
                }
            }
        }
        CanvasZOrderCommand::SendBackward => {
            for index in 0..records.len() {
                if !records[index].selected {
                    continue;
                }
                if index > 0 && !records[index - 1].selected {
                    records.swap(index - 1, index);
                }
            }
        }
    }
}

fn layer_z_index_updates(records: &[CanvasLayerRecord]) -> Vec<(CanvasRecordId, i32)> {
    let min_z = records
        .iter()
        .map(|record| record.z_index)
        .min()
        .unwrap_or_default();
    let base_z = normalized_layer_z_order_base(min_z, records.len());

    records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            let z_index = normalized_layer_z_index(base_z, index);
            (z_index != record.z_index).then(|| (record.id.clone(), z_index))
        })
        .collect()
}

fn normalized_layer_z_order_base(min_z: i32, record_count: usize) -> i32 {
    let max_offset = record_count.saturating_sub(1).min(i32::MAX as usize) as i32;
    if min_z > i32::MAX.saturating_sub(max_offset) {
        i32::MAX - max_offset
    } else {
        min_z
    }
}

fn normalized_layer_z_index(base_z: i32, index: usize) -> i32 {
    let offset = index.min(i32::MAX as usize) as i32;
    base_z.saturating_add(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeId, NodeId, ShapeId};

    #[test]
    fn bring_forward_crosses_sparse_adjacent_layer() {
        let mut records = vec![node("back", 1, 0, true), shape("front", 10, 1, false)];
        sort_layer_records(&mut records);

        let updates = reorder_layer_z_indices(&mut records, CanvasZOrderCommand::BringForward);

        assert_order(&records, ["shape:front", "node:back"]);
        assert_eq!(
            updates,
            vec![
                (CanvasRecordId::Shape(ShapeId::from("front")), 1),
                (CanvasRecordId::Node(NodeId::from("back")), 2),
            ]
        );
    }

    #[test]
    fn send_backward_crosses_duplicate_adjacent_layer() {
        let mut records = vec![node("back", 0, 0, false), shape("front", 0, 1, true)];
        sort_layer_records(&mut records);

        let updates = reorder_layer_z_indices(&mut records, CanvasZOrderCommand::SendBackward);

        assert_order(&records, ["shape:front", "node:back"]);
        assert_eq!(
            updates,
            vec![(CanvasRecordId::Node(NodeId::from("back")), 1)]
        );
    }

    #[test]
    fn bring_to_front_preserves_selected_relative_order() {
        let mut records = vec![
            node("node", 0, 0, true),
            shape("shape", 1, 1, false),
            edge("edge", 2, 2, true),
            shape("top", 3, 3, false),
        ];
        sort_layer_records(&mut records);

        let updates = reorder_layer_z_indices(&mut records, CanvasZOrderCommand::BringToFront);

        assert_order(
            &records,
            ["shape:shape", "shape:top", "node:node", "edge:edge"],
        );
        assert_eq!(
            updates,
            vec![
                (CanvasRecordId::Shape(ShapeId::from("shape")), 0),
                (CanvasRecordId::Shape(ShapeId::from("top")), 1),
                (CanvasRecordId::Node(NodeId::from("node")), 2),
                (CanvasRecordId::Edge(EdgeId::from("edge")), 3),
            ]
        );
    }

    fn node(id: &str, z_index: i32, ordinal: usize, selected: bool) -> CanvasLayerRecord {
        CanvasLayerRecord {
            id: CanvasRecordId::Node(NodeId::from(id)),
            z_index,
            ordinal,
            selected,
        }
    }

    fn edge(id: &str, z_index: i32, ordinal: usize, selected: bool) -> CanvasLayerRecord {
        CanvasLayerRecord {
            id: CanvasRecordId::Edge(EdgeId::from(id)),
            z_index,
            ordinal,
            selected,
        }
    }

    fn shape(id: &str, z_index: i32, ordinal: usize, selected: bool) -> CanvasLayerRecord {
        CanvasLayerRecord {
            id: CanvasRecordId::Shape(ShapeId::from(id)),
            z_index,
            ordinal,
            selected,
        }
    }

    fn assert_order<const N: usize>(records: &[CanvasLayerRecord], expected: [&str; N]) {
        let actual = records
            .iter()
            .map(|record| record.id.to_string())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
