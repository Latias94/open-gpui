use super::*;
use anyhow::Context as _;
use open_gpui::{FontId, GlyphId, RenderGlyphParams, px};
use std::{borrow::Cow, sync::Arc};
use windows::Win32::{
    Foundation::HMODULE,
    Graphics::{
        Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0},
        Direct3D11::{
            D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
            D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_USAGE_STAGING, D3D11CreateDevice,
        },
    },
};

const TEXTURE_KINDS: [AtlasTextureKind; 3] = [
    AtlasTextureKind::Monochrome,
    AtlasTextureKind::Polychrome,
    AtlasTextureKind::Subpixel,
];

#[derive(Clone, Copy, Debug)]
enum InsertionApi {
    Plain,
    Diagnostics,
}

const INSERTION_APIS: [InsertionApi; 2] = [InsertionApi::Plain, InsertionApi::Diagnostics];

impl InsertionApi {
    fn access(
        self,
        atlas: &DirectXAtlas,
        key: &AtlasKey,
        size: Size<DevicePixels>,
        bytes: &[u8],
        builder_calls: &mut usize,
        expected_outcome: AtlasAccessOutcome,
    ) -> anyhow::Result<Option<AtlasTile>> {
        let mut build = || {
            *builder_calls += 1;
            Ok(Some((size, Cow::Borrowed(bytes))))
        };
        match self {
            Self::Plain => atlas.get_or_insert_with(key, &mut build),
            Self::Diagnostics => {
                let access = atlas.get_or_insert_with_diagnostics(key, &mut build)?;
                assert_eq!(
                    access.diagnostic,
                    AtlasAccessDiagnostic::new(
                        key,
                        expected_outcome,
                        access.tile,
                        access.tile.map(|tile| tile.bounds.size),
                    )
                );
                Ok(access.tile)
            }
        }
    }
}

fn warp_device() -> (ID3D11Device, ID3D11DeviceContext) {
    let mut device = None;
    let mut context = None;
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
            Some(&mut context),
        )
        .expect("a real Direct3D 11 WARP device is required for atlas upload tests");
    }
    (
        device.expect("WARP device creation must return a device"),
        context.expect("WARP device creation must return an immediate context"),
    )
}

fn pixels(width: i32, height: i32) -> Size<DevicePixels> {
    Size {
        width: DevicePixels(width),
        height: DevicePixels(height),
    }
}

fn glyph_key(kind: AtlasTextureKind, glyph_id: u32) -> AtlasKey {
    AtlasKey::Glyph(RenderGlyphParams {
        font_id: FontId(0),
        glyph_id: GlyphId(glyph_id),
        font_size: px(14.0),
        subpixel_variant: Point { x: 0, y: 0 },
        scale_factor: 1.0,
        is_emoji: kind == AtlasTextureKind::Polychrome,
        subpixel_rendering: kind == AtlasTextureKind::Subpixel,
        dilation: 0,
    })
}

fn bytes_per_pixel(kind: AtlasTextureKind) -> usize {
    match kind {
        AtlasTextureKind::Monochrome => 1,
        AtlasTextureKind::Polychrome | AtlasTextureKind::Subpixel => 4,
    }
}

fn pattern(size: Size<DevicePixels>, kind: AtlasTextureKind, seed: u8) -> Vec<u8> {
    let width = usize::try_from(size.width.0).expect("test image width must be positive");
    let height = usize::try_from(size.height.0).expect("test image height must be positive");
    let channels = bytes_per_pixel(kind);
    let mut bytes = Vec::with_capacity(width * height * channels);
    for row in 0..height {
        for column in 0..width {
            for channel in 0..channels {
                bytes.push(seed.wrapping_add((row * 53 + column * 17 + channel * 71) as u8));
            }
        }
    }
    bytes
}

fn assert_empty_atlas(atlas: &DirectXAtlas) {
    let state = atlas.0.lock();
    assert_eq!(state.epoch, AtlasTextureLeaseEpoch::INITIAL);
    assert_eq!(state.next_texture_generation, 1);
    assert!(state.tiles_by_key.is_empty());
    for kind in TEXTURE_KINDS {
        let textures = state.texture_list(kind);
        assert!(textures.textures.is_empty(), "{kind:?}");
        assert!(textures.free_list.is_empty(), "{kind:?}");
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ResidentSnapshot {
    epoch: AtlasTextureLeaseEpoch,
    next_generation: u32,
    key_count: usize,
    texture: AtlasTextureInstanceId,
    texture_slots: usize,
    free_list: Vec<usize>,
    live_keys: u32,
    live_leases: u32,
    allocated_space: i32,
}

fn resident_snapshot(atlas: &DirectXAtlas, tile: AtlasTile) -> ResidentSnapshot {
    let state = atlas.0.lock();
    let texture = state
        .texture_if_resident(tile.texture_instance())
        .expect("the exact leased texture instance must remain resident");
    let textures = state.texture_list(tile.texture_id.kind);
    ResidentSnapshot {
        epoch: state.epoch,
        next_generation: state.next_texture_generation,
        key_count: state.tiles_by_key.len(),
        texture: texture.id,
        texture_slots: textures.textures.len(),
        free_list: textures.free_list.clone(),
        live_keys: texture.live_atlas_keys,
        live_leases: texture.live_visual_leases,
        allocated_space: texture.allocator.allocated_space(),
    }
}

struct StagingMap<'a> {
    context: &'a ID3D11DeviceContext,
    texture: &'a ID3D11Texture2D,
}

impl Drop for StagingMap<'_> {
    fn drop(&mut self) {
        unsafe {
            self.context.Unmap(self.texture, 0);
        }
    }
}

fn read_tile(atlas: &DirectXAtlas, tile: AtlasTile) -> anyhow::Result<Vec<u8>> {
    let (device, context, texture) = {
        let state = atlas.0.lock();
        let texture = state
            .texture_if_resident(tile.texture_instance())
            .context("cannot read a retired atlas texture instance")?;
        (
            state.device.clone(),
            state.device_context.clone(),
            texture.texture.clone(),
        )
    };
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe {
        texture.GetDesc(&mut desc);
    }
    let expected_format = match tile.texture_id.kind {
        AtlasTextureKind::Monochrome => DXGI_FORMAT_R8_UNORM,
        AtlasTextureKind::Polychrome => DXGI_FORMAT_B8G8R8A8_UNORM,
        AtlasTextureKind::Subpixel => DXGI_FORMAT_R8G8B8A8_UNORM,
    };
    assert_eq!(desc.Format, expected_format);
    let channels = bytes_per_pixel(tile.texture_id.kind);
    let left = usize::try_from(tile.bounds.origin.x.0)?;
    let top = usize::try_from(tile.bounds.origin.y.0)?;
    let width = usize::try_from(tile.bounds.size.width.0)?;
    let height = usize::try_from(tile.bounds.size.height.0)?;
    assert!(width > 0 && height > 0);
    assert!(left + width <= desc.Width as usize);
    assert!(top + height <= desc.Height as usize);
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    desc.MiscFlags = 0;
    let mut staging = None;
    unsafe {
        device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .context("creating the atlas readback staging texture")?;
    }
    let staging = staging.context("atlas readback staging texture was not returned")?;
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe {
        context.CopyResource(&staging, &texture);
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .context("mapping the atlas readback staging texture")?;
    }
    let _mapping = StagingMap {
        context: &context,
        texture: &staging,
    };
    assert!(!mapped.pData.is_null());
    let row_pitch = mapped.RowPitch as usize;
    let row_bytes = width * channels;
    assert!(row_pitch >= desc.Width as usize * channels);
    let mut bytes = Vec::with_capacity(row_bytes * height);
    for row in 0..height {
        let offset = (top + row) * row_pitch + left * channels;
        let source = unsafe { mapped.pData.cast::<u8>().add(offset) };
        let source_row = unsafe { std::slice::from_raw_parts(source, row_bytes) };
        bytes.extend_from_slice(source_row);
    }
    Ok(bytes)
}

#[test]
fn warp_short_sources_fail_before_allocation_and_allow_same_key_retry() -> anyhow::Result<()> {
    let (device, context) = warp_device();
    let size = pixels(7, 5);
    for api in INSERTION_APIS {
        for kind in TEXTURE_KINDS {
            let key = glyph_key(kind, 1);
            let bytes = pattern(size, kind, 19);
            for short_len in [0, 7 * bytes_per_pixel(kind) - 1, bytes.len() - 1] {
                let atlas = DirectXAtlas::new(&device, &context);
                let mut builder_calls = 0;
                assert!(
                    api.access(
                        &atlas,
                        &key,
                        size,
                        &bytes[..short_len],
                        &mut builder_calls,
                        AtlasAccessOutcome::Inserted,
                    )
                    .is_err(),
                    "{api:?}, {kind:?}, source length {short_len}"
                );
                assert_eq!(builder_calls, 1);
                assert_empty_atlas(&atlas);
                let tile = api
                    .access(
                        &atlas,
                        &key,
                        size,
                        &bytes,
                        &mut builder_calls,
                        AtlasAccessOutcome::Inserted,
                    )?
                    .context("a complete same-key retry must insert a tile")?;
                assert_eq!(builder_calls, 2);
                assert_eq!(tile.texture_id.kind, kind);
                assert_eq!(tile.texture_generation, 1);
                assert_eq!(tile.bounds.size, size);
                assert_eq!(
                    api.access(
                        &atlas,
                        &key,
                        pixels(0, 0),
                        &[],
                        &mut builder_calls,
                        AtlasAccessOutcome::Hit,
                    )?,
                    Some(tile)
                );
                assert_eq!(builder_calls, 2);
                assert_eq!(read_tile(&atlas, tile)?, bytes);
            }
        }
    }
    Ok(())
}

#[test]
fn warp_malformed_dimensions_do_not_allocate_or_publish_keys() -> anyhow::Result<()> {
    let (device, context) = warp_device();
    let bytes = vec![61; 16385 * 4];
    let malformed_sizes = [
        pixels(0, 3),
        pixels(3, 0),
        pixels(-1, 3),
        pixels(3, -1),
        pixels(16385, 1),
        pixels(1, 16385),
    ];
    for api in INSERTION_APIS {
        for kind in TEXTURE_KINDS {
            let atlas = DirectXAtlas::new(&device, &context);
            let key = glyph_key(kind, 2);
            let mut builder_calls = 0;
            for size in malformed_sizes {
                assert!(
                    api.access(
                        &atlas,
                        &key,
                        size,
                        &bytes,
                        &mut builder_calls,
                        AtlasAccessOutcome::Inserted,
                    )
                    .is_err(),
                    "{api:?}, {kind:?}, size {size:?}"
                );
                assert_empty_atlas(&atlas);
            }
            assert_eq!(builder_calls, malformed_sizes.len());
            let size = pixels(7, 5);
            let valid_bytes = pattern(size, kind, 29);
            let tile = api
                .access(
                    &atlas,
                    &key,
                    size,
                    &valid_bytes,
                    &mut builder_calls,
                    AtlasAccessOutcome::Inserted,
                )?
                .context("valid dimensions must permit a same-key retry")?;
            assert_eq!(tile.texture_generation, 1);
            assert_eq!(builder_calls, malformed_sizes.len() + 1);
            assert_eq!(read_tile(&atlas, tile)?, valid_bytes);
        }
    }
    Ok(())
}

#[test]
fn warp_rejection_preserves_existing_keys_and_atomic_texture_leases() -> anyhow::Result<()> {
    let (device, context) = warp_device();
    let size = pixels(7, 5);
    for api in INSERTION_APIS {
        for kind in TEXTURE_KINDS {
            let atlas = Arc::new(DirectXAtlas::new(&device, &context));
            let key = glyph_key(kind, 3);
            let rejected_key = glyph_key(kind, 4);
            let bytes = pattern(size, kind, 37);
            let mut builder_calls = 0;
            let tile = api
                .access(
                    &atlas,
                    &key,
                    size,
                    &bytes,
                    &mut builder_calls,
                    AtlasAccessOutcome::Inserted,
                )?
                .context("the existing tile must be inserted")?;
            let texture = tile.texture_instance();
            let unavailable = AtlasTextureInstanceId {
                generation: texture.generation + 1,
                ..texture
            };
            let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
            let before_failed_lease = resident_snapshot(&atlas, tile);
            assert!(matches!(
                platform_atlas
                    .clone()
                    .retain_texture_instances(&[texture, unavailable]),
                Err(AtlasTextureLeaseError::TextureUnavailable { texture: rejected, .. })
                    if rejected == unavailable
            ));
            assert_eq!(resident_snapshot(&atlas, tile), before_failed_lease);
            let lease = platform_atlas.retain_texture_instances(&[texture, texture])?;
            assert_eq!(lease.texture_instances(), &[texture]);
            assert_eq!(lease.validate(), Ok(()));
            let before_rejection = resident_snapshot(&atlas, tile);
            assert_eq!(before_rejection.live_keys, 1);
            assert_eq!(before_rejection.live_leases, 1);
            assert!(
                api.access(
                    &atlas,
                    &rejected_key,
                    size,
                    &bytes[..bytes.len() - 1],
                    &mut builder_calls,
                    AtlasAccessOutcome::Inserted,
                )
                .is_err()
            );
            assert_eq!(builder_calls, 2);
            assert_eq!(resident_snapshot(&atlas, tile), before_rejection);
            assert!(!atlas.0.lock().tiles_by_key.contains_key(&rejected_key));
            assert_eq!(lease.validate(), Ok(()));
            for lookup_api in INSERTION_APIS {
                assert_eq!(
                    lookup_api.access(
                        &atlas,
                        &key,
                        pixels(0, 0),
                        &[],
                        &mut builder_calls,
                        AtlasAccessOutcome::Hit,
                    )?,
                    Some(tile)
                );
            }
            assert_eq!(builder_calls, 2);
            assert_eq!(read_tile(&atlas, tile)?, bytes);
            drop(lease);
            assert_eq!(resident_snapshot(&atlas, tile).live_leases, 0);
        }
    }
    Ok(())
}

#[test]
fn warp_refused_upload_releases_only_the_unpublished_allocation() -> anyhow::Result<()> {
    let (device, context) = warp_device();
    for api in INSERTION_APIS {
        for kind in TEXTURE_KINDS {
            for remove_published_key in [false, true] {
                let atlas = Arc::new(DirectXAtlas::new(&device, &context));
                let key = glyph_key(kind, 5);
                let rejected_key = glyph_key(kind, 6);
                let size = pixels(7, 5);
                let bytes = pattern(size, kind, 43);
                let mut builder_calls = 0;
                let tile = api
                    .access(
                        &atlas,
                        &key,
                        size,
                        &bytes,
                        &mut builder_calls,
                        AtlasAccessOutcome::Inserted,
                    )?
                    .context("the leased tile must be inserted")?;
                let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
                let lease = platform_atlas.retain_texture_instances(&[tile.texture_instance()])?;
                if remove_published_key {
                    assert_eq!(
                        atlas.remove_with_diagnostics(&key).outcome,
                        AtlasRemoveOutcome::TextureRetained
                    );
                }
                let before_rejection = resident_snapshot(&atlas, tile);
                assert_eq!(before_rejection.live_keys, u32::from(!remove_published_key));
                assert_eq!(before_rejection.live_leases, 1);
                let rejected_size = {
                    let mut state = atlas.0.lock();
                    let resident = state
                        .texture_if_resident_mut(tile.texture_instance())
                        .expect("the seeded texture must be resident");
                    resident.bytes_per_pixel = if kind == AtlasTextureKind::Monochrome {
                        4
                    } else {
                        1
                    };
                    pixels(resident.allocator.size().width, 64)
                };
                let rejected_bytes = pattern(rejected_size, kind, 97);
                let result = api.access(
                    &atlas,
                    &rejected_key,
                    rejected_size,
                    &rejected_bytes,
                    &mut builder_calls,
                    AtlasAccessOutcome::Inserted,
                );
                {
                    let mut state = atlas.0.lock();
                    state
                        .texture_if_resident_mut(tile.texture_instance())
                        .expect("upload refusal must not retire a leased texture")
                        .bytes_per_pixel = bytes_per_pixel(kind) as u32;
                }
                let error = result.expect_err("a resident format mismatch must reject the upload");
                assert!(error.to_string().contains("pixel format"));
                assert_eq!(builder_calls, 2);
                assert_eq!(resident_snapshot(&atlas, tile), before_rejection);
                assert!(!atlas.0.lock().tiles_by_key.contains_key(&rejected_key));
                assert_eq!(lease.validate(), Ok(()));
                assert_eq!(read_tile(&atlas, tile)?, bytes);
                let retry = api
                    .access(
                        &atlas,
                        &rejected_key,
                        rejected_size,
                        &rejected_bytes,
                        &mut builder_calls,
                        AtlasAccessOutcome::Inserted,
                    )?
                    .context("the refused allocation must permit a valid same-key retry")?;
                assert_eq!(builder_calls, 3);
                assert_eq!(retry.texture_instance(), tile.texture_instance());
                assert_eq!(retry.bounds.origin.x, DevicePixels(0));
                assert!(retry.bounds.top() >= tile.bounds.bottom());
                assert!(!retry.bounds.intersects(&tile.bounds));
                assert_eq!(read_tile(&atlas, retry)?, rejected_bytes);
                assert_eq!(read_tile(&atlas, tile)?, bytes);
                assert_eq!(lease.validate(), Ok(()));
            }
        }
    }
    Ok(())
}

#[test]
fn warp_removed_keys_keep_leased_regions_unchanged_during_reinsertion() -> anyhow::Result<()> {
    let (device, context) = warp_device();
    let size = pixels(7, 5);
    for api in INSERTION_APIS {
        for kind in TEXTURE_KINDS {
            let atlas = Arc::new(DirectXAtlas::new(&device, &context));
            let key = glyph_key(kind, 7);
            let old_bytes = pattern(size, kind, 53);
            let new_bytes = pattern(size, kind, 113);
            let mut builder_calls = 0;
            let old_tile = api
                .access(
                    &atlas,
                    &key,
                    size,
                    &old_bytes,
                    &mut builder_calls,
                    AtlasAccessOutcome::Inserted,
                )?
                .context("the retained tile must be inserted")?;
            let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
            let lease = platform_atlas.retain_texture_instances(&[old_tile.texture_instance()])?;
            match api {
                InsertionApi::Plain => atlas.remove(&key),
                InsertionApi::Diagnostics => assert_eq!(
                    atlas.remove_with_diagnostics(&key).outcome,
                    AtlasRemoveOutcome::TextureRetained
                ),
            }
            assert!(!atlas.0.lock().tiles_by_key.contains_key(&key));
            let removed = resident_snapshot(&atlas, old_tile);
            assert_eq!(removed.live_keys, 0);
            assert_eq!(removed.live_leases, 1);
            let new_tile = api
                .access(
                    &atlas,
                    &key,
                    size,
                    &new_bytes,
                    &mut builder_calls,
                    AtlasAccessOutcome::Inserted,
                )?
                .context("a removed key must rebuild into a distinct region")?;
            assert_eq!(builder_calls, 2);
            assert_eq!(new_tile.texture_instance(), old_tile.texture_instance());
            assert!(!new_tile.bounds.intersects(&old_tile.bounds));
            assert_eq!(read_tile(&atlas, old_tile)?, old_bytes);
            assert_eq!(read_tile(&atlas, new_tile)?, new_bytes);
            assert_eq!(lease.validate(), Ok(()));
            assert_eq!(
                atlas.remove_with_diagnostics(&key).outcome,
                AtlasRemoveOutcome::TextureRetained
            );
            assert_eq!(read_tile(&atlas, old_tile)?, old_bytes);
            assert_eq!(resident_snapshot(&atlas, old_tile).live_keys, 0);
            drop(lease);
            let state = atlas.0.lock();
            assert!(
                state
                    .texture_if_resident(old_tile.texture_instance())
                    .is_none()
            );
            assert_eq!(
                state.texture_list(kind).free_list,
                vec![old_tile.texture_id.index as usize]
            );
        }
    }
    Ok(())
}

fn verify_multiline_readback(kind: AtlasTextureKind) -> anyhow::Result<()> {
    let (device, context) = warp_device();
    for api in INSERTION_APIS {
        let atlas = DirectXAtlas::new(&device, &context);
        let mut builder_calls = 0;
        let wide_size = pixels(1024, 16);
        let wide_bytes = pattern(wide_size, kind, 7);
        let wide_tile = api
            .access(
                &atlas,
                &glyph_key(kind, 8),
                wide_size,
                &wide_bytes,
                &mut builder_calls,
                AtlasAccessOutcome::Inserted,
            )?
            .context("the first shelf must be populated")?;
        let prefix_size = pixels(3, 5);
        let prefix_bytes = pattern(prefix_size, kind, 23);
        let prefix_tile = api
            .access(
                &atlas,
                &glyph_key(kind, 9),
                prefix_size,
                &prefix_bytes,
                &mut builder_calls,
                AtlasAccessOutcome::Inserted,
            )?
            .context("the new shelf must have an existing allocation")?;
        let size = pixels(7, 5);
        let bytes = pattern(size, kind, 83);
        let tile = api
            .access(
                &atlas,
                &glyph_key(kind, 10),
                size,
                &bytes,
                &mut builder_calls,
                AtlasAccessOutcome::Inserted,
            )?
            .context("the multiline test image must be inserted")?;
        assert_eq!(builder_calls, 3);
        assert_eq!(tile.texture_instance(), wide_tile.texture_instance());
        assert_eq!(tile.texture_instance(), prefix_tile.texture_instance());
        assert!(tile.bounds.origin.x.0 > 0, "{api:?}, {kind:?}");
        assert!(tile.bounds.origin.y.0 > 0, "{api:?}, {kind:?}");
        assert!(!tile.bounds.intersects(&wide_tile.bounds));
        assert!(!tile.bounds.intersects(&prefix_tile.bounds));
        assert_eq!(read_tile(&atlas, tile)?, bytes, "{api:?}, {kind:?}");
        assert_eq!(read_tile(&atlas, prefix_tile)?, prefix_bytes);
        assert_eq!(read_tile(&atlas, wide_tile)?, wide_bytes);
    }
    Ok(())
}

#[test]
fn warp_monochrome_multiline_upload_reads_back_at_nonzero_origin() -> anyhow::Result<()> {
    verify_multiline_readback(AtlasTextureKind::Monochrome)
}

#[test]
fn warp_polychrome_multiline_upload_reads_back_at_nonzero_origin() -> anyhow::Result<()> {
    verify_multiline_readback(AtlasTextureKind::Polychrome)
}

#[test]
fn warp_subpixel_multiline_upload_reads_back_at_nonzero_origin() -> anyhow::Result<()> {
    verify_multiline_readback(AtlasTextureKind::Subpixel)
}
