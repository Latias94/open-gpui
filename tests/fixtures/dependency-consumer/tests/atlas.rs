use open_gpui::{
    AtlasAccessOutcome, AtlasKey, AtlasRemoveOutcome, AtlasTextureLeaseError, DevicePixels,
    HeadlessAtlas, ImageId, PlatformAtlas, RenderImageParams, size,
};
use std::{borrow::Cow, cell::Cell, sync::Arc};

#[test]
fn downstream_headless_leases_keep_removed_instances_alive_until_final_drop() {
    let headless = Arc::new(HeadlessAtlas::default());
    let atlas: Arc<dyn PlatformAtlas> = headless.clone();
    let image = RenderImageParams {
        image_id: ImageId(73),
        frame_index: 3,
    };
    let key = AtlasKey::from(image);
    let pixel_size = size(DevicePixels(1), DevicePixels(1));
    let builds = Cell::new(0);
    let mut build = || {
        builds.set(builds.get() + 1);
        Ok(Some((
            pixel_size,
            Cow::Borrowed([17, 34, 51, 255].as_slice()),
        )))
    };

    let inserted = headless
        .get_or_insert_with_diagnostics(key.clone(), &mut build)
        .expect("an ordinary dependency must be able to insert into the shared headless atlas");
    let original = inserted.tile.expect("the builder must produce a tile");
    let old_instance = original.texture_instance();
    let epoch = atlas.atlas_texture_lease_epoch();
    assert_eq!(inserted.diagnostic.outcome, AtlasAccessOutcome::Inserted);
    assert_eq!(inserted.diagnostic.image, Some(image));
    assert_eq!(original.bounds.size, pixel_size);
    assert_ne!(old_instance.generation, 0);
    assert_eq!(original.texture_generation_padding, 0);

    let original_lease = Arc::clone(&atlas)
        .retain_texture_instances(&[old_instance, old_instance])
        .expect("the public lease API must retain the concrete headless texture");
    assert_eq!(original_lease.texture_instances(), &[old_instance]);
    assert_eq!(original_lease.epoch(), epoch);

    let removed = headless.remove_with_diagnostics(&key);
    assert_eq!(removed.outcome, AtlasRemoveOutcome::TextureRetained);
    assert_eq!(removed.image, Some(image));
    assert_eq!(removed.texture_id, Some(original.texture_id));
    let missing = atlas.remove_with_diagnostics(&key);
    assert_eq!(missing.outcome, AtlasRemoveOutcome::RemoveNoop);
    assert_eq!(missing.texture_id, None);
    let after_removal_lease = Arc::clone(&atlas)
        .retain_texture_instances(&[old_instance])
        .expect("a live lease must keep the removed-key instance available to new leases");

    let replacement = atlas
        .get_or_insert_with(key.clone(), &mut build)
        .expect("the removed key must be insertable through the owned public API")
        .expect("reinsertion must publish a replacement tile");
    let replacement_instance = replacement.texture_instance();
    assert_ne!(replacement_instance, old_instance);
    assert_eq!(replacement.bounds.size, pixel_size);
    assert_eq!(replacement.texture_generation_padding, 0);
    assert_eq!(builds.get(), 2);
    assert_eq!(atlas.atlas_texture_lease_epoch(), epoch);

    drop(original_lease);
    let final_old_lease = Arc::clone(&atlas)
        .retain_texture_instances(&[old_instance])
        .expect("dropping the first lease must preserve the other lease's old instance");
    drop(after_removal_lease);
    final_old_lease
        .validate()
        .expect("removal and reinsertion must not invalidate the surviving lease's epoch");
    drop(final_old_lease);
    let refused = Arc::clone(&atlas).retain_texture_instances(&[old_instance]);
    assert!(
        matches!(
            refused,
            Err(AtlasTextureLeaseError::TextureUnavailable { texture, epoch: actual_epoch })
                if texture == old_instance && actual_epoch == epoch
        ),
        "the old instance must refuse acquisition after its final actual lease drops"
    );

    let cached = atlas
        .get_or_insert_with_diagnostics(key.clone(), &mut || {
            panic!("the replacement key must stay cached when the old instance is released")
        })
        .expect("releasing the old instance must preserve the replacement key");
    assert_eq!(cached.tile, Some(replacement));
    assert_eq!(cached.diagnostic.outcome, AtlasAccessOutcome::Hit);
    assert_eq!(cached.diagnostic.image, Some(image));
    assert_eq!(cached.diagnostic.texture_id, Some(replacement.texture_id));
    assert_eq!(builds.get(), 2);
    let replacement_lease = Arc::clone(&atlas)
        .retain_texture_instances(&[replacement_instance])
        .expect("the replacement instance must remain usable after the old instance is freed");
    assert_eq!(
        replacement_lease.texture_instances(),
        &[replacement_instance]
    );
    replacement_lease
        .validate()
        .expect("the replacement's lease must remain in the original atlas epoch");
    drop(replacement_lease);

    let replacement_removed = atlas.remove_with_diagnostics(&key);
    assert_eq!(
        replacement_removed.outcome,
        AtlasRemoveOutcome::TextureFreed
    );
    assert_eq!(replacement_removed.texture_id, Some(replacement.texture_id));
}
