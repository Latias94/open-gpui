//! Command visual token resolution and sizing metrics.

use crate::color::ColorIntent;
use open_gpui_ui_core::{Size, UiPx, ui_px};

pub(crate) const DEFAULT_COMMAND_VIEWPORT_ITEM_COUNT: usize = 8;

pub(crate) const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}

/// Resolved command color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandColors {
    pub(crate) surface: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) muted_foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) shortcut_foreground: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl CommandColors {
    /// Returns surface color intent.
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

    /// Returns shortcut label color intent.
    pub const fn shortcut_foreground(self) -> ColorIntent {
        self.shortcut_foreground
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved command metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandMetrics {
    padding: UiPx,
    radius: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    max_height: UiPx,
    row_height: UiPx,
    overscan_count: usize,
    shortcut_min_width: UiPx,
}

impl CommandMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            padding: ui_px(6.0),
            radius: size.control_radius(),
            min_width: ui_px(320.0),
            max_width: ui_px(560.0),
            max_height: match size {
                Size::XSmall => ui_px(220.0),
                Size::Small => ui_px(260.0),
                Size::Medium => ui_px(340.0),
                Size::Large => ui_px(420.0),
            },
            row_height: size.list_row_h(),
            overscan_count: match size {
                Size::XSmall | Size::Small => 4,
                Size::Medium => 6,
                Size::Large => 8,
            },
            shortcut_min_width: match size {
                Size::XSmall | Size::Small => ui_px(48.0),
                Size::Medium => ui_px(64.0),
                Size::Large => ui_px(76.0),
            },
        }
    }

    /// Returns panel padding.
    pub const fn padding(self) -> UiPx {
        self.padding
    }

    /// Returns panel radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns minimum panel width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns maximum panel width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }

    /// Returns maximum command list height.
    pub const fn max_height(self) -> UiPx {
        self.max_height
    }

    /// Returns the fixed command result row height used by the virtualizer.
    pub const fn row_height(self) -> UiPx {
        self.row_height
    }

    /// Returns the number of rows kept beyond the visible command result viewport.
    pub const fn overscan_count(self) -> usize {
        self.overscan_count
    }

    /// Returns minimum shortcut label width.
    pub const fn shortcut_min_width(self) -> UiPx {
        self.shortcut_min_width
    }

    /// Returns the same metrics with a different fixed result row height.
    pub fn with_row_height(mut self, row_height: UiPx) -> Self {
        self.row_height = nonnegative_px(row_height);
        self
    }

    /// Returns the same metrics with a different overscan row budget.
    pub const fn with_overscan_count(mut self, overscan_count: usize) -> Self {
        self.overscan_count = overscan_count;
        self
    }
}
