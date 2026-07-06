//! Theme snapshots, registries, and component color recipes.

mod palette;
mod recipes;
mod registry;
mod resolver;
mod runtime;
mod schema;
mod snapshot;

pub use registry::{
    ThemeDefinition, ThemeRegistrationDiagnostics, ThemeRegistry, ThemeRegistryEntry,
    ThemeValidationError,
};
pub use resolver::ThemeResolver;
pub use runtime::{
    DARK_THEME_ID, DEFAULT_THEME_ID, HIGH_CONTRAST_THEME_ID, LIGHT_THEME_ID, ThemeContext,
    ThemeRuntime, ThemeRuntimeError, current_theme_context, init_theme_runtime, set_active_theme,
    set_active_theme_mode, theme_id_for_mode, try_theme_context,
};
pub use schema::{
    THEME_JSON_SCHEMA_VERSION, ThemeFileField, ThemeLoadError, register_theme_json_file,
    register_theme_json_str, theme_definition_from_json_file, theme_definition_from_json_str,
    theme_json_schema,
};
pub use snapshot::{ThemeColor, ThemeMode, ThemeSnapshot};
