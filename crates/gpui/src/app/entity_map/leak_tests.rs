use super::{AnyWeakEntity, Entity, EntityMap, LeakDetectorSnapshot};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn insert<T: 'static>(map: &mut EntityMap, value: T) -> Entity<T> {
    let slot = map.reserve();
    map.insert(slot, value)
}

fn assert_panics_with(f: impl FnOnce(), expected: &str) {
    let payload = catch_unwind(AssertUnwindSafe(f)).expect_err("operation should panic");
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .expect("panic payload should be a string");
    assert!(
        message.contains(expected),
        "expected panic containing {expected:?}, got {message:?}"
    );
}

struct DropCount(Rc<Cell<usize>>);

impl Drop for DropCount {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn clones_and_erased_conversions_track_distinct_strong_handles() {
    let mut map = EntityMap::new();
    let snapshot = map.leak_detector_snapshot();
    let entity = insert(&mut map, 42usize);
    let weak = entity.downgrade();
    let cloned = entity.clone();
    let cloned_handle_id = cloned.handle_id;
    let erased = cloned.into_any();
    assert_eq!(erased.handle_id, cloned_handle_id);
    let erased = erased
        .downcast::<u32>()
        .expect_err("a failed downcast must return the original handle");
    assert_eq!(erased.handle_id, cloned_handle_id);
    let erased_clone = erased.clone();
    let last_handle_id = erased_clone.handle_id;
    let last = erased_clone.downcast::<usize>().unwrap();
    assert_eq!(last.handle_id, last_handle_id);
    assert_ne!(entity.handle_id, cloned_handle_id);
    assert_ne!(entity.handle_id, last_handle_id);
    assert_ne!(cloned_handle_id, last_handle_id);
    {
        let ref_counts = map.ref_counts.read();
        let data = &ref_counts.leak_detector.entity_handles[&entity.entity_id()];
        assert_eq!(data.type_name, "usize");
        assert_eq!(data.handles.len(), 3);
        for handle_id in [entity.handle_id, cloned_handle_id, last_handle_id] {
            assert!(data.handles.contains_key(&handle_id));
        }
    }

    drop(entity);
    drop(erased);
    assert_eq!(*map.read(&last), 42);
    assert!(weak.is_upgradable());
    assert!(map.take_dropped().is_empty());
    assert_panics_with(
        || map.assert_no_new_leaks(&snapshot),
        "New entity leaks detected since snapshot",
    );

    drop(last);
    assert!(!weak.is_upgradable());
    assert!(weak.upgrade().is_none());
    map.assert_no_new_leaks(&snapshot);
    let dropped = map.take_dropped();
    assert_eq!(dropped.len(), 1);
    drop(dropped);
    weak.assert_released();
}

#[test]
fn weak_handles_track_only_successful_upgrades() {
    let mut map = EntityMap::new();
    let snapshot = map.leak_detector_snapshot();
    let entity = insert(&mut map, 42usize);
    let weak = entity.downgrade();
    let erased_weak = AnyWeakEntity::from(weak.clone());
    let erased_weak_clone = erased_weak.clone();
    assert_eq!(
        map.ref_counts.read().leak_detector.entity_handles[&entity.entity_id()]
            .handles
            .len(),
        1
    );

    let typed_upgrade = weak.upgrade().unwrap();
    let erased_upgrade = erased_weak_clone.upgrade().unwrap();
    assert_ne!(entity.handle_id, typed_upgrade.handle_id);
    assert_ne!(entity.handle_id, erased_upgrade.handle_id);
    assert_ne!(typed_upgrade.handle_id, erased_upgrade.handle_id);
    assert_eq!(
        map.ref_counts.read().leak_detector.entity_handles[&entity.entity_id()]
            .handles
            .len(),
        3
    );

    drop(entity);
    drop(typed_upgrade);
    assert!(weak.is_upgradable());
    drop(erased_upgrade);
    assert!(!weak.is_upgradable());
    assert!(weak.upgrade().is_none());
    assert!(erased_weak.upgrade().is_none());
    map.assert_no_new_leaks(&snapshot);
    let dropped = map.take_dropped();
    assert_eq!(dropped.len(), 1);
    drop(dropped);
    assert!(erased_weak_clone.upgrade().is_none());
    weak.assert_released();
    erased_weak.assert_released();
}

#[test]
fn assert_released_rejects_live_handles_and_pending_cleanup_after_diagnostics() {
    let mut map = EntityMap::new();
    let entity = insert(&mut map, 42usize);
    let weak = entity.downgrade();
    let retained = entity.clone().into_any();
    let erased_weak = retained.downgrade();
    drop(entity);

    assert_panics_with(|| weak.assert_released(), "Handles for usize leaked");
    assert!(erased_weak.is_upgradable());
    assert_panics_with(|| erased_weak.assert_released(), "Handles for usize leaked");
    assert!(map.take_dropped().is_empty());

    drop(retained);
    assert!(!weak.is_upgradable());
    assert!(erased_weak.upgrade().is_none());
    assert_panics_with(|| weak.assert_released(), "resources are retained");
    let dropped = map.take_dropped();
    assert_eq!(dropped.len(), 1);
    drop(dropped);
    weak.assert_released();
    erased_weak.assert_released();
}

#[test]
fn failed_release_assertion_preserves_snapshot_leak_detection() {
    let mut map = EntityMap::new();
    let snapshot = LeakDetectorSnapshot::default();
    let entity = insert(&mut map, 42usize);
    let weak = entity.downgrade();

    let release_check = catch_unwind(AssertUnwindSafe(|| weak.assert_released()));
    let snapshot_check = catch_unwind(AssertUnwindSafe(|| map.assert_no_new_leaks(&snapshot)));
    let temporary = weak.upgrade();
    let upgrade_succeeded = temporary.is_some();
    drop(temporary);
    let snapshot_after_upgrade =
        catch_unwind(AssertUnwindSafe(|| map.assert_no_new_leaks(&snapshot)));
    drop(entity);
    drop(map.take_dropped());
    drop(map);

    assert!(
        release_check.is_err(),
        "the release assertion must reject the retained strong handle"
    );
    assert!(
        snapshot_check.is_err(),
        "a failed release assertion must not hide retained handles from snapshot checks"
    );
    assert!(upgrade_succeeded, "the original entity must remain alive");
    assert!(
        snapshot_after_upgrade.is_err(),
        "dropping an upgraded handle must not hide the original retained handle"
    );
}

#[test]
fn assert_released_succeeds_after_cleanup_and_last_owner_drop() {
    let mut map = EntityMap::new();
    let ref_count_owner = map.ref_counts_drop_handle();
    let drops = Rc::new(Cell::new(0));
    let entity = insert(&mut map, DropCount(drops.clone()));
    let weak = entity.downgrade();
    let erased_weak = AnyWeakEntity::from(weak.clone());
    drop(entity);

    let dropped = map.take_dropped();
    assert_eq!(dropped.len(), 1);
    assert_eq!(drops.get(), 0);
    drop(dropped);
    assert_eq!(drops.get(), 1);
    weak.assert_released();
    erased_weak.assert_released();

    drop(map);
    drop(ref_count_owner);
    assert!(erased_weak.entity_ref_counts.upgrade().is_none());
    assert!(weak.upgrade().is_none());
    assert!(erased_weak.upgrade().is_none());
    weak.assert_released();
    erased_weak.assert_released();
}

#[test]
fn snapshots_exempt_existing_entities_but_detect_reused_slot_generations() {
    let mut map = EntityMap::new();
    let empty = LeakDetectorSnapshot::default();
    map.assert_no_new_leaks(&empty);
    let entity = insert(&mut map, 42usize);
    let old_id = entity.entity_id();
    assert_panics_with(
        || map.assert_no_new_leaks(&empty),
        "New entity leaks detected since snapshot",
    );

    let snapshot = map.leak_detector_snapshot();
    let cloned = entity.clone().into_any();
    let weak = entity.downgrade();
    let upgraded = weak.upgrade().unwrap();
    map.assert_no_new_leaks(&snapshot);
    drop(entity);
    drop(cloned);
    drop(upgraded);
    drop(map.take_dropped());
    weak.assert_released();

    let replacement = insert(&mut map, 99usize);
    assert_ne!(old_id, replacement.entity_id());
    assert!(weak.upgrade().is_none());
    weak.assert_released();
    assert_panics_with(
        || map.assert_no_new_leaks(&snapshot),
        "New entity leaks detected since snapshot",
    );
    drop(replacement);
    drop(map.take_dropped());
    map.assert_no_new_leaks(&snapshot);
    map.assert_no_new_leaks(&empty);
}

#[test]
fn uninserted_reservations_require_cleanup_without_leaking_handles() {
    let mut map = EntityMap::new();
    let snapshot = map.leak_detector_snapshot();
    let slot = map.reserve::<usize>();
    let weak = slot.downgrade();
    drop(slot);

    assert!(!weak.is_upgradable());
    assert!(weak.upgrade().is_none());
    map.assert_no_new_leaks(&snapshot);
    assert_panics_with(|| weak.assert_released(), "resources are retained");
    assert!(map.take_dropped().is_empty());
    weak.assert_released();
}

#[test]
fn ref_count_owner_keeps_tracking_alive_after_the_map_is_dropped() {
    let mut map = EntityMap::new();
    let ref_count_owner = map.ref_counts_drop_handle();
    let snapshot = map.leak_detector_snapshot();
    let drops = Rc::new(Cell::new(0));
    let entity = insert(&mut map, DropCount(drops.clone()));
    let weak = entity.downgrade();
    drop(map);
    assert_eq!(drops.get(), 1);

    let cloned = entity.clone().into_any();
    let upgraded = weak.upgrade().unwrap();
    drop(entity);
    drop(cloned);
    assert!(weak.is_upgradable());
    drop(upgraded);
    assert!(!weak.is_upgradable());
    ref_count_owner
        .read()
        .leak_detector
        .assert_no_new_leaks(&snapshot);
    assert_panics_with(|| weak.assert_released(), "resources are retained");
    drop(ref_count_owner);
    assert!(weak.upgrade().is_none());
}

#[test]
fn last_ref_count_owner_reports_handles_outliving_the_map() {
    let mut map = EntityMap::new();
    let ref_count_owner = map.ref_counts_drop_handle();
    let entity = insert(&mut map, 42usize);
    let weak = entity.downgrade();
    drop(map);

    assert_panics_with(|| drop(ref_count_owner), "Exited with leaked handles");
    assert!(!weak.is_upgradable());
    assert!(weak.upgrade().is_none());
    drop(entity);
}

#[test]
fn unwinding_releases_temporary_handles_before_entity_cleanup() {
    let mut map = EntityMap::new();
    let snapshot = map.leak_detector_snapshot();
    let drops = Rc::new(Cell::new(0));
    assert_panics_with(
        || {
            let entity = insert(&mut map, DropCount(drops.clone()));
            let erased = entity.clone().into_any();
            let weak = erased.downgrade();
            let _upgraded = weak.upgrade().unwrap();
            panic!("abandoned handle operation");
        },
        "abandoned handle operation",
    );

    map.assert_no_new_leaks(&snapshot);
    assert_eq!(drops.get(), 0);
    let dropped = map.take_dropped();
    assert_eq!(dropped.len(), 1);
    drop(dropped);
    assert_eq!(drops.get(), 1);
}

#[test]
fn dropping_a_leaking_map_during_unwind_preserves_the_original_panic() {
    let mut survivor = None;
    assert_panics_with(
        || {
            let mut map = EntityMap::new();
            survivor = Some(insert(&mut map, 42usize).into_any());
            panic!("original entity operation panic");
        },
        "original entity operation panic",
    );

    let survivor = survivor.unwrap();
    let weak = survivor.downgrade();
    assert!(!weak.is_upgradable());
    assert!(weak.upgrade().is_none());
    drop(survivor);
}
