use super::*;
use crate::native_test_window::NativeTestOpaqueWindow;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows::Win32::{
    Foundation::HMODULE,
    UI::WindowsAndMessaging::{IsWindow, IsWindowVisible},
};

struct WarpRenderer {
    renderer: DirectXRenderer,
    window: NativeTestOpaqueWindow,
    devices: DirectXDevices,
}

impl WarpRenderer {
    fn new(viewport: Size<DevicePixels>) -> Result<Self> {
        let devices = warp_devices()?;
        let window = NativeTestOpaqueWindow::create_hidden(Bounds::new(
            point(DevicePixels(0), DevicePixels(0)),
            viewport,
        ))?;
        let mut renderer = DirectXRenderer::new(window.hwnd(), &devices, true)?;
        renderer.resize(viewport)?;
        Ok(Self {
            renderer,
            window,
            devices,
        })
    }

    fn finish(self) -> Result<()> {
        wait_for_gpu_idle(
            self.renderer
                .devices
                .as_ref()
                .context("live WARP devices")?,
        )?;
        drop(self.renderer);
        drop(self.window);
        Ok(())
    }
}

struct WarpSceneSource {
    fixture: WarpRenderer,
    snapshot: Rc<RefCell<Option<Scene>>>,
}

impl PlatformHeadlessRenderer for WarpSceneSource {
    fn render_scene(&mut self, scene: &Scene, viewport: Size<DevicePixels>) -> Result<()> {
        self.fixture.renderer.resize(viewport)?;
        let outcome = self
            .fixture
            .renderer
            .draw(scene, WindowBackgroundAppearance::Opaque)?;
        anyhow::ensure!(
            outcome == PlatformWindowPresentOutcome::Submitted,
            "the WARP Scene source must submit its actual frame: {outcome:?}"
        );
        self.snapshot.replace(Some(scene.clone()));
        Ok(())
    }

    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        viewport: Size<DevicePixels>,
    ) -> Result<RgbaImage> {
        self.fixture.renderer.resize(viewport)?;
        let image = self
            .fixture
            .renderer
            .render_to_image(scene, WindowBackgroundAppearance::Opaque)?;
        self.snapshot.replace(Some(scene.clone()));
        Ok(image)
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.fixture.renderer.sprite_atlas()
    }
}

struct WarpSceneRoot(Arc<RenderImage>);

impl Render for WarpSceneRoot {
    fn render(
        &mut self,
        window: &mut Window,
        _: &mut open_gpui::Context<Self>,
    ) -> impl IntoElement {
        let scale_factor = window.scale_factor();
        div().relative().size_full().children([2., 18.].map(|left| {
            img(self.0.clone())
                .absolute()
                .left(px(left / scale_factor))
                .top(px(2. / scale_factor))
                .size(px(12. / scale_factor))
        }))
    }
}

pub(crate) fn with_warp_scene_template(
    test: impl FnOnce(&Scene, Size<DevicePixels>) -> Result<()>,
) -> Result<()> {
    let viewport = size(DevicePixels(32), DevicePixels(16));
    let snapshot = Rc::new(RefCell::new(None));
    let native_handle = Rc::new(Cell::new(None));
    let mut context =
        HeadlessAppContext::with_platform(Arc::new(NoopTextSystem::new()), Arc::new(()), {
            let snapshot = snapshot.clone();
            let native_handle = native_handle.clone();
            move || {
                let fixture = WarpRenderer::new(viewport)?;
                native_handle.set(Some(fixture.window.hwnd()));
                Ok(Some(Box::new(WarpSceneSource {
                    fixture,
                    snapshot: snapshot.clone(),
                })
                    as Box<dyn PlatformHeadlessRenderer>))
            }
        });
    let handle = context.open_window(size(px(16.), px(8.)), |_, cx| {
        cx.new(|_| {
            WarpSceneRoot(Arc::new(RenderImage::new(vec![image::Frame::new(
                RgbaImage::from_pixel(12, 12, image::Rgba([0, 255, 0, 255])),
            )])))
        })
    })?;
    context.update_window(handle.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
    })?;
    let captured = context.capture_screenshot(handle.into())?;
    assert_eq!(captured.dimensions(), (32, 16));
    let template = snapshot
        .borrow_mut()
        .take()
        .context("the real Window must provide a complete WARP Scene snapshot")?;
    assert_eq!(template.polychrome_sprites.len(), 2);
    assert!(!template.clip_shapes().is_empty());
    for (sprite, left) in template.polychrome_sprites.iter().zip([2., 18.]) {
        assert_eq!(
            sprite.bounds,
            Bounds::new(
                point(ScaledPixels(left), ScaledPixels(2.)),
                size(ScaledPixels(12.), ScaledPixels(12.)),
            )
        );
        assert_sprite_pixel(&captured, sprite, [0, 255, 0, 255]);
    }
    let native_handle = native_handle
        .get()
        .context("the WARP Scene source must own its hidden native target")?;
    assert!(!unsafe { IsWindowVisible(native_handle).as_bool() });
    let result = test(&template, viewport);
    let closed = context.update_window(handle.into(), |_, window, cx| {
        window.remove_window(cx);
    });
    context.run_until_parked();
    drop(context);
    assert!(!unsafe { IsWindow(Some(native_handle)).as_bool() });
    closed?;
    result
}

fn warp_devices() -> Result<DirectXDevices> {
    let mut device = None;
    let mut device_context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut device_context),
        )
    }
    .context("creating the WARP renderer device")?;
    let device = device.context("WARP must return a renderer device")?;
    let device_context = device_context.context("WARP must return an immediate context")?;
    let dxgi_device: IDXGIDevice = device.cast()?;
    let adapter: IDXGIAdapter1 = unsafe { dxgi_device.GetAdapter() }?.cast()?;
    let dxgi_factory = unsafe { adapter.GetParent::<IDXGIFactory6>() }?;
    Ok(DirectXDevices {
        adapter,
        dxgi_factory,
        device,
        device_context,
    })
}

fn image_key(image_id: usize) -> AtlasKey {
    AtlasKey::Image(RenderImageParams {
        image_id: ImageId(image_id),
        frame_index: 0,
    })
}

fn upload(atlas: &DirectXAtlas, key: AtlasKey, bgra: &[u8; 4]) -> Result<AtlasTile> {
    let bytes = bgra.repeat(12 * 12);
    atlas
        .get_or_insert_with(key, &mut || {
            Ok(Some((
                size(DevicePixels(12), DevicePixels(12)),
                Cow::Borrowed(bytes.as_slice()),
            )))
        })?
        .context("a complete WARP image must upload")
}

fn sprite_scene(template: &Scene, tiles: &[AtlasTile]) -> Scene {
    assert_eq!(template.polychrome_sprites.len(), 2);
    assert!(!tiles.is_empty() && tiles.len() <= template.polychrome_sprites.len());
    let mut scene = template.clone();
    scene.polychrome_sprites.truncate(tiles.len());
    for (sprite, tile) in scene.polychrome_sprites.iter_mut().zip(tiles) {
        assert!(sprite.renderer_transform().is_identity());
        assert!(sprite.clip.clip_count > 0);
        let clip_start = sprite.clip.first_clip as usize;
        let clip_end = clip_start + sprite.clip.clip_count as usize;
        assert!(clip_end <= template.clip_shapes().len());
        sprite.tile = *tile;
    }
    assert_eq!(scene.clip_shapes(), template.clip_shapes());
    scene
}

fn assert_pixel(image: &RgbaImage, x: u32, y: u32, expected: [u8; 4]) {
    let actual = image.get_pixel(x, y).0;
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
        "WARP pixel ({x}, {y}): {actual:?} != {expected:?}"
    );
}

fn assert_sprite_pixel(image: &RgbaImage, sprite: &PolychromeSprite, expected: [u8; 4]) {
    let center = sprite.bounds.center();
    assert_pixel(image, center.x.0 as u32, center.y.0 as u32, expected);
}

pub(crate) fn warp_raw_scene_receipt_retains_original_regions_and_views(
    template: &Scene,
    viewport: Size<DevicePixels>,
) -> Result<()> {
    let mut fixture = WarpRenderer::new(viewport)?;
    let atlas = fixture.renderer.atlas.clone();
    let key = image_key(1);
    let original = upload(&atlas, key.clone(), &[0, 255, 0, 255])?;
    let scene = sprite_scene(template, &[original, original]);
    let preflight = atlas.preflight(&scene)?;
    let view = preflight
        .texture_view(original.texture_instance())
        .and_then(|views| views[0].as_ref())
        .context("the receipt must freeze the actual SRV")?
        .clone();
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::TextureRetained
    );
    let replacement = upload(&atlas, key.clone(), &[0, 0, 255, 255])?;
    assert_eq!(replacement.texture_instance(), original.texture_instance());
    assert!(!replacement.bounds.intersects(&original.bounds));
    let current_view = atlas
        .get_texture_view(original.texture_instance())
        .and_then(|views| views[0].clone())
        .context("the receipt must keep the original resource resident")?;
    assert_eq!(view.as_raw(), current_view.as_raw());
    fixture.renderer.draw_scene_to_target(
        &scene,
        WindowBackgroundAppearance::Opaque,
        &preflight,
    )?;
    let original_pixels = fixture.renderer.read_render_target_to_image()?;
    assert_sprite_pixel(
        &original_pixels,
        &scene.polychrome_sprites[0],
        [0, 255, 0, 255],
    );
    assert_sprite_pixel(
        &original_pixels,
        &scene.polychrome_sprites[1],
        [0, 255, 0, 255],
    );
    drop(preflight);
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::TextureFreed
    );
    assert!(
        atlas
            .get_texture_view(original.texture_instance())
            .is_none()
    );
    assert_eq!(
        fixture
            .renderer
            .draw(&scene, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::RepaintRequired
    );
    assert!(
        fixture
            .renderer
            .render_to_image(&scene, WindowBackgroundAppearance::Opaque)
            .is_err()
    );
    assert_eq!(
        fixture.renderer.read_render_target_to_image()?,
        original_pixels
    );
    let reconstructed = upload(&atlas, key, &[255, 0, 0, 255])?;
    assert_eq!(reconstructed.texture_id, original.texture_id);
    assert_ne!(
        reconstructed.texture_instance(),
        original.texture_instance()
    );
    assert_eq!(
        fixture
            .renderer
            .draw(&scene, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::RepaintRequired
    );
    assert!(
        fixture
            .renderer
            .render_to_image(&scene, WindowBackgroundAppearance::Opaque)
            .is_err()
    );
    assert_eq!(
        fixture.renderer.read_render_target_to_image()?,
        original_pixels
    );
    let fresh = sprite_scene(template, &[reconstructed]);
    let fresh_pixels = fixture
        .renderer
        .render_to_image(&fresh, WindowBackgroundAppearance::Opaque)?;
    assert_sprite_pixel(
        &fresh_pixels,
        &fresh.polychrome_sprites[0],
        [0, 0, 255, 255],
    );
    assert_eq!(
        fixture
            .renderer
            .draw(&fresh, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::Submitted
    );
    fixture.finish()
}

pub(crate) fn warp_mixed_valid_and_stale_resources_refuse_before_target_clear(
    template: &Scene,
    viewport: Size<DevicePixels>,
) -> Result<()> {
    let mut fixture = WarpRenderer::new(viewport)?;
    let atlas = fixture.renderer.atlas.clone();
    let key = image_key(2);
    let old_tile = upload(&atlas, key.clone(), &[0, 255, 0, 255])?;
    atlas.handle_device_lost(&fixture.devices.device, &fixture.devices.device_context);
    let current = upload(&atlas, key, &[0, 0, 255, 255])?;
    assert_eq!(old_tile.texture_id, current.texture_id);
    assert_ne!(old_tile.texture_instance(), current.texture_instance());
    let before = fixture
        .renderer
        .render_to_image(&Scene::default(), WindowBackgroundAppearance::Opaque)?;
    let mixed = sprite_scene(template, &[current, old_tile]);
    assert_eq!(
        fixture
            .renderer
            .draw(&mixed, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::RepaintRequired
    );
    assert_eq!(fixture.renderer.read_render_target_to_image()?, before);
    assert!(
        fixture
            .renderer
            .render_to_image(&mixed, WindowBackgroundAppearance::Opaque)
            .is_err()
    );
    assert_eq!(fixture.renderer.read_render_target_to_image()?, before);
    let fresh = sprite_scene(template, &[current]);
    let fresh_pixels = fixture
        .renderer
        .render_to_image(&fresh, WindowBackgroundAppearance::Opaque)?;
    assert_sprite_pixel(
        &fresh_pixels,
        &fresh.polychrome_sprites[0],
        [255, 0, 0, 255],
    );
    assert_sprite_pixel(
        &fresh_pixels,
        &template.polychrome_sprites[1],
        [255, 255, 255, 255],
    );
    assert_eq!(
        fixture
            .renderer
            .draw(&fresh, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::Submitted
    );
    fixture.finish()
}

pub(crate) fn warp_renderer_recovery_invalidates_receipts_and_requires_fresh_paint(
    template: &Scene,
    viewport: Size<DevicePixels>,
) -> Result<()> {
    let mut fixture = WarpRenderer::new(viewport)?;
    let atlas = fixture.renderer.atlas.clone();
    let key = image_key(3);
    let old_tile = upload(&atlas, key.clone(), &[0, 255, 0, 255])?;
    let old_scene = sprite_scene(template, &[old_tile]);
    let old_preflight = atlas.preflight(&old_scene)?;
    let replacement_devices = warp_devices()?;
    assert_ne!(
        replacement_devices.device.as_raw(),
        fixture.devices.device.as_raw()
    );
    fixture.renderer.handle_device_lost(&replacement_devices)?;
    fixture.devices = replacement_devices;
    assert!(old_preflight.validate().is_err());
    assert!(
        fixture
            .renderer
            .render_to_image(&old_scene, WindowBackgroundAppearance::Opaque)
            .is_err()
    );
    let current = upload(&atlas, key.clone(), &[255, 0, 0, 255])?;
    let fresh = sprite_scene(template, &[current]);
    let fresh_preflight = atlas.preflight(&fresh)?;
    assert_eq!(current.texture_id, old_tile.texture_id);
    assert!(current.texture_generation > old_tile.texture_generation);
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::TextureRetained
    );
    assert_eq!(
        fixture
            .renderer
            .draw(&old_scene, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::RepaintRequired
    );
    let before_refused_receipt = fixture
        .renderer
        .render_to_image(&Scene::default(), WindowBackgroundAppearance::Opaque)?;
    assert!(
        fixture
            .renderer
            .draw_scene_to_target(
                &old_scene,
                WindowBackgroundAppearance::Opaque,
                &old_preflight,
            )
            .is_err()
    );
    assert_eq!(
        fixture.renderer.read_render_target_to_image()?,
        before_refused_receipt
    );
    drop(old_preflight);
    assert!(atlas.get_texture_view(current.texture_instance()).is_some());
    assert_eq!(
        fixture
            .renderer
            .draw(&old_scene, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::RepaintRequired
    );
    let image = fixture
        .renderer
        .render_to_image(&fresh, WindowBackgroundAppearance::Opaque)?;
    assert_sprite_pixel(&image, &fresh.polychrome_sprites[0], [0, 0, 255, 255]);
    assert_eq!(
        fixture
            .renderer
            .draw(&fresh, WindowBackgroundAppearance::Opaque)?,
        PlatformWindowPresentOutcome::Submitted
    );
    drop(fresh_preflight);
    assert!(atlas.get_texture_view(current.texture_instance()).is_none());
    fixture.finish()
}
