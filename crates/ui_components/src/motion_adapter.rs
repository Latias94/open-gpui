use open_gpui::{SubtreeTransform, SubtreeTransformError, SubtreeTransformOrigin, point, px, size};
use open_gpui_motion::MotionProjectionTransformSample;

/// Converts a checked renderer-neutral motion projection into GPUI's subtree transform authority.
///
/// The transformed element must use the sample's `target_bounds` as its final layout geometry.
/// Motion samples describe scale about that target's top-left corner, followed by translation.
pub fn subtree_transform_from_motion_projection(
    sample: MotionProjectionTransformSample,
) -> Result<SubtreeTransform, SubtreeTransformError> {
    let scale = sample.scale();
    let translation = sample.translation();
    SubtreeTransform::try_new(
        size(scale.x(), scale.y()),
        point(px(translation.x.as_f32()), px(translation.y.as_f32())),
        SubtreeTransformOrigin::TOP_LEFT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_motion::{MotionProjection, motion_point, motion_px, motion_rect, motion_size};

    #[test]
    fn checked_motion_sample_converts_to_gpui_transform() {
        let sample = MotionProjection::between(
            motion_rect(
                motion_point(motion_px(10.0), motion_px(20.0)),
                motion_size(motion_px(100.0), motion_px(50.0)),
            ),
            motion_rect(
                motion_point(motion_px(30.0), motion_px(60.0)),
                motion_size(motion_px(200.0), motion_px(100.0)),
            ),
        )
        .try_transform_sample(0.5)
        .unwrap();

        assert!(subtree_transform_from_motion_projection(sample).is_ok());
    }
}
