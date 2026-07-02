use open_gpui::{App, Global, Rgba};

use crate::color::ColorIntent;

use super::registry::{ThemeDefinition, ThemeRegistry, ThemeRegistryEntry, ThemeValidationError};
use super::snapshot::{ThemeMode, ThemeSnapshot};

/// Stable id for the built-in light theme.
pub const LIGHT_THEME_ID: &str = "light";

/// Stable id for the built-in dark theme.
pub const DARK_THEME_ID: &str = "dark";

/// Stable id for the built-in high-contrast theme.
pub const HIGH_CONTRAST_THEME_ID: &str = "high-contrast";

/// The default theme used when an application has not installed a runtime.
pub const DEFAULT_THEME_ID: &str = LIGHT_THEME_ID;

/// Failure returned when selecting a runtime theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeRuntimeError {
    /// No registered theme matched the requested id.
    UnknownThemeId(String),
}

impl ThemeRuntimeError {
    /// Returns the missing theme id.
    pub fn theme_id(&self) -> &str {
        match self {
            Self::UnknownThemeId(theme_id) => theme_id,
        }
    }
}

/// Render-time view over an immutable theme snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeContext<'a> {
    snapshot: ThemeSnapshot<'a>,
}

impl<'a> ThemeContext<'a> {
    /// Creates a context over an immutable theme snapshot.
    pub const fn new(snapshot: ThemeSnapshot<'a>) -> Self {
        Self { snapshot }
    }

    /// Creates a context over the built-in light theme.
    pub fn light() -> ThemeContext<'static> {
        ThemeContext::new(ThemeSnapshot::light())
    }

    /// Creates a context over the built-in dark theme.
    pub fn dark() -> ThemeContext<'static> {
        ThemeContext::new(ThemeSnapshot::dark())
    }

    /// Creates a context over the built-in high-contrast theme.
    pub fn high_contrast() -> ThemeContext<'static> {
        ThemeContext::new(ThemeSnapshot::high_contrast())
    }

    /// Returns the active immutable theme snapshot.
    pub const fn snapshot(self) -> ThemeSnapshot<'a> {
        self.snapshot
    }

    /// Returns the active theme mode.
    pub const fn mode(self) -> ThemeMode {
        self.snapshot.mode()
    }

    /// Returns the active theme revision.
    pub const fn revision(self) -> u64 {
        self.snapshot.revision()
    }

    /// Resolves a component color intent with this context.
    pub fn resolve(self, intent: ColorIntent) -> Rgba {
        self.snapshot.resolve(intent)
    }
}

impl<'a> From<ThemeSnapshot<'a>> for ThemeContext<'a> {
    fn from(snapshot: ThemeSnapshot<'a>) -> Self {
        Self::new(snapshot)
    }
}

/// GPUI app-global owner for component theme snapshots.
///
/// The runtime keeps the registry and the active theme id together so render
/// code can borrow a short-lived [`ThemeContext`] without copying color tables.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeRuntime {
    registry: ThemeRegistry,
    active_id: String,
}

impl Global for ThemeRuntime {}

impl ThemeRuntime {
    /// Creates a runtime from a registry and an active theme id.
    pub fn new(
        registry: ThemeRegistry,
        active_id: impl Into<String>,
    ) -> Result<Self, ThemeRuntimeError> {
        let active_id = active_id.into();
        if registry.entry(&active_id).is_none() {
            return Err(ThemeRuntimeError::UnknownThemeId(active_id));
        }
        Ok(Self {
            registry,
            active_id,
        })
    }

    /// Creates a runtime with built-in themes and the default active theme.
    pub fn with_builtins() -> Self {
        Self {
            registry: ThemeRegistry::with_builtins(),
            active_id: DEFAULT_THEME_ID.to_owned(),
        }
    }

    /// Returns the current app theme context, or the built-in light context when no runtime exists.
    pub fn current_context(cx: &App) -> ThemeContext<'_> {
        match try_theme_context(cx) {
            Some(context) => context,
            None => ThemeContext::light(),
        }
    }

    /// Returns the owned theme registry.
    pub const fn registry(&self) -> &ThemeRegistry {
        &self.registry
    }

    /// Returns the mutable owned registry.
    pub fn registry_mut(&mut self) -> &mut ThemeRegistry {
        &mut self.registry
    }

    /// Registers a user-supplied theme definition.
    pub fn register(
        &mut self,
        definition: ThemeDefinition,
    ) -> Result<&ThemeRegistryEntry, ThemeValidationError> {
        self.registry.register(definition)
    }

    /// Returns the active theme id.
    pub fn active_theme_id(&self) -> &str {
        &self.active_id
    }

    /// Returns the active immutable snapshot.
    pub fn active_snapshot(&self) -> ThemeSnapshot<'_> {
        self.registry
            .snapshot(&self.active_id)
            .expect("active theme id must refer to a registered theme")
    }

    /// Returns a render-time context for the active theme.
    pub fn context(&self) -> ThemeContext<'_> {
        ThemeContext::new(self.active_snapshot())
    }

    /// Selects a registered theme by id.
    pub fn set_active_theme(
        &mut self,
        theme_id: impl Into<String>,
    ) -> Result<(), ThemeRuntimeError> {
        let theme_id = theme_id.into();
        if self.registry.entry(&theme_id).is_none() {
            return Err(ThemeRuntimeError::UnknownThemeId(theme_id));
        }
        self.active_id = theme_id;
        Ok(())
    }

    /// Selects the built-in theme for a mode.
    pub fn set_active_mode(&mut self, mode: ThemeMode) -> Result<(), ThemeRuntimeError> {
        self.set_active_theme(theme_id_for_mode(mode))
    }
}

impl Default for ThemeRuntime {
    fn default() -> Self {
        Self::with_builtins()
    }
}

/// Returns the built-in theme id for a mode.
pub const fn theme_id_for_mode(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Light => LIGHT_THEME_ID,
        ThemeMode::Dark => DARK_THEME_ID,
        ThemeMode::HighContrast => HIGH_CONTRAST_THEME_ID,
    }
}

/// Installs the default component theme runtime if the app has none.
pub fn init_theme_runtime(cx: &mut App) {
    if !cx.has_global::<ThemeRuntime>() {
        cx.set_global(ThemeRuntime::default());
    }
}

/// Returns the current app theme context, installing the default runtime first.
pub fn current_theme_context(cx: &mut App) -> ThemeContext<'_> {
    init_theme_runtime(cx);
    cx.global::<ThemeRuntime>().context()
}

/// Returns the current app theme context if one has been installed.
pub fn try_theme_context(cx: &App) -> Option<ThemeContext<'_>> {
    cx.try_global::<ThemeRuntime>().map(ThemeRuntime::context)
}

/// Selects the active app theme, installing the default runtime first.
pub fn set_active_theme(
    cx: &mut App,
    theme_id: impl Into<String>,
) -> Result<(), ThemeRuntimeError> {
    init_theme_runtime(cx);
    cx.global_mut::<ThemeRuntime>().set_active_theme(theme_id)
}

/// Selects the active built-in app theme mode.
pub fn set_active_theme_mode(cx: &mut App, mode: ThemeMode) -> Result<(), ThemeRuntimeError> {
    set_active_theme(cx, theme_id_for_mode(mode))
}
