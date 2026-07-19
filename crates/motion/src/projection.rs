//! Renderer-neutral layout projection primitives.

use crate::{
    MotionEdge, MotionPoint, MotionPreference, MotionPx, MotionRect, motion_point, motion_rect,
    motion_size, reveal_rect_from_edge,
};
use std::fmt;

const TRANSLATE_EPSILON: f32 = 0.01;
const SCALE_EPSILON: f32 = 0.000_1;

/// Dimensionless axis scale sampled from a layout projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionProjectionScale {
    x: f32,
    y: f32,
}

impl MotionProjectionScale {
    const fn new(x: f32, y: f32) -> Self {
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
}

/// Why a layout projection cannot produce a finite, invertible transform sample.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MotionProjectionError {
    /// Progress must be finite before it can be clamped and sampled.
    NonFiniteProgress,
    /// Source and target origins and extents must be finite.
    NonFiniteGeometry,
    /// Transform projections require strictly positive source and target extents.
    NonPositiveExtent,
    /// The sampled translation or invertible axis scale is not representable by finite `f32`.
    UnrepresentableTransform,
}

impl fmt::Display for MotionProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NonFiniteProgress => "motion projection progress must be finite",
            Self::NonFiniteGeometry => "motion projection geometry must be finite",
            Self::NonPositiveExtent => {
                "motion transform projections require positive source and target extents"
            }
            Self::UnrepresentableTransform => {
                "motion projection transform is not representable by finite f32"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for MotionProjectionError {}

/// A renderer-neutral, checked scale-and-translation projection sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionProjectionTransformSample {
    target_bounds: MotionRect,
    progress: f32,
    translation: MotionPoint,
    scale: MotionProjectionScale,
}

impl MotionProjectionTransformSample {
    const fn new(
        target_bounds: MotionRect,
        progress: f32,
        translation: MotionPoint,
        scale: MotionProjectionScale,
    ) -> Self {
        Self {
            target_bounds,
            progress,
            translation,
            scale,
        }
    }

    /// Returns the final semantic layout bounds rendered by the consumer.
    pub const fn target_bounds(self) -> MotionRect {
        self.target_bounds
    }

    /// Returns the clamped projection progress.
    pub const fn progress(self) -> f32 {
        self.progress
    }

    /// Returns the translation relative to final target layout.
    pub const fn translation(self) -> MotionPoint {
        self.translation
    }

    /// Returns the checked, positive axis scale.
    pub const fn scale(self) -> MotionProjectionScale {
        self.scale
    }

    /// Returns the sampled visual bounds for final-size content.
    pub fn visual_bounds(self) -> MotionRect {
        motion_rect(
            motion_point(
                self.target_bounds.origin.x + self.translation.x,
                self.target_bounds.origin.y + self.translation.y,
            ),
            motion_size(
                self.target_bounds.size.width * self.scale.x(),
                self.target_bounds.size.height * self.scale.y(),
            ),
        )
    }
}

/// Projection from previous geometry to final target geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionProjection {
    source: MotionRect,
    target: MotionRect,
}

impl MotionProjection {
    /// Creates a projection between a source rect and final target rect.
    pub const fn between(source: MotionRect, target: MotionRect) -> Self {
        Self { source, target }
    }

    /// Returns the source rect.
    pub const fn source(self) -> MotionRect {
        self.source
    }

    /// Returns the final target rect.
    pub const fn target(self) -> MotionRect {
        self.target
    }

    /// Returns the sampled visual bounds after applying projection movement to final-size content.
    pub fn visual_bounds(self, progress: f32) -> Result<MotionRect, MotionProjectionError> {
        self.visual_bounds_with_preference(progress, MotionPreference::Animated)
    }

    /// Returns the sampled visual bounds while honoring reduced-motion final-state semantics.
    pub fn visual_bounds_with_preference(
        self,
        progress: f32,
        preference: MotionPreference,
    ) -> Result<MotionRect, MotionProjectionError> {
        self.try_transform_sample_with_preference(progress, preference)
            .map(MotionProjectionTransformSample::visual_bounds)
    }

    /// Samples a checked transform for final-size content.
    pub fn try_transform_sample(
        self,
        progress: f32,
    ) -> Result<MotionProjectionTransformSample, MotionProjectionError> {
        self.try_transform_sample_with_preference(progress, MotionPreference::Animated)
    }

    /// Samples a checked transform while honoring reduced-motion final-state semantics.
    pub fn try_transform_sample_with_preference(
        self,
        progress: f32,
        preference: MotionPreference,
    ) -> Result<MotionProjectionTransformSample, MotionProjectionError> {
        validate_projection_rect(self.source)?;
        validate_projection_rect(self.target)?;
        if !progress.is_finite() {
            return Err(MotionProjectionError::NonFiniteProgress);
        }
        let progress = if preference.is_immediate() {
            1.0
        } else {
            progress.clamp(0.0, 1.0)
        };
        let source_translation = self.source_translation()?;
        let source_scale = self.source_scale()?;
        let translation = motion_point(
            MotionPx::new(source_translation.x.as_f32() * (1.0 - progress)),
            MotionPx::new(source_translation.y.as_f32() * (1.0 - progress)),
        );
        let scale = MotionProjectionScale::new(
            lerp_scale(source_scale.x(), 1.0, progress),
            lerp_scale(source_scale.y(), 1.0, progress),
        );
        validate_translation(translation)?;
        validate_scale(scale)?;

        let sample = MotionProjectionTransformSample::new(
            self.target,
            progress,
            snap_translation(translation),
            snap_scale(scale),
        );
        validate_projection_rect(sample.visual_bounds())?;
        Ok(sample)
    }

    fn source_translation(self) -> Result<MotionPoint, MotionProjectionError> {
        let translation = snap_translation(motion_point(
            self.source.origin.x - self.target.origin.x,
            self.source.origin.y - self.target.origin.y,
        ));
        validate_translation(translation)?;
        Ok(translation)
    }

    fn source_scale(self) -> Result<MotionProjectionScale, MotionProjectionError> {
        let scale = snap_scale(MotionProjectionScale::new(
            checked_scale_ratio(self.source.size.width, self.target.size.width)?,
            checked_scale_ratio(self.source.size.height, self.target.size.height)?,
        ));
        validate_scale(scale)?;
        Ok(scale)
    }
}

/// Sampled clip for rendering final-size content through a moving or revealing viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionProjectionClip {
    content_bounds: MotionRect,
    visible_bounds: MotionRect,
    occlusion_bounds: MotionRect,
    progress: f32,
}

impl MotionProjectionClip {
    /// Creates a sampled clip from explicit final-content, visible, and occlusion bounds.
    pub fn new(
        content_bounds: MotionRect,
        visible_bounds: MotionRect,
        occlusion_bounds: MotionRect,
        progress: f32,
    ) -> Self {
        Self {
            content_bounds,
            visible_bounds,
            occlusion_bounds,
            progress: progress.clamp(0.0, 1.0),
        }
    }

    /// Creates a projection clip from previous geometry to final semantic geometry.
    pub fn from_projection(
        projection: MotionProjection,
        progress: f32,
    ) -> Result<Self, MotionProjectionError> {
        Self::from_projection_with_preference(projection, progress, MotionPreference::Animated)
    }

    /// Creates a projection clip while honoring reduced-motion final-state semantics.
    pub fn from_projection_with_preference(
        projection: MotionProjection,
        progress: f32,
        preference: MotionPreference,
    ) -> Result<Self, MotionProjectionError> {
        let sample = projection.try_transform_sample_with_preference(progress, preference)?;
        Ok(Self::new(
            sample.target_bounds(),
            sample.visual_bounds(),
            sample.target_bounds(),
            sample.progress(),
        ))
    }

    /// Creates a reveal clip inside final content bounds from the chosen edge.
    pub fn reveal(content_bounds: MotionRect, edge: MotionEdge, progress: f32) -> Self {
        let progress = progress.clamp(0.0, 1.0);
        Self::new(
            content_bounds,
            reveal_rect_from_edge(content_bounds, edge, progress),
            content_bounds,
            progress,
        )
    }

    /// Returns the final-size content bounds.
    pub const fn content_bounds(self) -> MotionRect {
        self.content_bounds
    }

    /// Returns the sampled visible viewport bounds.
    pub const fn visible_bounds(self) -> MotionRect {
        self.visible_bounds
    }

    /// Returns the occlusion bounds that cover the snapped final scene underneath.
    pub const fn occlusion_bounds(self) -> MotionRect {
        self.occlusion_bounds
    }

    /// Returns clamped clip progress.
    pub const fn progress(self) -> f32 {
        self.progress
    }
}

fn validate_projection_rect(rect: MotionRect) -> Result<(), MotionProjectionError> {
    let values = [
        rect.origin.x.as_f32(),
        rect.origin.y.as_f32(),
        rect.size.width.as_f32(),
        rect.size.height.as_f32(),
    ];
    if values.iter().any(|value| !value.is_finite()) {
        return Err(MotionProjectionError::NonFiniteGeometry);
    }
    if rect.size.width.as_f32() <= 0.0 || rect.size.height.as_f32() <= 0.0 {
        return Err(MotionProjectionError::NonPositiveExtent);
    }
    Ok(())
}

fn validate_translation(translation: MotionPoint) -> Result<(), MotionProjectionError> {
    if translation.x.as_f32().is_finite() && translation.y.as_f32().is_finite() {
        Ok(())
    } else {
        Err(MotionProjectionError::UnrepresentableTransform)
    }
}

fn validate_scale(scale: MotionProjectionScale) -> Result<(), MotionProjectionError> {
    if valid_scale_component(scale.x()) && valid_scale_component(scale.y()) {
        Ok(())
    } else {
        Err(MotionProjectionError::UnrepresentableTransform)
    }
}

fn valid_scale_component(value: f32) -> bool {
    value.is_normal() && value > 0.0 && value.recip().is_finite()
}

fn checked_scale_ratio(from: MotionPx, to: MotionPx) -> Result<f32, MotionProjectionError> {
    if from.as_f32() <= 0.0 || to.as_f32() <= 0.0 {
        return Err(MotionProjectionError::NonPositiveExtent);
    }
    let scale = from.as_f32() / to.as_f32();
    if valid_scale_component(scale) {
        Ok(scale)
    } else {
        Err(MotionProjectionError::UnrepresentableTransform)
    }
}

fn snap_translation(point: MotionPoint) -> MotionPoint {
    motion_point(
        MotionPx::new(snap_zero(point.x.as_f32(), TRANSLATE_EPSILON)),
        MotionPx::new(snap_zero(point.y.as_f32(), TRANSLATE_EPSILON)),
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
    if progress <= 0.0 {
        from
    } else if progress >= 1.0 {
        to
    } else {
        from * (1.0 - progress) + to * progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MotionEdge, MotionPreference, MotionPx, motion_point, motion_px, motion_rect, motion_size,
    };

    fn rect(x: f32, y: f32, width: f32, height: f32) -> crate::MotionRect {
        motion_rect(
            motion_point(MotionPx::new(x), MotionPx::new(y)),
            motion_size(MotionPx::new(width), MotionPx::new(height)),
        )
    }

    #[test]
    fn projection_visual_bounds_move_without_mutating_target_bounds() {
        let projection = MotionProjection::between(
            rect(10.0, 20.0, 100.0, 50.0),
            rect(30.0, 60.0, 200.0, 100.0),
        );

        assert_eq!(projection.target(), rect(30.0, 60.0, 200.0, 100.0));
        assert_eq!(
            projection.visual_bounds(0.0).unwrap(),
            rect(10.0, 20.0, 100.0, 50.0)
        );
        assert_eq!(
            projection.visual_bounds(1.0).unwrap(),
            rect(30.0, 60.0, 200.0, 100.0)
        );
    }

    #[test]
    fn projection_clip_keeps_final_content_bounds_and_samples_visible_bounds() {
        let projection = MotionProjection::between(
            rect(10.0, 20.0, 100.0, 50.0),
            rect(30.0, 60.0, 200.0, 100.0),
        );

        let start = MotionProjectionClip::from_projection(projection, 0.0).unwrap();
        assert_eq!(start.content_bounds(), rect(30.0, 60.0, 200.0, 100.0));
        assert_eq!(start.visible_bounds(), rect(10.0, 20.0, 100.0, 50.0));
        assert_eq!(start.occlusion_bounds(), rect(30.0, 60.0, 200.0, 100.0));

        let end = MotionProjectionClip::from_projection(projection, 1.0).unwrap();
        assert_eq!(end.content_bounds(), rect(30.0, 60.0, 200.0, 100.0));
        assert_eq!(end.visible_bounds(), rect(30.0, 60.0, 200.0, 100.0));
    }

    #[test]
    fn reveal_clip_samples_visible_bounds_inside_final_content() {
        let clip =
            MotionProjectionClip::reveal(rect(10.0, 20.0, 100.0, 50.0), MotionEdge::Left, 0.25);

        assert_eq!(clip.content_bounds(), rect(10.0, 20.0, 100.0, 50.0));
        assert_eq!(clip.visible_bounds(), rect(10.0, 20.0, 25.0, 50.0));
        assert_eq!(clip.occlusion_bounds(), rect(10.0, 20.0, 100.0, 50.0));
        assert_eq!(clip.progress(), 0.25);
    }

    #[test]
    fn near_identity_projection_snaps_to_neutral_values() {
        let projection = MotionProjection::between(
            rect(10.002, 20.002, 100.001, 50.001),
            rect(10.0, 20.0, 100.0, 50.0),
        );

        assert_eq!(
            projection.visual_bounds(0.0).unwrap(),
            rect(10.0, 20.0, 100.0, 50.0)
        );
    }

    #[test]
    fn reduced_motion_projection_samples_final_state_without_spatial_movement() {
        let projection =
            MotionProjection::between(rect(0.0, 0.0, 100.0, 50.0), rect(50.0, 30.0, 100.0, 50.0));

        assert_eq!(
            projection
                .visual_bounds_with_preference(0.25, MotionPreference::Reduced)
                .unwrap(),
            rect(50.0, 30.0, 100.0, 50.0)
        );
    }

    #[test]
    fn public_visual_bounds_helper_exposes_consumed_projection_capability() {
        let projection = MotionProjection::between(
            rect(10.0, 20.0, 100.0, 50.0),
            rect(30.0, 60.0, 200.0, 100.0),
        );

        assert_eq!(
            projection.visual_bounds(0.0).unwrap(),
            rect(10.0, 20.0, 100.0, 50.0)
        );
        assert_eq!(
            projection.visual_bounds(1.0).unwrap(),
            rect(30.0, 60.0, 200.0, 100.0)
        );
        assert_eq!(
            projection
                .visual_bounds_with_preference(0.25, MotionPreference::Reduced)
                .unwrap(),
            rect(30.0, 60.0, 200.0, 100.0)
        );
    }

    #[test]
    fn reveal_sample_uses_final_bounds_as_the_semantic_rect() {
        let target = rect(30.0, 60.0, 200.0, 100.0);
        let projection = MotionProjection::between(rect(10.0, 20.0, 100.0, 50.0), target);
        let clip = MotionProjectionClip::from_projection(projection, 0.25).unwrap();

        assert_eq!(clip.content_bounds(), target);
        assert_eq!(clip.occlusion_bounds(), target);
    }

    #[test]
    fn transform_sample_exposes_renderer_neutral_scale_and_translation() {
        let projection = MotionProjection::between(
            rect(10.0, 20.0, 100.0, 50.0),
            rect(30.0, 60.0, 200.0, 100.0),
        );

        let start = projection.try_transform_sample(0.0).unwrap();
        assert_eq!(
            start.translation(),
            motion_point(motion_px(-20.0), motion_px(-40.0))
        );
        assert_eq!(start.scale(), MotionProjectionScale::new(0.5, 0.5));
        assert_eq!(start.visual_bounds(), projection.source());

        let reduced = projection
            .try_transform_sample_with_preference(0.0, MotionPreference::Reduced)
            .unwrap();
        assert_eq!(reduced.progress(), 1.0);
        assert_eq!(reduced.translation(), MotionPoint::default());
        assert_eq!(reduced.scale(), MotionProjectionScale::new(1.0, 1.0));
    }

    #[test]
    fn transform_sample_rejects_zero_nonfinite_and_unrepresentable_geometry() {
        let zero_source =
            MotionProjection::between(rect(0.0, 0.0, 0.0, 10.0), rect(0.0, 0.0, 10.0, 10.0));
        assert_eq!(
            zero_source.try_transform_sample(0.0),
            Err(MotionProjectionError::NonPositiveExtent)
        );

        let nonfinite = MotionProjection::between(
            rect(f32::INFINITY, 0.0, 10.0, 10.0),
            rect(0.0, 0.0, 10.0, 10.0),
        );
        assert_eq!(
            nonfinite.try_transform_sample(0.0),
            Err(MotionProjectionError::NonFiniteGeometry)
        );

        let unrepresentable = MotionProjection::between(
            rect(0.0, 0.0, f32::MAX, 10.0),
            rect(0.0, 0.0, f32::MIN_POSITIVE, 10.0),
        );
        assert_eq!(
            unrepresentable.try_transform_sample(0.0),
            Err(MotionProjectionError::UnrepresentableTransform)
        );
        assert_eq!(
            MotionProjection::between(rect(0.0, 0.0, 10.0, 10.0), rect(0.0, 0.0, 10.0, 10.0),)
                .try_transform_sample(f32::NAN),
            Err(MotionProjectionError::NonFiniteProgress)
        );
    }
}
