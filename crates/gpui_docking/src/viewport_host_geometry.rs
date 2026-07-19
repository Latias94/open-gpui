use open_gpui::{Bounds, ContentMask, ElementGeometry, Hitbox, Pixels, Point};

/// Committed geometry for a rendered dock host.
///
/// Dock policies consume host-local and absolute layout coordinates. Platform routing consumes
/// window coordinates. Keeping the opaque GPUI geometry snapshot here makes that conversion
/// explicit and ensures transform-only frames invalidate stale route proofs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DockViewportHostGeometry {
    geometry: ElementGeometry,
    content_mask: ContentMask<Pixels>,
}

impl DockViewportHostGeometry {
    pub(crate) fn from_hitbox(hitbox: &Hitbox) -> Self {
        Self {
            geometry: hitbox.geometry(),
            content_mask: hitbox.displayed_content_mask(),
        }
    }

    pub(crate) fn layout_bounds(self) -> Bounds<Pixels> {
        self.geometry.layout_bounds()
    }

    pub(crate) fn window_to_host(self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        if !self.content_mask.bounds.contains(&position) {
            return None;
        }
        let position = self.geometry.window_to_local_point(position).ok()?;
        self.geometry
            .local_bounds()
            .contains(&position)
            .then_some(position)
    }

    #[cfg(test)]
    pub(crate) fn host_to_window(self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        self.geometry.local_to_window_point(position).ok()
    }

    pub(crate) fn host_to_layout(self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        self.geometry.local_to_layout_point(position).ok()
    }

    pub(crate) fn layout_to_window_bounds(self, bounds: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
        self.geometry.layout_to_window_bounds(bounds).ok()
    }

    #[cfg(test)]
    pub(crate) fn layout_to_host(self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        self.geometry.layout_to_local_point(position).ok()
    }

    #[cfg(test)]
    pub(crate) fn identity_with_content_mask_for_test(
        layout_bounds: Bounds<Pixels>,
        content_mask: Bounds<Pixels>,
    ) -> Self {
        Self {
            geometry: ElementGeometry::identity_for_test(layout_bounds),
            content_mask: ContentMask {
                bounds: content_mask,
            },
        }
    }
}

impl From<ElementGeometry> for DockViewportHostGeometry {
    fn from(geometry: ElementGeometry) -> Self {
        Self {
            content_mask: ContentMask {
                bounds: geometry.displayed_bounds(),
            },
            geometry,
        }
    }
}

#[cfg(test)]
impl From<Bounds<Pixels>> for DockViewportHostGeometry {
    fn from(bounds: Bounds<Pixels>) -> Self {
        Self::from(ElementGeometry::identity_for_test(bounds))
    }
}
