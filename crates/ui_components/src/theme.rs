//! Theme snapshots, registries, and component color recipes.

mod palette;
mod recipes;
mod registry;
mod resolver;
mod snapshot;

pub use registry::{
    ThemeDefinition, ThemeRegistrationDiagnostics, ThemeRegistry, ThemeRegistryEntry,
    ThemeValidationError,
};
pub use resolver::ThemeResolver;
pub use snapshot::{ThemeColor, ThemeMode, ThemeSnapshot};
