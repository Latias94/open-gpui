//! Source-owner projection helpers.

use super::component_contract_entry;

/// Returns source file or module paths that own the component implementation.
pub fn component_source_inputs(component: &str) -> &'static [&'static str] {
    component_contract_entry(component)
        .map(|entry| entry.source_inputs)
        .unwrap_or_else(|| panic!("missing source file mapping for `{component}`"))
}

/// Returns table submodules that own split render behavior.
pub fn table_render_owner_files() -> &'static [&'static str] {
    &[
        "table/body/mod.rs",
        "table/cell.rs",
        "table/editors.rs",
        "table/header.rs",
        "table/resize.rs",
    ]
}

/// Normalizes a source mapping entry into the manifest home path.
pub fn component_source_home(source_entry: &'static str) -> &'static str {
    source_entry
}
