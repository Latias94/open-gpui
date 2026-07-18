//! Theme snapshots, registries, and component color recipes.

mod palette;
mod recipes;
mod registry;
mod resolver;
mod runtime;
mod schema;
mod scope;
mod snapshot;

pub use registry::{
    ThemeDefinition, ThemeRegistrationDiagnostics, ThemeRegistry, ThemeRegistryEntry,
    ThemeValidationError,
};
pub use resolver::ThemeResolver;
pub use runtime::{
    DARK_THEME_ID, DEFAULT_THEME_ID, HIGH_CONTRAST_THEME_ID, LIGHT_THEME_ID, ThemeContext,
    ThemeSelectionError, app_theme_context, app_theme_id, clear_window_theme,
    install_theme_registry, override_window_theme, register_theme, registered_theme_context,
    set_app_theme, set_app_theme_mode, set_window_theme, theme_id_for_mode, theme_registry,
};
pub use schema::{
    THEME_JSON_SCHEMA_VERSION, ThemeFileField, ThemeLoadError, register_theme_json_file,
    register_theme_json_str, theme_definition_from_json_file, theme_definition_from_json_str,
    theme_json_schema,
};
pub use scope::ThemeScope;
pub(crate) use scope::scoped_theme_view_builder;
pub use snapshot::{ThemeColor, ThemeMode, ThemeSnapshot};
