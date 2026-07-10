//! Adjacent public surface ownership rows.

use super::{PublicSurfaceOwnerClass, PublicSurfaceOwnerEntry};

/// Public adjacent surfaces that are not primary component inventory rows.
pub const PUBLIC_SURFACE_OWNER_MAP: &[PublicSurfaceOwnerEntry] = &[
    PublicSurfaceOwnerEntry {
        name: "TreeState",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "tree/model.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "VirtualizedListState",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "virtualized_list/model.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "FormControlState",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "form_control.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "GpuiOverlayAdapterConfig",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "overlay/adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "GpuiOverlayState",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "overlay/adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "TextInputController",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "FormProjection",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "form_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "FormFieldConfig",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "form_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "FormFieldProjection",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "form_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "form_text_value",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "form_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "form_number_value",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "form_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "form_checkbox_value",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "form_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "form_select_value",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "form_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "ResourceAdapterLabels",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "resource_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "ResourceCollectionProjection",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "resource_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "ResourceMutationProjection",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "resource_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "resource_query_key_label",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "resource_adapter.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "init_text_input",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "VirtualizedListGpuiExt",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "UiA11yElementExt",
        owner: PublicSurfaceOwnerClass::GpuiAdapterHelper,
        home: "gpui_adapter",
    },
    PublicSurfaceOwnerEntry {
        name: "focus_ring_shadow_with_theme",
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
        home: "table/behavior/mod.rs",
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
        home: "virtualized_list/render_plan.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "VirtualizedListStickySectionSnapshot",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "virtualized_list/render_plan.rs",
    },
    PublicSurfaceOwnerEntry {
        name: "VirtualizedListStickyOverlaySnapshot",
        owner: PublicSurfaceOwnerClass::RendererNeutralStateContract,
        home: "virtualized_list/render_plan.rs",
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
        owner: PublicSurfaceOwnerClass::DeprecatedRemovalTarget,
        home: "removed",
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
        owner: PublicSurfaceOwnerClass::DeprecatedRemovalTarget,
        home: "removed",
    },
];
