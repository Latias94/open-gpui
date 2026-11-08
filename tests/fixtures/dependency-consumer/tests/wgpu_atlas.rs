#![cfg(all(feature = "wgpu-renderer", not(target_family = "wasm")))]

use open_gpui::{
    AtlasAccessOutcome, AtlasKey, AtlasRemoveOutcome, DevicePixels, ImageId, PlatformAtlas,
    RenderImageParams, block_on, size,
};
use open_gpui_wgpu::WgpuAtlas;
use std::{borrow::Cow, sync::Arc, time::Duration};

#[test]
fn application_owned_device_uploads_and_releases_a_leased_backend_atlas() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY | wgpu::Backends::GL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("the ordinary renderer consumer requires a native graphics adapter");
    let (device, queue) = block_on(
        adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("ordinary application atlas device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults()
                .using_resolution(adapter.limits())
                .using_alignment(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            default_queue: wgpu::QueueDescriptor::default(),
        }),
    )
    .expect("the application must create its own device and queue");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let device_lifetime = Arc::downgrade(&device);
    let queue_lifetime = Arc::downgrade(&queue);
    let errors = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let atlas = Arc::new(WgpuAtlas::new(
        device.clone(),
        queue.clone(),
        wgpu::TextureFormat::Rgba8Unorm,
    ));
    let atlas_lifetime = Arc::downgrade(&atlas);
    let key = AtlasKey::Image(RenderImageParams {
        image_id: ImageId(811),
        frame_index: 0,
    });
    let dimensions = size(DevicePixels(2), DevicePixels(1));
    assert!(
        atlas
            .get_or_insert_with(key.clone(), &mut || {
                Ok(Some((dimensions, Cow::Borrowed([0; 4].as_slice()))))
            })
            .is_err(),
        "an incomplete upload must refuse before publishing the key"
    );

    let pixels = [0, 0, 255, 255, 255, 0, 0, 255];
    let access = atlas
        .get_or_insert_with_diagnostics(key.clone(), &mut || {
            Ok(Some((dimensions, Cow::Borrowed(pixels.as_slice()))))
        })
        .expect("a complete same-key retry must use the application's device");
    assert_eq!(access.diagnostic.outcome, AtlasAccessOutcome::Inserted);
    let tile = access.tile.expect("the upload must publish a tile");
    assert_eq!(tile.bounds.size, dimensions);
    atlas.before_frame();
    let submitted = queue.submit(std::iter::empty());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submitted),
            timeout: Some(Duration::from_secs(10)),
        })
        .expect("the application must complete the atlas upload on its queue");

    let platform: Arc<dyn PlatformAtlas> = atlas.clone();
    let lease = platform
        .clone()
        .retain_texture_instances(&[tile.texture_instance()])
        .expect("ordinary callers must retain the uploaded texture through the public owner");
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::TextureRetained
    );
    drop(platform);
    drop(atlas);
    assert!(
        atlas_lifetime.upgrade().is_some(),
        "the live texture lease must retain its backend owner"
    );
    lease
        .validate()
        .expect("removing the key must preserve the leased native texture");
    drop(lease);
    assert!(
        atlas_lifetime.upgrade().is_none(),
        "releasing the final lease must release the backend owner"
    );
    assert!(
        block_on(errors.pop()).is_none(),
        "the real upload and retirement must produce no validation error"
    );
    drop(queue);
    drop(device);
    assert!(queue_lifetime.upgrade().is_none());
    assert!(device_lifetime.upgrade().is_none());
}
