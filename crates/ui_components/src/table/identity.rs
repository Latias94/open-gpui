use std::fmt::Write as _;

use open_gpui::ElementId;
use open_gpui_ui_core::{
    TableColumnId, TableColumnRegion, TableHeaderIdentity, TableResolvedHeaderIdentity,
    TableRowIdentity, TableRowIdentityKey, TableRowRegion, VirtualizerItemKey,
};

/// Typed builders for Table debug selectors used by diagnostics and integration tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableDebugSelector;

impl TableDebugSelector {
    /// Returns the selector for one row-pinning body band.
    pub fn body_region(table_id: &str, region: TableRowRegion) -> String {
        format!("table:{table_id}:body:{}", region.as_str())
    }

    /// Returns the selector for the vertically scrollable center body.
    pub fn body_scroll(table_id: &str) -> String {
        format!("table:{table_id}:body-scroll")
    }

    /// Returns the selector for one resolved row.
    pub fn row(table_id: &str, identity: &TableRowIdentity) -> String {
        Self::row_key(table_id, &identity.key())
    }

    /// Returns the selector for one resolved cell.
    pub fn cell(table_id: &str, identity: &TableRowIdentity, column_id: &TableColumnId) -> String {
        Self::cell_key(table_id, &identity.key(), column_id)
    }

    /// Returns the selector for one source-tree disclosure control.
    pub fn tree_toggle(table_id: &str, identity: &TableRowIdentity) -> String {
        Self::tree_toggle_key(table_id, &identity.key())
    }

    /// Returns the selector for one row's column-region lane.
    pub fn row_region(
        table_id: &str,
        identity: &TableRowIdentity,
        region: TableColumnRegion,
    ) -> String {
        Self::row_region_key(table_id, &identity.key(), region)
    }

    /// Returns the selector for one row's horizontally scrollable center lane.
    pub fn row_center_scroll(table_id: &str, identity: &TableRowIdentity) -> String {
        Self::row_center_scroll_key(table_id, &identity.key())
    }

    /// Returns the selector for one header column-region lane.
    pub fn header_region(table_id: &str, region: TableColumnRegion, depth: usize) -> String {
        let base = format!("table:{table_id}:header-region:{}", region.as_str());
        if depth == 0 {
            base
        } else {
            format!("{base}:row:{depth}")
        }
    }

    /// Returns the selector for one horizontally scrollable center header row.
    pub fn header_center_scroll(table_id: &str, depth: usize) -> String {
        let base = format!("table:{table_id}:header-center-scroll");
        if depth == 0 {
            base
        } else {
            format!("{base}:row:{depth}")
        }
    }

    /// Returns the component id shared by one cell editor and its nested control.
    pub fn cell_editor_id(
        table_id: &str,
        identity: &TableRowIdentity,
        column_id: &TableColumnId,
    ) -> String {
        Self::cell_editor_id_key(table_id, &identity.key(), column_id)
    }

    /// Returns the selector for one single-line cell editor root.
    pub fn text_input_editor_root(
        table_id: &str,
        identity: &TableRowIdentity,
        column_id: &TableColumnId,
    ) -> String {
        format!(
            "text-input:{}:root",
            Self::cell_editor_id(table_id, identity, column_id)
        )
    }

    /// Returns the selector for one multiline cell editor root.
    pub fn textarea_editor_root(
        table_id: &str,
        identity: &TableRowIdentity,
        column_id: &TableColumnId,
    ) -> String {
        format!(
            "textarea:{}:root",
            Self::cell_editor_id(table_id, identity, column_id)
        )
    }

    /// Returns the selector for one checkbox cell editor root.
    pub fn checkbox_editor_root(
        table_id: &str,
        identity: &TableRowIdentity,
        column_id: &TableColumnId,
    ) -> String {
        format!(
            "checkbox:{}:root",
            Self::cell_editor_id(table_id, identity, column_id)
        )
    }

    /// Returns the selector for one select cell editor trigger.
    pub fn select_editor_trigger(
        table_id: &str,
        identity: &TableRowIdentity,
        column_id: &TableColumnId,
    ) -> String {
        format!(
            "select:{}:trigger",
            Self::cell_editor_id(table_id, identity, column_id)
        )
    }

    /// Returns the selector for one resolved header fragment.
    pub fn header(table_id: &str, identity: &TableResolvedHeaderIdentity) -> String {
        table_header_debug_selector(table_id, identity)
    }

    pub(super) fn row_key(table_id: &str, key: &TableRowIdentityKey) -> String {
        format!("table:{table_id}:row:{}", key.as_str())
    }

    pub(super) fn cell_key(
        table_id: &str,
        key: &TableRowIdentityKey,
        column_id: &TableColumnId,
    ) -> String {
        format!(
            "table:{table_id}:cell:{}:{}",
            key.as_str(),
            column_id.as_str()
        )
    }

    pub(super) fn tree_toggle_key(table_id: &str, key: &TableRowIdentityKey) -> String {
        format!("table:{table_id}:tree-toggle:{}", key.as_str())
    }

    pub(super) fn row_region_key(
        table_id: &str,
        key: &TableRowIdentityKey,
        region: TableColumnRegion,
    ) -> String {
        format!(
            "table:{table_id}:row-region:{}:{}",
            key.as_str(),
            region.as_str()
        )
    }

    pub(super) fn row_center_scroll_key(table_id: &str, key: &TableRowIdentityKey) -> String {
        format!("table:{table_id}:row-center-scroll:{}", key.as_str())
    }

    pub(super) fn cell_editor_id_key(
        table_id: &str,
        key: &TableRowIdentityKey,
        column_id: &TableColumnId,
    ) -> String {
        format!(
            "table:{table_id}:cell:{}:{}:editor",
            key.as_str(),
            column_id.as_str()
        )
    }
}

pub(super) fn table_row_virtualizer_key_from_key(
    identity: &TableRowIdentityKey,
) -> VirtualizerItemKey {
    VirtualizerItemKey::new(identity.as_str().to_owned())
}

pub(super) fn table_row_element_id(table_id: &str, identity: &TableRowIdentityKey) -> ElementId {
    ElementId::from(TableDebugSelector::row_key(table_id, identity))
}

pub(super) fn table_cell_element_id(
    table_id: &str,
    row: &TableRowIdentityKey,
    column: &TableColumnId,
) -> ElementId {
    ElementId::from(TableDebugSelector::cell_key(table_id, row, column))
}

pub(super) fn table_tree_toggle_element_id(table_id: &str, row: &TableRowIdentityKey) -> ElementId {
    ElementId::from(TableDebugSelector::tree_toggle_key(table_id, row))
}

pub(super) fn table_header_element_id(
    table_id: &str,
    identity: &TableResolvedHeaderIdentity,
) -> ElementId {
    ElementId::from(format!(
        "table:{table_id}:header-cell:{}",
        encode_table_header_identity(identity)
    ))
}

pub(super) fn table_header_debug_selector(
    table_id: &str,
    identity: &TableResolvedHeaderIdentity,
) -> String {
    match identity.logical() {
        TableHeaderIdentity::Leaf(column) => {
            format!("table:{table_id}:header:{}", column.as_str())
        }
        TableHeaderIdentity::Group(_) | TableHeaderIdentity::Placeholder { .. } => format!(
            "table:{table_id}:header-cell:{}",
            encode_table_header_identity(identity)
        ),
    }
}

fn encode_table_header_identity(identity: &TableResolvedHeaderIdentity) -> String {
    let mut encoded = String::from("th1");
    match identity.logical() {
        TableHeaderIdentity::Leaf(column) => {
            encoded.push('l');
            push_text(&mut encoded, column.as_str());
        }
        TableHeaderIdentity::Group(path) => {
            encoded.push('g');
            let _ = write!(encoded, "{};", path.len());
            for group in path {
                push_text(&mut encoded, group.as_str());
            }
        }
        TableHeaderIdentity::Placeholder { leaf_column, depth } => {
            encoded.push('p');
            let _ = write!(encoded, "{depth};");
            push_text(&mut encoded, leaf_column.as_str());
        }
    }
    encoded.push('f');
    let _ = write!(encoded, "{};", identity.covered_leaves().len());
    for leaf in identity.covered_leaves() {
        push_text(&mut encoded, leaf.as_str());
    }
    encoded
}

fn push_text(encoded: &mut String, value: &str) {
    let _ = write!(encoded, "{}:", value.len());
    encoded.push_str(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::{
        TableGroupRowIdentity, TableGroupValueIdentity, TableRow, TableRowIdentity, TableState,
    };

    fn duplicate_identity(occurrence: usize) -> TableRowIdentity {
        let state = TableState::new([TableRow::new("duplicate"), TableRow::new("duplicate")]);
        TableRowIdentity::Source(
            state
                .source_row_identity_at("duplicate", occurrence)
                .expect("duplicate occurrence should resolve"),
        )
    }

    #[test]
    fn table_row_codec_keeps_namespaces_and_fallbacks_disjoint() {
        let duplicate = duplicate_identity(0);
        let legal_source = TableRowIdentity::source(duplicate.key().as_str().to_owned());
        let group = TableRowIdentity::group(TableGroupRowIdentity::new("team", "ops"));

        let keys = [duplicate, legal_source, group]
            .iter()
            .map(|identity| identity.key().as_str().to_owned())
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn debug_selectors_share_the_private_typed_codec() {
        let duplicate = duplicate_identity(1);
        let legal_source = TableRowIdentity::source(duplicate.key().as_str().to_owned());

        assert_ne!(
            TableDebugSelector::row("table", &duplicate),
            TableDebugSelector::row("table", &legal_source)
        );
        assert_eq!(
            TableDebugSelector::cell("table", &duplicate, &TableColumnId::new("name")),
            format!("table:table:cell:{}:name", duplicate.key().as_str())
        );
    }

    #[test]
    fn group_codec_preserves_types_and_normalizes_numeric_edge_cases() {
        let identity = |value| TableRowIdentity::group(TableGroupRowIdentity::new("value", value));
        let typed_keys = [
            identity(TableGroupValueIdentity::Empty),
            identity(TableGroupValueIdentity::Text(String::new())),
            identity(TableGroupValueIdentity::from(0.0)),
            identity(TableGroupValueIdentity::Bool(false)),
        ]
        .iter()
        .map(|identity| identity.key().as_str().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(typed_keys.len(), 4);

        assert_eq!(
            identity(TableGroupValueIdentity::from(f64::NAN)).key(),
            identity(TableGroupValueIdentity::from(f64::from_bits(
                0x7ff8_0000_0000_0001
            )))
            .key()
        );
        assert_eq!(
            identity(TableGroupValueIdentity::from(0.0)).key(),
            identity(TableGroupValueIdentity::from(-0.0)).key()
        );
    }

    #[test]
    fn row_cell_tree_and_header_ids_use_flat_names() {
        let state = TableState::new([TableRow::new("row")]);
        let resolved = state.resolve();
        let row = &resolved.final_model().rows()[0];
        assert_eq!(row.identity_key(), &row.identity().key());

        let ids = [
            table_row_element_id("table", row.identity_key()),
            table_cell_element_id("table", row.identity_key(), &TableColumnId::new("name")),
            table_tree_toggle_element_id("table", row.identity_key()),
            table_header_element_id("table", &TableResolvedHeaderIdentity::leaf("name")),
        ];
        assert!(
            ids.into_iter()
                .all(|identity| matches!(identity, ElementId::Name(_)))
        );
    }
}
