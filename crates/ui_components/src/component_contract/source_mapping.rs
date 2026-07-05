//! Source-owner projection helpers.

use super::component_contract_entry;

/// Returns source file or module paths that own the component implementation.
pub fn component_source_inputs(component: &str) -> &'static [&'static str] {
    component_contract_entry(component)
        .map(|entry| entry.source_inputs)
        .unwrap_or_else(|| panic!("missing source file mapping for `{component}`"))
}
