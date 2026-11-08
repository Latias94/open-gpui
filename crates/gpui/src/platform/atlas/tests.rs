use super::*;
use crate::{AtlasTextureId, Bounds, ImageId, Point, RenderImageParams, TileId};

const TILE_SIZE: Size<DevicePixels> = Size {
    width: DevicePixels(1),
    height: DevicePixels(1),
};

#[derive(Default)]
struct RecordingAtlasBackend {
    insert_calls: u32,
    fail_next_insert: bool,
    removed_tiles: Vec<AtlasTile>,
    tile_size: Option<Size<DevicePixels>>,
}

impl AtlasBackend for RecordingAtlasBackend {
    fn insert(
        &mut self,
        kind: AtlasTextureKind,
        size: Size<DevicePixels>,
        _bytes: &[u8],
    ) -> Result<AtlasTile> {
        self.insert_calls += 1;
        if std::mem::take(&mut self.fail_next_insert) {
            anyhow::bail!("backend failed");
        }
        Ok(AtlasTile {
            texture_id: AtlasTextureId { index: 0, kind },
            tile_id: TileId(self.insert_calls),
            padding: 0,
            bounds: Bounds {
                origin: Point::default(),
                size: self.tile_size.unwrap_or(size),
            },
            texture_generation: self.insert_calls,
            texture_generation_padding: 0,
        })
    }

    fn remove(&mut self, tile: AtlasTile) {
        self.removed_tiles.push(tile);
    }
}

fn image_key(image_id: usize) -> AtlasKey {
    AtlasKey::Image(RenderImageParams {
        image_id: ImageId(image_id),
        frame_index: 0,
    })
}

fn build_tile() -> Result<Option<(Size<DevicePixels>, Cow<'static, [u8]>)>> {
    Ok(Some((TILE_SIZE, Cow::Borrowed(&[0, 0, 0, 255]))))
}

#[test]
fn only_successful_inserts_are_cached() -> Result<()> {
    let mut state = AtlasState::new(RecordingAtlasBackend::default());
    let key = image_key(1);

    assert_eq!(
        state.get_or_insert_with(key.clone(), &mut || Ok(None))?,
        None
    );
    state
        .get_or_insert_with(key.clone(), &mut || anyhow::bail!("builder failed"))
        .expect_err("builder error should propagate");
    assert!(!state.contains(&key));
    assert_eq!(state.backend.insert_calls, 0);

    state.backend.fail_next_insert = true;
    state
        .get_or_insert_with(key.clone(), &mut build_tile)
        .expect_err("backend error should propagate");
    assert!(!state.contains(&key));
    assert_eq!(state.backend.insert_calls, 1);

    let tile = state
        .get_or_insert_with(key.clone(), &mut build_tile)?
        .expect("builder should produce a tile");
    assert_eq!(tile.texture_id.kind, key.texture_kind());
    assert_eq!(
        state.get_or_insert_with(key.clone(), &mut || {
            anyhow::bail!("cache hit must not call the builder")
        })?,
        Some(tile)
    );
    assert!(state.contains(&key));
    assert_eq!(state.backend.insert_calls, 2);
    Ok(())
}

#[test]
fn remove_and_clear_invalidate_keys() -> Result<()> {
    let mut state = AtlasState::new(RecordingAtlasBackend::default());
    let key = image_key(1);
    let other_key = image_key(2);
    let tile = state
        .get_or_insert_with(key.clone(), &mut build_tile)?
        .expect("builder should produce a tile");
    state
        .get_or_insert_with(other_key.clone(), &mut build_tile)?
        .expect("builder should produce another tile");

    state.remove(&key);
    state.remove(&key);
    assert!(!state.contains(&key));
    assert!(state.contains(&other_key));
    assert_eq!(state.backend.removed_tiles, vec![tile]);

    let mut reset_calls = 0;
    state.clear(|_| reset_calls += 1);
    assert_eq!(reset_calls, 1);
    assert!(!state.contains(&other_key));
    assert_eq!(state.backend.removed_tiles, vec![tile]);
    Ok(())
}

#[test]
fn diagnostics_share_successful_publication_and_exact_removal() -> Result<()> {
    let mut state = AtlasState::new(RecordingAtlasBackend::default());
    let key = image_key(3);
    let image = key.image_params();
    let unavailable = state.get_or_insert_with_diagnostics(key.clone(), &mut || Ok(None))?;
    assert_eq!(unavailable.tile, None);
    assert_eq!(unavailable.diagnostic.image, image);
    assert_eq!(
        unavailable.diagnostic.outcome,
        AtlasAccessOutcome::Unavailable
    );
    assert!(!state.contains(&key));

    let inserted = state.get_or_insert_with_diagnostics(key.clone(), &mut build_tile)?;
    let tile = inserted
        .tile
        .expect("successful insertion must return its tile");
    assert_eq!(inserted.diagnostic.image, image);
    assert_eq!(inserted.diagnostic.outcome, AtlasAccessOutcome::Inserted);
    assert_eq!(inserted.diagnostic.tile, Some(tile));
    assert_eq!(inserted.diagnostic.texture_id, Some(tile.texture_id));
    assert_eq!(inserted.diagnostic.tile_id, Some(tile.tile_id));
    assert_eq!(inserted.diagnostic.size, Some(TILE_SIZE));
    let hit = state.get_or_insert_with_diagnostics(key.clone(), &mut || {
        anyhow::bail!("diagnostic hits must not rebuild")
    })?;
    assert_eq!(hit.tile, Some(tile));
    assert_eq!(hit.diagnostic.outcome, AtlasAccessOutcome::Hit);
    assert_eq!(state.backend.insert_calls, 1);

    assert_eq!(
        state.remove_with_diagnostics(&key),
        AtlasRemoveDiagnostic::new(&key, AtlasRemoveOutcome::RemoveHit, Some(tile.texture_id),)
    );
    assert_eq!(
        state.remove_with_diagnostics(&key),
        AtlasRemoveDiagnostic::new(&key, AtlasRemoveOutcome::RemoveNoop, None)
    );
    assert_eq!(state.backend.removed_tiles, vec![tile]);
    assert!(!state.contains(&key));
    Ok(())
}

#[test]
fn insertion_diagnostics_keep_the_builder_size_before_backend_adjustment() -> Result<()> {
    let allocated_size = Size {
        width: DevicePixels(2),
        height: DevicePixels(2),
    };
    let mut state = AtlasState::new(RecordingAtlasBackend {
        tile_size: Some(allocated_size),
        ..Default::default()
    });
    let key = image_key(4);
    let inserted = state.get_or_insert_with_diagnostics(key.clone(), &mut build_tile)?;
    let tile = inserted
        .tile
        .expect("successful insertion must publish its tile");
    assert_eq!(tile.bounds.size, allocated_size);
    assert_eq!(inserted.diagnostic.size, Some(TILE_SIZE));

    let cached = state.get_or_insert_with_diagnostics(key, &mut || {
        anyhow::bail!("a hit must use the already published tile")
    })?;
    assert_eq!(cached.tile, Some(tile));
    assert_eq!(cached.diagnostic.size, Some(allocated_size));
    Ok(())
}
