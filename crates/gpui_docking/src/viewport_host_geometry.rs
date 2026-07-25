use open_gpui::{Bounds, ElementGeometry, HitTestSnapshot, Hitbox, Pixels, Point};

#[derive(Clone, Debug, PartialEq)]
enum DockViewportHostHitRegion {
    Committed(HitTestSnapshot),
    Synthetic(Bounds<Pixels>),
}

/// Committed geometry for a rendered dock host.
///
/// Dock policies consume host-local and absolute layout coordinates. Platform routing consumes
/// window coordinates. Keeping the opaque GPUI geometry snapshot here makes that conversion
/// explicit and ensures transform-only frames invalidate stale route proofs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DockViewportHostGeometry {
    geometry: ElementGeometry,
    hit_region: DockViewportHostHitRegion,
}

impl DockViewportHostGeometry {
    pub(crate) fn from_hitbox(hitbox: &Hitbox) -> Self {
        Self {
            geometry: hitbox.geometry(),
            hit_region: DockViewportHostHitRegion::Committed(hitbox.hit_test_snapshot()),
        }
    }

    pub(crate) fn layout_bounds(&self) -> Bounds<Pixels> {
        self.geometry.layout_bounds()
    }

    pub(crate) fn window_to_host(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        let eligible = match &self.hit_region {
            DockViewportHostHitRegion::Committed(snapshot) => {
                snapshot.is_window_point_target(position)
            }
            DockViewportHostHitRegion::Synthetic(bounds) => bounds.contains(&position),
        };
        if !eligible {
            return None;
        }
        let position = self.geometry.window_to_local_point(position).ok()?;
        self.geometry
            .local_bounds()
            .contains(&position)
            .then_some(position)
    }

    #[cfg(test)]
    pub(crate) fn host_to_window(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        self.geometry.local_to_window_point(position).ok()
    }

    pub(crate) fn host_to_layout(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        self.geometry.local_to_layout_point(position).ok()
    }

    pub(crate) fn layout_to_window_bounds(&self, bounds: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
        self.geometry.layout_to_window_bounds(bounds).ok()
    }

    #[cfg(test)]
    pub(crate) fn layout_to_host(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        self.geometry.layout_to_local_point(position).ok()
    }

    #[cfg(test)]
    pub(crate) fn identity_with_hit_region_for_test(
        layout_bounds: Bounds<Pixels>,
        hit_region: Bounds<Pixels>,
    ) -> Self {
        Self {
            geometry: ElementGeometry::identity_for_test(layout_bounds),
            hit_region: DockViewportHostHitRegion::Synthetic(hit_region),
        }
    }
}

#[cfg(test)]
impl From<Bounds<Pixels>> for DockViewportHostGeometry {
    fn from(bounds: Bounds<Pixels>) -> Self {
        Self::identity_with_hit_region_for_test(bounds, bounds)
    }
}
