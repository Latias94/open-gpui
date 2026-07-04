//! Projection APIs over canonical component contract rows.

use super::{
    COMPONENT_CONTRACT_ROWS, ComponentContractEntry, PublicSurfaceOwnerClass, SurfaceGalleryStatus,
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

/// Returns rows exported through the crate root and prelude defaults.
pub fn default_surface_rows() -> impl Iterator<Item = &'static ComponentContractEntry> {
    COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.default_export)
}

/// Returns rows that are expected to appear in a gallery catalog or adjacent readout.
pub fn gallery_surface_rows() -> impl Iterator<Item = &'static ComponentContractEntry> {
    COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.gallery_status != SurfaceGalleryStatus::NotInGallery)
}

/// Returns official component rows shown by the Components gallery.
pub fn official_component_rows() -> impl Iterator<Item = &'static ComponentContractEntry> {
    COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.gallery_status == SurfaceGalleryStatus::OfficialComponent)
}

/// Returns official overlay rows derived from the canonical contract table.
pub fn official_overlay_component_rows() -> impl Iterator<Item = &'static ComponentContractEntry> {
    COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.gallery_status == SurfaceGalleryStatus::OfficialOverlay)
}

/// Returns component recipe rows derived from the canonical contract table.
pub fn component_recipe_component_rows() -> impl Iterator<Item = &'static ComponentContractEntry> {
    COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.owner == PublicSurfaceOwnerClass::OfficialComponentRecipe)
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
