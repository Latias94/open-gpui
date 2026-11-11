use super::{PrimitiveRasterSnap, Window};
use crate::{Bounds, Corners, ImageTexture, Pixels, PolychromeSprite};

impl Window {
    /// Paints a renderer-local retained texture in ordinary GPUI painter order.
    ///
    /// The complete selected texel region fills `bounds`. Current opacity, rounded corners,
    /// clipping and subtree transforms apply as for ordinary images. An image imported into
    /// another atlas, or before this atlas's latest reset, is refused before scene insertion.
    /// The caller must order writes to the underlying image before the host submission and
    /// keep the recording receipt until that submission's GPU work completes.
    pub fn paint_image_texture(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        image: &ImageTexture,
    ) -> anyhow::Result<()> {
        self.invalidator.debug_assert_paint();
        anyhow::ensure!(
            image.belongs_to(&self.sprite_atlas),
            "image texture belongs to a different atlas"
        );
        image.validate().map_err(|invalidation| {
            anyhow::anyhow!(
                "image texture atlas epoch {} was replaced by epoch {}",
                invalidation.expected_epoch.get(),
                invalidation.actual_epoch.get(),
            )
        })?;
        anyhow::ensure!(
            [
                bounds.origin.x.0,
                bounds.origin.y.0,
                bounds.size.width.0,
                bounds.size.height.0,
                corner_radii.top_left.0,
                corner_radii.top_right.0,
                corner_radii.bottom_right.0,
                corner_radii.bottom_left.0,
            ]
            .into_iter()
            .all(f32::is_finite),
            "image texture paint geometry must be finite"
        );
        if bounds.size.width <= Pixels::ZERO || bounds.size.height <= Pixels::ZERO {
            return Ok(());
        }
        let Some(base_transform) = self.base_primitive_transform() else {
            return Ok(());
        };
        let local_bounds = self.device_local_bounds(bounds);
        let Some(transform) = self.primitive_raster_transform(
            base_transform,
            local_bounds,
            PrimitiveRasterSnap::NearestEdges,
        ) else {
            return Ok(());
        };
        let corner_radii = corner_radii
            .clamp_radii_for_quad_size(bounds.size)
            .scale(self.scale_factor());
        self.insert_scene_primitive(PolychromeSprite {
            order: 0,
            pad: image.shader_flags(),
            tile: image.tile(),
            grayscale: false.into(),
            opacity: self.element_opacity(),
            bounds: local_bounds,
            clip: Default::default(),
            corner_radii,
            transform,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DevicePixels, HeadlessAtlas, ImageId, ImageTextureAlpha, ImageTextureSampling,
        ImageTextureTransfer, PlatformAtlas, Point, RenderImageParams, Styled as _, TestAppContext,
        canvas, fill, point, px, red, size,
    };
    use std::{borrow::Cow, sync::Arc};

    fn image(
        atlas: Arc<dyn PlatformAtlas>,
        transfer: ImageTextureTransfer,
        alpha: ImageTextureAlpha,
        sampling: ImageTextureSampling,
    ) -> ImageTexture {
        let tile = atlas
            .get_or_insert_with(
                RenderImageParams {
                    image_id: ImageId(831),
                    frame_index: 0,
                }
                .into(),
                &mut || {
                    Ok(Some((
                        size(DevicePixels(2), DevicePixels(2)),
                        Cow::Borrowed(&[0_u8; 16][..]),
                    )))
                },
            )
            .unwrap()
            .unwrap();
        ImageTexture::from_atlas(atlas, tile, transfer, alpha, sampling, Arc::new(())).unwrap()
    }

    #[open_gpui::test]
    fn image_texture_refuses_foreign_atlas_even_when_raw_instances_match(cx: &mut TestAppContext) {
        let visual = cx.add_empty_window();
        visual.draw(Point::default(), size(px(80.0), px(80.0)), |_, _| {
            canvas(
                |_, _, _| (),
                |bounds, _, window, _| {
                    let local = image(
                        window.sprite_atlas.clone(),
                        ImageTextureTransfer::Srgb,
                        ImageTextureAlpha::Straight,
                        ImageTextureSampling::Linear,
                    );
                    let foreign = image(
                        Arc::new(HeadlessAtlas::default()),
                        ImageTextureTransfer::Srgb,
                        ImageTextureAlpha::Straight,
                        ImageTextureSampling::Linear,
                    );
                    assert_eq!(
                        local.tile().texture_instance(),
                        foreign.tile().texture_instance()
                    );
                    let before = window.next_frame.scene.len();
                    assert!(
                        window
                            .paint_image_texture(bounds, Corners::default(), &foreign)
                            .is_err()
                    );
                    assert_eq!(window.next_frame.scene.len(), before);
                    window
                        .paint_image_texture(bounds, Corners::default(), &local)
                        .unwrap();
                    assert_eq!(window.next_frame.scene.polychrome_sprites.len(), 1);
                    assert_eq!(window.next_frame.scene.polychrome_sprites[0].pad, 16);
                },
            )
            .size_full()
        });
    }

    #[open_gpui::test]
    fn image_texture_uses_normal_painter_order_and_refuses_nonfinite_geometry(
        cx: &mut TestAppContext,
    ) {
        let visual = cx.add_empty_window();
        visual.draw(Point::default(), size(px(80.0), px(80.0)), |_, _| {
            canvas(
                |_, _, _| (),
                |bounds, _, window, _| {
                    let source = image(
                        window.sprite_atlas.clone(),
                        ImageTextureTransfer::Linear,
                        ImageTextureAlpha::Premultiplied,
                        ImageTextureSampling::Nearest,
                    );
                    let before = window.next_frame.scene.len();
                    let invalid = Bounds::new(point(px(f32::NAN), px(0.0)), bounds.size);
                    assert!(
                        window
                            .paint_image_texture(invalid, Corners::default(), &source)
                            .is_err()
                    );
                    assert_eq!(window.next_frame.scene.len(), before);
                    window.paint_quad(fill(bounds, red()));
                    window
                        .paint_image_texture(bounds, Corners::all(px(4.0)), &source)
                        .unwrap();
                    window.paint_quad(fill(bounds, red()));
                    let sprite = window.next_frame.scene.polychrome_sprites.last().unwrap();
                    assert_eq!(sprite.pad, 27);
                    assert_eq!(sprite.tile, source.tile());
                    assert!(sprite.corner_radii.top_left.0 > 0.0);
                    assert_eq!(window.next_frame.scene.quads.len(), 2);
                    assert!(window.next_frame.scene.quads[0].order < sprite.order);
                    assert!(sprite.order < window.next_frame.scene.quads[1].order);
                },
            )
            .size_full()
        });
    }
}
