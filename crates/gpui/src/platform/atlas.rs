mod headless;

pub use headless::*;

use crate::{
    AtlasAccess, AtlasAccessDiagnostic, AtlasAccessOutcome, AtlasKey, AtlasRemoveDiagnostic,
    AtlasRemoveOutcome, AtlasTextureKind, AtlasTile, DevicePixels, Size,
};
use anyhow::Result;
use open_gpui_collections::FxHashMap;
use std::{borrow::Cow, collections::hash_map::Entry};

#[doc(hidden)]
pub trait AtlasBackend {
    fn insert(
        &mut self,
        kind: AtlasTextureKind,
        size: Size<DevicePixels>,
        bytes: &[u8],
    ) -> Result<AtlasTile>;

    fn remove(&mut self, tile: AtlasTile);

    fn remove_with_diagnostics(&mut self, tile: AtlasTile) -> AtlasRemoveOutcome {
        self.remove(tile);
        AtlasRemoveOutcome::RemoveHit
    }
}

#[doc(hidden)]
pub struct AtlasState<Backend> {
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
    pub backend: Backend,
}

impl<Backend> AtlasState<Backend> {
    pub fn new(backend: Backend) -> Self {
        Self {
            tiles_by_key: FxHashMap::default(),
            backend,
        }
    }

    pub fn contains(&self, key: &AtlasKey) -> bool {
        self.tiles_by_key.contains_key(key)
    }

    pub fn clear(&mut self, reset_backend: impl FnOnce(&mut Backend)) {
        self.tiles_by_key.clear();
        reset_backend(&mut self.backend);
    }
}

impl<Backend: Default> Default for AtlasState<Backend> {
    fn default() -> Self {
        Self::new(Backend::default())
    }
}

impl<Backend: AtlasBackend> AtlasState<Backend> {
    pub fn get_or_insert_with<'a>(
        &mut self,
        key: AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        self.get_or_insert_with_diagnostics(key, build)
            .map(|access| access.tile)
    }

    pub fn get_or_insert_with_diagnostics<'a>(
        &mut self,
        key: AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<AtlasAccess> {
        match self.tiles_by_key.entry(key) {
            Entry::Occupied(entry) => {
                let tile = *entry.get();
                Ok(AtlasAccess {
                    tile: Some(tile),
                    diagnostic: AtlasAccessDiagnostic::new(
                        entry.key(),
                        AtlasAccessOutcome::Hit,
                        Some(tile),
                        Some(tile.bounds.size),
                    ),
                })
            }
            Entry::Vacant(entry) => {
                profiling::scope!("new tile");
                let Some((size, bytes)) = build()? else {
                    return Ok(AtlasAccess {
                        tile: None,
                        diagnostic: AtlasAccessDiagnostic::new(
                            entry.key(),
                            AtlasAccessOutcome::Unavailable,
                            None,
                            None,
                        ),
                    });
                };
                let tile = self
                    .backend
                    .insert(entry.key().texture_kind(), size, &bytes)?;
                let diagnostic = AtlasAccessDiagnostic::new(
                    entry.key(),
                    AtlasAccessOutcome::Inserted,
                    Some(tile),
                    Some(size),
                );
                entry.insert(tile);
                Ok(AtlasAccess {
                    tile: Some(tile),
                    diagnostic,
                })
            }
        }
    }

    pub fn remove(&mut self, key: &AtlasKey) {
        if let Some(tile) = self.tiles_by_key.remove(key) {
            self.backend.remove(tile);
        }
    }

    pub fn remove_with_diagnostics(&mut self, key: &AtlasKey) -> AtlasRemoveDiagnostic {
        match self.tiles_by_key.remove(key) {
            Some(tile) => AtlasRemoveDiagnostic::new(
                key,
                self.backend.remove_with_diagnostics(tile),
                Some(tile.texture_id),
            ),
            None => AtlasRemoveDiagnostic::new(key, AtlasRemoveOutcome::RemoveNoop, None),
        }
    }
}

#[cfg(test)]
mod tests;
