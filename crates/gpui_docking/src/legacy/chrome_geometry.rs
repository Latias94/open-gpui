use open_gpui::{Bounds, Pixels, point, px, size};

const DOCK_TAB_BAR_HEIGHT: f32 = 36.0;
const DOCK_FLOATING_TITLE_HEIGHT: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockFloatingChromeBounds {
    pub(crate) title_bar_bounds: Bounds<Pixels>,
    pub(crate) content_bounds: Bounds<Pixels>,
}

pub(crate) fn dock_tab_bar_bounds(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        bounds.origin,
        size(
            bounds.size.width,
            px(DOCK_TAB_BAR_HEIGHT).min(bounds.size.height.max(px(0.0))),
        ),
    )
}

pub(crate) fn dock_presentation_tab_label_bounds(
    tab_bar_bounds: Bounds<Pixels>,
    tab_count: usize,
    index: usize,
) -> Bounds<Pixels> {
    if tab_count == 0 {
        return Bounds::new(
            tab_bar_bounds.origin,
            size(px(0.0), tab_bar_bounds.size.height),
        );
    }

    let width = tab_bar_bounds.size.width / tab_count as f32;
    Bounds::new(
        point(
            tab_bar_bounds.origin.x + width * index as f32,
            tab_bar_bounds.origin.y,
        ),
        size(width, tab_bar_bounds.size.height),
    )
}

pub(crate) fn dock_floating_chrome_bounds(bounds: Bounds<Pixels>) -> DockFloatingChromeBounds {
    let title_height = px(DOCK_FLOATING_TITLE_HEIGHT).min(bounds.size.height.max(px(0.0)));
    let title_bar_bounds = Bounds::new(bounds.origin, size(bounds.size.width, title_height));
    let content_bounds = Bounds::new(
        point(bounds.origin.x, bounds.origin.y + title_height),
        size(
            bounds.size.width,
            (bounds.size.height - title_height).max(px(0.0)),
        ),
    );

    DockFloatingChromeBounds {
        title_bar_bounds,
        content_bounds,
    }
}
