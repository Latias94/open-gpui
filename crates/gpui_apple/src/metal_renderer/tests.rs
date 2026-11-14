//! Native Metal renderer controls using the production `Scene::batches` iterator.
//! The ABI fixtures exercise preflight only; they do not provide a Scene clip arena.
//! Windows source review leaves macOS compilation and execution UNQUALIFIED.

use super::*;
use objc2_core_foundation::{CFBoolean, CFDictionary, CFString, CFType};
use objc2_core_video::{
    CVPixelBufferCreate, CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
    kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferMetalCompatibilityKey,
};
use open_gpui::{
    AppContext as _, AtlasKey, AtlasRemoveOutcome, AtlasTile, Context, Corners, HeadlessAppContext,
    Hsla, ImageId, IntoElement, MonochromeSprite, NoopTextSystem, PaddedBool32, PathBuilder,
    Pixels, PlatformAtlas, PlatformPixelBuffer, PolychromeSprite, Render, RenderImageParams,
    RenderSvgParams, SpriteAnimationId, Styled as _, SubtreeClip, SubtreeTransform,
    SubtreeTransformExt as _, TransformationMatrix, Window, canvas, fill, point, px, rgb,
};
use std::borrow::Cow;

#[repr(C)]
struct MonochromeFixture {
    order: u32,
    animation: SpriteAnimationId,
    tile: AtlasTile,
    bounds: Bounds<ScaledPixels>,
    clip: ClipEnvelope,
    color: Hsla,
    transform: PrimitiveTransform,
    transformation: TransformationMatrix,
}

#[repr(C)]
struct PolychromeFixture {
    order: u32,
    pad: u32,
    tile: AtlasTile,
    grayscale: PaddedBool32,
    opacity: f32,
    bounds: Bounds<ScaledPixels>,
    clip: ClipEnvelope,
    corner_radii: Corners<ScaledPixels>,
    transform: PrimitiveTransform,
}

#[test]
fn sprite_fixtures_match_production_layouts() {
    assert_eq!(
        [
            mem::size_of::<MonochromeFixture>(),
            mem::align_of::<MonochromeFixture>(),
            mem::offset_of!(MonochromeFixture, order),
            mem::offset_of!(MonochromeFixture, animation),
            mem::offset_of!(MonochromeFixture, tile),
            mem::offset_of!(MonochromeFixture, bounds),
            mem::offset_of!(MonochromeFixture, clip),
            mem::offset_of!(MonochromeFixture, color),
            mem::offset_of!(MonochromeFixture, transformation),
        ],
        [
            mem::size_of::<MonochromeSprite>(),
            mem::align_of::<MonochromeSprite>(),
            mem::offset_of!(MonochromeSprite, order),
            mem::offset_of!(MonochromeSprite, animation),
            mem::offset_of!(MonochromeSprite, tile),
            mem::offset_of!(MonochromeSprite, bounds),
            mem::offset_of!(MonochromeSprite, clip),
            mem::offset_of!(MonochromeSprite, color),
            mem::offset_of!(MonochromeSprite, transformation),
        ],
    );
    assert_eq!(mem::offset_of!(MonochromeFixture, transform), 104);
    assert_eq!(
        [
            mem::size_of::<PolychromeFixture>(),
            mem::align_of::<PolychromeFixture>(),
            mem::offset_of!(PolychromeFixture, order),
            mem::offset_of!(PolychromeFixture, pad),
            mem::offset_of!(PolychromeFixture, tile),
            mem::offset_of!(PolychromeFixture, grayscale),
            mem::offset_of!(PolychromeFixture, opacity),
            mem::offset_of!(PolychromeFixture, bounds),
            mem::offset_of!(PolychromeFixture, clip),
            mem::offset_of!(PolychromeFixture, corner_radii),
        ],
        [
            mem::size_of::<PolychromeSprite>(),
            mem::align_of::<PolychromeSprite>(),
            mem::offset_of!(PolychromeSprite, order),
            mem::offset_of!(PolychromeSprite, pad),
            mem::offset_of!(PolychromeSprite, tile),
            mem::offset_of!(PolychromeSprite, grayscale),
            mem::offset_of!(PolychromeSprite, opacity),
            mem::offset_of!(PolychromeSprite, bounds),
            mem::offset_of!(PolychromeSprite, clip),
            mem::offset_of!(PolychromeSprite, corner_radii),
        ],
    );
}

fn mixed_scene(monochrome: AtlasTile, polychrome: AtlasTile, monochrome_first: bool) -> Scene {
    let bounds = Bounds::new(
        point(ScaledPixels(0.0), ScaledPixels(0.0)),
        size(ScaledPixels(3.0), ScaledPixels(2.0)),
    );
    let monochrome = MonochromeFixture {
        order: u32::from(!monochrome_first),
        animation: SpriteAnimationId::NONE,
        tile: monochrome,
        bounds,
        clip: ClipEnvelope::default(),
        color: Hsla::default(),
        transform: PrimitiveTransform::IDENTITY,
        transformation: TransformationMatrix::default(),
    };
    let polychrome = PolychromeFixture {
        order: u32::from(monochrome_first),
        pad: 0,
        tile: polychrome,
        grayscale: false.into(),
        opacity: 1.0,
        bounds,
        clip: ClipEnvelope::default(),
        corner_radii: Corners::default(),
        transform: PrimitiveTransform::IDENTITY,
    };
    let mut scene = Scene::default();
    // SAFETY: Both fixtures mirror the corresponding repr(C) sprite's exact fields and order,
    // including the renderer and SVG transforms. Transmute also checks the complete byte sizes.
    unsafe {
        scene
            .monochrome_sprites
            .push(mem::transmute::<MonochromeFixture, MonochromeSprite>(
                monochrome,
            ));
        scene
            .polychrome_sprites
            .push(mem::transmute::<PolychromeFixture, PolychromeSprite>(
                polychrome,
            ));
    }
    scene.finish();
    scene
}

fn keys() -> (AtlasKey, AtlasKey) {
    (
        AtlasKey::Svg(RenderSvgParams {
            path: "metal-preflight.svg".into(),
            size: size(DevicePixels(3), DevicePixels(2)),
        }),
        AtlasKey::Image(RenderImageParams {
            image_id: ImageId(1),
            frame_index: 0,
        }),
    )
}

fn insert(atlas: &MetalAtlas, key: AtlasKey) -> Result<AtlasTile> {
    let channels = match key.texture_kind() {
        open_gpui::AtlasTextureKind::Monochrome => 1,
        _ => 4,
    };
    atlas
        .get_or_insert_with(key, &mut || {
            Ok(Some((
                size(DevicePixels(3), DevicePixels(2)),
                Cow::Owned(vec![97; 3 * 2 * channels]),
            )))
        })?
        .context("inserting preflight texture")
}

#[test]
fn mixed_missing_preflight_refuses_both_orders_and_recovers_after_reallocation() -> Result<()> {
    objc2::rc::autoreleasepool(|_| {
        let mut renderer = MetalRenderer::new_headless(Default::default())?;
        let (monochrome_key, polychrome_key) = keys();
        let mut monochrome = insert(&renderer.sprite_atlas, monochrome_key.clone())?;
        let polychrome = insert(&renderer.sprite_atlas, polychrome_key.clone())?;
        renderer.sprite_atlas.remove(&polychrome_key);
        let buffer_size = renderer.instance_buffer_pool.lock().buffer_size;
        for monochrome_first in [false, true] {
            let scene = mixed_scene(monochrome, polychrome, monochrome_first);
            assert_eq!(scene.batches().count(), 2);
            assert!(renderer.prepare_scene(&scene).is_err());
            assert_eq!(
                renderer.instance_buffer_pool.lock().buffer_size,
                buffer_size
            );
            assert!(renderer.depth_texture.is_none());
            assert!(renderer.headless_render_target.is_none());
            assert!(renderer.last_submitted_command_buffer.is_none());
        }
        assert_eq!(
            renderer
                .sprite_atlas
                .remove_with_diagnostics(&monochrome_key)
                .outcome,
            AtlasRemoveOutcome::TextureFreed
        );
        monochrome = insert(&renderer.sprite_atlas, monochrome_key)?;
        let replacement = insert(&renderer.sprite_atlas, polychrome_key)?;
        assert_eq!(replacement.texture_id, polychrome.texture_id);
        assert_ne!(
            replacement.texture_instance(),
            polychrome.texture_instance()
        );
        let stale = mixed_scene(monochrome, polychrome, true);
        assert!(renderer.prepare_scene(&stale).is_err());
        let recovered = mixed_scene(monochrome, replacement, true);
        let receipt = renderer.prepare_scene(&recovered)?;
        assert_eq!(receipt.batches.len(), 2);
        assert_eq!(receipt.atlas_lease.texture_instances().len(), 2);
        drop(receipt);
        let image = renderer
            .render_scene_to_image(&Scene::default(), size(DevicePixels(7), DevicePixels(3)))?;
        assert_eq!(image.dimensions(), (7, 3));
        assert!(image.pixels().all(|pixel| pixel.0 == [0, 0, 0, 255]));
        renderer.destroy();
        Ok(())
    })
}

#[test]
fn preflight_receipt_pins_actual_mixed_textures_after_key_removal() -> Result<()> {
    objc2::rc::autoreleasepool(|_| {
        let mut renderer = MetalRenderer::new_headless(Default::default())?;
        let (monochrome_key, polychrome_key) = keys();
        let monochrome = insert(&renderer.sprite_atlas, monochrome_key.clone())?;
        let polychrome = insert(&renderer.sprite_atlas, polychrome_key.clone())?;
        let scene = mixed_scene(monochrome, polychrome, true);
        let receipt = renderer.prepare_scene(&scene)?;
        for key in [&monochrome_key, &polychrome_key] {
            assert_eq!(
                renderer.sprite_atlas.remove_with_diagnostics(key).outcome,
                AtlasRemoveOutcome::TextureRetained
            );
        }
        for (batch, tile) in receipt.batches.iter().zip([monochrome, polychrome]) {
            let texture = match batch {
                PreparedMetalBatch::MonochromeSprites { texture, .. }
                | PreparedMetalBatch::PolychromeSprites { texture, .. } => texture,
                _ => panic!("the receipt must contain the exact mixed sprite batches"),
            };
            let resident = renderer
                .sprite_atlas
                .metal_texture(tile.texture_instance())
                .unwrap();
            assert!(ptr::eq(&**texture, &*resident));
            let channels = if tile == monochrome { 1 } else { 4 };
            let mut bytes = vec![0_u8; 3 * 2 * channels];
            // SAFETY: The preflight pins this exact tile and the destination has all six texels at the correct stride.
            unsafe {
                texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                    ptr::NonNull::from(bytes.as_mut_slice()).cast(),
                    3 * channels,
                    MTLRegion {
                        origin: MTLOrigin {
                            x: tile.bounds.origin.x.into(),
                            y: tile.bounds.origin.y.into(),
                            z: 0,
                        },
                        size: MTLSize {
                            width: 3,
                            height: 2,
                            depth: 1,
                        },
                    },
                    0,
                );
            }
            assert_eq!(bytes, vec![97; 3 * 2 * channels]);
        }
        drop(receipt);
        assert!(
            renderer
                .sprite_atlas
                .metal_texture(monochrome.texture_instance())
                .is_none()
        );
        assert!(
            renderer
                .sprite_atlas
                .metal_texture(polychrome.texture_instance())
                .is_none()
        );
        renderer.destroy();
        Ok(())
    })
}

#[test]
fn repeated_batches_pin_each_instance_once_and_preserve_every_draw_range() -> Result<()> {
    objc2::rc::autoreleasepool(|_| {
        let mut renderer = MetalRenderer::new_headless(Default::default())?;
        let (monochrome_key, polychrome_key) = keys();
        let monochrome = insert(&renderer.sprite_atlas, monochrome_key.clone())?;
        let polychrome = insert(&renderer.sprite_atlas, polychrome_key.clone())?;
        let mut scene = mixed_scene(monochrome, polychrome, true);
        let mut repeated = scene.monochrome_sprites[0];
        repeated.order = 2;
        scene.monochrome_sprites.push(repeated);
        scene.finish();
        assert_eq!(scene.batches().count(), 3);
        let receipt = renderer.prepare_scene(&scene)?;
        assert_eq!(receipt.atlas_lease.texture_instances().len(), 2);
        assert!(matches!(
            &receipt.batches[..],
            [
                PreparedMetalBatch::MonochromeSprites { range: first, .. },
                PreparedMetalBatch::PolychromeSprites { range: second, .. },
                PreparedMetalBatch::MonochromeSprites { range: third, .. },
            ] if *first == (0..1) && *second == (0..1) && *third == (1..2)
        ));
        renderer.sprite_atlas.remove(&monochrome_key);
        renderer.sprite_atlas.remove(&polychrome_key);
        drop(receipt);
        assert!(
            renderer
                .sprite_atlas
                .metal_texture(monochrome.texture_instance())
                .is_none()
        );
        assert!(
            renderer
                .sprite_atlas
                .metal_texture(polychrome.texture_instance())
                .is_none()
        );
        renderer.destroy();
        Ok(())
    })
}

fn video_buffer(luma: u8, chroma: [u8; 2]) -> Result<PlatformPixelBuffer> {
    let io_surface = CFDictionary::<CFString, CFType>::empty();
    let mut buffer = ptr::null_mut();
    // SAFETY: The attributes have their documented CF types and the output pointer is writable.
    let status = unsafe {
        let attributes = CFDictionary::<CFString, CFType>::from_slices(
            &[
                kCVPixelBufferIOSurfacePropertiesKey,
                kCVPixelBufferMetalCompatibilityKey,
            ],
            &[&io_surface, CFBoolean::new(true)],
        );
        CVPixelBufferCreate(
            None,
            16,
            16,
            kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
            Some(attributes.as_opaque()),
            ptr::NonNull::from(&mut buffer),
        )
    };
    anyhow::ensure!(
        status == kCVReturnSuccess,
        "creating test video buffer: {status}"
    );
    let buffer = ptr::NonNull::new(buffer).context("retrieving test video buffer")?;
    let flags = CVPixelBufferLockFlags::empty();
    // SAFETY: CoreVideo transfers ownership on success and no other user can access this buffer.
    let (buffer, status) = unsafe {
        let buffer = CFRetained::from_raw(buffer);
        let status = CVPixelBufferLockBaseAddress(&buffer, flags);
        (buffer, status)
    };
    anyhow::ensure!(
        status == kCVReturnSuccess,
        "locking test video buffer: {status}"
    );

    let fill: Result<()> = (|| {
        for (plane, texel) in [(0, &[luma][..]), (1, &chroma[..])] {
            let base = CVPixelBufferGetBaseAddressOfPlane(&buffer, plane).cast::<u8>();
            let stride = CVPixelBufferGetBytesPerRowOfPlane(&buffer, plane);
            let width = CVPixelBufferGetWidthOfPlane(&buffer, plane);
            anyhow::ensure!(
                !base.is_null() && width * texel.len() <= stride,
                "invalid test video plane"
            );
            for row in 0..CVPixelBufferGetHeightOfPlane(&buffer, plane) {
                for column in 0..width {
                    let offset = row * stride + column * texel.len();
                    // SAFETY: The destination texel is within the locked plane and disjoint from the source.
                    unsafe {
                        ptr::copy_nonoverlapping(texel.as_ptr(), base.add(offset), texel.len());
                    }
                }
            }
        }
        Ok(())
    })();
    // SAFETY: The successful lock used the same flags, and the CPU writes are complete.
    let status = unsafe { CVPixelBufferUnlockBaseAddress(&buffer, flags) };
    fill?;
    anyhow::ensure!(
        status == kCVReturnSuccess,
        "unlocking test video buffer: {status}"
    );
    Ok(buffer)
}

struct SurfaceBatchFixture([PlatformPixelBuffer; 3]);

impl Render for SurfaceBatchFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let [first, second, third] = self.0.clone();
        let bounds_at = |x| Bounds::new(point(px(x), px(0.0)), size(px(12.0), px(12.0)));
        let clip = SubtreeClip::try_rounded_rect(
            bounds_at(0.0),
            Corners::all(px(6.0)).map(|radius| size(*radius, *radius)),
        )
        .expect("valid rounded clip for the second surface");
        canvas(
            move |_, window, _| window.prepare_subtree_clip(&clip, bounds_at(16.0)),
            move |_, clip, window, _| {
                window.paint_surface(bounds_at(0.0), first);
                let mut separator = PathBuilder::fill();
                separator.move_to(point(px(8.0), px(0.0)));
                for (x, y) in [(40.0, 0.0), (40.0, 12.0), (8.0, 12.0), (8.0, 0.0)] {
                    separator.line_to(point(px(x), px(y)));
                }
                separator.close();
                window.paint_path(separator.build().expect("valid separator"), rgb(0xff0000));
                window
                    .with_prepared_subtree_clip(&clip, |window| {
                        window.paint_surface(bounds_at(16.0), second);
                    })
                    .expect("the second surface uses its prepaint clip");
                window.paint_surface(bounds_at(32.0), third);
            },
        )
        .size_full()
        .with_subtree_transform(
            SubtreeTransform::try_translation(point(px(2.0), px(2.0)))
                .expect("valid fixture translation"),
        )
    }
}

#[test]
fn surface_batches_read_back_with_global_instance_indices() -> Result<()> {
    objc2::rc::autoreleasepool(|_| {
        let fixture = SurfaceBatchFixture([
            video_buffer(64, [128, 128])?,
            video_buffer(96, [128, 192])?,
            video_buffer(128, [192, 128])?,
        ]);
        let mut app =
            HeadlessAppContext::with_platform(Arc::new(NoopTextSystem), Arc::new(()), || {
                Ok(Some(Box::new(MetalHeadlessRenderer::new()?)))
            });
        let viewport: Size<Pixels> = size(px(48.0), px(20.0));
        let handle = app.open_window(viewport, |_, app| app.new(|_| fixture))?;
        app.update_window(handle.into(), |_, window, _| {
            let _ = window.activate_window();
        })?;
        app.run_until_parked();
        app.simulate_window_scheduled_frame(handle.into())?;
        let capture = app.capture_frame(handle.into())?;
        let scale = capture.metadata.scale_factor;
        let image = capture.into_image()?;

        for (x, y, expected) in [
            (1.0, 8.0, [0, 0, 0, 255]),
            (5.0, 8.0, [65, 63, 65, 255]),
            (12.0, 8.0, [255, 0, 0, 255]),
            (19.0, 3.0, [255, 0, 0, 255]),
            (24.0, 8.0, [186, 50, 97, 255]),
            (40.0, 8.0, [129, 105, 242, 255]),
        ] {
            let actual = image.get_pixel((x * scale) as u32, (y * scale) as u32).0;
            assert!(
                actual
                    .into_iter()
                    .zip(expected)
                    .all(|(a, b)| a.abs_diff(b) <= 2),
                "pixel at ({x}, {y}) was {actual:?}, expected {expected:?}",
            );
        }
        Ok(())
    })
}

struct OpaqueClipFixture;

impl Render for OpaqueClipFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let clip_bounds = Bounds::new(point(px(4.0), px(4.0)), size(px(24.0), px(24.0)));
        let clip = SubtreeClip::try_rounded_rect(
            Bounds::new(point(px(0.0), px(0.0)), clip_bounds.size),
            Corners::all(px(12.0)).map(|radius| size(*radius, *radius)),
        )
        .expect("valid opaque clip");
        canvas(
            move |_, window, _| window.prepare_subtree_clip(&clip, clip_bounds),
            move |_, clip, window, _| {
                let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(32.0), px(32.0)));
                window.paint_quad(fill(bounds, rgb(0x0000ff)));
                let mut path = PathBuilder::fill();
                path.add_polygon(
                    &[
                        point(px(0.0), px(0.0)),
                        point(px(32.0), px(0.0)),
                        point(px(32.0), px(32.0)),
                        point(px(0.0), px(32.0)),
                    ],
                    true,
                );
                window.paint_path(path.build().expect("valid background path"), rgb(0x00ff00));
                window
                    .with_prepared_subtree_clip(&clip, |window| {
                        window.paint_quad(fill(clip_bounds, rgb(0xff0000)));
                    })
                    .expect("matching prepaint clip");
                window.paint_quad(fill(
                    Bounds::new(point(px(20.0), px(0.0)), size(px(12.0), px(32.0))),
                    open_gpui::rgba(0xffffff80),
                ));
            },
        )
        .size_full()
    }
}

#[test]
fn opaque_depth_preserves_paths_and_blending_outside_exact_rounded_clips() -> Result<()> {
    objc2::rc::autoreleasepool(|_| {
        let mut app =
            HeadlessAppContext::with_platform(Arc::new(NoopTextSystem), Arc::new(()), || {
                Ok(Some(Box::new(MetalHeadlessRenderer::new()?)))
            });
        let handle = app.open_window(size(px(32.0), px(32.0)), |_, app| {
            app.new(|_| OpaqueClipFixture)
        })?;
        app.update_window(handle.into(), |_, window, _| {
            let _ = window.activate_window();
        })?;
        app.run_until_parked();
        app.simulate_window_scheduled_frame(handle.into())?;
        let capture = app.capture_frame(handle.into())?;
        let scale = capture.metadata.scale_factor;
        let image = capture.into_image()?;
        for (x, y, expected) in [
            (1.0, 1.0, [0, 255, 0, 255]),
            (4.0, 4.0, [0, 255, 0, 255]),
            (16.0, 16.0, [255, 0, 0, 255]),
            (24.0, 16.0, [255, 128, 128, 255]),
            (30.0, 1.0, [128, 255, 128, 255]),
        ] {
            let actual = image.get_pixel((x * scale) as u32, (y * scale) as u32).0;
            assert_eq!(actual, expected, "pixel at ({x}, {y})");
        }
        Ok(())
    })
}

#[test]
fn manual_clip_gpu_readback_preserves_large_coordinates_tiny_uvs_and_fractional_edges() -> Result<()>
{
    objc2::rc::autoreleasepool(|_| {
        let mut renderer = MetalRenderer::new_headless(Default::default())?;
        let source = format!(
            "{}\n{}\n{}",
            include_str!(concat!(env!("OUT_DIR"), "/scene.h")),
            include_str!("../shaders.metal"),
            r#"
struct ClipPrecisionOutput {
  float4 position [[position]];
  float2 unit_vertex;
  float2 local_position;
  float4 clip_bounds [[flat]];
};

vertex ClipPrecisionOutput clip_precision_vertex(
    uint vertex_id [[vertex_id]],
    constant float4 *parameters [[buffer(0)]],
    constant Size_DevicePixels *viewport [[buffer(1)]]) {
  Bounds_ScaledPixels bounds = {{parameters[0].x, parameters[0].y},
                                {parameters[0].z, parameters[0].w}};
  PrimitiveTransform transform = {parameters[1].x, parameters[1].y,
                                  parameters[1].z, parameters[1].w};
  ClipEnvelope clip = {{{parameters[2].x, parameters[2].y},
                        {parameters[2].z, parameters[2].w}}, 0u, 1u};
  float2 unit_vertex = float2(float(vertex_id & 1u), 0.5 * float(vertex_id & 2u));
  ClippedVertex clipped = clip_transformed_primitive(unit_vertex, bounds, clip, transform);
  return ClipPrecisionOutput{to_device_position_impl(clipped.position, viewport),
                              clipped.unit_vertex, clipped.local_position, parameters[2]};
}

fragment float4 clip_precision_fragment(
    ClipPrecisionOutput input [[stage_in]],
    constant GpuClipShape *clip_shapes [[buffer(GlobalInputIndex_ClipShapes)]]) {
  ClipEnvelope clip = {{{input.clip_bounds.x, input.clip_bounds.y},
                        {input.clip_bounds.z, input.clip_bounds.w}}, 0u, 1u};
  if (!clip_stack_contains(input.position.xy, clip, clip_shapes)) {
    discard_fragment();
  }
  return float4(input.unit_vertex, input.local_position.x, 1.0);
}
"#,
        );
        let library = renderer.device.newLibraryWithSource_options_error(
            &NSString::from_str(&source),
            Some(&objc2_metal::MTLCompileOptions::new()),
        )?;
        let pipeline = build_pipeline_state(
            &renderer.device,
            &library,
            "clip_precision",
            "clip_precision_vertex",
            "clip_precision_fragment",
            MTLPixelFormat::RGBA32Float,
            false,
        )?;
        let viewport = size(DevicePixels(64), DevicePixels(4));
        renderer.update_depth_texture(viewport)?;
        let descriptor = MTLTextureDescriptor::new();
        unsafe {
            descriptor.setWidth(64);
            descriptor.setHeight(4);
        }
        descriptor.setPixelFormat(MTLPixelFormat::RGBA32Float);
        descriptor.setUsage(MTLTextureUsage::RenderTarget);
        let shared_storage = cfg!(target_os = "ios") || renderer.is_apple_gpu;
        descriptor.setStorageMode(if shared_storage {
            MTLStorageMode::Shared
        } else {
            MTLStorageMode::Managed
        });
        let target = renderer
            .device
            .newTextureWithDescriptor(&descriptor)
            .context("allocating clip precision render target")?;
        for (case, parameters) in [
            [
                [16_777_216.0f32, 0.0, 7.0, 2.0],
                [1.0, 1.0, -16_777_206.0, 0.0],
                [0.0, 0.0, 64.0, 4.0],
            ],
            [
                [0.0, 0.0, 1e-7, 2.0],
                [1e8, 1.0, 10.0, 0.0],
                [12.25, 0.0, 6.25, 4.0],
            ],
            [
                [8.0, 0.0, 16.0, 4.0],
                [1.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 16.5001, 1.5001],
            ],
        ]
        .into_iter()
        .enumerate()
        {
            let clip_shape = open_gpui::GpuClipShape {
                bounds: Bounds::new(
                    point(
                        ScaledPixels(parameters[2][0]),
                        ScaledPixels(parameters[2][1]),
                    ),
                    size(
                        ScaledPixels(parameters[2][2]),
                        ScaledPixels(parameters[2][3]),
                    ),
                ),
                ..Default::default()
            };
            let render_pass = MTLRenderPassDescriptor::new();
            let color = unsafe { render_pass.colorAttachments().objectAtIndexedSubscript(0) };
            color.setTexture(Some(&target));
            color.setLoadAction(MTLLoadAction::Clear);
            color.setStoreAction(MTLStoreAction::Store);
            color.setClearColor(MTLClearColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 0.0,
            });
            let depth = render_pass.depthAttachment();
            depth.setTexture(renderer.depth_texture.as_deref());
            depth.setLoadAction(MTLLoadAction::Clear);
            depth.setClearDepth(0.0);
            depth.setStoreAction(MTLStoreAction::DontCare);
            let command_buffer = renderer
                .command_queue
                .commandBuffer()
                .context("creating clip precision commands")?;
            let encoder = command_buffer
                .renderCommandEncoderWithDescriptor(&render_pass)
                .context("creating clip precision encoder")?;
            encoder.setRenderPipelineState(&pipeline);
            encoder.setViewport(MTLViewport {
                originX: 0.0,
                originY: 0.0,
                width: 64.0,
                height: 4.0,
                znear: 0.0,
                zfar: 1.0,
            });
            unsafe {
                encoder.setVertexBytes_length_atIndex(
                    ptr::NonNull::from(&parameters).cast(),
                    mem::size_of_val(&parameters),
                    0,
                );
                encoder.setVertexBytes_length_atIndex(
                    ptr::NonNull::from(&viewport).cast(),
                    mem::size_of_val(&viewport),
                    1,
                );
                encoder.setFragmentBytes_length_atIndex(
                    ptr::NonNull::from(&clip_shape).cast(),
                    mem::size_of_val(&clip_shape),
                    GlobalInputIndex::ClipShapes as usize,
                );
                encoder.drawPrimitives_vertexStart_vertexCount(
                    MTLPrimitiveType::TriangleStrip,
                    0,
                    4,
                );
            }
            encoder.endEncoding();
            #[cfg(target_os = "macos")]
            if !shared_storage {
                let blit = command_buffer
                    .blitCommandEncoder()
                    .context("synchronizing clip precision target")?;
                blit.synchronizeResource(ProtocolObject::from_ref(&*target));
                blit.endEncoding();
            }
            command_buffer.commit();
            command_buffer.waitUntilCompleted();
            anyhow::ensure!(
                command_buffer.status() == MTLCommandBufferStatus::Completed,
                "clip precision GPU failure: {:?}",
                command_buffer.error()
            );
            let mut pixels = [[0.0f32; 4]; 64 * 4];
            unsafe {
                target.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                    ptr::NonNull::from(&mut pixels).cast(),
                    64 * mem::size_of::<[f32; 4]>(),
                    MTLRegion {
                        origin: MTLOrigin { x: 0, y: 0, z: 0 },
                        size: MTLSize {
                            width: 64,
                            height: 4,
                            depth: 1,
                        },
                    },
                    0,
                );
            }
            let pixel =
                |pixel_x: usize, pixel_y: usize| -> [f32; 4] { pixels[pixel_y * 64 + pixel_x] };
            let expected_uv = match case {
                0 => 0.8125,
                1 => 0.65,
                _ => 0.53125,
            };
            let sample = pixel(16, 1);
            assert_eq!(sample[3], 1.0, "case {case}: missing pixel center");
            assert!(
                (sample[0] - expected_uv).abs() < 1e-5,
                "case {case}: UV was {}, expected {expected_uv}",
                sample[0]
            );
            if case == 0 {
                assert_eq!(pixel(17, 1)[3], 1.0);
                assert_eq!(pixel(18, 1)[3], 0.0);
            } else if case == 1 {
                assert!((sample[2] - 6.5e-8).abs() < 1e-12);
                assert_eq!(pixel(12, 1)[3], 1.0);
                assert_eq!(pixel(18, 1)[3], 0.0);
            } else {
                assert_eq!(pixel(17, 1)[3], 0.0);
                assert_eq!(pixel(16, 2)[3], 0.0);
            }
        }
        renderer.destroy();
        Ok(())
    })
}
