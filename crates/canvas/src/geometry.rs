use open_gpui::{Bounds, Pixels, Point, Size};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasViewport {
    pub origin: Point<Pixels>,
    pub zoom: f32,
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self {
            origin: Point::default(),
            zoom: 1.0,
        }
    }
}

impl CanvasViewport {
    pub fn new(origin: Point<Pixels>, zoom: f32) -> Result<Self, TransformError> {
        if !zoom.is_finite() || zoom <= 0.0 {
            return Err(TransformError::InvalidZoom(zoom));
        }

        Ok(Self { origin, zoom })
    }

    pub fn pan_by(&mut self, delta: Point<Pixels>) {
        self.origin += delta;
    }

    pub fn set_zoom(&mut self, zoom: f32) -> Result<(), TransformError> {
        if !zoom.is_finite() || zoom <= 0.0 {
            return Err(TransformError::InvalidZoom(zoom));
        }

        self.zoom = zoom;
        Ok(())
    }

    pub fn set_zoom_around_view_point(
        &mut self,
        zoom: f32,
        view_point: Point<Pixels>,
    ) -> Result<(), TransformError> {
        let document_point = self.view_to_document(view_point);
        self.set_zoom(zoom)?;
        self.origin = document_point - (view_point * (1.0 / self.zoom));
        Ok(())
    }

    pub fn zoom_by_around_view_point(
        &mut self,
        factor: f32,
        view_point: Point<Pixels>,
    ) -> Result<(), TransformError> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(TransformError::InvalidZoomFactor(factor));
        }

        self.set_zoom_around_view_point(self.zoom * factor, view_point)
    }

    pub fn document_to_view(&self, point: Point<Pixels>) -> Point<Pixels> {
        (point - self.origin) * self.zoom
    }

    pub fn view_to_document(&self, point: Point<Pixels>) -> Point<Pixels> {
        (point * (1.0 / self.zoom)) + self.origin
    }

    pub fn document_bounds_to_view(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        Bounds::new(
            self.document_to_view(bounds.origin),
            Size::new(
                bounds.size.width * self.zoom,
                bounds.size.height * self.zoom,
            ),
        )
    }

    pub fn view_bounds_to_document(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        Bounds::new(
            self.view_to_document(bounds.origin),
            Size::new(
                bounds.size.width * (1.0 / self.zoom),
                bounds.size.height * (1.0 / self.zoom),
            ),
        )
    }

    pub fn visible_document_bounds(&self, view_size: Size<Pixels>) -> Bounds<Pixels> {
        self.view_bounds_to_document(Bounds::new(Point::default(), view_size))
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum TransformError {
    #[error("invalid canvas zoom `{0}`")]
    InvalidZoom(f32),
    #[error("invalid canvas zoom factor `{0}`")]
    InvalidZoomFactor(f32),
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{point, px};

    #[test]
    fn round_trips_between_view_and_document_space() {
        let viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap();
        let document = point(px(110.0), px(70.0));
        let view = viewport.document_to_view(document);

        assert_eq!(view, point(px(20.0), px(40.0)));
        assert_eq!(viewport.view_to_document(view), document);
    }

    #[test]
    fn rejects_non_positive_zoom() {
        assert_eq!(
            CanvasViewport::new(point(px(0.0), px(0.0)), 0.0).unwrap_err(),
            TransformError::InvalidZoom(0.0)
        );
    }

    #[test]
    fn zooms_around_view_point_without_moving_document_anchor() {
        let mut viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 1.0).unwrap();
        let view_point = point(px(20.0), px(10.0));
        let document_point = viewport.view_to_document(view_point);

        viewport
            .set_zoom_around_view_point(2.0, view_point)
            .unwrap();

        assert_eq!(viewport.view_to_document(view_point), document_point);
        assert_eq!(viewport.zoom, 2.0);
    }

    #[test]
    fn rejects_invalid_zoom_factor() {
        let mut viewport = CanvasViewport::default();

        assert_eq!(
            viewport
                .zoom_by_around_view_point(0.0, point(px(0.0), px(0.0)))
                .unwrap_err(),
            TransformError::InvalidZoomFactor(0.0)
        );
    }

    #[test]
    fn returns_visible_document_bounds() {
        let viewport = CanvasViewport::new(point(px(100.0), px(50.0)), 2.0).unwrap();

        let bounds = viewport.visible_document_bounds(open_gpui::size(px(200.0), px(100.0)));

        assert_eq!(bounds.origin, point(px(100.0), px(50.0)));
        assert_eq!(bounds.size, open_gpui::size(px(100.0), px(50.0)));
    }
}
