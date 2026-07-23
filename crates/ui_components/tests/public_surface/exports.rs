use open_gpui_ui_components::{self as root, common, prelude};
use open_gpui_ui_core::{TableRowIdentity, TableState, UiPx};

#[test]
fn common_extended_diagnostic_and_adapter_paths_compile() {
    #[allow(unused_imports)]
    use root::gpui_adapter::{
        FieldControl as _, UiA11yElementExt as _, VirtualizedListGpuiExt as _,
    };

    let _root_button = root::Button::new("root-button", "Root button");
    let _common_button = common::Button::new("common-button", "Common button");
    let _prelude_button = prelude::Button::new("prelude-button", "Prelude button");
    let _root_table = root::Table::new("root-table", "Root table", TableState::new([]));
    let _common_table = common::Table::new("common-table", "Common table", TableState::new([]));
    let _prelude_table = prelude::Table::new("prelude-table", "Prelude table", TableState::new([]));
    let _root_restore =
        root::TableVirtualizerSnapshot::new([root::TableVirtualizerSnapshotItem::new(
            TableRowIdentity::source("row"),
            UiPx::ZERO,
        )]);
    let _prelude_restore =
        prelude::TableVirtualizerSnapshot::new([prelude::TableVirtualizerSnapshotItem::new(
            TableRowIdentity::source("row"),
            UiPx::ZERO,
        )]);
    let _root_materialization: Option<root::VirtualizedListMaterializationResult> = None;
    let _common_materialization: Option<common::VirtualizedListMaterializationTarget> = None;
    let _prelude_materialization: Option<prelude::VirtualizedListMaterializationResult> = None;

    let _extended_filter = root::TableGlobalFilter::new("filter", "Filter");
    let table = root::Table::new("diagnostic-table", "Diagnostic table", TableState::new([]));
    let _diagnostic: root::table::TableBehaviorSnapshot =
        table.behavior_snapshot(UiPx::ZERO, UiPx::ZERO);

    let _adapter: Option<root::gpui_adapter::TextInputController> = None;
    let _field_semantics: Option<root::gpui_adapter::FieldControlSemantics> = None;
}
