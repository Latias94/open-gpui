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
