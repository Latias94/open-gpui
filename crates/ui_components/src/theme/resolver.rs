use open_gpui::{App, Rgba, Window};

use crate::color::ColorIntent;

use super::runtime::{ThemeContext, current_theme_context, current_theme_snapshot};
use super::snapshot::ThemeSnapshot;

/// Theme resolution namespace for component color intents.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThemeResolver;

impl ThemeResolver {
    /// Resolves the nearest subtree, window, app, or built-in theme for the current render path.
    pub fn current(window: &mut Window, cx: &mut App) -> ThemeContext {
        current_theme_context(window, cx)
    }

    /// Resolves the nearest subtree, window, app, or built-in immutable theme snapshot.
    ///
    /// This read-only variant never initializes window state, updates entities, notifies,
    /// dispatches, or schedules a refresh. It is suitable for pure render-time adapters.
    pub fn current_snapshot(window: &Window, cx: &App) -> ThemeSnapshot {
        current_theme_snapshot(window, cx)
    }

    /// Resolves a color intent with an explicit theme snapshot.
    pub fn resolve_with(intent: ColorIntent, theme: &ThemeSnapshot) -> Rgba {
        theme.resolve(intent)
    }
}
