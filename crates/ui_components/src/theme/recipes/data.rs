use open_gpui_ui_core::ThemeTokens;

use crate::color::ColorIntent;
use crate::table::TableToolbarColors;
use crate::theme::palette::{DEFAULT_TEXT, DEFAULT_TEXT_MUTED};
use crate::theme::resolver::ThemeResolver;

impl ThemeResolver {
    pub(crate) const fn table_toolbar_colors(tokens: ThemeTokens) -> TableToolbarColors {
        TableToolbarColors {
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
        }
    }
}
