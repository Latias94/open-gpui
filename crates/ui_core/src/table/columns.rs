//! Column contracts and sizing defaults for renderer-neutral tables.

use crate::geometry::{UiPx, ui_px};

/// Default preferred width for a table column.
pub const TABLE_DEFAULT_COLUMN_WIDTH: UiPx = ui_px(128.0);

/// Default minimum width for a table column.
pub const TABLE_MIN_COLUMN_WIDTH: UiPx = ui_px(40.0);

/// Default maximum width for a table column.
pub const TABLE_MAX_COLUMN_WIDTH: UiPx = ui_px(1_000_000.0);
