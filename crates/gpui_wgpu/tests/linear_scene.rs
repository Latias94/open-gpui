//! Actual host-device recording and readback for the explicit scene composition domain.
//! HeadlessAppContext supplies Scene fixtures only; this does not qualify native presentation.

#![cfg(all(not(target_family = "wasm"), feature = "test-support"))]

use anyhow::{Context as _, Result};
use open_gpui::{
    AppContext as _, Bounds, ColorSpace, Context, DevicePixels, HeadlessAppContext, Hsla,
    ImageTexture, ImageTextureAlpha, ImageTextureSampling, ImageTextureTransfer, IntoElement,
    ParentElement as _, PathBuilder, PlatformAtlas, PlatformHeadlessRenderer, Render, RenderImage,
    Scene, Size, Styled as _, TextRenderingMode, Window, canvas, div, fill, linear_color_stop,
    linear_gradient, point, px, rgb, size,
};
use open_gpui_wgpu::{
    CosmicTextSystem, WgpuAtlas, WgpuImportedImageDescriptor, WgpuSceneColorSpace,
    WgpuSceneRenderer, wgpu,
};
use std::{
    borrow::Cow,
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

const WIDTH: u32 = 192;
const HEIGHT: u32 = 160;
const IBM_PLEX: &[u8] = include_bytes!("../../../tests/fixtures/fonts/IBMPlexSans-Regular.ttf");
const BACKGROUND: [f32; 3] = [64.0 / 255.0; 3];
const FOREGROUND: [f32; 3] = [192.0 / 255.0, 128.0 / 255.0, 64.0 / 255.0];

struct SceneSource {
    atlas: Arc<WgpuAtlas>,
    captured: Rc<RefCell<Option<Scene>>>,
}

impl PlatformHeadlessRenderer for SceneSource {
    fn render_scene(&mut self, scene: &Scene, _: Size<DevicePixels>) -> Result<()> {
        *self.captured.borrow_mut() = Some(scene.clone());
        Ok(())
    }

    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        _: Size<DevicePixels>,
    ) -> Result<image::RgbaImage> {
        *self.captured.borrow_mut() = Some(scene.clone());
        anyhow::bail!("this fixture exports a scene without executing GPU work")
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }
}

struct ColorRoot {
    images: Vec<ImageTexture>,
    decoded: Arc<RenderImage>,
}

impl Render for ColorRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let images = self.images.clone();
        let decoded = self.decoded.clone();
        div()
            .relative()
            .size_full()
            .bg(rgb(0x404040))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        let rect = |left, top, width, height| {
                            Bounds::new(
                                bounds.origin + point(px(left), px(top)),
                                size(px(width), px(height)),
                            )
                        };
                        let foreground: Hsla = rgb(0xc08040).into();
                        window.paint_quad(fill(rect(2.0, 2.0, 12.0, 10.0), rgb(0x808080)));
                        window
                            .paint_quad(fill(rect(18.0, 2.0, 12.0, 10.0), foreground.opacity(0.5)));
                        window.paint_quad(
                            fill(rect(34.0, 2.0, 12.0, 10.0), rgb(0x808080))
                                .border_widths(px(2.0))
                                .border_color(foreground.opacity(0.5)),
                        );
                        window.paint_quad(fill(
                            rect(50.0, 2.0, 28.0, 10.0),
                            linear_gradient(
                                90.0,
                                linear_color_stop(rgb(0x204060), 0.0),
                                linear_color_stop(rgb(0xa0c0e0), 1.0),
                            )
                            .color_space(ColorSpace::Srgb),
                        ));
                        for (left, color) in [
                            (2.0, foreground.opacity(0.5)),
                            (18.0, Hsla::from(rgb(0x808080))),
                        ] {
                            let mut path = PathBuilder::fill();
                            let path_bounds = rect(left, 18.0, 12.0, 10.0);
                            path.add_polygon(
                                &[
                                    path_bounds.origin,
                                    path_bounds.origin + point(px(12.0), px(0.0)),
                                    path_bounds.origin + point(px(12.0), px(10.0)),
                                    path_bounds.origin + point(px(0.0), px(10.0)),
                                ],
                                true,
                            );
                            window.paint_path(
                                path.build().expect("the rectangle must tessellate"),
                                color,
                            );
                        }
                        for (left, value) in [(50.0, 0x060606), (66.0, 0x080808)] {
                            window.paint_quad(fill(rect(left, 34.0, 6.0, 10.0), rgb(value)));
                            let mut path = PathBuilder::fill();
                            let origin = rect(left + 6.0, 34.0, 6.0, 10.0).origin;
                            path.add_polygon(
                                &[
                                    origin,
                                    origin + point(px(6.0), px(0.0)),
                                    origin + point(px(6.0), px(10.0)),
                                    origin + point(px(0.0), px(10.0)),
                                ],
                                true,
                            );
                            window.paint_path(
                                path.build().expect("the dark rectangle must tessellate"),
                                rgb(value),
                            );
                        }
                        for (index, image) in images.iter().enumerate() {
                            window
                                .paint_image_texture(
                                    rect(34.0 + index as f32 * 16.0, 18.0, 12.0, 10.0),
                                    Default::default(),
                                    image,
                                )
                                .expect("the exact renderer atlas must accept its imported image");
                        }
                        let image_bounds = rect(2.0, 34.0, 12.0, 10.0);
                        window
                            .paint_image(
                                image_bounds,
                                image_bounds,
                                Default::default(),
                                decoded,
                                0,
                                false,
                            )
                            .expect("the ordinary sRGB-numeric atlas image must paint");
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .left(px(2.0))
                    .top(px(48.0))
                    .w(px(80.0))
                    .h(px(28.0))
                    .font_family("IBM Plex Sans")
                    .text_size(px(24.0))
                    .text_color(rgb(0xc08040))
                    .child("MW"),
            )
    }
}

fn scene_fixture(
    atlas: &Arc<WgpuAtlas>,
    images: Vec<ImageTexture>,
) -> Result<(HeadlessAppContext, Scene)> {
    let captured = Rc::new(RefCell::new(None));
    let mut app = HeadlessAppContext::with_platform(
        Arc::new(CosmicTextSystem::new_without_system_fonts("IBM Plex Sans")),
        Arc::new(()),
        {
            let atlas = atlas.clone();
            let captured = captured.clone();
            move || {
                Ok(Some(Box::new(SceneSource {
                    atlas: atlas.clone(),
                    captured: captured.clone(),
                })))
            }
        },
    );
    app.update(|cx| cx.set_text_rendering_mode(TextRenderingMode::Grayscale));
    app.text_system().add_fonts(vec![Cow::Borrowed(IBM_PLEX)])?;
    let decoded = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        image::RgbaImage::from_pixel(2, 2, image::Rgba([64, 128, 192, 128])),
    )]));
    let window = app.open_window(size(px(96.0), px(80.0)), move |_, cx| {
        cx.new(|_| ColorRoot { images, decoded })
    })?;
    app.update_window(window.into(), |_, window, cx| {
        assert_eq!(window.scale_factor(), 2.0);
        window.refresh();
        window.draw(cx).clear(cx);
    })?;
    assert!(app.capture_screenshot(window.into()).is_err());
    let scene = captured
        .borrow_mut()
        .take()
        .context("the fixture must export a painted Scene")?;
    assert_eq!(scene.paths.len(), 4);
    assert_eq!(scene.polychrome_sprites.len(), 4);
    assert!(!scene.monochrome_sprites.is_empty());
    assert!(scene.subpixel_sprites.is_empty());
    Ok((app, scene))
}

fn imported_images(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    atlas: &Arc<WgpuAtlas>,
) -> Result<Vec<ImageTexture>> {
    [
        (
            wgpu::TextureFormat::Rgba16Float,
            ImageTextureTransfer::Linear,
            [0.03125, 0.125, 0.25, 0.5],
        ),
        (
            wgpu::TextureFormat::Rgba8Unorm,
            ImageTextureTransfer::Srgb,
            [64.0 / 255.0, 32.0 / 255.0, 16.0 / 255.0, 128.0 / 255.0],
        ),
        (
            wgpu::TextureFormat::Rgba8UnormSrgb,
            ImageTextureTransfer::Srgb,
            [0.03125, 0.125, 0.25, 128.0 / 255.0],
        ),
    ]
    .into_iter()
    .map(|(format, transfer, rgba)| {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("host-produced sampled-domain premultiplied image"),
            size: wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("original host encoder produces the imported image"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: rgba[0],
                            g: rgba[1],
                            b: rgba[2],
                            a: rgba[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        let image = atlas.import_image(WgpuImportedImageDescriptor {
            texture,
            mip_level: 0,
            array_layer: 0,
            region: Bounds::new(
                point(DevicePixels(0), DevicePixels(0)),
                size(DevicePixels(2), DevicePixels(2)),
            ),
            transfer,
            alpha: ImageTextureAlpha::Premultiplied,
            sampling: ImageTextureSampling::Nearest,
            owner: None,
        })?;
        assert_eq!(image.alpha(), ImageTextureAlpha::Premultiplied);
        assert_eq!(
            image.sampled_transfer(),
            if format == wgpu::TextureFormat::Rgba8Unorm {
                ImageTextureTransfer::Srgb
            } else {
                ImageTextureTransfer::Linear
            }
        );
        Ok(image)
    })
    .collect()
}

fn copy_to_readback(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::Texture,
) -> wgpu::Buffer {
    let texel_bytes = if target.format() == wgpu::TextureFormat::Rgba16Float {
        8
    } else {
        4
    };
    let row_bytes = (WIDTH * texel_bytes).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("original host encoder color-domain readback"),
        size: u64::from(row_bytes) * u64::from(HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(HEIGHT),
            },
        },
        target.size(),
    );
    buffer
}

struct Readback {
    color_space: WgpuSceneColorSpace,
    format: wgpu::TextureFormat,
    pixels: Vec<[f32; 4]>,
}

impl Readback {
    fn logical_pixel(&self, left: u32, top: u32) -> [f32; 4] {
        self.pixels[((top * 2 + 1) * WIDTH + left * 2 + 1) as usize]
    }

    fn assert_pixel(&self, left: u32, top: u32, working_rgb: [f32; 3]) {
        let mut expected = [working_rgb[0], working_rgb[1], working_rgb[2], 1.0];
        if self.format.has_srgb_suffix() {
            for component in &mut expected[..3] {
                *component = encode_srgb(*component);
            }
        }
        let actual = self.logical_pixel(left, top);
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= 0.012,
                "{:?} {:?} pixel ({left}, {top}): {actual} != {expected}",
                self.color_space,
                self.format,
            );
        }
    }
}

fn decode_srgb(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn encode_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn working(color_space: WgpuSceneColorSpace, rgb: [f32; 3]) -> [f32; 3] {
    match color_space {
        WgpuSceneColorSpace::SrgbEncoded => rgb,
        WgpuSceneColorSpace::LinearSrgb => rgb.map(decode_srgb),
    }
}

fn over(foreground: [f32; 3], alpha: f32, background: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|channel| foreground[channel] * alpha + background[channel] * (1.0 - alpha))
}

fn half_to_f32(bytes: [u8; 2]) -> f32 {
    let bits = u16::from_le_bytes(bytes);
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((bits >> 10) & 31);
    let mantissa = f32::from(bits & 1023);
    assert_ne!(exponent, 31, "scene output must contain finite values");
    if exponent == 0 {
        sign * mantissa * 2.0_f32.powi(-24)
    } else {
        sign * (1.0 + mantissa / 1024.0) * 2.0_f32.powi(exponent - 15)
    }
}

fn read_pixels(buffer: &wgpu::Buffer, format: wgpu::TextureFormat) -> Result<Vec<[f32; 4]>> {
    let float = format == wgpu::TextureFormat::Rgba16Float;
    let texel_bytes = if float { 8 } else { 4 };
    let row_bytes = (WIDTH * texel_bytes).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let mapped = buffer.slice(..).get_mapped_range()?;
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT) as usize);
    for row in mapped
        .chunks_exact(row_bytes as usize)
        .take(HEIGHT as usize)
    {
        for texel in row[..(WIDTH * texel_bytes) as usize].chunks_exact(texel_bytes as usize) {
            pixels.push(std::array::from_fn(|channel| {
                if float {
                    half_to_f32([texel[channel * 2], texel[channel * 2 + 1]])
                } else {
                    f32::from(texel[channel]) / 255.0
                }
            }));
        }
    }
    drop(mapped);
    buffer.unmap();
    Ok(pixels)
}

#[test]
fn host_records_linear_srgb_and_preserves_the_legacy_default() -> Result<()> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = open_gpui::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        apply_limit_buckets: false,
        ..Default::default()
    }))?;
    let info = adapter.get_info();
    println!(
        "OPEN_GPUI_LINEAR_SCENE_GPU backend={:?} name={} driver={}",
        info.backend, info.name, info.driver
    );
    let (device, queue) = open_gpui::block_on(
        adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("host-owned linear scene device"),
            required_features: adapter.features() & wgpu::Features::DUAL_SOURCE_BLENDING,
            required_limits: wgpu::Limits::downlevel_defaults()
                .using_resolution(adapter.limits())
                .using_alignment(adapter.limits()),
            ..Default::default()
        }),
    )?;
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let errors = Arc::new(Mutex::new(Vec::new()));
    device.on_uncaptured_error(Arc::new({
        let errors = errors.clone();
        move |error| errors.lock().unwrap().push(error.to_string())
    }));
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("one original host encoder for all scene composition modes"),
    });
    let mut recordings = Vec::new();
    let mut readbacks = Vec::new();
    for (mode, format) in [
        (None, wgpu::TextureFormat::Rgba8Unorm),
        (
            Some(WgpuSceneColorSpace::SrgbEncoded),
            wgpu::TextureFormat::Rgba8Unorm,
        ),
        (None, wgpu::TextureFormat::Rgba8UnormSrgb),
        (
            Some(WgpuSceneColorSpace::SrgbEncoded),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ),
        (
            Some(WgpuSceneColorSpace::LinearSrgb),
            wgpu::TextureFormat::Rgba8Unorm,
        ),
        (
            Some(WgpuSceneColorSpace::LinearSrgb),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ),
        (
            Some(WgpuSceneColorSpace::LinearSrgb),
            wgpu::TextureFormat::Rgba16Float,
        ),
    ] {
        let mut renderer = match mode {
            None => WgpuSceneRenderer::new(&adapter, device.clone(), queue.clone(), format)?,
            Some(mode) => WgpuSceneRenderer::new_with_color_space(
                &adapter,
                device.clone(),
                queue.clone(),
                format,
                mode,
            )?,
        };
        let images = imported_images(&device, &mut encoder, renderer.sprite_atlas())?;
        let (_app, scene) = scene_fixture(renderer.sprite_atlas(), images)?;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("original host scene target"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        recordings.push(renderer.record(
            &scene,
            &mut encoder,
            &target,
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        )?);
        readbacks.push((
            mode.unwrap_or_default(),
            format,
            copy_to_readback(&device, &mut encoder, &target),
        ));
    }
    let submission = queue.submit([encoder.finish()]);
    let mappings = readbacks
        .iter()
        .map(|(_, _, buffer)| {
            let (sender, receiver) = mpsc::channel();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            receiver
        })
        .collect::<Vec<_>>();
    device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: Some(Duration::from_secs(30)),
    })?;
    for mapping in mappings {
        mapping.recv_timeout(Duration::from_secs(30))??;
    }
    {
        let errors = errors.lock().unwrap();
        assert!(errors.is_empty(), "{errors:?}");
    }
    let results = readbacks
        .iter()
        .map(|(color_space, format, buffer)| {
            Ok(Readback {
                color_space: *color_space,
                format: *format,
                pixels: read_pixels(buffer, *format)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(results[0].pixels, results[1].pixels);
    assert_eq!(results[2].pixels, results[3].pixels);
    assert!(results[2].logical_pixel(8, 6)[0] > results[5].logical_pixel(8, 6)[0] + 0.2);
    for result in &results {
        let mode = result.color_space;
        let background = working(mode, BACKGROUND);
        let foreground = working(mode, FOREGROUND);
        let gray = working(mode, [128.0 / 255.0; 3]);
        result.assert_pixel(90, 40, background);
        result.assert_pixel(8, 6, gray);
        result.assert_pixel(24, 6, over(foreground, 0.5, background));
        result.assert_pixel(35, 6, over(foreground, 0.5, gray));
        result.assert_pixel(8, 22, over(foreground, 0.5, background));
        result.assert_pixel(24, 22, gray);
        result.assert_pixel(8, 38, over(foreground, 128.0 / 255.0, background));
        if mode == WgpuSceneColorSpace::LinearSrgb && result.format.has_srgb_suffix() {
            for (left, value) in [(52, 6.0 / 255.0), (68, 8.0 / 255.0)] {
                let expected = [decode_srgb(value); 3];
                result.assert_pixel(left, 38, expected);
                result.assert_pixel(left + 6, 38, expected);
            }
        }
        let t = (64.75 - 50.0) / 28.0;
        let gradient = std::array::from_fn(|channel| {
            let first = encode_srgb([32.0 / 255.0, 64.0 / 255.0, 96.0 / 255.0][channel]);
            let last = encode_srgb([160.0 / 255.0, 192.0 / 255.0, 224.0 / 255.0][channel]);
            decode_srgb(first * (1.0 - t) + last * t)
        });
        result.assert_pixel(64, 6, working(mode, gradient));
        for (index, sampled_rgb, sampled_alpha, linear) in [
            (0, [0.03125, 0.125, 0.25], 0.5, true),
            (
                1,
                [64.0 / 255.0, 32.0 / 255.0, 16.0 / 255.0],
                128.0 / 255.0,
                false,
            ),
            (
                2,
                [0.03125, 0.125, 0.25]
                    .map(|value| decode_srgb((encode_srgb(value) * 255.0).round() / 255.0)),
                128.0 / 255.0,
                true,
            ),
        ] {
            let mut straight = sampled_rgb.map(|component| component / sampled_alpha);
            if linear && mode == WgpuSceneColorSpace::SrgbEncoded {
                straight = straight.map(encode_srgb);
            } else if !linear && mode == WgpuSceneColorSpace::LinearSrgb {
                straight = straight.map(decode_srgb);
            }
            result.assert_pixel(
                40 + index * 16,
                22,
                over(straight, sampled_alpha, background),
            );
        }
    }
    let legacy = &results[0];
    let mut glyph_pixels = 0;
    for top in 96..152 {
        for left in 4..164 {
            let offset = (top * WIDTH + left) as usize;
            let coverage = ((legacy.pixels[offset][0] - BACKGROUND[0])
                / (FOREGROUND[0] - BACKGROUND[0]))
                .clamp(0.0, 1.0);
            if coverage <= 0.05 {
                continue;
            }
            glyph_pixels += 1;
            for result in &results[4..] {
                let expected = over(
                    FOREGROUND.map(decode_srgb),
                    coverage,
                    BACKGROUND.map(decode_srgb),
                );
                for channel in 0..3 {
                    let expected = if result.format.has_srgb_suffix() {
                        encode_srgb(expected[channel])
                    } else {
                        expected[channel]
                    };
                    assert!(
                        (result.pixels[offset][channel] - expected).abs() <= 0.016,
                        "mono glyph color or coverage changed at ({left}, {top}), channel {channel}"
                    );
                }
            }
        }
    }
    assert!(
        glyph_pixels > 50,
        "real mono glyph pixels must reach the original host target"
    );
    drop(recordings);
    Ok(())
}
