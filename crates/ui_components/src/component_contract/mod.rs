//! Typed component-library contract registry.
//!
//! The registry records product-level component intent for public-surface tests,
//! gallery dogfood consumers, documentation checks, and future component tooling.
//! It is deliberately renderer-neutral: rows name GPUI adapter helpers, but they
//! do not store GPUI runtime handles, callbacks, elements, windows, or contexts.

mod api_inventory;
mod projections;
mod rows;
mod source_mapping;
mod surfaces;
mod types;

pub use api_inventory::{
    COMPONENT_API_INVENTORY, component_public_methods, component_render_inputs,
    public_owner_for_component_inventory,
};
pub use projections::{
    component_contract_default_export, component_contract_docs_status,
    component_contract_docs_token, component_contract_entry, component_contract_family,
    component_contract_gallery_status, component_contract_source_home,
    component_inventory_default_export, public_surface_default_export,
};
pub use rows::{
    COMPONENT_CONTRACT_REGISTRY, COMPONENT_RECIPE_COMPONENTS, OFFICIAL_OVERLAY_COMPONENTS,
};
pub use source_mapping::{
    component_source_home, component_source_inputs, table_render_owner_files,
};
pub use surfaces::PUBLIC_SURFACE_OWNER_MAP;
pub use types::{
    CallbackApi, ComponentApiInventoryEntry, ComponentContractEntry, DefaultSeedApi,
    PublicSurfaceOwnerClass, PublicSurfaceOwnerEntry, SurfaceDocsStatus, SurfaceGalleryStatus,
};
