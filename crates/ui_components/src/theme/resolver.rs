use open_gpui::{App, Rgba, Window};

use crate::color::ColorIntent;

use super::runtime::{ThemeContext, current_theme_context};
use super::snapshot::ThemeSnapshot;

/// Theme resolution namespace for component color intents.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThemeResolver;

impl ThemeResolver {
    /// Resolves the nearest subtree, window, app, or built-in theme for the current render path.
    pub fn current(window: &mut Window, cx: &mut App) -> ThemeContext {
        current_theme_context(window, cx)
    }

    /// Resolves a color intent with an explicit theme snapshot.
    pub fn resolve_with(intent: ColorIntent, theme: ThemeSnapshot<'_>) -> Rgba {
        theme.resolve(intent)
    }
}
