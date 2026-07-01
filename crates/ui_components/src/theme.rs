//! Theme snapshots, registries, and component color recipes.

mod palette;
mod recipes;
mod registry;
mod resolver;
mod schema;
mod snapshot;

pub use registry::{
    ThemeDefinition, ThemeRegistrationDiagnostics, ThemeRegistry, ThemeRegistryEntry,
    ThemeValidationError,
};
pub use resolver::ThemeResolver;
pub use schema::{
    THEME_JSON_SCHEMA_VERSION, ThemeFileField, ThemeLoadError, register_theme_json_file,
    register_theme_json_str, theme_definition_from_json_file, theme_definition_from_json_str,
    theme_json_schema,
};
pub use snapshot::{ThemeColor, ThemeMode, ThemeSnapshot};
