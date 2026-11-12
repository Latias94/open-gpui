//! Public consumer of GPUI scenes recorded into a host-owned GPU submission.
//! The headless platform supplies simulated window admission for scene production.
//! Only the host's queue submission and readback below establish actual GPU execution.

#![cfg(all(not(target_family = "wasm"), feature = "test-support"))]

use anyhow::{Context as _, Result};
use open_gpui::{
    AppContext as _, AtlasKey, AtlasRemoveOutcome, Bounds, Context, DevicePixels,
    HeadlessAppContext, ImageTexture, ImageTextureAlpha, ImageTextureSampling,
    ImageTextureTransfer, IntoElement, ParentElement as _, PathBuilder, PlatformAtlas,
    PlatformHeadlessRenderer, Render, RenderImage, RenderImageParams, Scene, Size, Styled as _,
    SubtreeClip, SubtreeClipExt as _, SubtreeTransform, SubtreeTransformExt as _, Window,
    WindowHandle, block_on, canvas, div, fill, point, px, rgb, size,
};
use open_gpui_wgpu::{
    CosmicTextSystem, WgpuAtlas, WgpuImportedImageDescriptor, WgpuSceneRecording,
    WgpuSceneRenderer, wgpu,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

struct SceneSource {
    atlas: Arc<WgpuAtlas>,
    captured: Rc<RefCell<Option<Scene>>>,
}

impl PlatformHeadlessRenderer for SceneSource {
    fn render_scene(&mut self, _: &Scene, _: Size<DevicePixels>) -> Result<()> {
        Ok(())
    }

    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        _: Size<DevicePixels>,
    ) -> Result<image::RgbaImage> {
        *self.captured.borrow_mut() = Some(scene.clone());
        anyhow::bail!("this scene source exports scenes without producing a screenshot")
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }
}

struct PaintedRoot {
    alternate: bool,
    image: Arc<RenderImage>,
}

impl Render for PaintedRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let alternate = self.alternate;
        let image = self.image.clone();
        div()
            .relative()
            .size_full()
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
                        window.paint_quad(fill(
                            rect(2.0, 2.0, 18.0, 12.0),
                            rgb(if alternate { 0x0000ff } else { 0xff0000 }),
                        ));
                        window.paint_quad(fill(
                            rect(8.0, 6.0, 14.0, 10.0),
                            rgb(if alternate { 0xff0000 } else { 0x00ff00 }),
                        ));
                        let mut path = PathBuilder::fill();
                        path.add_polygon(
                            &[
                                bounds.origin + point(px(2.0), px(17.0)),
                                bounds.origin + point(px(12.0), px(17.0)),
                                bounds.origin + point(px(2.0), px(23.0)),
                            ],
                            true,
                        );
                        window.paint_path(
                            path.build().expect("the triangle must tessellate"),
                            rgb(if alternate { 0xff00ff } else { 0xffff00 }),
                        );
                        let image_bounds = rect(25.0, 15.0, 8.0, 6.0);
                        window
                            .paint_image(
                                image_bounds,
                                image_bounds,
                                Default::default(),
                                image,
                                0,
                                false,
                            )
                            .expect("the renderer's exact atlas must accept the decoded image");
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
                    .left(px(25.0))
                    .top(px(2.0))
                    .w(px(6.0))
                    .h(px(8.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .absolute()
                            .left(px(-3.0))
                            .top_0()
                            .w(px(12.0))
                            .h(px(8.0))
                            .bg(rgb(if alternate { 0x00ffff } else { 0xffffff })),
                    ),
            )
    }
}

fn capture_scene(
    cx: &mut HeadlessAppContext,
    captured: &Rc<RefCell<Option<Scene>>>,
    alternate: bool,
) -> Result<(WindowHandle<PaintedRoot>, Scene)> {
    let bgra = if alternate {
        [255, 0, 0, 255]
    } else {
        [0, 0, 255, 255]
    };
    let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        image::RgbaImage::from_pixel(3, 2, image::Rgba(bgra)),
    )]));
    let viewport = if alternate {
        size(px(41.0), px(27.0))
    } else {
        size(px(37.0), px(23.0))
    };
    let handle = cx.open_window(viewport, move |_, app| {
        app.new(|_| PaintedRoot { alternate, image })
    })?;
    cx.update_window(handle.into(), |_, window, app| {
        assert_eq!(window.scale_factor(), 2.0);
        window.refresh();
        window.draw(app).clear(app);
        let facts = window.presentation_facts();
        assert!(facts.frame_accepted_generation.is_some());
    })?;
    assert!(cx.capture_screenshot(handle.into()).is_err());
    let scene = captured
        .borrow_mut()
        .take()
        .context("the painted scene must be exported")?;
    assert!(scene.quads.len() >= 3);
    assert_eq!(scene.paths.len(), 1);
    assert_eq!(scene.polychrome_sprites.len(), 1);
    Ok((handle, scene))
}

fn probe_host_callback(device: &wgpu::Device, errors: &AtomicUsize, expected: usize) -> Result<()> {
    let _invalid = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("host callback ownership probe"),
        size: 4,
        usage: wgpu::BufferUsages::empty(),
        mapped_at_creation: false,
    });
    device.poll(wgpu::PollType::Poll)?;
    assert_eq!(
        errors.load(Ordering::SeqCst),
        expected,
        "the host callback must remain installed"
    );
    Ok(())
}

fn copy_to_readback(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::Texture,
) -> wgpu::Buffer {
    let row_bytes = (target.width() * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("host scene readback"),
        size: u64::from(row_bytes) * u64::from(target.height()),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(target.height()),
            },
        },
        target.size(),
    );
    buffer
}

fn assert_pixels(buffer: &wgpu::Buffer, width: u32, cases: &[(u32, u32, [u8; 4])]) -> Result<()> {
    let row_bytes = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) as usize;
    let pixels = buffer.slice(..).get_mapped_range()?;
    for &(left, top, expected) in cases {
        let offset = (top as usize * 2 + 1) * row_bytes + (left as usize * 2 + 1) * 4;
        assert_eq!(
            &pixels[offset..offset + 4],
            &expected,
            "logical pixel ({left}, {top})"
        );
    }
    Ok(())
}

#[test]
fn host_records_two_gpui_scenes_and_retains_resources_through_one_submission() -> Result<()> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY | wgpu::Backends::GL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))?;
    for (bind_groups, uniforms, attachment_bytes, combined_buffers) in
        [(2, 3, 32, 6), (3, 2, 32, 6), (3, 3, 4, 6), (3, 3, 32, 5)]
    {
        let limits = wgpu::Limits {
            max_bind_groups: bind_groups,
            max_uniform_buffers_per_shader_stage: uniforms,
            max_color_attachment_bytes_per_sample: attachment_bytes,
            max_buffers_and_acceleration_structures_per_shader_stage: combined_buffers,
            ..wgpu::Limits::downlevel_defaults()
                .using_resolution(adapter.limits())
                .using_alignment(adapter.limits())
        };
        let (limited_device, limited_queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("host device with insufficient GPUI bindings"),
                required_limits: limits,
                ..Default::default()
            }))?;
        let limited_device = Arc::new(limited_device);
        let errors = Arc::new(AtomicUsize::new(0));
        limited_device.on_uncaptured_error(Arc::new({
            let errors = errors.clone();
            move |_| {
                errors.fetch_add(1, Ordering::SeqCst);
            }
        }));
        assert!(
            WgpuSceneRenderer::new(
                &adapter,
                limited_device.clone(),
                Arc::new(limited_queue),
                wgpu::TextureFormat::Rgba8Unorm,
            )
            .is_err()
        );
        limited_device.poll(wgpu::PollType::Poll)?;
        assert_eq!(
            errors.load(Ordering::SeqCst),
            0,
            "unsupported limits must refuse before GPU validation"
        );
    }
    let (device, queue) = block_on(
        adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("external scene host device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults()
                .using_resolution(adapter.limits())
                .using_alignment(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            default_queue: wgpu::QueueDescriptor::default(),
        }),
    )?;
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let errors = Arc::new(AtomicUsize::new(0));
    device.on_uncaptured_error(Arc::new({
        let errors = errors.clone();
        move |error| {
            eprintln!("host device callback: {error}");
            errors.fetch_add(1, Ordering::SeqCst);
        }
    }));
    let mut renderer = WgpuSceneRenderer::new(
        &adapter,
        device.clone(),
        queue.clone(),
        wgpu::TextureFormat::Rgba8Unorm,
    )?;
    probe_host_callback(&device, &errors, 1)?;
    let atlas = renderer.sprite_atlas().clone();
    let atlas_lifetime = Arc::downgrade(&atlas);
    let captured = Rc::new(RefCell::new(None));
    let mut cx = HeadlessAppContext::with_platform(
        Arc::new(CosmicTextSystem::new_without_system_fonts("sans-serif")),
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
    let (first_window, first_scene) = capture_scene(&mut cx, &captured, false)?;
    let (second_window, second_scene) = capture_scene(&mut cx, &captured, true)?;
    let descriptor = wgpu::TextureDescriptor {
        label: Some("first host scene target"),
        size: wgpu::Extent3d {
            width: 74,
            height: 46,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    };
    let first_target = device.create_texture(&descriptor);
    let second_target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("second host scene target"),
        size: wgpu::Extent3d {
            width: 82,
            height: 54,
            depth_or_array_layers: 1,
        },
        mip_level_count: 2,
        ..descriptor.clone()
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("one host encoder for both GPUI scenes"),
    });
    for invalid in [
        wgpu::TextureDescriptor {
            format: wgpu::TextureFormat::Bgra8Unorm,
            ..descriptor.clone()
        },
        wgpu::TextureDescriptor {
            usage: wgpu::TextureUsages::COPY_SRC,
            ..descriptor.clone()
        },
        wgpu::TextureDescriptor {
            dimension: wgpu::TextureDimension::D3,
            ..descriptor.clone()
        },
        wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                depth_or_array_layers: 2,
                ..descriptor.size
            },
            ..descriptor.clone()
        },
        wgpu::TextureDescriptor {
            sample_count: 4,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            ..descriptor.clone()
        },
    ] {
        let invalid_target = device.create_texture(&invalid);
        let rejected = renderer.record(
            &first_scene,
            &mut encoder,
            &invalid_target,
            wgpu::LoadOp::Load,
        );
        assert!(
            rejected.is_err(),
            "target descriptor must be refused: {invalid:?}"
        );
    }
    let first_recording: WgpuSceneRecording = renderer.record(
        &first_scene,
        &mut encoder,
        &first_target,
        wgpu::LoadOp::Clear(wgpu::Color::BLUE),
    )?;
    {
        let view = second_target.create_view(&wgpu::TextureViewDescriptor {
            mip_level_count: Some(1),
            ..Default::default()
        });
        let _background = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("host-owned green background before GPUI Load"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            ..Default::default()
        });
    }
    let second_recording: WgpuSceneRecording = renderer.record(
        &second_scene,
        &mut encoder,
        &second_target,
        wgpu::LoadOp::Load,
    )?;
    let readbacks = [
        copy_to_readback(&device, &mut encoder, &first_target),
        copy_to_readback(&device, &mut encoder, &second_target),
    ];
    probe_host_callback(&device, &errors, 2)?;
    let mut image_keys = Vec::new();
    for handle in [first_window, second_window] {
        image_keys.push(handle.update(&mut cx, |root, _, _| {
            AtlasKey::Image(RenderImageParams {
                image_id: root.image.id,
                frame_index: 0,
            })
        })?);
        cx.update_window(handle.into(), |_, window, app| window.remove_window(app))?;
    }
    drop(cx);
    for key in image_keys {
        assert_eq!(
            atlas.remove_with_diagnostics(&key).outcome,
            AtlasRemoveOutcome::TextureRetained
        );
    }
    drop(first_scene);
    drop(second_scene);
    drop(atlas);
    drop(renderer);
    assert!(
        atlas_lifetime.upgrade().is_some(),
        "recordings must retain the exact atlas owner"
    );
    let submission = queue.submit([encoder.finish()]);
    let mappings = readbacks.each_ref().map(|buffer| {
        let (sender, receiver) = mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        receiver
    });
    device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: Some(Duration::from_secs(30)),
    })?;
    for mapping in mappings {
        mapping.recv_timeout(Duration::from_secs(30))??;
    }
    assert_eq!(
        errors.load(Ordering::SeqCst),
        2,
        "only the two host callback probes may fail"
    );
    assert_pixels(
        &readbacks[0],
        74,
        &[
            (0, 0, [0, 0, 255, 255]),
            (4, 4, [255, 0, 0, 255]),
            (10, 8, [0, 255, 0, 255]),
            (21, 14, [0, 255, 0, 255]),
            (28, 5, [255, 255, 255, 255]),
            (23, 5, [0, 0, 255, 255]),
            (33, 5, [0, 0, 255, 255]),
            (4, 19, [255, 255, 0, 255]),
            (28, 18, [255, 0, 0, 255]),
            (35, 21, [0, 0, 255, 255]),
        ],
    )?;
    assert_pixels(
        &readbacks[1],
        82,
        &[
            (0, 0, [0, 255, 0, 255]),
            (4, 4, [0, 0, 255, 255]),
            (10, 8, [255, 0, 0, 255]),
            (21, 14, [255, 0, 0, 255]),
            (28, 5, [0, 255, 255, 255]),
            (23, 5, [0, 255, 0, 255]),
            (33, 5, [0, 255, 0, 255]),
            (4, 19, [255, 0, 255, 255]),
            (28, 18, [0, 0, 255, 255]),
            (39, 25, [0, 255, 0, 255]),
        ],
    )?;
    for buffer in readbacks {
        buffer.unmap();
    }
    drop(first_recording);
    assert!(atlas_lifetime.upgrade().is_some());
    drop(second_recording);
    assert!(
        atlas_lifetime.upgrade().is_none(),
        "completed recordings must release the atlas"
    );
    assert_eq!(errors.load(Ordering::SeqCst), 2);
    Ok(())
}

struct ImportedRoot {
    images: Vec<ImageTexture>,
}

impl Render for ImportedRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let images = self.images.clone();
        let clipped = images[4].clone();
        div()
            .size_full()
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        for (index, image) in images.iter().enumerate() {
                            window
                                .paint_image_texture(
                                    Bounds::new(
                                        bounds.origin
                                            + point(px(2.0 + index as f32 * 12.0), px(2.0)),
                                        size(px(10.0), px(10.0)),
                                    ),
                                    Default::default(),
                                    image,
                                )
                                .expect("a current imported image must paint");
                        }
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
                    .top(px(2.0))
                    .size(px(3.0))
                    .bg(rgb(0xffff00)),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        window
                            .paint_image_texture(bounds, Default::default(), &clipped)
                            .expect("the transformed image must paint");
                    },
                )
                .absolute()
                .left(px(2.0))
                .top(px(18.0))
                .w(px(16.0))
                .h(px(10.0))
                .with_subtree_clip(
                    SubtreeClip::try_rounded_rect(
                        Bounds::new(point(px(3.0), px(1.0)), size(px(6.0), px(8.0))),
                        Default::default(),
                    )
                    .expect("the imported image clip must be valid"),
                )
                .with_subtree_transform(
                    SubtreeTransform::try_translation(point(px(2.0), px(0.0)))
                        .expect("the imported image translation must be valid"),
                ),
            )
    }
}

fn imported_scene(
    atlas: &Arc<WgpuAtlas>,
    images: Vec<ImageTexture>,
) -> Result<(HeadlessAppContext, Scene)> {
    let captured = Rc::new(RefCell::new(None));
    let mut app = HeadlessAppContext::with_platform(
        Arc::new(CosmicTextSystem::new_without_system_fonts("sans-serif")),
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
    let handle = app.open_window(size(px(100.0), px(32.0)), move |_, app| {
        app.new(|_| ImportedRoot { images })
    })?;
    assert!(app.capture_screenshot(handle.into()).is_err());
    let scene = captured
        .borrow_mut()
        .take()
        .context("the imported image composition must produce a scene")?;
    assert_eq!(scene.polychrome_sprites.len(), 9);
    Ok((app, scene))
}

struct HostImageOwner(Arc<AtomicUsize>);

impl Drop for HostImageOwner {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn produce_imported_images(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    atlas: &Arc<WgpuAtlas>,
    owner: &Arc<HostImageOwner>,
    alternate: bool,
) -> Result<Vec<ImageTexture>> {
    let cases = [
        (
            wgpu::TextureFormat::Rgba8Unorm,
            ImageTextureTransfer::Srgb,
            ImageTextureAlpha::Straight,
            wgpu::Color {
                r: 0.5,
                g: 0.25,
                b: 0.125,
                a: 0.5,
            },
        ),
        (
            wgpu::TextureFormat::Bgra8Unorm,
            ImageTextureTransfer::Linear,
            ImageTextureAlpha::Straight,
            wgpu::Color {
                r: 0.25,
                g: 0.5,
                b: 1.0,
                a: 0.5,
            },
        ),
        (
            wgpu::TextureFormat::Rgba8UnormSrgb,
            ImageTextureTransfer::Srgb,
            ImageTextureAlpha::Premultiplied,
            wgpu::Color {
                r: 0.125,
                g: 0.25,
                b: 0.5,
                a: 0.5,
            },
        ),
        (
            wgpu::TextureFormat::Rgba16Float,
            ImageTextureTransfer::Linear,
            ImageTextureAlpha::Premultiplied,
            wgpu::Color {
                r: 0.125,
                g: 0.25,
                b: 0.5,
                a: 0.5,
            },
        ),
        (
            wgpu::TextureFormat::Bgra8UnormSrgb,
            ImageTextureTransfer::Srgb,
            ImageTextureAlpha::Opaque,
            wgpu::Color {
                r: 0.25,
                g: 0.5,
                b: 0.75,
                a: 0.0,
            },
        ),
        (
            wgpu::TextureFormat::Rgba8Unorm,
            ImageTextureTransfer::Srgb,
            ImageTextureAlpha::Premultiplied,
            wgpu::Color {
                r: 0.25,
                g: 0.125,
                b: 0.0625,
                a: 0.5,
            },
        ),
        (
            wgpu::TextureFormat::Rgba8Unorm,
            ImageTextureTransfer::Srgb,
            ImageTextureAlpha::Opaque,
            wgpu::Color::BLACK,
        ),
        (
            wgpu::TextureFormat::Rgba8Unorm,
            ImageTextureTransfer::Srgb,
            ImageTextureAlpha::Opaque,
            wgpu::Color::BLACK,
        ),
    ];
    let mut images = Vec::new();
    for (index, (format, transfer, alpha, color)) in cases.into_iter().enumerate() {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("application-produced GPUI sampled image"),
            size: wgpu::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 2,
            },
            mip_level_count: 2,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        for (mip_level, array_layer, clear) in [
            (0, 0, wgpu::Color::RED),
            (1, 0, wgpu::Color::GREEN),
            (1, 1, if alternate { wgpu::Color::BLUE } else { color }),
        ] {
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_mip_level: mip_level,
                mip_level_count: Some(1),
                base_array_layer: array_layer,
                array_layer_count: Some(1),
                ..Default::default()
            });
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("host producer before GPUI sampling"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });
        }
        if index >= 6 && !alternate {
            paint_host_texel_pattern(device, encoder, &texture);
        }
        let descriptor = WgpuImportedImageDescriptor {
            texture,
            mip_level: 1,
            array_layer: 1,
            region: Bounds::new(
                point(DevicePixels(1), DevicePixels(1)),
                size(DevicePixels(2), DevicePixels(2)),
            ),
            transfer,
            alpha,
            sampling: if index % 2 == 0 {
                ImageTextureSampling::Linear
            } else {
                ImageTextureSampling::Nearest
            },
            owner: Some(owner.clone()),
        };
        images.push(atlas.import_image(descriptor)?);
    }
    Ok(images)
}

fn paint_host_texel_pattern(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("host texel pattern"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
            r#"
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let vertices = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(vertices[index], 0.0, 1.0);
}
@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = vec2<u32>(position.xy);
    if (texel.x == 0u || texel.x == 3u || texel.y == 0u || texel.y == 3u) {
        return vec4(1.0, 0.0, 1.0, 1.0);
    }
    if (texel.y == 1u) {
        return select(vec4(1.0, 0.0, 0.0, 1.0), vec4(0.0, 1.0, 0.0, 1.0), texel.x == 2u);
    }
    return select(vec4(0.0, 0.0, 1.0, 1.0), vec4(1.0), texel.x == 2u);
}
"#,
        )),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("host texel pattern pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: texture.format(),
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_mip_level: 1,
        mip_level_count: Some(1),
        base_array_layer: 1,
        array_layer_count: Some(1),
        ..Default::default()
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("host-produced region and filtering control"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        ..Default::default()
    });
    pass.set_pipeline(&pipeline);
    pass.draw(0..3, 0..1);
}

fn assert_imported_pixels(buffer: &wgpu::Buffer, alternate: bool) -> Result<()> {
    let row_bytes = (200_u32 * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) as usize;
    let pixels = buffer.slice(..).get_mapped_range()?;
    let expected = [
        [64, 32, 16, 255],
        [69, 94, 128, 255],
        [69, 94, 128, 255],
        [68, 94, 128, 255],
        [137, 188, 225, 255],
        [64, 32, 16, 255],
    ];
    for (index, expected) in expected.into_iter().enumerate() {
        let offset = 17 * row_bytes + (2 + index * 12 + 6) * 2 * 4;
        let expected = if alternate {
            [0, 0, 255, 255]
        } else {
            expected
        };
        for (channel, expected) in pixels[offset..offset + 4].iter().zip(expected) {
            assert!(
                channel.abs_diff(expected) <= 2,
                "import {index}: {:?} differs from {expected:?}",
                &pixels[offset..offset + 4],
            );
        }
    }
    let overlay = 7 * row_bytes + 7 * 4;
    assert_eq!(&pixels[overlay..overlay + 4], &[255, 255, 0, 255]);
    let background = 29 * row_bytes + 195 * 4;
    assert_eq!(&pixels[background..background + 4], &[0, 0, 0, 255]);
    for (left, expected) in [
        (5, [0, 0, 0, 255]),
        (
            8,
            if alternate {
                [0, 0, 255, 255]
            } else {
                [137, 188, 225, 255]
            },
        ),
        (15, [0, 0, 0, 255]),
    ] {
        let offset = 45 * row_bytes + (left * 2 + 1) * 4;
        for (channel, expected) in pixels[offset..offset + 4].iter().zip(expected) {
            assert!(
                channel.abs_diff(expected) <= 2,
                "clipped import at {left}: {:?}",
                &pixels[offset..offset + 4]
            );
        }
    }
    for (left, top, expected) in [
        (74, 2, [255, 0, 0, 255]),
        (83, 2, [0, 255, 0, 255]),
        (74, 11, [0, 0, 255, 255]),
        (83, 11, [255, 255, 255, 255]),
        (78, 5, [134, 115, 64, 255]),
        (90, 5, [255, 0, 0, 255]),
    ] {
        let offset = (top * 2 + 1) * row_bytes + (left * 2 + 1) * 4;
        let expected = if alternate {
            [0, 0, 255, 255]
        } else {
            expected
        };
        for (channel, expected) in pixels[offset..offset + 4].iter().zip(expected) {
            assert!(
                channel.abs_diff(expected) <= 2,
                "filter control ({left}, {top}): {:?}",
                &pixels[offset..offset + 4]
            );
        }
    }
    Ok(())
}

#[test]
fn host_produced_images_preserve_subresources_transfer_alpha_and_original_lifetimes() -> Result<()>
{
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: false,
        apply_limit_buckets: false,
        ..Default::default()
    }))?;
    let (device, queue) = block_on(
        adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits::downlevel_defaults()
                .using_resolution(adapter.limits())
                .using_alignment(adapter.limits()),
            ..Default::default()
        }),
    )?;
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let errors = Arc::new(AtomicUsize::new(0));
    device.on_uncaptured_error(Arc::new({
        let errors = errors.clone();
        move |error| {
            eprintln!("external image validation: {error}");
            errors.fetch_add(1, Ordering::SeqCst);
        }
    }));
    let mut renderer = WgpuSceneRenderer::new(
        &adapter,
        device.clone(),
        queue.clone(),
        wgpu::TextureFormat::Rgba8Unorm,
    )?;
    let atlas = renderer.sprite_atlas().clone();
    let mut encoder = device.create_command_encoder(&Default::default());
    let mut recordings = Vec::new();
    let mut readbacks = Vec::new();
    let mut counters = Vec::new();
    for alternate in [false, true] {
        let counter = Arc::new(AtomicUsize::new(0));
        let owner = Arc::new(HostImageOwner(counter.clone()));
        let images = produce_imported_images(&device, &mut encoder, &atlas, &owner, alternate)?;
        let stale_image = images[0].clone();
        let (app, scene) = imported_scene(&atlas, images)?;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("host imported image composition"),
            size: wgpu::Extent3d {
                width: 200,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        recordings.push(renderer.record(
            &scene,
            &mut encoder,
            &target,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        )?);
        readbacks.push(copy_to_readback(&device, &mut encoder, &target));
        drop(app);
        drop(scene);
        drop(owner);
        atlas.clear();
        assert!(stale_image.validate().is_err());
        drop(stale_image);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        counters.push(counter);
    }
    drop(renderer);
    drop(atlas);
    let submission = queue.submit([encoder.finish()]);
    let mappings = readbacks
        .iter()
        .map(|buffer| {
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
    for (readback, alternate) in readbacks.iter().zip([false, true]) {
        assert_imported_pixels(readback, alternate)?;
    }
    for readback in readbacks {
        readback.unmap();
    }
    drop(recordings.remove(0));
    assert_eq!(counters[0].load(Ordering::SeqCst), 1);
    assert_eq!(counters[1].load(Ordering::SeqCst), 0);
    drop(recordings);
    assert_eq!(counters[1].load(Ordering::SeqCst), 1);
    assert_eq!(errors.load(Ordering::SeqCst), 0);
    Ok(())
}
