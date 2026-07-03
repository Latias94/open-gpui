//! Renderer-neutral layout projection primitives.

use crate::{
    MotionEdge, MotionPreference, UiPoint, UiPx, UiRect, reveal_rect_from_edge, ui_point, ui_rect,
    ui_size,
};

const TRANSLATE_EPSILON: f32 = 0.01;
const SCALE_EPSILON: f32 = 0.000_1;

/// Renderer-neutral two-axis scale used by layout projection samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionProjectionScale {
    x: f32,
    y: f32,
}

impl MotionProjectionScale {
    /// Identity scale.
    pub const IDENTITY: Self = Self { x: 1.0, y: 1.0 };

    /// Creates a two-axis scale.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns the horizontal scale.
    pub const fn x(self) -> f32 {
        self.x
    }

    /// Returns the vertical scale.
    pub const fn y(self) -> f32 {
        self.y
    }

    fn sanitized(self) -> Self {
        Self {
            x: sanitize_scale(self.x),
            y: sanitize_scale(self.y),
        }
    }

    fn reciprocal(self) -> Self {
        let scale = self.sanitized();
        Self::new(1.0 / scale.x, 1.0 / scale.y)
    }
}

/// Projection from previous geometry to final target geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionProjection {
    source: UiRect,
    target: UiRect,
    tree_scale: MotionProjectionScale,
}

impl MotionProjection {
    /// Creates a projection between a source rect and final target rect.
    pub const fn between(source: UiRect, target: UiRect) -> Self {
        Self {
            source,
            target,
            tree_scale: MotionProjectionScale::IDENTITY,
        }
    }

    /// Creates a projection with parent/tree scale correction data.
    pub fn with_tree_scale(
        source: UiRect,
        target: UiRect,
        tree_scale: MotionProjectionScale,
    ) -> Self {
        Self {
            source,
            target,
            tree_scale: tree_scale.sanitized(),
        }
    }

    /// Returns the source rect.
    pub const fn source(self) -> UiRect {
        self.source
    }

    /// Returns the final target rect.
    pub const fn target(self) -> UiRect {
        self.target
    }

    /// Returns the tree scale carried for adapter correction.
    pub const fn tree_scale(self) -> MotionProjectionScale {
        self.tree_scale
    }

    /// Returns the projection sample at clamped unit progress.
    pub fn sample(self, progress: f32) -> MotionProjectionSample {
        self.sample_with_preference(progress, MotionPreference::Animated)
    }

    /// Returns the projection sample for a preference, with reduced motion completing immediately.
    pub fn sample_with_preference(
        self,
        progress: f32,
        preference: MotionPreference,
    ) -> MotionProjectionSample {
        let progress = if preference.is_immediate() {
            1.0
        } else {
            progress.clamp(0.0, 1.0)
        };
        let source_translation = self.source_translation();
        let source_scale = self.source_scale();
        let translation = ui_point(
            UiPx::new(source_translation.x.as_f32() * (1.0 - progress)),
            UiPx::new(source_translation.y.as_f32() * (1.0 - progress)),
        );
        let scale = MotionProjectionScale::new(
            lerp_scale(source_scale.x(), 1.0, progress),
            lerp_scale(source_scale.y(), 1.0, progress),
        );

        MotionProjectionSample::new(
            self.target,
            progress,
            snap_translation(translation),
            snap_scale(scale),
            self.tree_scale,
            self.tree_scale.reciprocal(),
        )
    }

    fn source_translation(self) -> UiPoint {
        let tree_scale = self.tree_scale.sanitized();
        snap_translation(ui_point(
            (self.source.origin.x - self.target.origin.x) / tree_scale.x(),
            (self.source.origin.y - self.target.origin.y) / tree_scale.y(),
        ))
    }

    fn source_scale(self) -> MotionProjectionScale {
        snap_scale(MotionProjectionScale::new(
            divide_or_identity(self.source.size.width, self.target.size.width),
            divide_or_identity(self.source.size.height, self.target.size.height),
        ))
    }
}

/// Sampled projection data for rendering final-size content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionProjectionSample {
    target_bounds: UiRect,
    progress: f32,
    translation: UiPoint,
    scale: MotionProjectionScale,
    tree_scale: MotionProjectionScale,
    scale_correction: MotionProjectionScale,
}

impl MotionProjectionSample {
    /// Creates a projection sample from explicit values.
    pub const fn new(
        target_bounds: UiRect,
        progress: f32,
        translation: UiPoint,
        scale: MotionProjectionScale,
        tree_scale: MotionProjectionScale,
        scale_correction: MotionProjectionScale,
    ) -> Self {
        Self {
            target_bounds,
            progress,
            translation,
            scale,
            tree_scale,
            scale_correction,
        }
    }

    /// Returns the final semantic bounds.
    pub const fn target_bounds(self) -> UiRect {
        self.target_bounds
    }

    /// Returns the sampled visual bounds after applying projection translation and scale.
    ///
    /// The semantic layout target remains `target_bounds`; adapters can use this rectangle for an
    /// overlay or clip while keeping child content laid out at the final size.
    pub fn visual_bounds(self) -> UiRect {
        ui_rect(
            ui_point(
                self.target_bounds.origin.x + self.translation.x,
                self.target_bounds.origin.y + self.translation.y,
            ),
            ui_size(
                self.target_bounds.size.width * self.scale.x(),
                self.target_bounds.size.height * self.scale.y(),
            ),
        )
    }

    /// Returns clamped projection progress.
    pub const fn progress(self) -> f32 {
        self.progress
    }

    /// Returns the transform-like translation to apply to final-size content.
    pub const fn translation(self) -> UiPoint {
        self.translation
    }

    /// Returns the transform-like scale to apply to final-size content.
    pub const fn scale(self) -> MotionProjectionScale {
        self.scale
    }

    /// Returns the parent/tree scale used for correction.
    pub const fn tree_scale(self) -> MotionProjectionScale {
        self.tree_scale
    }

    /// Returns the reciprocal scale correction adapters can apply to child content.
    pub const fn scale_correction(self) -> MotionProjectionScale {
        self.scale_correction
    }

    /// Returns a reveal rect sampled inside the final target bounds.
    pub fn reveal_rect(self, edge: MotionEdge) -> UiRect {
        reveal_rect_from_edge(self.target_bounds, edge, self.progress)
    }
}

fn divide_or_identity(from: UiPx, to: UiPx) -> f32 {
    let denominator = to.as_f32();
    if denominator.abs() <= f32::EPSILON || !denominator.is_finite() {
        1.0
    } else {
        sanitize_scale(from.as_f32() / denominator)
    }
}

fn sanitize_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn snap_translation(point: UiPoint) -> UiPoint {
    ui_point(
        UiPx::new(snap_zero(point.x.as_f32(), TRANSLATE_EPSILON)),
        UiPx::new(snap_zero(point.y.as_f32(), TRANSLATE_EPSILON)),
    )
}

fn snap_scale(scale: MotionProjectionScale) -> MotionProjectionScale {
    MotionProjectionScale::new(
        snap_one(scale.x(), SCALE_EPSILON),
        snap_one(scale.y(), SCALE_EPSILON),
    )
}

fn snap_zero(value: f32, epsilon: f32) -> f32 {
    if value.abs() <= epsilon { 0.0 } else { value }
}

fn snap_one(value: f32, epsilon: f32) -> f32 {
    if (value - 1.0).abs() <= epsilon {
        1.0
    } else {
        value
    }
}

fn lerp_scale(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MotionEdge, MotionPreference, UiPx, reveal_rect_from_edge, ui_point, ui_rect, ui_size,
    };

    fn rect(x: f32, y: f32, width: f32, height: f32) -> crate::UiRect {
        ui_rect(
            ui_point(UiPx::new(x), UiPx::new(y)),
            ui_size(UiPx::new(width), UiPx::new(height)),
        )
    }

    #[test]
    fn projection_describes_source_to_target_transform_without_mutating_target_bounds() {
        let projection = MotionProjection::between(
            rect(10.0, 20.0, 100.0, 50.0),
            rect(30.0, 60.0, 200.0, 100.0),
        );

        let start = projection.sample(0.0);
        assert_eq!(start.target_bounds(), rect(30.0, 60.0, 200.0, 100.0));
        assert_eq!(start.visual_bounds(), rect(10.0, 20.0, 100.0, 50.0));
        assert_eq!(
            start.translation(),
            ui_point(UiPx::new(-20.0), UiPx::new(-40.0))
        );
        assert_eq!(start.scale(), MotionProjectionScale::new(0.5, 0.5));

        let end = projection.sample(1.0);
        assert_eq!(end.target_bounds(), rect(30.0, 60.0, 200.0, 100.0));
        assert_eq!(end.visual_bounds(), rect(30.0, 60.0, 200.0, 100.0));
        assert_eq!(end.translation(), ui_point(UiPx::ZERO, UiPx::ZERO));
        assert_eq!(end.scale(), MotionProjectionScale::IDENTITY);
    }

    #[test]
    fn near_identity_projection_snaps_to_neutral_values() {
        let projection = MotionProjection::between(
            rect(10.002, 20.002, 100.001, 50.001),
            rect(10.0, 20.0, 100.0, 50.0),
        );

        let sample = projection.sample(0.0);
        assert_eq!(sample.translation(), ui_point(UiPx::ZERO, UiPx::ZERO));
        assert_eq!(sample.scale(), MotionProjectionScale::IDENTITY);
    }

    #[test]
    fn tree_scale_correction_is_carried_as_data() {
        let projection = MotionProjection::with_tree_scale(
            rect(0.0, 0.0, 100.0, 50.0),
            rect(50.0, 30.0, 100.0, 50.0),
            MotionProjectionScale::new(2.0, 2.0),
        );

        let start = projection.sample(0.0);
        assert_eq!(
            start.translation(),
            ui_point(UiPx::new(-25.0), UiPx::new(-15.0))
        );
        assert_eq!(
            start.scale_correction(),
            MotionProjectionScale::new(0.5, 0.5)
        );
        assert_eq!(start.tree_scale(), MotionProjectionScale::new(2.0, 2.0));
    }

    #[test]
    fn reduced_motion_projection_samples_final_state_without_spatial_movement() {
        let projection =
            MotionProjection::between(rect(0.0, 0.0, 100.0, 50.0), rect(50.0, 30.0, 100.0, 50.0));

        let sample = projection.sample_with_preference(0.25, MotionPreference::Reduced);
        assert_eq!(sample.progress(), 1.0);
        assert_eq!(sample.translation(), ui_point(UiPx::ZERO, UiPx::ZERO));
        assert_eq!(sample.scale(), MotionProjectionScale::IDENTITY);
    }

    #[test]
    fn reveal_sample_uses_final_bounds_as_the_semantic_rect() {
        let target = rect(30.0, 60.0, 200.0, 100.0);
        let projection = MotionProjection::between(rect(10.0, 20.0, 100.0, 50.0), target);
        let sample = projection.sample(0.25);

        assert_eq!(sample.target_bounds(), target);
        assert_eq!(
            sample.reveal_rect(MotionEdge::Right),
            reveal_rect_from_edge(target, MotionEdge::Right, 0.25)
        );
    }
}
