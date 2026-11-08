use super::*;
use crate::{ImageId, RenderImageParams};
use std::sync::Arc;

fn insert_image(atlas: &HeadlessAtlas, image_id: usize) -> anyhow::Result<(AtlasKey, AtlasTile)> {
    let key = AtlasKey::Image(RenderImageParams {
        image_id: ImageId(image_id),
        frame_index: 0,
    });
    let mut build = || {
        Ok(Some((
            Size {
                width: DevicePixels(1),
                height: DevicePixels(1),
            },
            Cow::Borrowed(&[0_u8, 0, 0, 255][..]),
        )))
    };
    let tile = atlas
        .get_or_insert_with(key.clone(), &mut build)?
        .expect("test image should allocate an atlas tile");
    Ok((key, tile))
}

#[test]
fn texture_lease_deduplicates_and_delays_texture_retirement() -> anyhow::Result<()> {
    let atlas = Arc::new(HeadlessAtlas::default());
    let (key, tile) = insert_image(&atlas, 1)?;
    let texture = tile.texture_instance();
    let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();

    let lease = platform_atlas
        .retain_texture_instances(&[texture, texture])
        .expect("resident texture should be retainable");
    assert_eq!(lease.texture_instances(), &[texture]);

    atlas.remove(&key);
    {
        let state = atlas.0.lock();
        let refs = state
            .backend
            .texture_refs
            .get(&texture)
            .expect("leased texture must remain resident after key removal");
        assert_eq!(refs.live_atlas_keys, 0);
        assert_eq!(refs.live_visual_leases, 1);
    }

    drop(lease);
    assert!(!atlas.0.lock().backend.texture_refs.contains_key(&texture));
    Ok(())
}

#[test]
fn empty_texture_lease_has_no_unsafe_acquire_or_release_obligation() {
    let atlas = Arc::new(HeadlessAtlas::default());
    let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();

    let lease = platform_atlas
        .retain_texture_instances(&[])
        .expect("an empty texture set should produce an inert lease");
    assert!(lease.texture_instances().is_empty());
    drop(lease);

    let state = atlas.0.lock();
    assert_eq!(state.backend.lease_acquire_calls, 0);
    assert_eq!(state.backend.lease_release_calls, 0);
}

#[test]
fn texture_lease_acquisition_is_atomic_when_one_texture_is_unavailable() -> anyhow::Result<()> {
    let atlas = Arc::new(HeadlessAtlas::default());
    let (_, tile) = insert_image(&atlas, 2)?;
    let texture = tile.texture_instance();
    let unavailable = AtlasTextureInstanceId {
        texture_id: AtlasTextureId {
            index: tile.texture_id.index + 100,
            kind: tile.texture_id.kind,
        },
        generation: texture.generation + 100,
    };
    let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();

    assert!(matches!(
        platform_atlas.retain_texture_instances(&[texture, unavailable]),
        Err(AtlasTextureLeaseError::TextureUnavailable { texture: rejected, .. })
            if rejected == unavailable
    ));
    assert_eq!(
        atlas.0.lock().backend.texture_refs[&texture].live_visual_leases,
        0
    );
    let state = atlas.0.lock();
    assert_eq!(state.backend.lease_acquire_calls, 1);
    assert_eq!(state.backend.lease_release_calls, 0);
    Ok(())
}

#[test]
fn texture_lease_reports_epoch_invalidation_after_renderer_reset() -> anyhow::Result<()> {
    let atlas = Arc::new(HeadlessAtlas::default());
    let (_, tile) = insert_image(&atlas, 3)?;
    let texture = tile.texture_instance();
    let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
    let lease = platform_atlas
        .retain_texture_instances(&[texture])
        .expect("resident texture should be retainable");
    let expected_epoch = lease.epoch();
    let replacement = AtlasTextureInstanceId {
        texture_id: tile.texture_id,
        generation: texture.generation + 1,
    };

    let actual_epoch = {
        let mut state = atlas.0.lock();
        state.clear(|backend| {
            backend.epoch = backend.epoch.next();
            backend.texture_refs.clear();
            backend.texture_refs.insert(
                replacement,
                HeadlessAtlasTextureRefs {
                    live_atlas_keys: 1,
                    live_visual_leases: 7,
                },
            );
        });
        state.backend.epoch
    };

    assert_eq!(
        lease.validate(),
        Err(crate::AtlasTextureLeaseInvalidation {
            expected_epoch,
            actual_epoch,
        })
    );
    drop(lease);
    assert_eq!(
        atlas.0.lock().backend.texture_refs[&replacement].live_visual_leases,
        7,
        "dropping an old-epoch lease must not mutate a replacement in the same slot"
    );
    Ok(())
}

#[test]
fn texture_lease_acquisition_is_atomic_when_one_count_overflows() -> anyhow::Result<()> {
    let atlas = Arc::new(HeadlessAtlas::default());
    let (_, first_tile) = insert_image(&atlas, 4)?;
    let (_, second_tile) = insert_image(&atlas, 5)?;
    let first_texture = first_tile.texture_instance();
    let second_texture = second_tile.texture_instance();
    atlas
        .0
        .lock()
        .backend
        .texture_refs
        .get_mut(&second_texture)
        .expect("second texture should be resident")
        .live_visual_leases = u32::MAX;
    let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();

    assert!(matches!(
        platform_atlas.retain_texture_instances(&[first_texture, second_texture]),
        Err(AtlasTextureLeaseError::LeaseCountOverflow { texture, .. })
            if texture == second_texture
    ));

    let state = atlas.0.lock();
    assert_eq!(
        state.backend.texture_refs[&first_texture].live_visual_leases,
        0
    );
    assert_eq!(
        state.backend.texture_refs[&second_texture].live_visual_leases,
        u32::MAX
    );
    assert_eq!(state.backend.lease_acquire_calls, 1);
    assert_eq!(state.backend.lease_release_calls, 0);
    Ok(())
}

#[test]
fn texture_lease_rejects_a_different_generation_in_the_same_slot() -> anyhow::Result<()> {
    let atlas = Arc::new(HeadlessAtlas::default());
    let (_, tile) = insert_image(&atlas, 6)?;
    let texture = tile.texture_instance();
    let unavailable = AtlasTextureInstanceId {
        texture_id: texture.texture_id,
        generation: texture.generation + 1,
    };
    let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();

    assert!(matches!(
        platform_atlas.retain_texture_instances(&[texture, unavailable]),
        Err(AtlasTextureLeaseError::TextureUnavailable { texture: rejected, .. })
            if rejected == unavailable
    ));
    let state = atlas.0.lock();
    assert_eq!(state.backend.texture_refs[&texture].live_visual_leases, 0);
    assert_eq!(state.backend.lease_release_calls, 0);
    Ok(())
}

#[test]
fn old_epoch_lease_drop_does_not_release_a_matching_replacement_instance() -> anyhow::Result<()> {
    let atlas = Arc::new(HeadlessAtlas::default());
    let (_, tile) = insert_image(&atlas, 7)?;
    let texture = tile.texture_instance();
    let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
    let lease = platform_atlas
        .retain_texture_instances(&[texture])
        .expect("resident texture should be retainable");
    let expected_epoch = lease.epoch();
    let actual_epoch = {
        let mut state = atlas.0.lock();
        state.clear(|backend| {
            backend.epoch = backend.epoch.next();
            backend.texture_refs.clear();
            backend.texture_refs.insert(
                texture,
                HeadlessAtlasTextureRefs {
                    live_atlas_keys: 1,
                    live_visual_leases: 7,
                },
            );
        });
        state.backend.epoch
    };

    assert_eq!(
        lease.validate(),
        Err(crate::AtlasTextureLeaseInvalidation {
            expected_epoch,
            actual_epoch,
        })
    );
    drop(lease);
    assert_eq!(
        atlas.0.lock().backend.texture_refs[&texture].live_visual_leases,
        7
    );
    Ok(())
}

#[test]
fn identifier_exhaustion_does_not_partially_allocate_or_publish() -> anyhow::Result<()> {
    for next_id in [u32::MAX - 1, u32::MAX] {
        let atlas = HeadlessAtlas::default();
        let (resident_key, resident_tile) = insert_image(&atlas, 8)?;
        let absent_key = AtlasKey::Image(RenderImageParams {
            image_id: ImageId(9),
            frame_index: 0,
        });
        atlas.0.lock().backend.next_id = next_id;

        assert!(insert_image(&atlas, 9).is_err());

        let state = atlas.0.lock();
        assert_eq!(state.backend.next_id, next_id);
        assert_eq!(state.backend.texture_refs.len(), 1);
        assert!(
            state
                .backend
                .texture_refs
                .contains_key(&resident_tile.texture_instance())
        );
        assert!(state.contains(&resident_key));
        assert!(!state.contains(&absent_key));
    }

    let atlas = HeadlessAtlas::default();
    atlas.0.lock().backend.next_id = u32::MAX - 2;
    let (_, tile) = insert_image(&atlas, 10)?;
    assert_eq!(tile.texture_id.index, u32::MAX - 1);
    assert_eq!(tile.tile_id, TileId(u32::MAX));
    assert_eq!(tile.texture_generation, u32::MAX - 1);
    assert!(insert_image(&atlas, 11).is_err());
    let state = atlas.0.lock();
    assert_eq!(state.backend.next_id, u32::MAX);
    assert_eq!(state.backend.texture_refs.len(), 1);
    Ok(())
}

#[test]
fn access_preserves_diagnostics_and_recovers_from_builder_failure() -> anyhow::Result<()> {
    let atlas = HeadlessAtlas::default();
    let key = AtlasKey::Image(RenderImageParams {
        image_id: ImageId(12),
        frame_index: 0,
    });
    let size = Size {
        width: DevicePixels(1),
        height: DevicePixels(1),
    };
    let mut build = || Ok(Some((size, Cow::Borrowed(&[0_u8, 0, 0, 255][..]))));

    let access = atlas.get_or_insert_with_diagnostics(key.clone(), &mut build)?;
    let tile = access.tile.expect("builder should allocate an atlas tile");
    assert_eq!(
        access.diagnostic.outcome,
        crate::AtlasAccessOutcome::Inserted
    );
    assert_eq!(access.diagnostic.tile, Some(tile));
    assert_eq!(access.diagnostic.size, Some(size));
    assert_eq!(tile.texture_id.kind, key.texture_kind());
    assert_eq!(tile.texture_generation, tile.texture_id.index);
    assert_ne!(tile.texture_generation, 0);
    assert_eq!(tile.texture_generation_padding, 0);
    assert!(atlas.contains(&key));

    let hit = atlas.get_or_insert_with_diagnostics(key.clone(), &mut || {
        panic!("a resident key must not invoke the builder")
    })?;
    assert_eq!(hit.tile, Some(tile));
    assert_eq!(hit.diagnostic.outcome, crate::AtlasAccessOutcome::Hit);
    let removal = atlas.remove_with_diagnostics(&key);
    assert_eq!(removal.outcome, AtlasRemoveOutcome::TextureFreed);
    assert_eq!(removal.texture_id, Some(tile.texture_id));
    assert!(!atlas.contains(&key));
    assert!(atlas.0.lock().backend.texture_refs.is_empty());

    let unavailable = atlas.get_or_insert_with_diagnostics(key.clone(), &mut || Ok(None))?;
    assert_eq!(unavailable.tile, None);
    assert_eq!(
        unavailable.diagnostic.outcome,
        crate::AtlasAccessOutcome::Unavailable
    );
    assert!(!atlas.contains(&key));
    assert!(
        atlas
            .get_or_insert_with(key.clone(), &mut || {
                Err(anyhow::anyhow!("builder rejected image"))
            })
            .is_err()
    );
    assert!(!atlas.contains(&key));
    assert_eq!(atlas.0.lock().backend.next_id, tile.tile_id.0);
    assert!(atlas.0.lock().backend.texture_refs.is_empty());

    let (_, retried) = insert_image(&atlas, 12)?;
    assert!(retried.texture_id.index > tile.tile_id.0);
    assert!(atlas.contains(&key));
    Ok(())
}

#[test]
fn concurrent_same_key_insertion_builds_once_under_the_shared_lock() -> anyhow::Result<()> {
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        thread,
        time::Duration,
    };

    let atlas = HeadlessAtlas::default();
    let key = AtlasKey::Image(RenderImageParams {
        image_id: ImageId(15),
        frame_index: 0,
    });
    let size = Size {
        width: DevicePixels(1),
        height: DevicePixels(1),
    };
    let build_calls = AtomicUsize::new(0);
    let timeout = Duration::from_secs(5);
    let (builder_started, builder_started_rx) = mpsc::channel();
    let (release_builder, release_builder_rx) = mpsc::channel();
    let (contender_started, contender_started_rx) = mpsc::channel();

    let (first_tile, contender_tile, builder_holds_lock) = thread::scope(|scope| -> Result<_> {
        let atlas = &atlas;
        let build_calls = &build_calls;
        let first_key = key.clone();
        let first = scope.spawn(move || {
            atlas.get_or_insert_with(first_key, &mut || {
                build_calls.fetch_add(1, Ordering::Relaxed);
                builder_started.send(())?;
                release_builder_rx.recv_timeout(timeout)?;
                Ok(Some((size, Cow::Borrowed(&[0_u8, 0, 0, 255][..]))))
            })
        });
        builder_started_rx.recv_timeout(timeout)?;
        let builder_holds_lock = atlas.0.try_lock().is_none();

        let contender = scope.spawn(move || {
            contender_started.send(())?;
            atlas.get_or_insert_with(key, &mut || {
                build_calls.fetch_add(1, Ordering::Relaxed);
                Ok(Some((size, Cow::Borrowed(&[0_u8, 0, 0, 255][..]))))
            })
        });
        contender_started_rx.recv_timeout(timeout)?;
        release_builder.send(())?;

        let first_tile = first
            .join()
            .expect("first insertion thread should not panic")?;
        let contender_tile = contender
            .join()
            .expect("contending insertion thread should not panic")?;
        Ok((first_tile, contender_tile, builder_holds_lock))
    })?;

    assert!(builder_holds_lock, "builder must hold the atlas lock");
    assert!(first_tile.is_some());
    assert_eq!(first_tile, contender_tile);
    assert_eq!(build_calls.load(Ordering::Relaxed), 1);
    assert_eq!(atlas.0.lock().backend.texture_refs.len(), 1);
    Ok(())
}

#[test]
fn texture_identity_remains_monotonic_after_removal_and_clear() -> anyhow::Result<()> {
    let atlas = HeadlessAtlas::default();
    let (key, first) = insert_image(&atlas, 13)?;
    atlas.remove(&key);
    let (_, second) = insert_image(&atlas, 13)?;
    assert!(second.texture_id.index > first.tile_id.0);
    assert!(second.texture_generation > first.texture_generation);
    let previous_epoch = atlas.atlas_texture_lease_epoch();

    {
        let mut state = atlas.0.lock();
        state.clear(|backend| {
            backend.epoch = backend.epoch.next();
            backend.texture_refs.clear();
        });
        assert_eq!(state.backend.next_id, second.tile_id.0);
        assert!(state.backend.texture_refs.is_empty());
    }
    assert!(!atlas.contains(&key));
    assert_eq!(atlas.atlas_texture_lease_epoch(), previous_epoch.next());

    let (_, third) = insert_image(&atlas, 13)?;
    assert!(third.texture_id.index > second.tile_id.0);
    assert!(third.texture_generation > second.texture_generation);
    assert!(atlas.contains(&key));
    Ok(())
}

#[test]
fn retiring_a_removed_texture_does_not_retire_the_reinserted_key() -> anyhow::Result<()> {
    let atlas = Arc::new(HeadlessAtlas::default());
    let (key, first) = insert_image(&atlas, 14)?;
    let first_texture = first.texture_instance();
    let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
    let first_lease = platform_atlas
        .clone()
        .retain_texture_instances(&[first_texture])
        .expect("resident texture should be retainable");

    let removal = atlas.remove_with_diagnostics(&key);
    assert_eq!(removal.outcome, AtlasRemoveOutcome::TextureRetained);
    assert_eq!(removal.texture_id, Some(first.texture_id));
    assert_eq!(
        atlas.remove_with_diagnostics(&key).outcome,
        AtlasRemoveOutcome::RemoveNoop
    );
    assert!(!atlas.contains(&key));

    let (_, second) = insert_image(&atlas, 14)?;
    let second_texture = second.texture_instance();
    assert!(second.texture_id.index > first.tile_id.0);
    assert_ne!(second_texture, first_texture);
    let second_lease = platform_atlas
        .retain_texture_instances(&[second_texture])
        .expect("replacement texture should be retainable");

    drop(first_lease);
    {
        let state = atlas.0.lock();
        assert!(!state.backend.texture_refs.contains_key(&first_texture));
        assert_eq!(
            state.backend.texture_refs[&second_texture].live_atlas_keys,
            1
        );
        assert_eq!(
            state.backend.texture_refs[&second_texture].live_visual_leases,
            1
        );
    }
    drop(second_lease);
    assert_eq!(
        atlas.0.lock().backend.texture_refs[&second_texture].live_visual_leases,
        0
    );
    assert!(atlas.contains(&key));
    let removal = atlas.remove_with_diagnostics(&key);
    assert_eq!(removal.outcome, AtlasRemoveOutcome::TextureFreed);
    assert_eq!(removal.texture_id, Some(second.texture_id));
    assert!(atlas.0.lock().backend.texture_refs.is_empty());
    Ok(())
}

#[test]
fn builder_unwind_releases_an_outer_lease_and_keeps_the_key_retryable() -> anyhow::Result<()> {
    let atlas = Arc::new(HeadlessAtlas::default());
    let (removed_key, retained_tile) = insert_image(&atlas, 15)?;
    let platform_atlas: Arc<dyn PlatformAtlas> = atlas.clone();
    let retained_instance = retained_tile.texture_instance();
    let retained_lease = platform_atlas
        .clone()
        .retain_texture_instances(&[retained_instance])
        .expect("the original resource must be retainable");
    atlas.remove(&removed_key);
    let failed_key = AtlasKey::Image(RenderImageParams {
        image_id: ImageId(16),
        frame_index: 0,
    });

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _retained_lease = retained_lease;
        let _ = atlas.get_or_insert_with(failed_key.clone(), &mut || {
            panic!("the caller's raster builder unwound")
        });
    }));
    assert!(outcome.is_err());
    assert!(!atlas.contains(&failed_key));
    assert!(matches!(
        platform_atlas
            .clone()
            .retain_texture_instances(&[retained_instance]),
        Err(AtlasTextureLeaseError::TextureUnavailable { texture, .. })
            if texture == retained_instance
    ));
    assert_eq!(atlas.0.lock().backend.lease_release_calls, 1);

    let (_, recovered_tile) = insert_image(&atlas, 16)?;
    assert!(atlas.contains(&failed_key));
    let recovered_lease = platform_atlas
        .retain_texture_instances(&[recovered_tile.texture_instance()])
        .expect("a retry after builder unwind must own a fresh resource");
    recovered_lease
        .validate()
        .expect("the retry must keep its epoch");
    Ok(())
}
