//! Component conformance gates for the foundation gallery.

/// One component conformance gate shown by the Components page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentConformanceGate {
    /// Stable gate id.
    pub id: &'static str,
    /// Visible gate title.
    pub title: &'static str,
    /// Behavior or contract that this gate protects.
    pub summary: &'static str,
    /// Durable test or document evidence for this gate.
    pub evidence: &'static [&'static str],
}

/// Regression-prone component behaviors that every new slice should keep covered.
pub const COMPONENT_CONFORMANCE_GATES: &[ComponentConformanceGate] = &[
    ComponentConformanceGate {
        id: "public-api-exports",
        title: "Public API exports",
        summary: "The component contract registry, crate root, and prelude exports stay aligned for every shipped component type.",
        evidence: &[
            "crates/ui_components/src/component_contract/mod.rs",
            "crates/ui_components/src/component_contract/rows.rs",
            "crates/ui_components/src/component_contract/projections.rs",
            "crates/ui_components/src/component_contract/api_inventory.rs",
            "crates/ui_components/src/lib.rs",
            "crates/ui_components/src/prelude.rs",
            "crates/ui_components/tests/public_surface.rs",
        ],
    },
    ComponentConformanceGate {
        id: "gallery-metadata",
        title: "Gallery metadata",
        summary: "Components samples expose stable ids, real resolved state, and page signals.",
        evidence: &[
            "examples/ui-foundation-gallery/src/pages/components.rs",
            "examples/ui-foundation-gallery/tests/foundation_gallery.rs",
        ],
    },
    ComponentConformanceGate {
        id: "scroll-redraw",
        title: "Scroll redraw persistence",
        summary: "ScrollArea default handles survive reconstructed component values and reset only by policy.",
        evidence: &[
            "ScrollAreaRuntime",
            "scroll_area_default_handle_survives_reconstructed_component_values",
            "scroll_area_reset_key_resets_default_runtime_handle",
        ],
    },
    ComponentConformanceGate {
        id: "splitter-runtime",
        title: "Splitter runtime constraints",
        summary: "Splitter runtime fractions keep min/max and collapsed-panel restore behavior centralized.",
        evidence: &[
            "SplitterState::with_panel_fractions",
            "SplitterState::resized_by",
            "splitter_runtime_fraction_overrides_still_use_resize_constraints",
        ],
    },
    ComponentConformanceGate {
        id: "tabs-overflow",
        title: "Tabs overflow and roving focus",
        summary: "Tabs keep disabled-item skipping, tab-stop metadata, and vertical rail overflow dogfood.",
        evidence: &[
            "workspace-tabs",
            "components_page_tabs_samples_expose_roving_focus_contract",
            "docs/verification.md",
        ],
    },
    ComponentConformanceGate {
        id: "table-virtualization",
        title: "Table row models and scroll ownership",
        summary: "Table keeps stable row ids, grouped/expanded row metadata, aggregate metadata, pinned columns, pinned rows, resize handles, content-fit width growth, single-line and multiline editors, column visibility controls, and nested scroll ownership.",
        evidence: &[
            "TableState::resolve",
            "Table::behavior_snapshot",
            "TableHeaderAction",
            "release-rollup",
            "grouped-custom-aggregation",
            "release-resize",
            "content-fit-release",
            "toggle-release",
            "select-release",
            "multiline-release",
            "row-pinning",
            "filter-board",
            "TableColumnWidthPolicy",
            "TableGlobalFilter",
            "TablePredicateFilter",
            "TableFacetedFilter",
            "TableRangeFilter",
            "TableColumnVisibility",
            "TableColumnVisibilityChange",
            "TableToolbar",
            "components_gallery_smoke_global_filter_updates_table_rows",
            "components_gallery_smoke_predicate_filter_updates_table_rows",
            "components_gallery_smoke_faceted_filter_updates_table_rows",
            "components_gallery_smoke_range_filter_updates_table_rows",
            "components_gallery_smoke_content_fit_table_cell_edit_widens_name_column",
            "components_gallery_smoke_checkbox_table_cell_updates_sample_rows",
            "components_gallery_smoke_select_table_cell_updates_sample_rows",
            "components_gallery_smoke_multiline_table_cell_updates_sample_rows",
            "components_gallery_smoke_column_visibility_updates_release_matrix",
            "components_gallery_smoke_table_scroll_stays_inside_sample",
            "components_gallery_smoke_focused_table_scroll_stays_inside_sample",
            "components_gallery_smoke_grouped_table_scroll_stays_inside_sample",
            "components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample",
            "components_gallery_smoke_grouped_table_column_reorder_updates_sample",
            "components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample",
            "components_page_table_samples_expose_virtualized_row_model_contract",
            "components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample",
            "components_gallery_smoke_resizable_table_resize_updates_sample",
        ],
    },
    ComponentConformanceGate {
        id: "tree-renderer",
        title: "Tree renderer contract",
        summary: "Tree composes renderer-neutral hierarchy state with GPUI focus, expansion, selection, and local scroll ownership.",
        evidence: &[
            "Tree::state",
            "Tree::behavior_snapshot",
            "TreeBehaviorSnapshot",
            "TreeState::keyboard_action_for_key",
            "TreeState::typeahead_target",
            "tree_behavior_snapshot_virtualizes_visible_rows_with_stable_metadata",
            "tree_runtime_expands_reveals_and_selects_items",
            "tree_runtime_typeahead_focuses_visible_matching_row",
            "components_gallery_smoke_tree_expands_and_selects",
            "components_gallery_smoke_tree_lazy_branches_emit_load_metadata",
            "components_gallery_smoke_tree_card_wheel_does_not_leak_to_page",
            "components_gallery_smoke_virtualized_tree_scrolls_inside_sample",
        ],
    },
    ComponentConformanceGate {
        id: "virtualized-list-renderer",
        title: "VirtualizedList renderer contract",
        summary: "VirtualizedList keeps its state contract, row reveal logic, and inner scroll ownership aligned with the rendered adapter.",
        evidence: &[
            "VirtualizedList::behavior_snapshot_with_viewport",
            "VirtualizedListBehaviorSnapshot",
            "VirtualizedListState::navigation_target",
            "virtualized_list_runtime_reveals_active_row_and_emits_activation",
            "components_gallery_smoke_virtualized_list_scroll_stays_inside_sample",
            "components_gallery_smoke_virtualized_list_card_wheel_does_not_leak_to_page",
            "components_gallery_smoke_virtualized_list_keyboard_reveals_and_activates",
        ],
    },
    ComponentConformanceGate {
        id: "state-contract-readouts",
        title: "State contract readouts",
        summary: "Renderer-neutral TreeState and VirtualizedListState stay visible beside concrete renderers.",
        evidence: &[
            "state_contract_readout_pairs",
            "TreeState::keyboard_action_for_key",
            "VirtualizedListState::navigation_target",
            "components_page_state_contract_samples_expose_tree_and_virtualized_list_contracts",
        ],
    },
    ComponentConformanceGate {
        id: "choice-surfaces",
        title: "Choice identity and navigation",
        summary: "Choice surfaces keep stable value identity, shared listbox navigation, and focused gallery readouts aligned.",
        evidence: &[
            "choice.rs",
            "roving_focus.rs",
            "components_page_search_samples_expose_combobox_and_command_contracts",
            "component_gallery_shell_reads_choice_active_metadata_from_resolved_state",
            "components_gallery_smoke_focused_command_samples_cover_depth_behaviors",
        ],
    },
    ComponentConformanceGate {
        id: "a11y-labels",
        title: "A11y labels and associations",
        summary: "Icon-only controls and label associations remain explicit instead of relying on visual text.",
        evidence: &[
            "IconButton::new",
            "Label::for_control",
            "components_page_samples_keep_explicit_a11y_metadata",
        ],
    },
];
