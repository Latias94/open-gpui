//! Typed component-library contract tables.
//!
//! The tables record product-level component intent for public-surface tests,
//! gallery dogfood consumers, documentation checks, and future component tooling.
//! It is deliberately renderer-neutral: rows name GPUI adapter helpers, but they
//! do not store GPUI runtime handles, callbacks, elements, windows, or contexts.

mod api_inventory;
mod evidence;
mod projections;
mod rows;
mod source_mapping;
mod surfaces;
mod types;

pub use api_inventory::{
    COMPONENT_API_INVENTORY, component_public_methods, component_render_inputs,
    public_owner_for_component_inventory,
};
pub use evidence::{COMPONENT_A11Y_EVIDENCE, COMPONENT_CONFORMANCE_GATES, component_a11y_evidence};
pub use projections::{
    component_contract_default_export, component_contract_docs_status,
    component_contract_docs_token, component_contract_entry, component_contract_family,
    component_contract_gallery_status, component_contract_source_home,
    component_inventory_default_export, component_recipe_component_rows,
    official_overlay_component_rows, public_surface_default_export,
};
pub use rows::COMPONENT_CONTRACT_ROWS;
pub use source_mapping::{
    component_source_home, component_source_inputs, table_render_owner_files,
};
pub use surfaces::PUBLIC_SURFACE_OWNER_MAP;
pub use types::{
    CallbackApi, ComponentA11yEvidence, ComponentApiInventoryEntry, ComponentConformanceGate,
    ComponentContractEntry, DefaultSeedApi, PublicSurfaceOwnerClass, PublicSurfaceOwnerEntry,
    SurfaceDocsStatus, SurfaceGalleryStatus,
};
