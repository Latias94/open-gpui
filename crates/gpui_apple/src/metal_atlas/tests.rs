//! Native Metal controls adapted from Zed's fixed `635026fabc` atlas regressions.
//! Windows source review leaves macOS compilation and execution UNQUALIFIED.

use super::*;
use open_gpui::{AtlasAccessOutcome, ImageId, RenderImageParams, point, size};

#[expect(
    clippy::arc_with_non_send_sync,
    reason = "PlatformAtlas uses Arc without requiring Send or Sync"
)]
fn create_atlas() -> Result<Arc<MetalAtlas>> {
    let device = objc2_metal::MTLCreateSystemDefaultDevice()
        .context("native Metal atlas qualification requires an available Metal device")?;
    let supports_shared_storage =
        cfg!(target_os = "ios") || device.supportsFamily(objc2_metal::MTLGPUFamily::Apple1);
    Ok(Arc::new(MetalAtlas::new(device, supports_shared_storage)))
}

fn image_key(image_id: usize) -> AtlasKey {
    AtlasKey::Image(RenderImageParams {
        image_id: ImageId(image_id),
        frame_index: 0,
    })
}

fn pixels(width: i32, height: i32) -> Size<DevicePixels> {
    size(DevicePixels(width), DevicePixels(height))
}

fn insert_image(
    atlas: &MetalAtlas,
    key: AtlasKey,
    dimensions: Size<DevicePixels>,
    value: u8,
) -> Result<AtlasTile> {
    atlas
        .get_or_insert_with(key, &mut || {
            let byte_count = dimensions.width.0 as usize * dimensions.height.0 as usize * 4;
            Ok(Some((dimensions, Cow::Owned(vec![value; byte_count]))))
        })?
        .context("retrieving inserted atlas tile")
}

fn read_tile(atlas: &MetalAtlas, tile: AtlasTile) -> Result<Vec<u8>> {
    let texture = atlas
        .metal_texture(tile.texture_instance())
        .context("reading resident atlas texture")?;
    let channels = match tile.texture_id.kind {
        AtlasTextureKind::Monochrome => 1,
        AtlasTextureKind::Polychrome => 4,
        AtlasTextureKind::Subpixel => anyhow::bail!("unsupported test texture kind"),
    };
    let row_bytes = tile.bounds.size.width.0 as usize * channels;
    let mut bytes = vec![0; row_bytes * tile.bounds.size.height.0 as usize];
    // SAFETY: The destination holds every row of this resident tile at the texture's channel width.
    unsafe {
        texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
            std::ptr::NonNull::from(bytes.as_mut_slice()).cast(),
            row_bytes,
            MTLRegion {
                origin: MTLOrigin {
                    x: tile.bounds.origin.x.into(),
                    y: tile.bounds.origin.y.into(),
                    z: 0,
                },
                size: MTLSize {
                    width: tile.bounds.size.width.into(),
                    height: tile.bounds.size.height.into(),
                    depth: 1,
                },
            },
            0,
        );
    }
    Ok(bytes)
}

#[test]
fn test_metal_texture_is_none_after_last_tile_removed() -> Result<()> {
    let atlas = create_atlas()?;
    let key = image_key(1);
    let tile = insert_image(&atlas, key.clone(), pixels(64, 64), 31)?;
    assert!(atlas.metal_texture(tile.texture_instance()).is_some());
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::TextureFreed
    );
    assert!(!atlas.contains(&key));
    assert!(atlas.metal_texture(tile.texture_instance()).is_none());
    let replacement = insert_image(&atlas, key.clone(), pixels(64, 64), 47)?;
    assert_eq!(replacement.texture_id, tile.texture_id);
    assert_ne!(replacement.texture_instance(), tile.texture_instance());
    assert!(atlas.metal_texture(tile.texture_instance()).is_none());
    assert!(
        atlas
            .metal_texture(replacement.texture_instance())
            .is_some()
    );
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::TextureFreed
    );
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::RemoveNoop
    );
    Ok(())
}

#[test]
fn invalid_and_stale_instance_lookups_do_not_touch_replacements() -> Result<()> {
    let atlas = create_atlas()?;
    let key = image_key(2);
    let tile = insert_image(&atlas, key.clone(), pixels(1, 1), 73)?;
    for instance in [
        AtlasTextureInstanceId {
            generation: 0,
            ..tile.texture_instance()
        },
        AtlasTextureInstanceId {
            generation: tile.texture_generation + 1,
            ..tile.texture_instance()
        },
        AtlasTextureInstanceId {
            texture_id: AtlasTextureId {
                index: u32::MAX,
                ..tile.texture_id
            },
            ..tile.texture_instance()
        },
        AtlasTextureInstanceId {
            texture_id: AtlasTextureId {
                kind: AtlasTextureKind::Subpixel,
                ..tile.texture_id
            },
            ..tile.texture_instance()
        },
    ] {
        assert!(atlas.metal_texture(instance).is_none());
    }
    atlas.remove(&key);
    let replacement = insert_image(&atlas, key.clone(), pixels(1, 1), 89)?;
    assert_eq!(
        atlas.0.lock().backend.remove_with_diagnostics(tile),
        AtlasRemoveOutcome::RemoveHit
    );
    assert!(atlas.contains(&key));
    assert_eq!(read_tile(&atlas, replacement)?, vec![89; 4]);
    Ok(())
}

#[test]
fn removed_published_regions_stay_intact_while_leased() -> Result<()> {
    let atlas = create_atlas()?;
    let keeper_key = image_key(3);
    let removed_key = image_key(4);
    let replacement_key = image_key(5);
    let keeper = insert_image(&atlas, keeper_key.clone(), pixels(64, 64), 17)?;
    let removed = insert_image(&atlas, removed_key.clone(), pixels(700, 700), 41)?;
    assert_eq!(keeper.texture_instance(), removed.texture_instance());
    let (lease, textures) = atlas.retain_metal_textures(&[removed.texture_instance()])?;
    assert_eq!(textures.len(), 1);
    let allocated_space = atlas
        .0
        .lock()
        .backend
        .texture_if_resident(removed.texture_instance())
        .unwrap()
        .allocator
        .allocated_space();
    assert_eq!(
        atlas.remove_with_diagnostics(&removed_key).outcome,
        AtlasRemoveOutcome::TextureRetained
    );
    assert_eq!(
        atlas
            .0
            .lock()
            .backend
            .texture_if_resident(removed.texture_instance())
            .unwrap()
            .allocator
            .allocated_space(),
        allocated_space
    );
    let replacement = insert_image(&atlas, replacement_key, pixels(700, 700), 83)?;
    assert_ne!(replacement.texture_instance(), removed.texture_instance());
    assert_eq!(read_tile(&atlas, removed)?, vec![41; 700 * 700 * 4]);
    assert_eq!(read_tile(&atlas, keeper)?, vec![17; 64 * 64 * 4]);
    assert_eq!(
        atlas.remove_with_diagnostics(&keeper_key).outcome,
        AtlasRemoveOutcome::TextureRetained
    );
    assert!(lease.validate().is_ok());
    drop(textures);
    drop(lease);
    assert!(atlas.metal_texture(removed.texture_instance()).is_none());
    Ok(())
}

#[test]
fn texture_snapshot_acquisition_is_atomic_and_deduplicated() -> Result<()> {
    let atlas = create_atlas()?;
    let key = image_key(6);
    let tile = insert_image(&atlas, key.clone(), pixels(3, 2), 61)?;
    let instance = tile.texture_instance();
    let unavailable = AtlasTextureInstanceId {
        generation: instance.generation + 1,
        ..instance
    };
    for instances in [vec![instance, unavailable], vec![unavailable, instance]] {
        assert!(atlas.retain_metal_textures(&instances).is_err());
        assert_eq!(
            atlas
                .0
                .lock()
                .backend
                .texture_if_resident(instance)
                .unwrap()
                .live_visual_leases,
            0
        );
    }
    let (lease, textures) = atlas.retain_metal_textures(&[instance, instance])?;
    assert_eq!(lease.texture_instances(), &[instance]);
    assert_eq!(textures.len(), 1);
    assert_eq!(
        atlas
            .0
            .lock()
            .backend
            .texture_if_resident(instance)
            .unwrap()
            .live_visual_leases,
        1
    );
    atlas.remove(&key);
    assert_eq!(read_tile(&atlas, tile)?, vec![61; 3 * 2 * 4]);
    drop(textures);
    drop(lease);
    assert!(atlas.metal_texture(instance).is_none());
    Ok(())
}

#[test]
fn lease_overflow_does_not_partially_retain_a_mixed_set() -> Result<()> {
    let atlas = create_atlas()?;
    let first = insert_image(&atlas, image_key(7), pixels(700, 700), 13)?;
    let second = insert_image(&atlas, image_key(8), pixels(700, 700), 23)?;
    assert_ne!(first.texture_instance(), second.texture_instance());
    atlas
        .0
        .lock()
        .backend
        .texture_if_resident_mut(second.texture_instance())
        .unwrap()
        .live_visual_leases = u32::MAX;
    let result =
        atlas.retain_metal_textures(&[first.texture_instance(), second.texture_instance()]);
    assert!(matches!(
        result
            .err()
            .and_then(|error| error.downcast::<AtlasTextureLeaseError>().ok()),
        Some(AtlasTextureLeaseError::LeaseCountOverflow { texture, .. })
            if texture == second.texture_instance()
    ));
    let mut state = atlas.0.lock();
    assert_eq!(
        state
            .backend
            .texture_if_resident(first.texture_instance())
            .unwrap()
            .live_visual_leases,
        0
    );
    state
        .backend
        .texture_if_resident_mut(second.texture_instance())
        .unwrap()
        .live_visual_leases = 0;
    Ok(())
}

#[test]
fn test_invalid_upload_does_not_allocate() -> Result<()> {
    let atlas = create_atlas()?;
    let mut state = atlas.0.lock();
    for (kind, short_bytes, short_error) in [
        (
            AtlasTextureKind::Monochrome,
            &[][..],
            "atlas upload byte length is 0, expected at least 1",
        ),
        (
            AtlasTextureKind::Polychrome,
            &[0][..],
            "atlas upload byte length is 1, expected at least 4",
        ),
    ] {
        for (width, height, bytes, expected_error) in [
            (1, 1, short_bytes, short_error),
            (0, 1, &[][..], "atlas dimensions must be positive"),
            (1, 0, &[][..], "atlas dimensions must be positive"),
            (i32::MIN, 1, &[][..], "atlas dimensions must be positive"),
            (1, i32::MIN, &[][..], "atlas dimensions must be positive"),
            (16385, 1, &[][..], "atlas dimensions exceed 16384"),
            (1, 16385, &[][..], "atlas dimensions exceed 16384"),
        ] {
            assert_eq!(
                state
                    .backend
                    .insert(kind, pixels(width, height), bytes)
                    .err()
                    .map(|error| error.to_string()),
                Some(expected_error.to_owned())
            );
            assert_eq!(state.backend.next_texture_generation, 1);
            for textures in [
                &state.backend.monochrome_textures,
                &state.backend.polychrome_textures,
            ] {
                assert_eq!((textures.textures.len(), textures.free_list.len()), (0, 0));
            }
        }
    }
    Ok(())
}

#[test]
fn failed_owned_insertions_leave_the_key_available_for_retry() -> Result<()> {
    let atlas = create_atlas()?;
    for diagnostic in [false, true] {
        let key = image_key(9 + usize::from(diagnostic));
        let mut build = || Ok(Some((pixels(3, 2), Cow::Borrowed(&[0u8; 23][..]))));
        let refused = if diagnostic {
            atlas
                .get_or_insert_with_diagnostics(key.clone(), &mut build)
                .map(|access| access.tile)
        } else {
            atlas.get_or_insert_with(key.clone(), &mut build)
        };
        assert!(refused.is_err());
        assert!(!atlas.contains(&key));
        assert!(
            atlas
                .get_or_insert_with(key.clone(), &mut || anyhow::bail!("builder failure"))
                .is_err()
        );
        assert!(!atlas.contains(&key));
        let unavailable = atlas.get_or_insert_with_diagnostics(key.clone(), &mut || Ok(None))?;
        assert_eq!(
            unavailable.diagnostic.outcome,
            AtlasAccessOutcome::Unavailable
        );
        assert!(!atlas.contains(&key));
        let bytes = [37; 24];
        let access = atlas.get_or_insert_with_diagnostics(key.clone(), &mut || {
            Ok(Some((pixels(3, 2), Cow::Borrowed(&bytes))))
        })?;
        assert_eq!(access.diagnostic.outcome, AtlasAccessOutcome::Inserted);
        let tile = access
            .tile
            .context("successful retry must publish its tile")?;
        assert!(atlas.contains(&key));
        let hit = atlas.get_or_insert_with_diagnostics(key, &mut || {
            panic!("a hit must not invoke its builder")
        })?;
        assert_eq!(hit.diagnostic.outcome, AtlasAccessOutcome::Hit);
        assert_eq!(hit.tile, Some(tile));
        assert_eq!(read_tile(&atlas, tile)?, bytes);
    }
    Ok(())
}

#[test]
fn destination_refusal_cancels_only_the_unpublished_allocation() -> Result<()> {
    let atlas = create_atlas()?;
    let keeper = insert_image(&atlas, image_key(11), pixels(64, 64), 57)?;
    let (lease, textures) = atlas.retain_metal_textures(&[keeper.texture_instance()])?;
    let mut state = atlas.0.lock();
    let backend = &mut state.backend;
    let before = backend
        .texture_if_resident(keeper.texture_instance())
        .unwrap()
        .allocator
        .allocated_space();
    let unpublished = backend.allocate(pixels(700, 700), AtlasTextureKind::Polychrome)?;
    let bytes = vec![103; 700 * 700 * 4];
    let upload = AtlasUpload::new(AtlasTextureKind::Polychrome, pixels(700, 700), &bytes)?;
    assert!(
        backend
            .texture_if_resident(unpublished.texture_instance())
            .unwrap()
            .upload(point(DevicePixels(-1), DevicePixels(0)), &upload)
            .is_err()
    );
    backend.cancel_unpublished_allocation(unpublished);
    let resident = backend
        .texture_if_resident(keeper.texture_instance())
        .unwrap();
    assert_eq!(resident.allocator.allocated_space(), before);
    assert_eq!(resident.live_atlas_keys, 1);
    assert_eq!(resident.live_visual_leases, 1);
    drop(state);
    assert_eq!(read_tile(&atlas, keeper)?, vec![57; 64 * 64 * 4]);
    let replacement = insert_image(&atlas, image_key(12), pixels(700, 700), 113)?;
    assert_eq!(replacement.texture_instance(), keeper.texture_instance());
    assert!(lease.validate().is_ok());
    drop(textures);
    drop(lease);
    Ok(())
}

#[test]
fn failed_backend_upload_cancels_its_slot_and_allows_retry() -> Result<()> {
    let atlas = create_atlas()?;
    let key = image_key(13);
    {
        let mut state = atlas.0.lock();
        let descriptor = MTLTextureDescriptor::new();
        // SAFETY: The 1x1 control uses positive dimensions supported by every Metal device.
        unsafe {
            descriptor.setWidth(1);
            descriptor.setHeight(1);
        }
        descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        descriptor.setStorageMode(if state.backend.supports_shared_storage {
            MTLStorageMode::Shared
        } else {
            MTLStorageMode::Managed
        });
        let undersized = state
            .backend
            .device
            .newTextureWithDescriptor(&descriptor)
            .context("creating refusal control")?;
        state
            .backend
            .push_texture(pixels(8, 8), AtlasTextureKind::Polychrome)?
            .metal_texture = undersized;
    }
    assert!(insert_image(&atlas, key.clone(), pixels(3, 2), 19).is_err());
    assert!(!atlas.contains(&key));
    {
        let state = atlas.0.lock();
        assert!(state.backend.polychrome_textures.textures[0].is_none());
        assert_eq!(state.backend.polychrome_textures.free_list, [0]);
    }
    let tile = insert_image(&atlas, key, pixels(3, 2), 29)?;
    assert_eq!(tile.texture_id.index, 0);
    assert_eq!(tile.texture_generation, 2);
    assert_eq!(read_tile(&atlas, tile)?, vec![29; 24]);
    Ok(())
}

#[test]
fn test_texture_upload_preserves_pixel_layout() -> Result<()> {
    let atlas = create_atlas()?;
    let mut state = atlas.0.lock();
    for (kind, channels) in [
        (AtlasTextureKind::Monochrome, 1),
        (AtlasTextureKind::Polychrome, 4),
    ] {
        let texture = state.backend.push_texture(pixels(8, 8), kind)?;
        texture.upload(
            Point::default(),
            &AtlasUpload::new(kind, pixels(8, 8), &vec![0; 8 * 8 * channels])?,
        )?;
        let bytes = (1..=3 * 2 * channels)
            .map(|value| value as u8)
            .collect::<Vec<_>>();
        texture.upload(
            point(DevicePixels(2), DevicePixels(3)),
            &AtlasUpload::new(kind, pixels(3, 2), &bytes)?,
        )?;
        let mut actual = vec![0; 8 * 8 * channels];
        // SAFETY: The destination contains all 8x8 texels with the format's channel width.
        unsafe {
            texture
                .metal_texture
                .getBytes_bytesPerRow_fromRegion_mipmapLevel(
                    std::ptr::NonNull::from(actual.as_mut_slice()).cast(),
                    8 * channels,
                    MTLRegion {
                        origin: MTLOrigin { x: 0, y: 0, z: 0 },
                        size: MTLSize {
                            width: 8,
                            height: 8,
                            depth: 1,
                        },
                    },
                    0,
                );
        }
        let mut expected = vec![0; actual.len()];
        for row in 0..2 {
            let destination = ((row + 3) * 8 + 2) * channels;
            let source = row * 3 * channels;
            expected[destination..destination + 3 * channels]
                .copy_from_slice(&bytes[source..source + 3 * channels]);
        }
        assert_eq!(actual, expected);
        let upload = AtlasUpload::new(kind, pixels(3, 2), &bytes)?;
        for origin in [
            point(DevicePixels(-1), DevicePixels(0)),
            point(DevicePixels(0), DevicePixels(-1)),
            point(
                DevicePixels(texture.metal_texture.width() as i32 - 2),
                DevicePixels(0),
            ),
            point(
                DevicePixels(0),
                DevicePixels(texture.metal_texture.height() as i32 - 1),
            ),
            point(DevicePixels(i32::MAX), DevicePixels(i32::MAX)),
        ] {
            assert!(texture.upload(origin, &upload).is_err());
        }
    }
    Ok(())
}
