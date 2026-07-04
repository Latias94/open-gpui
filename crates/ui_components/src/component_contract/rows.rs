//! Component contract row facade.

#[path = "rows/catalog.rs"]
mod catalog;
#[path = "rows/lists.rs"]
mod lists;

pub use catalog::COMPONENT_CONTRACT_ROWS;
pub use lists::{COMPONENT_RECIPE_COMPONENTS, OFFICIAL_OVERLAY_COMPONENTS};
