//! Renderer-neutral layout projection primitives.

use crate::{
    MotionEdge, MotionPreference, UiPoint, UiPx, UiRect, reveal_rect_from_edge, ui_point, ui_rect,
    ui_size,
};

const TRANSLATE_EPSILON: f32 = 0.01;
const SCALE_EPSILON: f32 = 0.000_1;

#[derive(Debug, Clone, Copy, PartialEq)]
struct MotionProjectionScale {
    x: f32,
    y: f32,
}

impl MotionProjectionScale {
    const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    const fn x(self) -> f32 {
        self.x
    }

    const fn y(self) -> f32 {
        self.y
    }
}

/// Projection from previous geometry to final target geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionProjection {
    source: UiRect,
    target: UiRect,
}

impl MotionProjection {
    /// Creates a projection between a source rect and final target rect.
    pub const fn between(source: UiRect, target: UiRect) -> Self {
        Self { source, target }
    }

    /// Returns the source rect.
    pub const fn source(self) -> UiRect {
        self.source
    }

    /// Returns the final target rect.
    pub const fn target(self) -> UiRect {
        self.target
    }

    /// Returns the sampled visual bounds after applying projection movement to final-size content.
    pub fn visual_bounds(self, progress: f32) -> UiRect {
        self.visual_bounds_with_preference(progress, MotionPreference::Animated)
    }

    /// Returns the sampled visual bounds while honoring reduced-motion final-state semantics.
    pub fn visual_bounds_with_preference(
        self,
        progress: f32,
        preference: MotionPreference,
    ) -> UiRect {
        self.sample_with_preference(progress, preference)
            .visual_bounds()
    }

    fn sample_with_preference(
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
        )
    }

    fn source_translation(self) -> UiPoint {
        snap_translation(ui_point(
            self.source.origin.x - self.target.origin.x,
            self.source.origin.y - self.target.origin.y,
        ))
    }

    fn source_scale(self) -> MotionProjectionScale {
        snap_scale(MotionProjectionScale::new(
            divide_or_identity(self.source.size.width, self.target.size.width),
            divide_or_identity(self.source.size.height, self.target.size.height),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MotionProjectionSample {
    target_bounds: UiRect,
    progress: f32,
    translation: UiPoint,
    scale: MotionProjectionScale,
}

/// Sampled clip for rendering final-size content through a moving or revealing viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionProjectionClip {
    content_bounds: UiRect,
    visible_bounds: UiRect,
    occlusion_bounds: UiRect,
    progress: f32,
}

impl MotionProjectionClip {
    /// Creates a sampled clip from explicit final-content, visible, and occlusion bounds.
    pub fn new(
        content_bounds: UiRect,
        visible_bounds: UiRect,
        occlusion_bounds: UiRect,
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
    pub fn from_projection(projection: MotionProjection, progress: f32) -> Self {
        Self::from_projection_with_preference(projection, progress, MotionPreference::Animated)
    }

    /// Creates a projection clip while honoring reduced-motion final-state semantics.
    pub fn from_projection_with_preference(
        projection: MotionProjection,
        progress: f32,
        preference: MotionPreference,
    ) -> Self {
        let sample = projection.sample_with_preference(progress, preference);
        Self::new(
            sample.target_bounds(),
            sample.visual_bounds(),
            sample.target_bounds(),
            sample.progress(),
        )
    }

    /// Creates a reveal clip inside final content bounds from the chosen edge.
    pub fn reveal(content_bounds: UiRect, edge: MotionEdge, progress: f32) -> Self {
        let progress = progress.clamp(0.0, 1.0);
        Self::new(
            content_bounds,
            reveal_rect_from_edge(content_bounds, edge, progress),
            content_bounds,
            progress,
        )
    }

    /// Returns the final-size content bounds.
    pub const fn content_bounds(self) -> UiRect {
        self.content_bounds
    }

    /// Returns the sampled visible viewport bounds.
    pub const fn visible_bounds(self) -> UiRect {
        self.visible_bounds
    }

    /// Returns the occlusion bounds that cover the snapped final scene underneath.
    pub const fn occlusion_bounds(self) -> UiRect {
        self.occlusion_bounds
    }

    /// Returns clamped clip progress.
    pub const fn progress(self) -> f32 {
        self.progress
    }
}

impl MotionProjectionSample {
    const fn new(
        target_bounds: UiRect,
        progress: f32,
        translation: UiPoint,
        scale: MotionProjectionScale,
    ) -> Self {
        Self {
            target_bounds,
            progress,
            translation,
            scale,
        }
    }

    const fn target_bounds(self) -> UiRect {
        self.target_bounds
    }

    fn visual_bounds(self) -> UiRect {
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

    const fn progress(self) -> f32 {
        self.progress
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
    use crate::{MotionEdge, MotionPreference, UiPx, ui_point, ui_rect, ui_size};

    fn rect(x: f32, y: f32, width: f32, height: f32) -> crate::UiRect {
        ui_rect(
            ui_point(UiPx::new(x), UiPx::new(y)),
            ui_size(UiPx::new(width), UiPx::new(height)),
        )
    }

    #[test]
    fn projection_visual_bounds_move_without_mutating_target_bounds() {
        let projection = MotionProjection::between(
            rect(10.0, 20.0, 100.0, 50.0),
            rect(30.0, 60.0, 200.0, 100.0),
        );

        assert_eq!(projection.target(), rect(30.0, 60.0, 200.0, 100.0));
        assert_eq!(projection.visual_bounds(0.0), rect(10.0, 20.0, 100.0, 50.0));
        assert_eq!(
            projection.visual_bounds(1.0),
            rect(30.0, 60.0, 200.0, 100.0)
        );
    }

    #[test]
    fn projection_clip_keeps_final_content_bounds_and_samples_visible_bounds() {
        let projection = MotionProjection::between(
            rect(10.0, 20.0, 100.0, 50.0),
            rect(30.0, 60.0, 200.0, 100.0),
        );

        let start = MotionProjectionClip::from_projection(projection, 0.0);
        assert_eq!(start.content_bounds(), rect(30.0, 60.0, 200.0, 100.0));
        assert_eq!(start.visible_bounds(), rect(10.0, 20.0, 100.0, 50.0));
        assert_eq!(start.occlusion_bounds(), rect(30.0, 60.0, 200.0, 100.0));

        let end = MotionProjectionClip::from_projection(projection, 1.0);
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

        assert_eq!(projection.visual_bounds(0.0), rect(10.0, 20.0, 100.0, 50.0));
    }

    #[test]
    fn reduced_motion_projection_samples_final_state_without_spatial_movement() {
        let projection =
            MotionProjection::between(rect(0.0, 0.0, 100.0, 50.0), rect(50.0, 30.0, 100.0, 50.0));

        assert_eq!(
            projection.visual_bounds_with_preference(0.25, MotionPreference::Reduced),
            rect(50.0, 30.0, 100.0, 50.0)
        );
    }

    #[test]
    fn public_visual_bounds_helper_exposes_consumed_projection_capability() {
        let projection = MotionProjection::between(
            rect(10.0, 20.0, 100.0, 50.0),
            rect(30.0, 60.0, 200.0, 100.0),
        );

        assert_eq!(projection.visual_bounds(0.0), rect(10.0, 20.0, 100.0, 50.0));
        assert_eq!(
            projection.visual_bounds(1.0),
            rect(30.0, 60.0, 200.0, 100.0)
        );
        assert_eq!(
            projection.visual_bounds_with_preference(0.25, MotionPreference::Reduced),
            rect(30.0, 60.0, 200.0, 100.0)
        );
    }

    #[test]
    fn reveal_sample_uses_final_bounds_as_the_semantic_rect() {
        let target = rect(30.0, 60.0, 200.0, 100.0);
        let projection = MotionProjection::between(rect(10.0, 20.0, 100.0, 50.0), target);
        let clip = MotionProjectionClip::from_projection(projection, 0.25);

        assert_eq!(clip.content_bounds(), target);
        assert_eq!(clip.occlusion_bounds(), target);
    }
}
