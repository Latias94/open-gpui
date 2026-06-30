use open_gpui::Rgba;

use crate::color::ColorIntent;

use super::snapshot::ThemeSnapshot;

/// Theme resolution namespace for component color intents.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThemeResolver;

impl ThemeResolver {
    /// Resolves a color intent with the default light theme snapshot.
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
