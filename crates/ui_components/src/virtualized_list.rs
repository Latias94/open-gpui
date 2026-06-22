//! Renderer-neutral state for virtualized list surfaces.

#[cfg(test)]
use open_gpui_ui_core::ui_px;
use open_gpui_ui_core::{Size, UiPx};

/// Scroll alignment requested when a virtualized row should be revealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VirtualizedListScrollStrategy {
    /// Keep the row visible with the smallest scroll movement.
    #[default]
    Nearest,
    /// Align the row to the top edge of the viewport.
    Top,
    /// Align the row to the viewport center.
    Center,
    /// Align the row to the bottom edge of the viewport.
    Bottom,
}

impl VirtualizedListScrollStrategy {
    /// Returns the stable scroll strategy label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
        }
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
}

/// Resolved activation payload for a virtualized row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualizedListActivation {
    index: usize,
}

impl VirtualizedListActivation {
    /// Creates an activation payload for a visible item index.
    pub const fn new(index: usize) -> Self {
        Self { index }
    }

    /// Returns the activated item index.
    pub const fn index(self) -> usize {
        self.index
    }
}

/// Resolved virtualized-list state used by tests, adapters, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListState {
    size: Size,
    disabled: bool,
    item_count: usize,
    active_index: Option<usize>,
    selected_index: Option<usize>,
    viewport_item_count: usize,
    metrics: VirtualizedListMetrics,
}

impl VirtualizedListState {
    /// Resolves public state for a virtualized list.
    pub fn resolve(
        size: Size,
        disabled: bool,
        item_count: usize,
        active_index: Option<usize>,
        selected_index: Option<usize>,
        viewport_item_count: Option<usize>,
    ) -> Self {
        let selected_index = selected_index.and_then(|index| valid_index(index, item_count));
        let active_index = if disabled || item_count == 0 {
            None
        } else {
            active_index
                .and_then(|index| valid_index(index, item_count))
                .or(selected_index)
                .or(Some(0))
        };
        let selected_index = if disabled { None } else { selected_index };

        Self {
            size,
            disabled,
            item_count,
            active_index,
            selected_index,
            viewport_item_count: viewport_item_count.filter(|count| *count > 0).unwrap_or(1),
            metrics: VirtualizedListMetrics::from_size(size),
        }
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the list should ignore navigation and activation.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the total item count.
    pub const fn item_count(&self) -> usize {
        self.item_count
    }

    /// Returns the active descendant index.
    pub const fn active_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Returns the selected row index.
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Returns the estimated number of rows visible in the viewport.
    pub const fn viewport_item_count(&self) -> usize {
        self.viewport_item_count
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> VirtualizedListMetrics {
        self.metrics
    }

    /// Returns whether the list has no items to render.
    pub const fn visible_empty(&self) -> bool {
        self.item_count == 0
    }

    /// Returns the target index for an APG-style navigation key.
    pub fn navigation_target(&self, key: &str) -> Option<usize> {
        if self.disabled {
            return None;
        }

        virtualized_list_navigation_target(
            key,
            self.active_index?,
            self.item_count,
            self.viewport_item_count,
        )
    }

    /// Returns activation payload for Enter or Space.
    pub fn activation_for_key(&self, key: &str) -> Option<VirtualizedListActivation> {
        if self.disabled || !matches!(key, "enter" | "space") {
            return None;
        }

        self.active_index.map(VirtualizedListActivation::new)
    }

    /// Clamps a requested item index into the list range.
    pub fn clamped_index(&self, index: usize) -> Option<usize> {
        valid_index(index, self.item_count).or_else(|| self.item_count.checked_sub(1))
    }
}

/// Resolves virtualized-list navigation for APG-style key names.
pub fn virtualized_list_navigation_target(
    key: &str,
    current: usize,
    item_count: usize,
    viewport_item_count: usize,
) -> Option<usize> {
    if item_count == 0 || current >= item_count {
        return None;
    }

    match key {
        "home" => Some(0),
        "end" => item_count.checked_sub(1),
        "up" => Some(current.saturating_sub(1)),
        "down" => Some((current + 1).min(item_count - 1)),
        "pageup" => Some(current.saturating_sub(viewport_item_count.max(1))),
        "pagedown" => Some((current + viewport_item_count.max(1)).min(item_count - 1)),
        _ => None,
    }
}

const fn valid_index(index: usize, item_count: usize) -> Option<usize> {
    if index < item_count {
        Some(index)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtualized_list_state_clamps_active_and_preserves_metrics() {
        let state =
            VirtualizedListState::resolve(Size::Small, false, 10, Some(12), Some(4), Some(5));

        assert_eq!(state.size(), Size::Small);
        assert_eq!(state.item_count(), 10);
        assert_eq!(state.active_index(), Some(4));
        assert_eq!(state.selected_index(), Some(4));
        assert_eq!(state.viewport_item_count(), 5);
        assert_eq!(state.metrics().row_height(), ui_px(28.0));
        assert!(!state.visible_empty());
    }

    #[test]
    fn virtualized_list_navigation_stays_inside_range() {
        let state = VirtualizedListState::resolve(Size::Medium, false, 12, Some(6), None, Some(4));

        assert_eq!(state.navigation_target("home"), Some(0));
        assert_eq!(state.navigation_target("end"), Some(11));
        assert_eq!(state.navigation_target("up"), Some(5));
        assert_eq!(state.navigation_target("down"), Some(7));
        assert_eq!(state.navigation_target("pageup"), Some(2));
        assert_eq!(state.navigation_target("pagedown"), Some(10));
    }

    #[test]
    fn virtualized_list_empty_or_disabled_state_has_no_targets() {
        let empty = VirtualizedListState::resolve(Size::Medium, false, 0, None, None, None);
        let disabled =
            VirtualizedListState::resolve(Size::Medium, true, 10, Some(2), Some(2), None);

        assert!(empty.visible_empty());
        assert_eq!(empty.active_index(), None);
        assert_eq!(empty.navigation_target("down"), None);
        assert_eq!(disabled.active_index(), None);
        assert_eq!(disabled.selected_index(), None);
        assert_eq!(disabled.activation_for_key("enter"), None);
    }

    #[test]
    fn virtualized_list_scroll_strategy_labels_are_stable() {
        assert_eq!(VirtualizedListScrollStrategy::Nearest.as_str(), "nearest");
        assert_eq!(VirtualizedListScrollStrategy::Top.as_str(), "top");
        assert_eq!(VirtualizedListScrollStrategy::Center.as_str(), "center");
        assert_eq!(VirtualizedListScrollStrategy::Bottom.as_str(), "bottom");
    }
}
