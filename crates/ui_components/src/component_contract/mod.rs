//! Federated component product metadata and typed owner projections.
//!
//! Product rows own only stable component identity, revision, family, and required scenario ids.
//! Public modules own exports, Gallery owns selectors and probes, native test artifacts own
//! executable coordinates, and DevTools receives immutable metadata through this interface.

mod projections;
mod rows;
mod types;

pub use projections::{
    common_public_exports, component_contract_entry, component_contract_metadata,
    default_public_exports, diagnostic_public_exports,
};
pub use rows::{COMPONENT_CONTRACT_GLOBAL_SCENARIOS, COMPONENT_CONTRACT_ROWS};
pub use types::{
    ComponentContractEntry, ComponentContractId, ComponentContractMetadata,
    ComponentContractRevision, ComponentFamily, PublicApiExport, PublicApiTier,
};
