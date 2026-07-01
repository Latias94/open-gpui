use open_gpui::{div, px};
use open_gpui_ui_components::{ColorIntent, FocusRing, gpui_adapter::gpui_role_from_ui};
use open_gpui_ui_core::{
    Orientation, OverlayLayerKind, OverlayLayerPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, OverlayPresence, Role, UiPoint, UiPx, UiSize, rect, semantic, ui_point,
    ui_px, ui_size,
};

#[derive(Debug)]
struct DefaultSeedApi {
    builder: &'static str,
    runtime_value: &'static str,
}

#[derive(Debug)]
struct CallbackApi {
    name: &'static str,
    payload: &'static str,
}

#[derive(Debug)]
struct ComponentApiInventoryEntry {
    component: &'static str,
    controlled_inputs: &'static [&'static str],
    default_seeds: &'static [DefaultSeedApi],
    legacy_seed_inputs: &'static [&'static str],
    policy_hints: &'static [&'static str],
    callbacks: &'static [CallbackApi],
    renderer_neutral_state: bool,
    no_interaction_note: Option<&'static str>,
}

impl ComponentApiInventoryEntry {
    fn has_classification(&self) -> bool {
        !component_render_inputs(self.component).is_empty()
            || !self.controlled_inputs.is_empty()
            || !self.default_seeds.is_empty()
            || !self.legacy_seed_inputs.is_empty()
            || !self.policy_hints.is_empty()
            || !self.callbacks.is_empty()
            || self.no_interaction_note.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PublicSurfaceOwnerClass {
    OfficialComponent,
    OfficialComponentRecipe,
    RendererNeutralStateContract,
    GpuiAdapterHelper,
    DiagnosticSurface,
    DeprecatedRemovalTarget,
    InternalImplementationDetail,
}

#[derive(Debug, Clone, Copy)]
struct PublicSurfaceOwnerEntry {
    name: &'static str,
    owner: PublicSurfaceOwnerClass,
    home: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SurfacePrimitiveStatus {
    NotPrimitive,
    PublicPrimitiveModule,
    RemovedPrimitiveModule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SurfaceGalleryStatus {
    OfficialComponent,
    OfficialOverlay,
    AdapterOnly,
    InternalAnatomy,
    StateContract,
    NotInGallery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SurfaceDocsStatus {
    ComponentCatalog,
    ComponentContract,
    ComponentContractOrVerification,
    Verification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SurfaceManifestEntry {
    name: String,
    owner: PublicSurfaceOwnerClass,
    home: String,
    root_export: bool,
    prelude_export: bool,
    primitive_status: SurfacePrimitiveStatus,
    adapter_only: bool,
    diagnostic_only: bool,
    gallery_status: SurfaceGalleryStatus,
    docs_status: SurfaceDocsStatus,
    docs_token: Option<&'static str>,
}

const PUBLIC_SURFACE_OWNER_MAP: &[PublicSurfaceOwnerEntry] = &[
    PublicSurfaceOwnerEntry {
        name: "TreeState",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "tree.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "VirtualizedListState",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "virtualized_list.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "GpuiOverlayAdapterConfig",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "GpuiOverlayState",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "TextInputController",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "init_text_input",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "focus_ring_shadow",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "gpui_px_from_ui",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "gpui_point_from_ui",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "gpui_size_from_ui",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "TableBehaviorSnapshot",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "table/behavior.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "TableToolbarColors",
        owner: PublicSurfaceOwnerClass::OfficialComponentRecipe,
        home: "table/toolbar.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "TreeBehaviorSnapshot",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "tree/render_plan.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "VirtualizedListBehaviorSnapshot",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "virtualized_list.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "CommandBehaviorSnapshot",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "command/render_plan.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "ToolbarItem",
        owner: PublicSurfaceOwnerClass::InternalImplementationDetail,
        home: "toolbar.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "SidebarItem",
        owner: PublicSurfaceOwnerClass::InternalImplementationDetail,
        home: "sidebar.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "ListboxOption",
        owner: PublicSurfaceOwnerClass::InternalImplementationDetail,
        home: "listbox.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "primitives::active_descendant",
        owner: PublicSurfaceOwnerClass::DeprecatedRemovalTarget,
        home: "removed",
    },
    PublicSurfaceOwnerEntry {
        name: "primitives::collection",
        owner: PublicSurfaceOwnerClass::DeprecatedRemovalTarget,
        home: "removed",
    },
    PublicSurfaceOwnerEntry {
        name: "primitives::controllable_state",
        owner: PublicSurfaceOwnerClass::DeprecatedRemovalTarget,
        home: "removed",
    },
    PublicSurfaceOwnerEntry {
        name: "primitives::overlay",
        owner: PublicSurfaceOwnerClass::DeprecatedRemovalTarget,
        home: "removed",
    },
    PublicSurfaceOwnerEntry {
        name: "primitives::field_state",
        owner: PublicSurfaceOwnerClass::InternalImplementationDetail,
        home: "primitives/field_state.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "primitives::focus_ring",
        owner: PublicSurfaceOwnerClass::InternalImplementationDetail,
        home: "primitives/focus_ring.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "primitives::roving_focus_group",
        owner: PublicSurfaceOwnerClass::InternalImplementationDetail,
        home: "primitives/roving_focus_group.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "primitives::trigger_a11y",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "primitives/trigger_a11y.rs",
    },
];

const COMPONENT_API_INVENTORY: &[ComponentApiInventoryEntry] = &[
    ComponentApiInventoryEntry {
        component: "Accordion",
        controlled_inputs: &["open_values"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open_values",
            runtime_value: "open_values",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["mode", "collapsible"],
        callbacks: &[CallbackApi {
            name: "on_open_change",
            payload: "AccordionOpenChange",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Button",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[CallbackApi {
            name: "on_click",
            payload: "ClickEvent",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Badge",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("display-only primitive"),
    },
    ComponentApiInventoryEntry {
        component: "Collapsible",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["content"],
        callbacks: &[CallbackApi {
            name: "on_open_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Link",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["href", "external"],
        callbacks: &[CallbackApi {
            name: "on_activate",
            payload: "LinkActivation",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Breadcrumb",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["item", "disabled"],
        callbacks: &[CallbackApi {
            name: "on_activate",
            payload: "BreadcrumbActivation",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Tag",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["variant", "removable"],
        callbacks: &[CallbackApi {
            name: "on_remove",
            payload: "TagRemove",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "ToastStack",
        controlled_inputs: &["toasts"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["max_visible", "toast"],
        callbacks: &[
            CallbackApi {
                name: "on_action",
                payload: "ToastAction",
            },
            CallbackApi {
                name: "on_dismiss",
                payload: "ToastDismiss",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "IconButton",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[CallbackApi {
            name: "on_click",
            payload: "ClickEvent",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Slider",
        controlled_inputs: &["value"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["min", "max", "step"],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "SliderChange",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "NumberInput",
        controlled_inputs: &["value"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["min", "max", "step", "read_only", "invalid", "required"],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "NumberInputChange",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Switch",
        controlled_inputs: &["checked"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Checkbox",
        controlled_inputs: &["checked", "checked_state"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[CallbackApi {
            name: "on_toggle",
            payload: "Toggled",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "RadioGroup",
        controlled_inputs: &[],
        default_seeds: &[DefaultSeedApi {
            builder: "default_selected",
            runtime_value: "selected",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation"],
        callbacks: &[CallbackApi {
            name: "on_selection_change",
            payload: "RadioSelection",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Toggle",
        controlled_inputs: &["pressed"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "ToggleGroup",
        controlled_inputs: &["selected_values"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_selected_values",
                runtime_value: "selected_values",
            },
            DefaultSeedApi {
                builder: "default_focused",
                runtime_value: "focused",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation", "mode", "selection_required", "item"],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "ToggleGroupSelectionChange",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Toolbar",
        controlled_inputs: &[],
        default_seeds: &[DefaultSeedApi {
            builder: "default_focused",
            runtime_value: "focused",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation"],
        callbacks: &[CallbackApi {
            name: "on_select",
            payload: "ToolbarSelection",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Sidebar",
        controlled_inputs: &["collapsed", "selected"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_focused",
            runtime_value: "focused",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["side", "variant", "collapse_mode"],
        callbacks: &[CallbackApi {
            name: "on_selection_change",
            payload: "SidebarSelection",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Tree",
        controlled_inputs: &[],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_selected",
                runtime_value: "selected",
            },
            DefaultSeedApi {
                builder: "default_focused",
                runtime_value: "focused",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "virtualized",
            "viewport_item_count",
            "overscan_count",
            "draggable",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_select",
                payload: "TreeSelection",
            },
            CallbackApi {
                name: "on_toggle",
                payload: "TreeToggle",
            },
            CallbackApi {
                name: "on_move",
                payload: "TreeMove",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Listbox",
        controlled_inputs: &["selected", "active"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["embedded", "typeahead_query"],
        callbacks: &[CallbackApi {
            name: "on_select",
            payload: "ListboxSelection",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Select",
        controlled_inputs: &["open", "selected", "active"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "placement",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_select",
                payload: "SelectSelection",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Combobox",
        controlled_inputs: &["open", "selected", "active"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_query",
                runtime_value: "query",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &["placement", "outside_press_policy"],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_select",
                payload: "ComboboxSelection",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Command",
        controlled_inputs: &[
            "open",
            "query",
            "selected",
            "selected_values",
            "active",
            "index_snapshot",
        ],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_query",
                runtime_value: "query",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "dialog",
            "dialog_enabled",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "viewport_item_count",
            "row_height",
            "overscan",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_query_change",
                payload: "String",
            },
            CallbackApi {
                name: "on_select",
                payload: "CommandSelection",
            },
            CallbackApi {
                name: "on_selected_values_change",
                payload: "CommandSelectionChange",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Label",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["for_control"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("control-association primitive"),
    },
    ComponentApiInventoryEntry {
        component: "TextInput",
        controlled_inputs: &["value"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["controller"],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "String",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Textarea",
        controlled_inputs: &["value"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["rows"],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "String",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Field",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["control"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("composition wrapper"),
    },
    ComponentApiInventoryEntry {
        component: "Tabs",
        controlled_inputs: &[],
        default_seeds: &[DefaultSeedApi {
            builder: "default_selected",
            runtime_value: "selected",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation", "activation_mode"],
        callbacks: &[CallbackApi {
            name: "on_selection_change",
            payload: "TabsSelection",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "ScrollArea",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["axis", "reset_on_key", "preserve_scroll", "scroll_handle"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Splitter",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation", "panel"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Table",
        controlled_inputs: &["state", "column_sizing", "column_order"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_focused_row",
            runtime_value: "focused_row",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "virtualizer_snapshot",
            "expansion_mode",
            "filtering_mode",
            "sorting_mode",
            "faceting_mode",
            "facet_metadata",
            "pagination_mode",
            "pagination_totals",
            "row_pinning_policy",
            "enable_column_resizing",
            "column_resize_mode",
            "column_resize_direction",
            "column_order",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_sort_requested",
                payload: "TableHeaderAction",
            },
            CallbackApi {
                name: "on_column_sizing_change",
                payload: "TableColumnSizingChange",
            },
            CallbackApi {
                name: "on_column_order_change",
                payload: "TableColumnOrderChange",
            },
            CallbackApi {
                name: "on_row_activate",
                payload: "TableRowActivation",
            },
            CallbackApi {
                name: "on_row_selection_change",
                payload: "TableRowSelectionChange",
            },
            CallbackApi {
                name: "on_row_expansion_request",
                payload: "TableRowExpansionToggle",
            },
            CallbackApi {
                name: "on_cell_edit_change",
                payload: "TableCellEditChange",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "TableFacetedFilter",
        controlled_inputs: &["open", "query", "selected_values"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_query",
                runtime_value: "query",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "facets",
            "placeholder",
            "empty_label",
            "clear_label",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_query_change",
                payload: "String",
            },
            CallbackApi {
                name: "on_change",
                payload: "TableFacetedFilterChange",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "TableColumnVisibility",
        controlled_inputs: &["open", "visibility"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_visibility",
                runtime_value: "visibility",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "columns",
            "empty_label",
            "show_all_label",
            "reset_label",
            "disabled",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_change",
                payload: "TableColumnVisibilityChange",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "TableGlobalFilter",
        controlled_inputs: &["query"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_query",
            runtime_value: "query",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &["placeholder", "clear_label", "disabled"],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "TableGlobalFilterChange",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "TablePredicateFilter",
        controlled_inputs: &["operator", "value"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_operator",
                runtime_value: "operator",
            },
            DefaultSeedApi {
                builder: "default_value",
                runtime_value: "value",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "operator_option",
            "operators",
            "placeholder",
            "clear_label",
            "disabled",
            "tokens",
        ],
        callbacks: &[CallbackApi {
            name: "on_change",
            payload: "TablePredicateFilterChange",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "TableToolbar",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["control", "secondary_control", "summary", "tokens"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("slot container"),
    },
    ComponentApiInventoryEntry {
        component: "TableRangeFilter",
        controlled_inputs: &["open"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_min_text",
                runtime_value: "min_text",
            },
            DefaultSeedApi {
                builder: "default_max_text",
                runtime_value: "max_text",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "facets",
            "range",
            "clear_label",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_change",
                payload: "TableRangeFilterChange",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "VirtualizedList",
        controlled_inputs: &[],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_active_index",
                runtime_value: "active_index",
            },
            DefaultSeedApi {
                builder: "default_selected_index",
                runtime_value: "selected_index",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &["viewport_item_count", "row_height", "overscan"],
        callbacks: &[CallbackApi {
            name: "on_activate",
            payload: "VirtualizedListActivation",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "StatusCue",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("feedback readout"),
    },
    ComponentApiInventoryEntry {
        component: "EmptyState",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("feedback readout"),
    },
    ComponentApiInventoryEntry {
        component: "Separator",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["orientation", "decorative"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("display-only primitive"),
    },
    ComponentApiInventoryEntry {
        component: "Kbd",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("display-only primitive"),
    },
    ComponentApiInventoryEntry {
        component: "Progress",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["indeterminate"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("status readout"),
    },
    ComponentApiInventoryEntry {
        component: "Skeleton",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("display-only loading placeholder"),
    },
    ComponentApiInventoryEntry {
        component: "Avatar",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["accessible_label"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("identity readout"),
    },
    ComponentApiInventoryEntry {
        component: "AvatarGroup",
        controlled_inputs: &[],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &["avatar", "max_visible"],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: Some("identity readout collection"),
    },
    ComponentApiInventoryEntry {
        component: "Tooltip",
        controlled_inputs: &["open"],
        default_seeds: &[],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "open_intent",
            "placement_side",
            "placement_alignment",
            "delay",
        ],
        callbacks: &[],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "HoverCard",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "open_intent",
            "placement_side",
            "placement_alignment",
            "delay",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[CallbackApi {
            name: "on_open_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Popover",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[CallbackApi {
            name: "on_open_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Dialog",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[CallbackApi {
            name: "on_open_change",
            payload: "bool",
        }],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "AlertDialog",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_cancel",
                payload: "()",
            },
            CallbackApi {
                name: "on_action",
                payload: "()",
            },
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Sheet",
        controlled_inputs: &["open"],
        default_seeds: &[DefaultSeedApi {
            builder: "default_open",
            runtime_value: "open",
        }],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "side",
            "modal_mode",
            "close_affordance",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_close",
                payload: "()",
            },
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "Menu",
        controlled_inputs: &["open"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_focused_value",
                runtime_value: "focused_value",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "placement",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_select",
                payload: "MenuSelection",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
    ComponentApiInventoryEntry {
        component: "ContextMenu",
        controlled_inputs: &["open"],
        default_seeds: &[
            DefaultSeedApi {
                builder: "default_open",
                runtime_value: "open",
            },
            DefaultSeedApi {
                builder: "default_focused_value",
                runtime_value: "focused_value",
            },
        ],
        legacy_seed_inputs: &[],
        policy_hints: &[
            "anchor_point",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
        ],
        callbacks: &[
            CallbackApi {
                name: "on_open_change",
                payload: "bool",
            },
            CallbackApi {
                name: "on_select",
                payload: "MenuSelection",
            },
        ],
        renderer_neutral_state: true,
        no_interaction_note: None,
    },
];

fn component_render_inputs(component: &str) -> &'static [&'static str] {
    match component {
        "Button" => &["variant", "disabled", "selected"],
        "Badge" => &["variant"],
        "IconButton" => &["variant", "disabled"],
        "Switch" => &["label", "disabled"],
        "Checkbox" => &[
            "label",
            "aria_label",
            "indeterminate",
            "disabled",
            "required",
            "invalid",
        ],
        "RadioGroup" => &["label", "disabled", "required", "item"],
        "Toggle" => &["variant", "disabled"],
        "Toolbar" => &["disabled", "item", "items"],
        "Sidebar" => &["disabled", "section", "sections"],
        "Tree" => &[
            "item",
            "items",
            "virtualized",
            "viewport_item_count",
            "overscan_count",
            "draggable",
        ],
        "Listbox" => &[
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "empty_label",
        ],
        "Select" => &[
            "placeholder",
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "full_width",
        ],
        "Combobox" => &[
            "placeholder",
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "required",
            "empty_label",
        ],
        "Command" => &[
            "placeholder",
            "trigger_label",
            "item",
            "items",
            "group",
            "groups",
            "index_snapshot",
            "disabled",
            "dialog_description",
            "loading",
            "idle",
            "empty_label",
        ],
        "Label" => &["required", "disabled"],
        "TextInput" => &[
            "placeholder",
            "display_mode",
            "disabled",
            "read_only",
            "invalid",
            "required",
        ],
        "Textarea" => &[
            "placeholder",
            "rows",
            "disabled",
            "read_only",
            "invalid",
            "required",
        ],
        "Field" => &[
            "help_text",
            "help",
            "error_text",
            "error",
            "required",
            "disabled",
            "invalid",
        ],
        "Tabs" => &["item"],
        "Splitter" => &["disabled", "panel"],
        "Table" => &[
            "label",
            "row_measure_mode",
            "overscan",
            "row_height",
            "header_height",
            "viewport_extent",
            "min_column_width",
            "expansion_mode",
            "enable_column_resizing",
            "column_resize_mode",
            "column_resize_direction",
            "content_fit_columns",
        ],
        "TableFacetedFilter" => &[
            "facets",
            "selected_values",
            "open",
            "default_open",
            "query",
            "default_query",
            "placeholder",
            "empty_label",
            "clear_label",
            "disabled",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_query_change",
            "on_change",
        ],
        "TableColumnVisibility" => &[
            "columns",
            "visibility",
            "default_visibility",
            "open",
            "default_open",
            "empty_label",
            "show_all_label",
            "reset_label",
            "disabled",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_change",
        ],
        "TablePredicateFilter" => &[
            "operator",
            "default_operator",
            "value",
            "default_value",
            "operator_option",
            "operators",
            "placeholder",
            "clear_label",
            "disabled",
            "tokens",
            "on_change",
        ],
        "TableToolbar" => &[
            "control",
            "controls",
            "secondary_control",
            "secondary_controls",
            "summary",
            "tokens",
        ],
        "VirtualizedList" => &["disabled", "viewport_item_count", "row_height", "overscan"],
        "StatusCue" => &["intent"],
        "EmptyState" => &["description", "intent"],
        "Separator" => &["orientation", "vertical", "decorative"],
        "Progress" => &["value", "indeterminate"],
        "Skeleton" => &["subtle"],
        "Avatar" => &["source", "fallback", "accessible_label"],
        "AvatarGroup" => &["avatars", "avatar", "max_visible"],
        "Tooltip" => &["disabled"],
        "HoverCard" => &["disabled"],
        "Popover" => &["disabled"],
        "Dialog" => &["description", "disabled"],
        "AlertDialog" => &[
            "intent",
            "cancel_label",
            "disabled",
            "cancel_disabled",
            "action_disabled",
        ],
        "Sheet" => &["description", "disabled"],
        "Menu" => &["item", "items", "disabled"],
        "ContextMenu" => &["item", "items"],
        _ => &[],
    }
}

fn component_source_inputs(component: &str) -> &'static [&'static str] {
    match component {
        "Accordion" => &["accordion.rs"],
        "Button" => &["button.rs"],
        "Badge" => &["badge.rs"],
        "Breadcrumb" => &["breadcrumb.rs"],
        "Collapsible" => &["collapsible.rs"],
        "Link" => &["link.rs"],
        "Tag" => &["tag.rs"],
        "ToastStack" => &["toast.rs"],
        "IconButton" => &["icon_button.rs"],
        "Switch" => &["switch.rs"],
        "Checkbox" => &["checkbox.rs"],
        "RadioGroup" => &["radio.rs"],
        "Toggle" => &["toggle.rs"],
        "ToggleGroup" => &["toggle_group.rs"],
        "Toolbar" => &["toolbar.rs"],
        "Sidebar" => &["sidebar.rs"],
        "Tree" => &["tree.rs", "tree/movement.rs", "tree/render_plan.rs"],
        "Listbox" => &["listbox.rs"],
        "Select" => &["select.rs"],
        "Combobox" => &["combobox.rs"],
        "Command" => &["command.rs"],
        "Label" => &["label.rs"],
        "TextInput" => &["text_input.rs"],
        "Textarea" => &["textarea.rs"],
        "Field" => &["field.rs"],
        "Tabs" => &["tabs.rs"],
        "ScrollArea" => &["scroll_area.rs"],
        "Splitter" => &["splitter.rs"],
        "Table" => &["table/mod.rs", "table/resolve.rs"],
        "TableColumnVisibility" => &["table/column_visibility"],
        "TableFacetedFilter" => &["table/faceted_filter"],
        "TableGlobalFilter" => &["table/global_filter"],
        "TablePredicateFilter" => &["table/predicate_filter"],
        "TableRangeFilter" => &["table/range_filter"],
        "TableToolbar" => &["table/toolbar.rs"],
        "VirtualizedList" => &["virtualized_list.rs"],
        "StatusCue" => &["feedback.rs"],
        "EmptyState" => &["feedback.rs"],
        "Separator" => &["separator.rs"],
        "Kbd" => &["kbd.rs"],
        "Progress" => &["progress.rs"],
        "Skeleton" => &["skeleton.rs"],
        "Avatar" => &["avatar.rs"],
        "AvatarGroup" => &["avatar.rs"],
        "Tooltip" => &["tooltip.rs"],
        "HoverCard" => &["hover_card.rs"],
        "Popover" => &["popover.rs"],
        "Dialog" => &["dialog.rs"],
        "AlertDialog" => &["alert_dialog.rs"],
        "Sheet" => &["sheet.rs"],
        "Menu" => &["menu.rs"],
        "ContextMenu" => &["context_menu.rs"],
        "Slider" => &["slider.rs"],
        "NumberInput" => &["number_input.rs"],
        _ => panic!("missing source file mapping for `{component}`"),
    }
}

fn table_render_owner_files() -> &'static [&'static str] {
    &[
        "table/body/mod.rs",
        "table/cell.rs",
        "table/editors.rs",
        "table/header.rs",
        "table/resize.rs",
    ]
}

fn component_public_methods(component: &str) -> &'static [&'static str] {
    match component {
        "Accordion" => &[
            "new",
            "mode",
            "collapsible",
            "open_values",
            "default_open_values",
            "item",
            "tokens",
            "on_open_change",
            "state",
        ],
        "Button" => &[
            "new", "variant", "disabled", "selected", "tokens", "on_click", "state",
        ],
        "Badge" => &["new", "variant", "tokens", "state"],
        "Collapsible" => &[
            "new",
            "open",
            "default_open",
            "disabled",
            "content",
            "tokens",
            "on_open_change",
            "state",
        ],
        "Link" => &[
            "new",
            "disabled",
            "external",
            "tokens",
            "on_activate",
            "state",
        ],
        "Breadcrumb" => &[
            "new",
            "disabled",
            "item",
            "items",
            "tokens",
            "on_activate",
            "state",
        ],
        "Tag" => &[
            "new",
            "variant",
            "removable",
            "disabled",
            "tokens",
            "on_remove",
            "state",
        ],
        "ToastStack" => &[
            "new",
            "toast",
            "toasts",
            "max_visible",
            "tokens",
            "on_action",
            "on_dismiss",
            "state",
        ],
        "IconButton" => &[
            "new",
            "variant",
            "disabled",
            "tokens",
            "on_click",
            "accessible_label",
            "state",
        ],
        "Slider" => &[
            "new",
            "value",
            "min",
            "max",
            "step",
            "disabled",
            "tokens",
            "on_change",
            "state",
        ],
        "NumberInput" => &[
            "new",
            "value",
            "min",
            "max",
            "step",
            "disabled",
            "read_only",
            "invalid",
            "required",
            "tokens",
            "on_change",
            "state",
        ],
        "Switch" => &[
            "new",
            "label",
            "checked",
            "disabled",
            "tokens",
            "on_change",
            "state",
        ],
        "Checkbox" => &[
            "new",
            "label",
            "aria_label",
            "checked",
            "indeterminate",
            "checked_state",
            "disabled",
            "required",
            "invalid",
            "tokens",
            "on_toggle",
            "state",
        ],
        "RadioGroup" => &[
            "new",
            "label",
            "orientation",
            "default_selected",
            "disabled",
            "required",
            "tokens",
            "item",
            "on_selection_change",
            "state",
        ],
        "Toggle" => &[
            "new",
            "variant",
            "pressed",
            "disabled",
            "tokens",
            "on_change",
            "state",
        ],
        "ToggleGroup" => &[
            "new",
            "orientation",
            "mode",
            "selected_values",
            "default_selected_values",
            "default_focused",
            "selection_required",
            "disabled",
            "tokens",
            "item",
            "items",
            "on_change",
            "state",
        ],
        "Toolbar" => &[
            "new",
            "orientation",
            "default_focused",
            "disabled",
            "tokens",
            "item",
            "items",
            "on_select",
            "state",
        ],
        "Sidebar" => &[
            "new",
            "side",
            "left",
            "right",
            "variant",
            "collapse_mode",
            "collapsed",
            "disabled",
            "selected",
            "default_focused",
            "tokens",
            "section",
            "sections",
            "on_selection_change",
            "state",
        ],
        "Tree" => &[
            "new",
            "item",
            "default_selected",
            "default_focused",
            "virtualized",
            "viewport_item_count",
            "overscan_count",
            "draggable",
            "on_select",
            "on_toggle",
            "on_move",
            "items",
            "state",
            "behavior_snapshot",
        ],
        "Listbox" => &[
            "new",
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "embedded",
            "selected",
            "active",
            "typeahead_query",
            "empty_label",
            "tokens",
            "on_select",
            "state",
        ],
        "Select" => &[
            "new",
            "placeholder",
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "full_width",
            "open",
            "default_open",
            "selected",
            "active",
            "placement",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_select",
            "state",
        ],
        "Combobox" => &[
            "new",
            "placeholder",
            "option",
            "options",
            "group",
            "groups",
            "disabled",
            "required",
            "open",
            "default_open",
            "default_query",
            "selected",
            "active",
            "empty_label",
            "placement",
            "outside_press_policy",
            "tokens",
            "on_open_change",
            "on_select",
            "state",
        ],
        "Command" => &[
            "new",
            "placeholder",
            "trigger_label",
            "item",
            "items",
            "group",
            "groups",
            "index_snapshot",
            "disabled",
            "open",
            "default_open",
            "dialog",
            "dialog_enabled",
            "dialog_description",
            "query",
            "default_query",
            "selection_mode",
            "multi_select",
            "selected",
            "selected_values",
            "active",
            "viewport_item_count",
            "row_height",
            "overscan",
            "loading",
            "idle",
            "empty_label",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_query_change",
            "on_select",
            "on_selected_values_change",
            "state",
            "behavior_snapshot",
            "behavior_snapshot_with_viewport",
        ],
        "Label" => &[
            "new",
            "for_control",
            "required",
            "disabled",
            "tokens",
            "state",
        ],
        "TextInput" => &[
            "new",
            "controller",
            "value",
            "on_change",
            "placeholder",
            "display_mode",
            "disabled",
            "read_only",
            "invalid",
            "required",
            "tokens",
            "state",
        ],
        "Textarea" => &[
            "new",
            "value",
            "on_change",
            "placeholder",
            "rows",
            "disabled",
            "read_only",
            "invalid",
            "required",
            "tokens",
            "state",
        ],
        "Field" => &[
            "new",
            "help_text",
            "help",
            "error_text",
            "error",
            "required",
            "disabled",
            "invalid",
            "tokens",
            "control",
            "state",
        ],
        "Tabs" => &[
            "new",
            "orientation",
            "activation_mode",
            "default_selected",
            "tokens",
            "item",
            "on_selection_change",
            "state",
        ],
        "ScrollArea" => &[
            "new",
            "axis",
            "vertical",
            "horizontal",
            "both",
            "scroll_handle",
            "reset_on_key",
            "preserve_scroll",
            "state",
        ],
        "Splitter" => &[
            "new",
            "orientation",
            "horizontal",
            "vertical",
            "disabled",
            "panel",
            "state",
        ],
        "Table" => &[
            "new",
            "label",
            "row_measure_mode",
            "overscan",
            "row_height",
            "header_height",
            "viewport_extent",
            "expansion_mode",
            "min_column_width",
            "virtualizer_snapshot",
            "default_focused_row",
            "on_sort_requested",
            "on_column_order_change",
            "enable_column_resizing",
            "column_resize_mode",
            "column_resize_direction",
            "on_column_sizing_change",
            "on_row_selection_change",
            "on_row_activate",
            "on_row_expansion_request",
            "on_cell_edit_change",
            "state",
            "behavior_snapshot",
        ],
        "TableFacetedFilter" => &[
            "new",
            "facets",
            "selected_values",
            "open",
            "default_open",
            "query",
            "default_query",
            "placeholder",
            "empty_label",
            "clear_label",
            "disabled",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_query_change",
            "on_change",
            "state",
        ],
        "TableColumnVisibility" => &[
            "new",
            "columns",
            "visibility",
            "default_visibility",
            "open",
            "default_open",
            "empty_label",
            "show_all_label",
            "reset_label",
            "disabled",
            "viewport_item_count",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_change",
            "state",
        ],
        "TableGlobalFilter" => &[
            "new",
            "query",
            "default_query",
            "placeholder",
            "clear_label",
            "disabled",
            "tokens",
            "on_change",
            "state",
        ],
        "TablePredicateFilter" => &[
            "new",
            "operator",
            "default_operator",
            "value",
            "default_value",
            "operator_option",
            "operators",
            "placeholder",
            "clear_label",
            "disabled",
            "tokens",
            "on_change",
            "state",
        ],
        "TableToolbar" => &[
            "new",
            "control",
            "controls",
            "secondary_control",
            "secondary_controls",
            "summary",
            "tokens",
            "state",
        ],
        "TableRangeFilter" => &[
            "new",
            "facets",
            "range",
            "default_min_text",
            "default_max_text",
            "open",
            "default_open",
            "clear_label",
            "disabled",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_change",
            "state",
        ],
        "VirtualizedList" => &[
            "new",
            "from_shared_items",
            "disabled",
            "default_active_index",
            "default_selected_index",
            "viewport_item_count",
            "row_height",
            "overscan",
            "on_activate",
            "state",
            "behavior_snapshot",
            "behavior_snapshot_with_viewport",
        ],
        "StatusCue" => &["new", "intent", "tokens", "state"],
        "EmptyState" => &["new", "description", "intent", "tokens", "state"],
        "Separator" => &[
            "new",
            "orientation",
            "vertical",
            "decorative",
            "tokens",
            "state",
        ],
        "Kbd" => &["new", "tokens", "state"],
        "Progress" => &["new", "value", "indeterminate", "tokens", "state"],
        "Skeleton" => &["new", "subtle", "tokens", "state"],
        "Avatar" => &[
            "new",
            "source",
            "fallback",
            "accessible_label",
            "tokens",
            "state",
        ],
        "AvatarGroup" => &["new", "avatar", "avatars", "max_visible", "tokens", "state"],
        "Tooltip" => &[
            "new",
            "element",
            "disabled",
            "open",
            "open_intent",
            "placement_side",
            "placement_alignment",
            "delay",
            "tokens",
            "state",
        ],
        "HoverCard" => &[
            "new",
            "element",
            "disabled",
            "open",
            "default_open",
            "open_intent",
            "placement_side",
            "placement_alignment",
            "delay",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "state",
        ],
        "Popover" => &[
            "new",
            "element",
            "disabled",
            "open",
            "default_open",
            "placement_side",
            "placement_alignment",
            "outside_press_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "state",
        ],
        "Dialog" => &[
            "new",
            "element",
            "description",
            "disabled",
            "open",
            "default_open",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "state",
        ],
        "AlertDialog" => &[
            "new",
            "intent",
            "cancel_label",
            "disabled",
            "cancel_disabled",
            "action_disabled",
            "open",
            "default_open",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_cancel",
            "on_action",
            "on_open_change",
            "state",
        ],
        "Sheet" => &[
            "new",
            "element",
            "description",
            "disabled",
            "open",
            "default_open",
            "side",
            "modal_mode",
            "close_affordance",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_close",
            "on_open_change",
            "state",
        ],
        "Menu" => &[
            "new",
            "item",
            "items",
            "disabled",
            "open",
            "default_open",
            "default_focused_value",
            "placement",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_select",
            "state",
        ],
        "ContextMenu" => &[
            "new",
            "item",
            "items",
            "open",
            "default_open",
            "anchor_point",
            "default_focused_value",
            "outside_press_policy",
            "escape_key_policy",
            "initial_focus_intent",
            "focus_restore_intent",
            "tokens",
            "on_open_change",
            "on_select",
            "state",
        ],
        _ => panic!("missing public method baseline for `{component}`"),
    }
}

fn component_public_methods_from_source(component: &str) -> Vec<String> {
    const MARKER_PREFIX: &str = "impl ";

    let marker = format!("{MARKER_PREFIX}{component} {{");
    let source_paths = component_source_paths(component);
    let mut methods = Vec::new();
    let mut found_impl = false;

    for source_path in &source_paths {
        let source = read_source_file(source_path);
        let mut search_start = 0usize;

        while let Some(relative_impl_start) = source[search_start..].find(&marker) {
            found_impl = true;
            let impl_start = search_start + relative_impl_start;
            let body_start = source[impl_start..]
                .find('{')
                .map(|offset| impl_start + offset)
                .expect("impl body should open with `{`");

            let mut depth = 0usize;
            let mut body_end = None;
            for (index, ch) in source[body_start..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            body_end = Some(body_start + index);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let body_end = body_end.expect("impl body should close");
            let body = &source[body_start + 1..body_end];

            for line in body.lines() {
                let trimmed = line.trim_start();
                if let Some(signature) = trimmed.strip_prefix("pub const fn ") {
                    let before_paren = signature
                        .split_once('(')
                        .map(|(name, _)| name)
                        .unwrap_or(signature);
                    let name = before_paren
                        .split_once('<')
                        .map(|(name, _)| name)
                        .unwrap_or(before_paren)
                        .trim();
                    methods.push(name.to_string());
                } else if let Some(signature) = trimmed.strip_prefix("pub fn ") {
                    let before_paren = signature
                        .split_once('(')
                        .map(|(name, _)| name)
                        .unwrap_or(signature);
                    let name = before_paren
                        .split_once('<')
                        .map(|(name, _)| name)
                        .unwrap_or(before_paren)
                        .trim();
                    methods.push(name.to_string());
                }
            }

            search_start = body_end + 1;
        }
    }

    if !found_impl {
        panic!(
            "missing `{marker}` in component source mapping for `{component}`: {source_paths:?}"
        );
    }

    methods
}

fn component_source_paths(component: &str) -> Vec<std::path::PathBuf> {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();

    for source_entry in component_source_inputs(component) {
        let mapped_path = source_dir.join(source_entry);
        if mapped_path.is_file() {
            paths.push(mapped_path);
        } else if mapped_path.is_dir() {
            collect_rs_files(&mapped_path, &mut paths);
        } else if let Some(module_name) = source_entry.strip_suffix(".rs") {
            let mod_path = source_dir.join(module_name).join("mod.rs");
            if mod_path.is_file() {
                paths.push(mod_path);
            } else {
                panic!("component source input `{source_entry}` does not exist");
            }
        } else {
            panic!("component source input `{source_entry}` must be a .rs file or directory");
        }
    }

    paths.sort();
    paths.dedup();
    paths
}

fn read_source_file(source_path: &std::path::Path) -> String {
    std::fs::read_to_string(source_path)
        .unwrap_or_else(|error| panic!("failed to read {source_path:?}: {error}"))
}

fn collect_rs_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read source dir {dir:?}: {error}"));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("failed to read source dir entry: {error}"))
            .path();
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn ui_component_source_files() -> Vec<std::path::PathBuf> {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&source_dir, &mut files);
    files.sort();
    files
}

#[test]
fn crate_root_and_prelude_exports_remain_explicit() {
    use open_gpui_ui_components::{self as root, prelude};

    let root_overlay: root::OverlayResolvedState = root::OverlayResolvedState::resolve(
        OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
    );
    let prelude_overlay: prelude::OverlayResolvedState = prelude::OverlayResolvedState::resolve(
        OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
    );
    let root_button = root::Button::new("save", "Save");
    let root_accordion = root::Accordion::new("accordion")
        .mode(root::AccordionMode::Multiple)
        .item(root::AccordionItem::new("one", "One", "One content"));
    let root_alert_dialog = root::AlertDialog::new(
        "delete",
        "Delete",
        "Delete item?",
        "This removes it.",
        "Delete",
    );
    let root_sheet = root::Sheet::new("sheet", "Open sheet", "Sheet", "Sheet content");
    let root_hover_card = root::HoverCard::new("hover-card", "Profile", "Profile details");
    let root_sidebar = root::Sidebar::new("sidebar", "Primary navigation");
    let root_toolbar = root::Toolbar::new("toolbar", "Editor");
    let root_listbox = root::Listbox::new("listbox", "Choices");
    let root_select = root::Select::new("select", "Choice");
    let root_select_option = root::TableSelectOption::new("ready", "Ready");
    let root_combobox = root::Combobox::new("combobox", "Search");
    let root_command = root::Command::new("command", "Commands");
    let root_command_items = vec![root::CommandItem::new("open", "Open")];
    let root_command_snapshot = root::CommandIndexSnapshot::new("root-v1")
        .mode(root::CommandIndexSnapshotMode::PreRankedFilter)
        .item(root::CommandItemDescriptor::new("open", "Open"));
    let root_command_snapshot: root::CommandBehaviorSnapshot =
        root::Command::new("root-command-plan", "Commands")
            .items(root_command_items)
            .index_snapshot(root_command_snapshot)
            .behavior_snapshot();
    let _root_command_row: Option<&root::CommandRowBehaviorSnapshot> =
        root_command_snapshot.rows().first();
    let root_menu_state = root::Menu::new("root-menu", "Menu")
        .default_open(true)
        .default_focused_value("more")
        .item(root::MenuItem::submenu(
            "more",
            "More",
            [root::MenuItem::action("nested", "Nested")],
        ))
        .state();
    let root_menu_submenu_navigation: root::MenuSubmenuNavigation = root_menu_state
        .submenu_navigation_target("right")
        .expect("root MenuSubmenuNavigation should be exported");
    let root_menu_submenu_surface: root::MenuSubmenuSurface = root::MenuSubmenuSurface::resolve(
        rect(
            ui_point(ui_px(0.0), ui_px(0.0)),
            ui_size(ui_px(120.0), ui_px(32.0)),
        ),
        ui_size(ui_px(180.0), ui_px(96.0)),
        OverlayPlacementSide::Right,
        OverlayPlacementAlignment::Start,
        UiPx::ZERO,
        None,
    );
    let root_menu_safe_hover_corridor: root::MenuSafeHoverCorridor =
        root_menu_submenu_surface.hover_corridor();
    let root_scroll = root::ScrollArea::new("scroll", div());
    let root_splitter = root::Splitter::new("split");
    let root_tabs = root::Tabs::new("tabs");
    let root_global_filter = root::TableGlobalFilter::new("global-filter", "Search");
    let root_predicate_filter = root::TablePredicateFilter::new("predicate-filter", "Name", "name");
    let root_table_toolbar =
        root::TableToolbar::new("table-toolbar", "Filters").summary("2 rows visible");
    let root_faceted_filter = root::TableFacetedFilter::new("status-filter", "Status", "status");
    let root_column_visibility = root::TableColumnVisibility::new("column-visibility", "Columns")
        .columns([root::TableColumn::new("status", "Status")]);
    let root_avatar = root::Avatar::new("avatar", "Ada Lovelace");
    let root_separator = root::Separator::new("separator");
    let root_kbd = root::Kbd::new("kbd", "Ctrl+K");
    let root_progress = root::Progress::new("progress", "Progress");
    let root_skeleton = root::Skeleton::new("skeleton");
    let root_status_cue = root::StatusCue::new("status", "Ready");
    let root_empty_state = root::EmptyState::new("empty", "No results");
    let root_collapsible = root::Collapsible::new("collapsible", "Details").default_open(true);
    let root_slider = root::Slider::new("slider", "Volume").value(40.0);
    let root_number_input = root::NumberInput::new("number", "Quantity").value(3.0);
    let root_link = root::Link::new("link", "Docs", "/docs").external(true);
    let root_breadcrumb = root::Breadcrumb::new("breadcrumb", "Path")
        .item(root::BreadcrumbItemDescriptor::new("home", "Home").href("/"))
        .item(root::BreadcrumbItemDescriptor::new("docs", "Docs").current(true));
    let root_tag = root::Tag::new("tag", "ready", "Ready").removable(true);
    let root_toast_stack = root::ToastStack::new("toasts", "Notifications")
        .toast(root::Toast::new("saved", "Saved").intent(root::ToastIntent::Success));
    let root_toggle_group = root::ToggleGroup::new("toggle-group", "Alignment")
        .item(root::ToggleGroupItem::new("left", "Left"))
        .item(root::ToggleGroupItem::new("right", "Right"))
        .selected_values(["left"]);
    let prelude_button = prelude::Button::new("save", "Save");
    let prelude_accordion = prelude::Accordion::new("accordion")
        .mode(prelude::AccordionMode::Single)
        .item(prelude::AccordionItem::new("one", "One", "One content"));
    let prelude_alert_dialog = prelude::AlertDialog::new(
        "delete",
        "Delete",
        "Delete item?",
        "This removes it.",
        "Delete",
    );
    let prelude_sheet = prelude::Sheet::new("sheet", "Open sheet", "Sheet", "Sheet content");
    let prelude_hover_card = prelude::HoverCard::new("hover-card", "Profile", "Profile details");
    let prelude_sidebar = prelude::Sidebar::new("sidebar", "Primary navigation");
    let prelude_toolbar = prelude::Toolbar::new("toolbar", "Editor");
    let prelude_listbox = prelude::Listbox::new("listbox", "Choices");
    let prelude_select = prelude::Select::new("select", "Choice");
    let prelude_select_option = prelude::TableSelectOption::new("blocked", "Blocked");
    let prelude_combobox = prelude::Combobox::new("combobox", "Search");
    let prelude_command = prelude::Command::new("command", "Commands");
    let prelude_command_items = vec![prelude::CommandItem::new("open", "Open")];
    let prelude_command_snapshot = prelude::CommandIndexSnapshot::new("prelude-v1")
        .mode(prelude::CommandIndexSnapshotMode::PreFiltered)
        .item(prelude::CommandItemDescriptor::new("open", "Open"));
    let prelude_command_snapshot: prelude::CommandBehaviorSnapshot =
        prelude::Command::new("prelude-command-plan", "Commands")
            .items(prelude_command_items)
            .index_snapshot(prelude_command_snapshot)
            .behavior_snapshot();
    let _prelude_command_row: Option<&prelude::CommandRowBehaviorSnapshot> =
        prelude_command_snapshot.rows().first();
    let prelude_menu_state = prelude::Menu::new("prelude-menu", "Menu")
        .default_open(true)
        .default_focused_value("more")
        .item(prelude::MenuItem::submenu(
            "more",
            "More",
            [prelude::MenuItem::action("nested", "Nested")],
        ))
        .state();
    let prelude_menu_submenu_navigation: prelude::MenuSubmenuNavigation = prelude_menu_state
        .submenu_navigation_target("right")
        .expect("prelude MenuSubmenuNavigation should be exported");
    let prelude_menu_submenu_surface: prelude::MenuSubmenuSurface =
        prelude::MenuSubmenuSurface::resolve(
            rect(
                ui_point(ui_px(0.0), ui_px(0.0)),
                ui_size(ui_px(120.0), ui_px(32.0)),
            ),
            ui_size(ui_px(180.0), ui_px(96.0)),
            OverlayPlacementSide::Right,
            OverlayPlacementAlignment::Start,
            UiPx::ZERO,
            None,
        );
    let prelude_menu_safe_hover_corridor: prelude::MenuSafeHoverCorridor =
        prelude_menu_submenu_surface.hover_corridor();
    let prelude_scroll = prelude::ScrollArea::new("scroll", div());
    let prelude_splitter = prelude::Splitter::new("split");
    let prelude_tabs = prelude::Tabs::new("tabs");
    let prelude_global_filter = prelude::TableGlobalFilter::new("global-filter", "Search");
    let prelude_predicate_filter =
        prelude::TablePredicateFilter::new("predicate-filter", "Name", "name");
    let prelude_table_toolbar =
        prelude::TableToolbar::new("table-toolbar", "Filters").summary("2 rows visible");
    let prelude_faceted_filter =
        prelude::TableFacetedFilter::new("status-filter", "Status", "status");
    let prelude_column_visibility =
        prelude::TableColumnVisibility::new("column-visibility", "Columns")
            .columns([prelude::TableColumn::new("status", "Status")]);
    let prelude_avatar = prelude::Avatar::new("avatar", "Ada Lovelace");
    let prelude_separator = prelude::Separator::new("separator");
    let prelude_kbd = prelude::Kbd::new("kbd", "Ctrl+K");
    let prelude_progress = prelude::Progress::new("progress", "Progress");
    let prelude_skeleton = prelude::Skeleton::new("skeleton");
    let prelude_status_cue = prelude::StatusCue::new("status", "Ready");
    let prelude_empty_state = prelude::EmptyState::new("empty", "No results");
    let prelude_collapsible =
        prelude::Collapsible::new("collapsible", "Details").default_open(false);
    let prelude_slider = prelude::Slider::new("slider", "Volume").value(20.0);
    let prelude_number_input = prelude::NumberInput::new("number", "Quantity").value(5.0);
    let prelude_link = prelude::Link::new("link", "Docs", "/docs");
    let prelude_breadcrumb = prelude::Breadcrumb::new("breadcrumb", "Path")
        .items([prelude::BreadcrumbItemDescriptor::new("home", "Home")]);
    let prelude_tag =
        prelude::Tag::new("tag", "ready", "Ready").variant(prelude::TagVariant::Outline);
    let prelude_toast_stack =
        prelude::ToastStack::new("toasts", "Notifications").toasts([prelude::Toast::new(
            "saved", "Saved",
        )
        .action("Undo")
        .pinned()]);
    let prelude_toggle_group = prelude::ToggleGroup::new("toggle-group", "Alignment")
        .mode(prelude::ToggleGroupSelectionMode::Multiple)
        .items([
            prelude::ToggleGroupItem::new("bold", "Bold"),
            prelude::ToggleGroupItem::new("italic", "Italic"),
        ])
        .default_selected_values(["bold"]);

    let _ = (
        root_button.state(),
        root_accordion.state(),
        root_alert_dialog.state(),
        root_sheet.state(),
        root_hover_card.state(),
        root_sidebar.state(),
        root_toolbar.state(),
        root_listbox.state(),
        root_select.state(),
        root_select_option.value(),
        root_combobox.state(),
        root_command.state(),
        root_command_snapshot.role(),
        root_menu_submenu_navigation.focused_value(),
        root_menu_safe_hover_corridor.bounds(),
        root_scroll.state(),
        root_splitter.state(),
        root_tabs.state(),
        root_global_filter.state(),
        root_predicate_filter.state(),
        root_table_toolbar.state(),
        root_faceted_filter.state(),
        root_column_visibility.state(),
        root_avatar.state(),
        root_separator.state(),
        root_kbd.state(),
        root_progress.state(),
        root_skeleton.state(),
        root_status_cue.state(),
        root_empty_state.state(),
        root_collapsible.state(),
        root_slider.state(),
        root_number_input.state(),
        root_link.state(),
        root_breadcrumb.state(),
        root_tag.state(),
        root_toast_stack.state(),
        root_toggle_group.state(),
        prelude_button.state(),
        prelude_accordion.state(),
        prelude_alert_dialog.state(),
        prelude_sheet.state(),
        prelude_hover_card.state(),
        prelude_sidebar.state(),
        prelude_toolbar.state(),
        prelude_listbox.state(),
        prelude_select.state(),
        prelude_select_option.value(),
        prelude_combobox.state(),
        prelude_command.state(),
        prelude_command_snapshot.row_role(),
        prelude_menu_submenu_navigation.focused_value(),
        prelude_menu_safe_hover_corridor.bounds(),
        prelude_scroll.state(),
        prelude_splitter.state(),
        prelude_tabs.state(),
        prelude_global_filter.state(),
        prelude_predicate_filter.state(),
        prelude_table_toolbar.state(),
        prelude_faceted_filter.state(),
        prelude_column_visibility.state(),
        prelude_avatar.state(),
        prelude_separator.state(),
        prelude_kbd.state(),
        prelude_progress.state(),
        prelude_skeleton.state(),
        prelude_status_cue.state(),
        prelude_empty_state.state(),
        prelude_collapsible.state(),
        prelude_slider.state(),
        prelude_number_input.state(),
        prelude_link.state(),
        prelude_breadcrumb.state(),
        prelude_tag.state(),
        prelude_toast_stack.state(),
        prelude_toggle_group.state(),
        root::toggle_group_navigation_target(Orientation::Horizontal, "right", 0, &[false, false]),
        prelude::toggle_group_navigation_target(
            Orientation::Horizontal,
            "right",
            0,
            &[false, false],
        ),
        root_overlay.policy().kind(),
        prelude_overlay.policy().kind(),
    );
}

#[test]
fn gpui_role_mapping_covers_neutral_image_and_separator_fallback() {
    assert_eq!(gpui_role_from_ui(Role::Image), open_gpui::Role::Image);
    assert_eq!(gpui_role_from_ui(Role::Link), open_gpui::Role::Link);
    assert_eq!(gpui_role_from_ui(Role::Separator), open_gpui::Role::Group);
    assert_eq!(gpui_role_from_ui(Role::Slider), open_gpui::Role::Slider);
    assert_eq!(gpui_role_from_ui(Role::Tree), open_gpui::Role::Tree);
    assert_eq!(gpui_role_from_ui(Role::TreeItem), open_gpui::Role::TreeItem);
    assert_eq!(gpui_role_from_ui(Role::Table), open_gpui::Role::Table);
    assert_eq!(gpui_role_from_ui(Role::Row), open_gpui::Role::Row);
    assert_eq!(
        gpui_role_from_ui(Role::ColumnHeader),
        open_gpui::Role::ColumnHeader
    );
    assert_eq!(gpui_role_from_ui(Role::Cell), open_gpui::Role::Cell);
}

fn official_component_catalog_names_from_gallery_source() -> Vec<String> {
    let names =
        component_catalog_names_from_gallery_constructor("ComponentCatalogEntry::official(");
    assert!(
        !names.is_empty(),
        "Components gallery source should contain official catalog entries"
    );
    names
}

fn component_catalog_names_from_gallery_constructor(constructor: &str) -> Vec<String> {
    const GALLERY_COMPONENTS_SOURCE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/ui-foundation-gallery/src/pages/components/catalog.rs"
    );

    let source = std::fs::read_to_string(GALLERY_COMPONENTS_SOURCE)
        .unwrap_or_else(|error| panic!("failed to read {GALLERY_COMPONENTS_SOURCE}: {error}"));
    let mut remaining = source.as_str();
    let mut names = Vec::new();

    while let Some(marker_index) = remaining.find(constructor) {
        remaining = &remaining[marker_index + constructor.len()..];
        let name_start = remaining
            .find('"')
            .unwrap_or_else(|| panic!("missing catalog name opener after {constructor}"));
        remaining = &remaining[name_start + 1..];
        let name_end = remaining
            .find('"')
            .unwrap_or_else(|| panic!("missing catalog name closer after {constructor}"));
        names.push(remaining[..name_end].to_string());
        remaining = &remaining[name_end + 1..];
    }

    names
}

fn surface_manifest() -> Vec<SurfaceManifestEntry> {
    let root_exports = default_reexport_tokens("lib.rs");
    let prelude_exports = default_reexport_tokens("prelude.rs");
    let mut entries = Vec::new();

    for entry in COMPONENT_API_INVENTORY {
        entries.push(SurfaceManifestEntry {
            name: entry.component.to_owned(),
            owner: public_owner_for_component_inventory(entry.component),
            home: component_source_inputs(entry.component)
                .first()
                .map(|source| component_source_home(*source))
                .unwrap_or("unknown")
                .to_owned(),
            root_export: root_exports.contains(entry.component),
            prelude_export: prelude_exports.contains(entry.component),
            primitive_status: SurfacePrimitiveStatus::NotPrimitive,
            adapter_only: false,
            diagnostic_only: false,
            gallery_status: component_gallery_status(entry.component)
                .unwrap_or(SurfaceGalleryStatus::NotInGallery),
            docs_status: SurfaceDocsStatus::ComponentCatalog,
            docs_token: Some(entry.component),
        });
    }

    for entry in PUBLIC_SURFACE_OWNER_MAP {
        entries.push(SurfaceManifestEntry {
            name: entry.name.to_owned(),
            owner: entry.owner,
            home: entry.home.to_owned(),
            root_export: root_exports.contains(entry.name),
            prelude_export: prelude_exports.contains(entry.name),
            primitive_status: primitive_status_for_surface(entry),
            adapter_only: entry.owner == PublicSurfaceOwnerClass::GpuiAdapterHelper,
            diagnostic_only: entry.owner == PublicSurfaceOwnerClass::DiagnosticSurface,
            gallery_status: component_gallery_status(entry.name)
                .unwrap_or(SurfaceGalleryStatus::NotInGallery),
            docs_status: docs_status_for_surface(entry),
            docs_token: docs_token_for_surface(entry),
        });
    }

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
}

fn public_owner_for_component_inventory(component: &str) -> PublicSurfaceOwnerClass {
    match component {
        "TableColumnVisibility"
        | "TableFacetedFilter"
        | "TableGlobalFilter"
        | "TablePredicateFilter"
        | "TableRangeFilter"
        | "TableToolbar" => PublicSurfaceOwnerClass::OfficialComponentRecipe,
        _ => PublicSurfaceOwnerClass::OfficialComponent,
    }
}

fn component_gallery_status(name: &str) -> Option<SurfaceGalleryStatus> {
    for constructor in [
        (
            "ComponentCatalogEntry::official(",
            SurfaceGalleryStatus::OfficialComponent,
        ),
        (
            "ComponentCatalogEntry::adapter_only(",
            SurfaceGalleryStatus::AdapterOnly,
        ),
        (
            "ComponentCatalogEntry::internal_anatomy(",
            SurfaceGalleryStatus::InternalAnatomy,
        ),
        (
            "ComponentCatalogEntry::state_contract(",
            SurfaceGalleryStatus::StateContract,
        ),
        (
            "ComponentCatalogEntry::deferred(",
            SurfaceGalleryStatus::NotInGallery,
        ),
    ] {
        if component_catalog_names_from_gallery_constructor(constructor.0)
            .iter()
            .any(|entry| entry == name)
        {
            return Some(constructor.1);
        }
    }

    if overlay_catalog_names_from_gallery_source()
        .iter()
        .any(|entry| entry == name)
    {
        return Some(SurfaceGalleryStatus::OfficialOverlay);
    }

    None
}

fn component_source_home(source_entry: &'static str) -> &'static str {
    match source_entry {
        "command.rs" => "command/mod.rs",
        source => source,
    }
}

fn overlay_catalog_names_from_gallery_source() -> Vec<String> {
    component_catalog_names_from_source_constructor(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/ui-foundation-gallery/src/pages/overlay.rs"
        ),
        "OverlayCatalogEntry::official(",
    )
}

fn component_catalog_names_from_source_constructor(
    source_path: &str,
    constructor: &str,
) -> Vec<String> {
    let source = std::fs::read_to_string(source_path)
        .unwrap_or_else(|error| panic!("failed to read {source_path}: {error}"));
    let mut remaining = source.as_str();
    let mut names = Vec::new();

    while let Some(marker_index) = remaining.find(constructor) {
        remaining = &remaining[marker_index + constructor.len()..];
        let name_start = remaining
            .find('"')
            .unwrap_or_else(|| panic!("missing catalog name opener after {constructor}"));
        remaining = &remaining[name_start + 1..];
        let name_end = remaining
            .find('"')
            .unwrap_or_else(|| panic!("missing catalog name closer after {constructor}"));
        names.push(remaining[..name_end].to_string());
        remaining = &remaining[name_end + 1..];
    }

    names
}

fn primitive_status_for_surface(entry: &PublicSurfaceOwnerEntry) -> SurfacePrimitiveStatus {
    if entry.name.starts_with("primitives::") && entry.home == "removed" {
        SurfacePrimitiveStatus::RemovedPrimitiveModule
    } else if entry.name.starts_with("primitives::") {
        SurfacePrimitiveStatus::PublicPrimitiveModule
    } else {
        SurfacePrimitiveStatus::NotPrimitive
    }
}

fn docs_status_for_surface(entry: &PublicSurfaceOwnerEntry) -> SurfaceDocsStatus {
    match entry.owner {
        PublicSurfaceOwnerClass::OfficialComponent
        | PublicSurfaceOwnerClass::OfficialComponentRecipe
        | PublicSurfaceOwnerClass::RendererNeutralStateContract
        | PublicSurfaceOwnerClass::GpuiAdapterHelper
        | PublicSurfaceOwnerClass::InternalImplementationDetail => {
            SurfaceDocsStatus::ComponentContract
        }
        PublicSurfaceOwnerClass::DiagnosticSurface => {
            SurfaceDocsStatus::ComponentContractOrVerification
        }
        PublicSurfaceOwnerClass::DeprecatedRemovalTarget => SurfaceDocsStatus::Verification,
    }
}

fn docs_token_for_surface(entry: &PublicSurfaceOwnerEntry) -> Option<&'static str> {
    if entry.home == "removed" {
        Some(entry.name)
    } else if entry.owner == PublicSurfaceOwnerClass::GpuiAdapterHelper {
        Some("open_gpui_ui_components::gpui_adapter")
    } else if entry.name.starts_with("primitives::") {
        Some("ui_components::primitives")
    } else {
        Some(entry.name.rsplit("::").next().unwrap_or(entry.name))
    }
}

fn component_api_entry(component: &str) -> &'static ComponentApiInventoryEntry {
    COMPONENT_API_INVENTORY
        .iter()
        .find(|entry| entry.component == component)
        .unwrap_or_else(|| panic!("missing component API inventory row for `{component}`"))
}

fn assert_inventory_contains_controlled_input(component: &str, input: &str) {
    let entry = component_api_entry(component);
    assert!(
        entry.controlled_inputs.contains(&input),
        "{component} inventory should classify `{input}` as a controlled input"
    );
}

fn assert_inventory_contains_default_seed(component: &str, builder: &str, runtime_value: &str) {
    let entry = component_api_entry(component);
    assert!(
        entry
            .default_seeds
            .iter()
            .any(|seed| seed.builder == builder && seed.runtime_value == runtime_value),
        "{component} inventory should classify `{builder}` as a default seed for `{runtime_value}`"
    );
}

fn assert_inventory_contains_callback(component: &str, name: &str, payload: &str) {
    let entry = component_api_entry(component);
    assert!(
        entry
            .callbacks
            .iter()
            .any(|callback| callback.name == name && callback.payload == payload),
        "{component} inventory should document callback `{name}` payload `{payload}`"
    );
}

#[test]
fn component_api_inventory_covers_official_gallery_catalog() {
    use std::collections::BTreeSet;

    let inventory_names = COMPONENT_API_INVENTORY
        .iter()
        .map(|entry| entry.component.to_string())
        .collect::<BTreeSet<_>>();
    let catalog_names = official_component_catalog_names_from_gallery_source()
        .into_iter()
        .collect::<BTreeSet<_>>();

    let missing = catalog_names
        .difference(&inventory_names)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "official Components catalog entries need public API inventory rows: {missing:?}"
    );

    for overlay in [
        "Tooltip",
        "HoverCard",
        "Popover",
        "Dialog",
        "AlertDialog",
        "Sheet",
        "Menu",
        "ContextMenu",
    ] {
        assert!(
            inventory_names.contains(overlay),
            "overlay component `{overlay}` needs a public API inventory row"
        );
    }
}

#[test]
fn surface_manifest_classifies_public_surface_once() {
    use std::collections::{BTreeMap, BTreeSet};

    let mut owners = BTreeMap::new();
    for entry in surface_manifest() {
        let previous = owners.insert(entry.name.clone(), entry.owner);
        assert!(
            previous.is_none(),
            "`{}` appears in multiple public surface owner classes",
            entry.name
        );
    }

    let covered_classes = owners.values().copied().collect::<BTreeSet<_>>();
    for expected_class in [
        PublicSurfaceOwnerClass::OfficialComponent,
        PublicSurfaceOwnerClass::RendererNeutralStateContract,
        PublicSurfaceOwnerClass::GpuiAdapterHelper,
        PublicSurfaceOwnerClass::DeprecatedRemovalTarget,
        PublicSurfaceOwnerClass::InternalImplementationDetail,
    ] {
        assert!(
            covered_classes.contains(&expected_class),
            "public surface owner map should contain at least one {expected_class:?} entry"
        );
    }
}

#[test]
fn surface_manifest_aligns_adjacent_gallery_statuses() {
    let status_expectations = [
        (
            "ComponentCatalogEntry::state_contract(",
            SurfaceGalleryStatus::StateContract,
        ),
        (
            "ComponentCatalogEntry::adapter_only(",
            SurfaceGalleryStatus::AdapterOnly,
        ),
        (
            "ComponentCatalogEntry::internal_anatomy(",
            SurfaceGalleryStatus::InternalAnatomy,
        ),
    ];
    let manifest = surface_manifest();

    for (constructor, expected_status) in status_expectations {
        let names = component_catalog_names_from_gallery_constructor(constructor);
        assert!(
            !names.is_empty(),
            "gallery constructor `{constructor}` should remain covered by the owner map"
        );

        for name in names {
            let entries = manifest
                .iter()
                .filter(|entry| entry.name == name)
                .collect::<Vec<_>>();
            assert_eq!(
                entries.len(),
                1,
                "gallery catalog entry `{name}` should have exactly one adjacent public surface owner"
            );
            assert_eq!(
                entries[0].gallery_status, expected_status,
                "gallery catalog entry `{name}` changed manifest gallery status"
            );
        }
    }
}

#[test]
fn surface_manifest_homes_point_to_real_sources() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let lib_source = std::fs::read_to_string(source_dir.join("lib.rs"))
        .unwrap_or_else(|error| panic!("failed to read lib.rs: {error}"));
    let gpui_adapter_source = public_module_source(&lib_source, "gpui_adapter")
        .expect("lib.rs should expose a gpui_adapter module");

    for entry in surface_manifest() {
        if entry.home == "removed" {
            continue;
        } else if entry.home == "gpui_adapter" {
            assert!(
                gpui_adapter_source.contains(entry.name.as_str()),
                "`{}` should stay exported through the gpui_adapter owner group",
                entry.name
            );
        } else {
            let path = source_dir.join(entry.home.as_str());
            assert!(
                path.is_file() || path.is_dir(),
                "`{}` owner home `{}` should point to a real source file or module directory",
                entry.name,
                entry.home
            );
        }
    }
}

#[test]
fn surface_manifest_tracks_exports_gallery_and_docs_contracts() {
    use std::collections::BTreeSet;

    let manifest = surface_manifest();
    let names = manifest
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<BTreeSet<_>>();
    for required in [
        "Button",
        "Listbox",
        "Select",
        "Combobox",
        "Command",
        "Tooltip",
        "Dialog",
        "TableBehaviorSnapshot",
        "TreeState",
        "VirtualizedListState",
        "GpuiOverlayAdapterConfig",
        "TextInputController",
        "primitives::trigger_a11y",
    ] {
        assert!(
            names.contains(required),
            "surface manifest should include `{required}`"
        );
    }

    let component_contract = include_str!("../../../../docs/ui/component-contract.md");
    let verification = include_str!("../../../../docs/verification.md");
    for entry in &manifest {
        match entry.owner {
            PublicSurfaceOwnerClass::OfficialComponent => {
                assert!(
                    entry.root_export,
                    "{} should be exported from crate root",
                    entry.name
                );
                assert!(
                    entry.prelude_export,
                    "{} should be exported from prelude",
                    entry.name
                );
                assert!(
                    matches!(
                        entry.gallery_status,
                        SurfaceGalleryStatus::OfficialComponent
                            | SurfaceGalleryStatus::OfficialOverlay
                    ),
                    "official manifest entry `{}` should be present in a gallery catalog",
                    entry.name
                );
            }
            PublicSurfaceOwnerClass::OfficialComponentRecipe => {
                assert!(
                    entry.root_export,
                    "{} should be exported from crate root",
                    entry.name
                );
                assert!(
                    entry.prelude_export,
                    "{} should be exported from prelude",
                    entry.name
                );
                assert_eq!(
                    entry.gallery_status,
                    SurfaceGalleryStatus::NotInGallery,
                    "component recipe `{}` should be documented by docs/signals rather than standalone catalog status",
                    entry.name
                );
            }
            PublicSurfaceOwnerClass::GpuiAdapterHelper => {
                assert!(
                    entry.adapter_only,
                    "{} should be flagged adapter-only",
                    entry.name
                );
                assert!(
                    !entry.prelude_export,
                    "adapter-only surface `{}` must not leak into prelude",
                    entry.name
                );
            }
            PublicSurfaceOwnerClass::DiagnosticSurface => {
                assert!(
                    entry.diagnostic_only,
                    "{} should be flagged diagnostic-only",
                    entry.name
                );
            }
            PublicSurfaceOwnerClass::RendererNeutralStateContract
            | PublicSurfaceOwnerClass::DeprecatedRemovalTarget
            | PublicSurfaceOwnerClass::InternalImplementationDetail => {}
        }

        match entry.primitive_status {
            SurfacePrimitiveStatus::PublicPrimitiveModule => {
                assert!(
                    entry.home.starts_with("primitives/"),
                    "primitive manifest entry `{}` should point to primitives source",
                    entry.name
                );
            }
            SurfacePrimitiveStatus::RemovedPrimitiveModule => {
                assert_eq!(
                    entry.home, "removed",
                    "removed primitive `{}` should not point at a compatibility file",
                    entry.name
                );
            }
            SurfacePrimitiveStatus::NotPrimitive => {}
        }

        let Some(token) = entry.docs_token else {
            continue;
        };
        match entry.docs_status {
            SurfaceDocsStatus::ComponentCatalog => {
                assert!(
                    names.contains(entry.name.as_str()),
                    "component catalog surface `{}` should remain in manifest",
                    entry.name
                );
            }
            SurfaceDocsStatus::ComponentContract => {
                assert!(
                    component_contract.contains(token),
                    "component contract docs should mention manifest token `{token}`"
                );
            }
            SurfaceDocsStatus::ComponentContractOrVerification => {
                assert!(
                    component_contract.contains(token) || verification.contains(token),
                    "component contract or verification docs should mention manifest token `{token}`"
                );
            }
            SurfaceDocsStatus::Verification => {
                assert!(
                    verification.contains(token)
                        || verification.contains("primitive_deletion_target_inventory"),
                    "verification docs should mention removed manifest token `{token}`"
                );
            }
        }
    }
}

#[test]
fn primitive_owner_map_classifies_every_public_primitive_module_once() {
    use std::collections::BTreeMap;

    let modules = public_primitive_modules_from_mod();
    let mut owners = BTreeMap::new();
    for entry in PUBLIC_SURFACE_OWNER_MAP
        .iter()
        .filter(|entry| entry.name.starts_with("primitives::"))
    {
        let module = entry
            .name
            .strip_prefix("primitives::")
            .expect("primitive owner entry should use primitives:: prefix");
        let previous = owners.insert(module.to_owned(), entry.owner);
        assert!(
            previous.is_none(),
            "primitive module `{module}` should have exactly one owner class"
        );
    }

    owners.retain(|_, owner| *owner != PublicSurfaceOwnerClass::DeprecatedRemovalTarget);
    assert_eq!(
        owners.keys().cloned().collect::<Vec<_>>(),
        modules,
        "every remaining public primitives module should be explicitly classified after U2 removes shallow aliases"
    );
}

#[test]
fn primitive_deletion_target_inventory_blocks_removed_shallow_reexports() {
    let deletion_targets = PUBLIC_SURFACE_OWNER_MAP
        .iter()
        .filter(|entry| entry.owner == PublicSurfaceOwnerClass::DeprecatedRemovalTarget)
        .map(|entry| {
            entry
                .name
                .strip_prefix("primitives::")
                .unwrap_or_else(|| panic!("deletion target `{}` should be a primitive", entry.name))
                .to_owned()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        deletion_targets,
        vec![
            "active_descendant".to_string(),
            "collection".to_string(),
            "controllable_state".to_string(),
            "overlay".to_string(),
        ],
        "U2 should delete only the known shallow primitive pass-through modules"
    );

    let primitives_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/primitives");
    let public_modules = public_primitive_modules_from_mod();
    for module in deletion_targets {
        let source_path = primitives_dir.join(format!("{module}.rs"));
        assert!(
            !source_path.exists(),
            "removed shallow primitive module `{module}` should not keep a compatibility file"
        );
        assert!(
            !public_modules.contains(&module),
            "removed shallow primitive module `{module}` should not stay in primitives/mod.rs"
        );
    }
}

#[test]
fn primitive_modules_do_not_reexport_ui_core_as_pass_through_aliases() {
    let primitives_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/primitives");
    let entries = std::fs::read_dir(&primitives_dir)
        .unwrap_or_else(|error| panic!("failed to read {primitives_dir:?}: {error}"));
    let mut offenders = Vec::new();

    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("failed to read primitive source entry: {error}"))
            .path();
        if path.file_name().is_some_and(|name| name == "mod.rs")
            || path.extension().is_none_or(|extension| extension != "rs")
        {
            continue;
        }

        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));
        if source.contains("pub use open_gpui_ui_core::") {
            offenders.push(
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("<unknown>")
                    .to_owned(),
            );
        }
    }

    assert_eq!(
        offenders,
        Vec::<String>::new(),
        "ui_components::primitives must own adapter behavior, not pass through ui_core aliases"
    );
}

#[test]
fn component_contract_docs_match_current_public_surface_vocabulary() {
    let contract = include_str!("../../../../docs/ui/component-contract.md");

    for required in [
        "components::catalog::COMPONENT_CATALOG",
        "components/catalog.rs",
        "components/render.rs",
        "`adapter-only`",
        "`internal-anatomy`",
        "`state-contract`",
        "`TableBehaviorSnapshot`",
        "not component facades",
        "default application",
        "state API",
        "`ThemeRegistry` is the app-level owner",
        "Virtualized adapters share a crate-private row-window projection",
        "`open_gpui_ui_components::choice`",
        "`open_gpui_ui_components::gpui_adapter`",
    ] {
        assert!(
            contract.contains(required),
            "component contract docs should mention `{required}`"
        );
    }

    for removed in [
        "`ui_components::primitives::active_descendant`",
        "`ui_components::primitives::collection`",
        "`ui_components::primitives::controllable_state`",
        "`ui_components::primitives::overlay`",
        "theme registry gap",
    ] {
        assert!(
            !contract.contains(removed),
            "component contract docs should not preserve removed or stale contract `{removed}`"
        );
    }
}

#[test]
fn table_component_source_mapping_tracks_split_render_owners() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    assert!(
        !source_dir.join("table.rs").exists(),
        "Table should resolve through table/mod.rs instead of the old single-file adapter"
    );
    assert_eq!(
        component_source_inputs("Table"),
        ["table/mod.rs", "table/resolve.rs"]
    );

    for owner in table_render_owner_files() {
        assert!(
            source_dir.join(owner).is_file(),
            "split Table render owner `{owner}` should exist"
        );
    }
}

#[test]
fn component_source_mapping_expands_split_component_directories() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source_files = component_source_paths("TableRangeFilter")
        .into_iter()
        .map(|path| {
            path.strip_prefix(&source_dir)
                .unwrap_or_else(|error| panic!("failed to strip source dir from {path:?}: {error}"))
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();

    assert!(
        source_files.contains(&"table/range_filter/component.rs".to_string()),
        "split component directory mapping should include its public component file"
    );
    assert!(
        source_files.contains(&"table/range_filter/state.rs".to_string()),
        "split component directory mapping should include adjacent public contract files"
    );
}

#[test]
fn component_api_inventory_rows_are_unique_and_classified() {
    let mut seen = std::collections::BTreeSet::new();
    for entry in COMPONENT_API_INVENTORY {
        assert!(
            seen.insert(entry.component),
            "component API inventory contains duplicate row for `{}`",
            entry.component
        );
        assert!(
            entry.has_classification(),
            "{} must document at least one API ownership bucket or no-interaction note",
            entry.component
        );
        assert!(
            entry.renderer_neutral_state,
            "{} resolved state must remain renderer-neutral",
            entry.component
        );
    }
}

#[test]
fn component_api_inventory_tracks_public_method_surface() {
    for entry in COMPONENT_API_INVENTORY {
        let source_methods = component_public_methods_from_source(entry.component);
        let expected_methods = component_public_methods(entry.component)
            .iter()
            .map(|method| method.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            source_methods, expected_methods,
            "{} public method surface drifted; update COMPONENT_API_INVENTORY and the method baseline together",
            entry.component
        );
    }
}

#[test]
fn component_api_inventory_uses_stable_ownership_vocabulary() {
    const CURRENT_CALLBACK_NAMES: &[&str] = &[
        "on_activate",
        "on_action",
        "on_cancel",
        "on_change",
        "on_click",
        "on_close",
        "on_cell_edit_change",
        "on_column_order_change",
        "on_column_sizing_change",
        "on_dismiss",
        "on_move",
        "on_open_change",
        "on_query_change",
        "on_remove",
        "on_row_activate",
        "on_row_selection_change",
        "on_row_expansion_request",
        "on_select",
        "on_selected_values_change",
        "on_selection_change",
        "on_sort_requested",
        "on_toggle",
    ];
    const CURRENT_LEGACY_SEED_INPUTS: &[(&str, &str)] = &[];

    for entry in COMPONENT_API_INVENTORY {
        for seed in entry.default_seeds {
            assert!(
                seed.builder.starts_with("default_"),
                "{} default seed `{}` must use default_* naming",
                entry.component,
                seed.builder
            );
            assert!(
                !seed.runtime_value.is_empty(),
                "{} default seed `{}` must name the adapter-owned runtime value it seeds",
                entry.component,
                seed.builder
            );
        }

        for callback in entry.callbacks {
            assert!(
                CURRENT_CALLBACK_NAMES.contains(&callback.name),
                "{} callback `{}` is not part of the current inventory vocabulary",
                entry.component,
                callback.name
            );
            assert!(
                !callback.payload.is_empty(),
                "{} callback `{}` must document its payload type",
                entry.component,
                callback.name
            );
        }

        for legacy_seed in entry.legacy_seed_inputs {
            assert!(
                CURRENT_LEGACY_SEED_INPUTS.contains(&(entry.component, *legacy_seed)),
                "{} legacy seed `{}` needs an explicit migration decision before U2",
                entry.component,
                legacy_seed
            );
        }
    }
}

#[test]
fn component_api_inventory_keeps_regression_sentinels_for_stateful_components() {
    assert_inventory_contains_controlled_input("TextInput", "value");
    assert_inventory_contains_callback("TextInput", "on_change", "String");
    assert_inventory_contains_controlled_input("Textarea", "value");
    assert_inventory_contains_callback("Textarea", "on_change", "String");
    assert_inventory_contains_controlled_input("Switch", "checked");
    assert_inventory_contains_callback("Switch", "on_change", "bool");
    assert_inventory_contains_default_seed("Popover", "default_open", "open");
    assert_inventory_contains_callback("Popover", "on_open_change", "bool");
    assert_inventory_contains_controlled_input("Select", "selected");
    assert_inventory_contains_controlled_input("Select", "active");
    assert_inventory_contains_callback("Select", "on_select", "SelectSelection");
    assert_inventory_contains_default_seed("Combobox", "default_query", "query");
    assert_inventory_contains_controlled_input("Command", "query");
    assert_inventory_contains_controlled_input("Command", "selected_values");
    assert_inventory_contains_controlled_input("Command", "index_snapshot");
    assert_inventory_contains_default_seed("Command", "default_query", "query");
    assert_inventory_contains_callback("Command", "on_query_change", "String");
    assert_inventory_contains_callback(
        "Command",
        "on_selected_values_change",
        "CommandSelectionChange",
    );
    assert_inventory_contains_default_seed("Tabs", "default_selected", "selected");
    assert_inventory_contains_default_seed("RadioGroup", "default_selected", "selected");
    assert_inventory_contains_default_seed("Toolbar", "default_focused", "focused");
    assert_inventory_contains_default_seed("Sidebar", "default_focused", "focused");
    assert_inventory_contains_default_seed("Tree", "default_selected", "selected");
    assert_inventory_contains_default_seed("Tree", "default_focused", "focused");
    assert_inventory_contains_callback("Tree", "on_toggle", "TreeToggle");
    assert_inventory_contains_callback("Tree", "on_move", "TreeMove");
    assert_inventory_contains_default_seed(
        "VirtualizedList",
        "default_active_index",
        "active_index",
    );
    assert_inventory_contains_default_seed(
        "VirtualizedList",
        "default_selected_index",
        "selected_index",
    );
    assert_inventory_contains_callback(
        "VirtualizedList",
        "on_activate",
        "VirtualizedListActivation",
    );
    assert_inventory_contains_default_seed("Menu", "default_focused_value", "focused_value");
    assert_inventory_contains_default_seed("ContextMenu", "default_focused_value", "focused_value");
    assert_inventory_contains_default_seed("Table", "default_focused_row", "focused_row");
    assert_inventory_contains_controlled_input("TableGlobalFilter", "query");
    assert_inventory_contains_default_seed("TableGlobalFilter", "default_query", "query");
    assert_inventory_contains_callback("TableGlobalFilter", "on_change", "TableGlobalFilterChange");
    assert_inventory_contains_controlled_input("TablePredicateFilter", "operator");
    assert_inventory_contains_controlled_input("TablePredicateFilter", "value");
    assert_inventory_contains_default_seed("TablePredicateFilter", "default_operator", "operator");
    assert_inventory_contains_default_seed("TablePredicateFilter", "default_value", "value");
    assert_inventory_contains_callback(
        "TablePredicateFilter",
        "on_change",
        "TablePredicateFilterChange",
    );
    assert_inventory_contains_controlled_input("TableColumnVisibility", "visibility");
    assert_inventory_contains_controlled_input("TableColumnVisibility", "open");
    assert_inventory_contains_default_seed(
        "TableColumnVisibility",
        "default_visibility",
        "visibility",
    );
    assert_inventory_contains_default_seed("TableColumnVisibility", "default_open", "open");
    assert_inventory_contains_callback(
        "TableColumnVisibility",
        "on_change",
        "TableColumnVisibilityChange",
    );
    assert_inventory_contains_callback("Table", "on_row_activate", "TableRowActivation");
    assert_inventory_contains_callback(
        "Table",
        "on_row_selection_change",
        "TableRowSelectionChange",
    );
    assert_inventory_contains_callback(
        "Table",
        "on_row_expansion_request",
        "TableRowExpansionToggle",
    );
    assert_inventory_contains_callback("Table", "on_column_order_change", "TableColumnOrderChange");
}

#[test]
fn public_resolved_state_contracts_avoid_gpui_runtime_types() {
    const FORBIDDEN: &[&str] = &[
        "Window",
        "App",
        "Context<",
        "RenderOnce",
        "IntoElement",
        "ElementId",
        "Entity<",
        "FocusHandle",
        "ScrollHandle",
        "Rc<dyn",
    ];
    let mut checked = 0;
    for source_file in ui_component_source_files() {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        for state in public_contract_structs(&source, &["State"]) {
            checked += 1;
            let fields = uncommented_lines(state.fields);
            for forbidden in FORBIDDEN {
                assert!(
                    !fields.contains(forbidden),
                    "{file_name}::{} leaks forbidden runtime/render type `{forbidden}`",
                    state.name
                );
            }
        }
    }

    assert!(
        checked >= 40,
        "expected to scan all public resolved-state structs, scanned {checked}"
    );
}

#[test]
fn public_contract_extraction_blockers_match_allowlist() {
    const BLOCKER_TOKENS: &[&str] = &["GpuiOverlayState", "open_gpui::Pixels", "Point<Pixels>"];
    let expected: [(&str, &str, &str); 0] = [];
    let mut expected = expected
        .into_iter()
        .map(|(file, contract, token)| {
            PublicContractBlocker::new(file.to_owned(), contract.to_owned(), token.to_owned())
        })
        .collect::<Vec<_>>();
    expected.sort();

    let mut actual = public_contract_extraction_blockers(BLOCKER_TOKENS);
    actual.sort();

    assert_eq!(
        actual, expected,
        "public component contracts gained or removed extraction blockers; update this inventory as U2-U6 migrate them"
    );
}

#[test]
fn adapter_only_public_surfaces_match_allowlist() {
    let expected = [
        ("focus.rs", "BoxShadow"),
        ("focus.rs", "focus_ring_shadow"),
        ("overlay.rs", "GpuiOverlayState"),
        ("scroll_area.rs", "ScrollHandle"),
        ("text_input.rs", "Entity<TextInputController>"),
        ("text_input.rs", "EntityInputHandler"),
        ("text_input.rs", "TextInputController"),
        ("textarea.rs", "EntityInputHandler"),
    ];
    let mut expected = expected
        .into_iter()
        .map(|(file, token)| PublicSurfaceBlocker::new(file.to_owned(), token.to_owned()))
        .collect::<Vec<_>>();
    expected.sort();

    let mut actual = public_surface_blockers(&[
        "BoxShadow",
        "Entity<TextInputController>",
        "EntityInputHandler",
        "GpuiOverlayState",
        "ScrollHandle",
        "TextInputController",
        "focus_ring_shadow",
    ]);
    actual.sort();

    assert_eq!(
        actual, expected,
        "adapter-only public surfaces changed; update this inventory as U6 classifies or narrows GPUI-specific APIs"
    );
}

#[test]
fn gpui_adapter_exports_group_runtime_specific_surfaces() {
    use open_gpui_ui_components::{self as root, prelude};

    let module_text_input = root::text_input::TextInput::new("module-text-input", "Module input");
    let _module_state: root::text_input::TextInputState = module_text_input.state();
    let _module_colors: Option<root::text_input::TextInputColors> = None;
    let _module_metrics: Option<root::text_input::TextInputMetrics> = None;
    let _module_display_mode: root::text_input::TextInputDisplayMode =
        root::text_input::TextInputDisplayMode::Plain;
    let module_textarea = root::textarea::Textarea::new("module-textarea", "Module textarea");
    let _module_textarea_state: root::textarea::TextareaState = module_textarea.state();
    let _module_textarea_colors: Option<root::textarea::TextareaColors> = None;
    let _module_textarea_metrics: Option<root::textarea::TextareaMetrics> = None;

    let root_overlay = root::gpui_adapter::GpuiOverlayAdapterConfig::new(
        OverlayLayerKind::Tooltip,
        OverlayPresence::open(),
    )
    .state();

    let _root_init: fn(&mut open_gpui::App) = root::gpui_adapter::init_text_input;
    let _root_controller: Option<root::gpui_adapter::TextInputController> = None;
    let _root_px: fn(UiPx) -> open_gpui::Pixels = root::gpui_adapter::gpui_px_from_ui;
    let _root_point: fn(UiPoint) -> open_gpui::Point<open_gpui::Pixels> =
        root::gpui_adapter::gpui_point_from_ui;
    let _root_size: fn(UiSize) -> open_gpui::Size<open_gpui::Pixels> =
        root::gpui_adapter::gpui_size_from_ui;
    let _prelude_button: prelude::Button = prelude::Button::new("save", "Save");
    let _prelude_textarea: prelude::Textarea = prelude::Textarea::new("notes", "Notes");
    let _prelude_display_mode: prelude::TextInputDisplayMode = prelude::TextInputDisplayMode::Plain;

    assert_eq!(
        root_overlay.deferred_priority(),
        root::gpui_adapter::default_deferred_priority(OverlayLayerKind::Tooltip)
    );
    assert_eq!(
        root_overlay.snap_margin(),
        root::gpui_adapter::DEFAULT_OVERLAY_SAFE_MARGIN
    );
    assert_eq!(
        root::gpui_adapter::focus_ring_shadow(FocusRing::from_color(ColorIntent::new(
            semantic::FOCUS_RING,
            0x2f80ed,
        )))[0]
            .spread_radius,
        px(2.0)
    );
}

#[test]
fn adapter_only_helpers_do_not_leak_from_default_exports() {
    let adapter_only_tokens = [
        "TextInputController",
        "init_text_input",
        "focus_ring_shadow",
        "GpuiOverlayState",
        "GpuiOverlayAdapterConfig",
        "gpui_px_from_ui",
    ];

    for file_name in ["lib.rs", "prelude.rs"] {
        let source =
            std::fs::read_to_string(format!("{}/src/{file_name}", env!("CARGO_MANIFEST_DIR")))
                .unwrap_or_else(|error| panic!("failed to read {file_name}: {error}"));
        let default_interface = if file_name == "lib.rs" {
            source_without_gpui_adapter_module(&source)
        } else {
            source
        };

        for token in adapter_only_tokens {
            assert!(
                !default_interface.contains(token),
                "{file_name} default interface must not expose adapter-only token `{token}`"
            );
        }
    }

    let text_input_source =
        std::fs::read_to_string(format!("{}/src/text_input.rs", env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_else(|error| panic!("failed to read text_input.rs: {error}"));
    assert!(text_input_source.contains("pub(crate) mod adapter"));
    assert!(
        !text_input_source.contains("pub use adapter"),
        "text_input must not re-export its internal adapter module"
    );
}

#[test]
fn public_reexports_stay_explicit_without_wildcards() {
    let mut wildcard_exports = Vec::new();
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = [
        ("ui_components/src/lib.rs", manifest_dir.join("src/lib.rs")),
        (
            "ui_components/src/prelude.rs",
            manifest_dir.join("src/prelude.rs"),
        ),
        (
            "ui_core/src/lib.rs",
            manifest_dir.join("../ui_core/src/lib.rs"),
        ),
        (
            "ui_core/src/prelude.rs",
            manifest_dir.join("../ui_core/src/prelude.rs"),
        ),
    ];

    for (file_name, path) in files {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));

        for (line_number, line) in source.lines().enumerate() {
            if line.contains("pub use ") && line.contains("::*") {
                wildcard_exports.push(format!("{file_name}:{}", line_number + 1));
            }
        }
    }

    assert_eq!(
        wildcard_exports,
        Vec::<String>::new(),
        "public re-exports must stay explicit, including adapter-only groupings"
    );
}

fn public_primitive_modules_from_mod() -> Vec<String> {
    let source_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/primitives/mod.rs");
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {source_path:?}: {error}"));
    let mut modules = source
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("pub mod ")
                .and_then(|module| module.strip_suffix(';'))
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    modules.sort();
    modules
}

#[test]
fn crate_root_and_prelude_reexports_stay_intentionally_aligned() {
    let root_exports = default_reexport_tokens("lib.rs");
    let prelude_exports = default_reexport_tokens("prelude.rs");
    let root_only = root_exports
        .difference(&prelude_exports)
        .cloned()
        .collect::<Vec<_>>();
    let prelude_only = prelude_exports
        .difference(&root_exports)
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(
        root_only,
        Vec::<String>::new(),
        "crate root exports tokens not exposed through prelude; update prelude.rs or document the intentional root-only token here"
    );
    assert_eq!(
        prelude_only,
        vec![
            "ActiveDescendant".to_string(),
            "CollectionPosition".to_string(),
            "ControllableState".to_string(),
            "Sizable".to_string(),
            "Size".to_string(),
            "ThemeTokens".to_string(),
            "UiA11yElementExt".to_string(),
        ],
        "prelude-only exports must stay intentional; update the allowlist when the convenience prelude grows"
    );
}

fn default_reexport_tokens(file_name: &str) -> std::collections::BTreeSet<String> {
    let source = std::fs::read_to_string(format!("{}/src/{file_name}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|error| panic!("failed to read {file_name}: {error}"));
    let source = if file_name == "lib.rs" {
        source_without_gpui_adapter_module(&source)
    } else {
        source
    };
    let mut exports = std::collections::BTreeSet::new();
    let mut statement = String::new();
    let mut collecting = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if collecting {
            statement.push(' ');
            statement.push_str(trimmed);
        } else if trimmed.starts_with("pub use ") {
            statement.clear();
            statement.push_str(trimmed);
            collecting = true;
        }

        if collecting && trimmed.ends_with(';') {
            collect_public_reexport_tokens(&statement, &mut exports);
            statement.clear();
            collecting = false;
        }
    }

    exports
}

fn collect_public_reexport_tokens(
    statement: &str,
    exports: &mut std::collections::BTreeSet<String>,
) {
    let statement = statement.trim().trim_end_matches(';');
    let Some(rest) = statement.strip_prefix("pub use ") else {
        return;
    };
    if rest.contains("::*") {
        return;
    }

    if let Some((_, group)) = rest.split_once("::{") {
        let group = group.trim_end_matches('}');
        for item in group.split(',') {
            collect_public_reexport_token(item, exports);
        }
    } else {
        collect_public_reexport_token(rest, exports);
    }
}

fn collect_public_reexport_token(item: &str, exports: &mut std::collections::BTreeSet<String>) {
    let token = item.trim();
    if token.is_empty() {
        return;
    }

    let exported_name = token
        .split_once(" as ")
        .map(|(_, alias)| alias.trim())
        .unwrap_or(token)
        .rsplit("::")
        .next()
        .unwrap_or(token)
        .trim();

    if !exported_name.is_empty() {
        exports.insert(exported_name.to_owned());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PublicContractBlocker {
    file: String,
    contract: String,
    token: String,
}

impl PublicContractBlocker {
    fn new(file: String, contract: String, token: String) -> Self {
        Self {
            file,
            contract,
            token,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PublicSurfaceBlocker {
    file: String,
    token: String,
}

impl PublicSurfaceBlocker {
    fn new(file: String, token: String) -> Self {
        Self { file, token }
    }
}

struct PublicContractStruct<'a> {
    name: &'a str,
    fields: &'a str,
}

fn public_contract_structs<'a>(
    source: &'a str,
    suffixes: &[&str],
) -> Vec<PublicContractStruct<'a>> {
    let mut states = Vec::new();
    let mut search_from = 0;

    while let Some(relative_start) = source[search_from..].find("pub struct ") {
        let start = search_from + relative_start;
        let name_start = start + "pub struct ".len();
        let name_end = source[name_start..]
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .map(|offset| name_start + offset)
            .unwrap_or(source.len());
        let name = &source[name_start..name_end];

        search_from = name_end;
        if !suffixes.iter().any(|suffix| name.ends_with(suffix)) {
            continue;
        }
        if ["EmptyState"].contains(&name) {
            continue;
        }

        let Some(open_brace) = source[name_end..].find('{').map(|offset| name_end + offset) else {
            continue;
        };
        let Some(close_brace) = matching_brace(source, open_brace) else {
            continue;
        };

        states.push(PublicContractStruct {
            name,
            fields: &source[open_brace + 1..close_brace],
        });
        search_from = close_brace + 1;
    }

    states
}

fn public_contract_extraction_blockers(tokens: &[&str]) -> Vec<PublicContractBlocker> {
    let mut blockers = Vec::new();
    for source_file in ui_component_source_files() {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        for contract in public_contract_structs(&source, &["State", "Metrics"]) {
            let fields = uncommented_lines(contract.fields);
            for token in tokens {
                if fields.contains(token) {
                    blockers.push(PublicContractBlocker::new(
                        file_name.to_owned(),
                        contract.name.to_owned(),
                        (*token).to_owned(),
                    ));
                }
            }
        }
    }

    blockers
}

fn public_surface_blockers(tokens: &[&str]) -> Vec<PublicSurfaceBlocker> {
    let mut blockers = Vec::new();
    for source_file in ui_component_source_files() {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        let source = if matches!(file_name, "lib.rs" | "prelude.rs") {
            source_without_gpui_adapter_module(&source)
        } else {
            source
        };
        let surface = public_api_surface(&uncommented_lines(&source));

        for token in tokens {
            if surface.contains(token) {
                blockers.push(PublicSurfaceBlocker::new(
                    file_name.to_owned(),
                    (*token).to_owned(),
                ));
            }
        }
    }

    blockers
}

fn source_without_gpui_adapter_module(source: &str) -> String {
    let Some((module_start, close_brace)) = public_module_bounds(source, "gpui_adapter") else {
        return source.to_owned();
    };

    let mut stripped = String::with_capacity(source.len());
    stripped.push_str(&source[..module_start]);
    stripped.push_str(&source[close_brace + 1..]);
    stripped
}

fn public_module_source<'a>(source: &'a str, module_name: &str) -> Option<&'a str> {
    let (module_start, close_brace) = public_module_bounds(source, module_name)?;
    Some(&source[module_start..=close_brace])
}

fn public_module_bounds(source: &str, module_name: &str) -> Option<(usize, usize)> {
    let module_marker = format!("pub mod {module_name}");
    let module_start = source.find(&module_marker)?;
    let open_brace = source[module_start..]
        .find('{')
        .map(|offset| module_start + offset)?;
    let close_brace = matching_brace(source, open_brace)?;
    Some((module_start, close_brace))
}

fn public_api_surface(source: &str) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let mut surface = Vec::new();
    let mut line_index = 0usize;

    while line_index < lines.len() {
        let line = lines[line_index];
        let trimmed = line.trim_start();

        if trimmed.starts_with("pub use ") {
            while line_index < lines.len() {
                let signature_line = lines[line_index];
                surface.push(signature_line);
                line_index += 1;
                if signature_line.contains(';') {
                    break;
                }
            }
            continue;
        }

        if trimmed.starts_with("pub fn ") {
            while line_index < lines.len() {
                let signature_line = lines[line_index];
                surface.push(signature_line);
                line_index += 1;
                if signature_line.contains('{') || signature_line.contains(';') {
                    break;
                }
            }
            continue;
        }

        if trimmed.starts_with("pub const ")
            || trimmed.starts_with("pub type ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("impl EntityInputHandler for ")
        {
            surface.push(line);
            line_index += 1;
            continue;
        }

        line_index += 1;
    }

    surface.join("\n")
}

fn matching_brace(source: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;

    for (offset, ch) in source[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open_brace + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn uncommented_lines(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with("///")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
