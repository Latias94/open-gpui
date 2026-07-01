//! Shared virtualized row-window projection for component render diagnostics.

#[cfg(test)]
use open_gpui_ui_core::UiPx;
use open_gpui_ui_core::{VirtualizerItemMeasurement, VirtualizerResolvedState};

/// One projected row in a virtualized render window.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RowWindowItem<T> {
    index: usize,
    render_key: String,
    measurement: VirtualizerItemMeasurement,
    row: T,
}

impl<T> RowWindowItem<T> {
    /// Creates a row-window item from virtualizer measurement metadata.
    fn new(measurement: &VirtualizerItemMeasurement, row: T) -> Self {
        Self {
            index: measurement.index(),
            render_key: measurement.key().as_str().to_owned(),
            measurement: measurement.clone(),
            row,
        }
    }

    /// Returns the source row index used by the virtualizer.
    #[cfg(test)]
    pub(crate) const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable render key supplied to the virtualizer.
    #[cfg(test)]
    pub(crate) fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns the virtual row size.
    #[cfg(test)]
    pub(crate) const fn virtual_size(&self) -> UiPx {
        self.measurement.size()
    }

    /// Consumes the projection item and returns its parts.
    pub(crate) fn into_parts(self) -> (usize, String, VirtualizerItemMeasurement, T) {
        (self.index, self.render_key, self.measurement, self.row)
    }
}

/// Projected rows plus the row-window metadata shared by virtualized components.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RowWindow<T> {
    rows: Vec<RowWindowItem<T>>,
    visible_row_count: usize,
    overscan_count: usize,
}

impl<T> RowWindow<T> {
    /// Projects a virtualizer window into component row payloads.
    pub(crate) fn project(
        virtualizer: &VirtualizerResolvedState,
        mut row_at: impl FnMut(usize) -> Option<T>,
    ) -> Self {
        let rows = virtualizer
            .items()
            .iter()
            .filter_map(|measurement| {
                row_at(measurement.index()).map(|row| RowWindowItem::new(measurement, row))
            })
            .collect();

        Self {
            rows,
            visible_row_count: virtualizer.visible_items().len(),
            overscan_count: virtualizer.overscan(),
        }
    }

    /// Returns the projected rows after overscan.
    #[cfg(test)]
    pub(crate) fn rows(&self) -> &[RowWindowItem<T>] {
        &self.rows
    }

    /// Consumes the projection and returns projected rows after overscan.
    pub(crate) fn into_rows(self) -> Vec<RowWindowItem<T>> {
        self.rows
    }

    /// Returns the number of visible rows before overscan.
    pub(crate) const fn visible_row_count(&self) -> usize {
        self.visible_row_count
    }

    /// Returns the overscan row budget.
    pub(crate) const fn overscan_count(&self) -> usize {
        self.overscan_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::{VirtualizerItemKey, VirtualizerState, ui_px};

    #[test]
    fn row_window_projection_preserves_ranges_keys_and_measurements() {
        let source = ["alpha", "bravo", "charlie", "delta"];
        let virtualizer = VirtualizerState::new(source.len(), ui_px(20.0))
            .with_viewport_extent(ui_px(40.0))
            .with_overscan(1)
            .with_scroll_offset(ui_px(20.0))
            .resolve_fixed_window(|index| VirtualizerItemKey::new(source[index]));

        let window = RowWindow::project(&virtualizer, |index| source.get(index).copied());

        assert_eq!(
            window.visible_row_count(),
            virtualizer.visible_items().len()
        );
        assert_eq!(window.overscan_count(), 1);
        assert_eq!(
            window
                .rows()
                .iter()
                .map(|row| (row.index(), row.render_key().to_owned(), row.virtual_size()))
                .collect::<Vec<_>>(),
            virtualizer
                .items()
                .iter()
                .map(|item| (item.index(), item.key().as_str().to_owned(), item.size()))
                .collect::<Vec<_>>()
        );
    }
}
