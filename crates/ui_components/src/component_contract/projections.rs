//! Projection APIs over canonical component contract rows.

use super::{
    COMPONENT_CONTRACT_ROWS, ComponentApiInventoryEntry, ComponentContractEntry,
    PublicSurfaceOwnerEntry, SurfaceDocsStatus, SurfaceGalleryStatus,
};

/// Returns the canonical product metadata row for a public surface token.
pub const fn component_contract_entry(name: &str) -> Option<&'static ComponentContractEntry> {
    let mut index = 0;
    while index < COMPONENT_CONTRACT_ROWS.len() {
        if token_eq(COMPONENT_CONTRACT_ROWS[index].name, name) {
            return Some(&COMPONENT_CONTRACT_ROWS[index]);
        }
        index += 1;
    }

    None
}

/// Returns whether an API inventory component is intended for root/prelude defaults.
pub fn component_inventory_default_export(entry: &ComponentApiInventoryEntry) -> bool {
    component_contract_entry(entry.component).is_some_and(|entry| entry.default_export)
}

/// Returns whether an adjacent public surface is intended for root/prelude defaults.
pub fn public_surface_default_export(entry: &PublicSurfaceOwnerEntry) -> bool {
    component_contract_entry(entry.name).is_some_and(|entry| entry.default_export)
}

/// Returns the contract-owned gallery status for a component or adjacent surface.
pub const fn component_contract_gallery_status(name: &str) -> SurfaceGalleryStatus {
    match component_contract_entry(name) {
        Some(entry) => entry.gallery_status,
        None => SurfaceGalleryStatus::NotInGallery,
    }
}

/// Returns the contract-owned component family or ownership group.
pub const fn component_contract_family(name: &str) -> Option<&'static str> {
    match component_contract_entry(name) {
        Some(entry) => entry.family,
        None => None,
    }
}

/// Returns the primary contract-owned source home for a public surface.
pub const fn component_contract_source_home(name: &str) -> Option<&'static str> {
    match component_contract_entry(name) {
        Some(entry) => Some(entry.source_home),
        None => None,
    }
}

/// Returns whether the surface should be exported through root and prelude defaults.
pub const fn component_contract_default_export(name: &str) -> bool {
    match component_contract_entry(name) {
        Some(entry) => entry.default_export,
        None => false,
    }
}

/// Returns the contract-owned docs coverage status for a public surface.
pub const fn component_contract_docs_status(name: &str) -> Option<SurfaceDocsStatus> {
    match component_contract_entry(name) {
        Some(entry) => Some(entry.docs_status),
        None => None,
    }
}

/// Returns the docs token that should prove documentation coverage for a public surface.
pub const fn component_contract_docs_token(name: &str) -> Option<&'static str> {
    match component_contract_entry(name) {
        Some(entry) => entry.docs_token,
        None => None,
    }
}

const fn token_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }

    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }

    true
}
