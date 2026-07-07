use crate::color::{ColorIntent, ColorState};
use crate::focus::FocusRing;
use open_gpui_ui_core::{Size, ThemeTokens, UiPx, ui_px};

/// Resolved virtualized-list color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualizedListColors {
    surface: ColorIntent,
    foreground: ColorIntent,
    muted_foreground: ColorIntent,
    border: ColorIntent,
    row_background: ColorIntent,
    row_alternate_background: ColorIntent,
    row_hover_background: ColorIntent,
    row_active_background: ColorIntent,
    row_selected_background: ColorIntent,
    row_disabled_foreground: ColorIntent,
    separator: ColorIntent,
    badge_background: ColorIntent,
    badge_foreground: ColorIntent,
    status_foreground: ColorIntent,
    active_indicator: ColorIntent,
    active_indicator_moving: ColorIntent,
    sticky_overlay_background: ColorIntent,
    sticky_overlay_foreground: ColorIntent,
    focus_ring: ColorIntent,
}

impl VirtualizedListColors {
    /// Resolves virtualized-list colors from shared theme tokens.
    pub const fn from_tokens(tokens: ThemeTokens) -> Self {
        Self {
            surface: ColorIntent::new(tokens.surface, 0xffffff),
            foreground: ColorIntent::new(tokens.text, 0x2f3845),
            muted_foreground: ColorIntent::new(tokens.text_muted, 0x667085),
            border: ColorIntent::new(tokens.border, 0xe2e4dc),
            row_background: ColorIntent::new(tokens.surface, 0xffffff),
            row_alternate_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Default,
                0xf8f9f3,
            ),
            row_hover_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Hover,
                0xeef2f7,
            ),
            row_active_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::FocusVisible,
                0xeef2f7,
            ),
            row_selected_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Selected,
                0xe7f0ff,
            ),
            row_disabled_foreground: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Disabled,
                0x8b93a1,
            ),
            separator: ColorIntent::new(tokens.border, 0xe2e4dc),
            badge_background: ColorIntent::with_state(
                tokens.surface_muted,
                ColorState::Selected,
                0xeef2f7,
            ),
            badge_foreground: ColorIntent::new(tokens.text_muted, 0x475467),
            status_foreground: ColorIntent::with_state(
                tokens.text_muted,
                ColorState::Message,
                0x475467,
            ),
            active_indicator: ColorIntent::new(tokens.focus_ring, 0x2f80ed),
            active_indicator_moving: ColorIntent::with_state(
                tokens.accent,
                ColorState::FocusVisible,
                0x2563eb,
            ),
            sticky_overlay_background: ColorIntent::with_state(
                tokens.surface,
                ColorState::Overlay,
                0xffffff,
            ),
            sticky_overlay_foreground: ColorIntent::new(tokens.text, 0x2f3845),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                0x2f80ed,
            ),
        }
    }

    /// Returns root surface color intent.
    pub const fn surface(self) -> ColorIntent {
        self.surface
    }

    /// Returns foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns muted foreground color intent.
    pub const fn muted_foreground(self) -> ColorIntent {
        self.muted_foreground
    }

    /// Returns border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns default row background color intent.
    pub const fn row_background(self) -> ColorIntent {
        self.row_background
    }

    /// Returns alternate row background color intent.
    pub const fn row_alternate_background(self) -> ColorIntent {
        self.row_alternate_background
    }

    /// Returns hover row background color intent.
    pub const fn row_hover_background(self) -> ColorIntent {
        self.row_hover_background
    }

    /// Returns active row background color intent.
    pub const fn row_active_background(self) -> ColorIntent {
        self.row_active_background
    }

    /// Returns selected row background color intent.
    pub const fn row_selected_background(self) -> ColorIntent {
        self.row_selected_background
    }

    /// Returns disabled row foreground color intent.
    pub const fn row_disabled_foreground(self) -> ColorIntent {
        self.row_disabled_foreground
    }

    /// Returns separator color intent.
    pub const fn separator(self) -> ColorIntent {
        self.separator
    }

    /// Returns badge background color intent.
    pub const fn badge_background(self) -> ColorIntent {
        self.badge_background
    }

    /// Returns badge foreground color intent.
    pub const fn badge_foreground(self) -> ColorIntent {
        self.badge_foreground
    }

    /// Returns status foreground color intent.
    pub const fn status_foreground(self) -> ColorIntent {
        self.status_foreground
    }

    /// Returns idle active-indicator color intent.
    pub const fn active_indicator(self) -> ColorIntent {
        self.active_indicator
    }

    /// Returns moving active-indicator color intent.
    pub const fn active_indicator_moving(self) -> ColorIntent {
        self.active_indicator_moving
    }

    /// Returns sticky overlay background color intent.
    pub const fn sticky_overlay_background(self) -> ColorIntent {
        self.sticky_overlay_background
    }

    /// Returns sticky overlay foreground color intent.
    pub const fn sticky_overlay_foreground(self) -> ColorIntent {
        self.sticky_overlay_foreground
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }

    /// Returns focus-ring metadata for the root listbox surface.
    pub const fn focus_ring_shape(self) -> FocusRing {
        FocusRing::new(self.focus_ring, ui_px(1.0))
    }
}

/// Resolved virtualized-list metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualizedListMetrics {
    row_height: UiPx,
    overscan_count: usize,
}

impl VirtualizedListMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            row_height: size.list_row_h(),
            overscan_count: match size {
                Size::XSmall | Size::Small => 4,
                Size::Medium => 5,
                Size::Large => 6,
            },
        }
    }

    /// Returns the default fixed row height.
    pub const fn row_height(self) -> UiPx {
        self.row_height
    }

    /// Returns the number of rows the adapter should keep beyond the viewport.
    pub const fn overscan_count(self) -> usize {
        self.overscan_count
    }

    /// Returns the same metrics with a different row height.
    pub fn with_row_height(mut self, row_height: UiPx) -> Self {
        self.row_height = nonnegative_px(row_height);
        self
    }

    /// Returns the same metrics with a different overscan budget.
    pub const fn with_overscan_count(mut self, overscan_count: usize) -> Self {
        self.overscan_count = overscan_count;
        self
    }
}

pub(super) const DEFAULT_VIRTUALIZED_LIST_VIEWPORT_ITEM_COUNT: usize = 8;

pub(super) const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}
