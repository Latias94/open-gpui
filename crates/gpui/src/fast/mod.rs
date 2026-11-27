//! Retained layout and frame output for entity-backed views.

#[cfg(not(target_family = "wasm"))]
pub(crate) mod a11y;
pub(crate) mod dependencies;
pub(crate) mod layout;
pub(crate) mod layout_key;
pub(crate) mod retained;

#[cfg(test)]
mod oracle_tests;
