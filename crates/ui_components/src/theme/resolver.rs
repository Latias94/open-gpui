use open_gpui::{App, Rgba};

use crate::color::ColorIntent;

use super::runtime::{ThemeContext, try_theme_context};
use super::snapshot::ThemeSnapshot;

/// Theme resolution namespace for component color intents.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThemeResolver;

impl ThemeResolver {
    /// Returns the current app theme context, or the default light context when no runtime exists.
    pub fn current(cx: &App) -> ThemeContext<'_> {
        match try_theme_context(cx) {
            Some(context) => context,
            None => ThemeContext::light(),
        }
    }

    /// Resolves a color intent with the legacy default light theme snapshot.
    ///
    /// Production render paths should prefer [`Self::current`] or [`Self::resolve_with`].
    pub fn resolve(intent: ColorIntent) -> Rgba {
        Self::resolve_with(intent, ThemeSnapshot::light())
    }

    /// Resolves a color intent with an explicit theme snapshot.
    pub fn resolve_with(intent: ColorIntent, theme: ThemeSnapshot<'_>) -> Rgba {
        theme.resolve(intent)
    }

    /// Resolves a color intent by using only the fallback RGB.
    pub fn resolve_fallback(intent: ColorIntent) -> Rgba {
        open_gpui::rgb(intent.fallback_rgb())
    }
}
