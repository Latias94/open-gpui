use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn insert(map: &mut EntityMap, value: usize) -> Entity<usize> {
    let slot = map.reserve();
    map.insert(slot, value)
}

#[test]
fn scoped_reads_include_previously_read_entities_and_merge_nested_scopes() {
    let mut map = EntityMap::new();
    let first = insert(&mut map, 1);
    let second = insert(&mut map, 2);
    map.take_accessed();

    map.read(&first);
    let outer = map.access_scope();
    map.read(&first);
    let inner = map.access_scope();
    map.read(&second);
    assert_eq!(inner.finish(), FxHashSet::from_iter([second.entity_id()]));
    let both = FxHashSet::from_iter([first.entity_id(), second.entity_id()]);
    assert_eq!(outer.finish(), both);
    assert_eq!(map.take_accessed(), both);

    drop(second);
    map.take_dropped();
    map.extend_accessed(&both);
    assert_eq!(
        map.take_accessed(),
        FxHashSet::from_iter([first.entity_id()])
    );
}

#[test]
fn cached_reads_keep_leased_entities_without_reviving_released_generations() {
    let mut map = EntityMap::new();
    let value = insert(&mut map, 42);
    let recorded = map.take_accessed();
    let lease = map.lease(&value);
    map.take_accessed();
    map.extend_accessed(&recorded);
    assert_eq!(map.take_accessed(), recorded);
    map.end_lease(lease);

    let old_id = value.entity_id();
    drop(value);
    map.take_dropped();
    let replacement = insert(&mut map, 99);
    assert_ne!(old_id, replacement.entity_id());
    map.take_accessed();
    map.extend_accessed(&recorded);
    assert!(map.take_accessed().is_empty());
}

#[test]
fn nested_draws_isolate_reads_and_restore_an_active_outer_scope() {
    let mut map = EntityMap::new();
    let first = insert(&mut map, 1);
    let second = insert(&mut map, 2);
    let third = insert(&mut map, 3);
    let outer_frame = map.frame_access_scope();
    let outer = map.access_scope();
    map.read(&first);
    {
        let _inner_frame = map.frame_access_scope();
        let inner = map.access_scope();
        map.read(&second);
        assert_eq!(inner.finish(), FxHashSet::from_iter([second.entity_id()]));
        assert_eq!(
            map.take_accessed(),
            FxHashSet::from_iter([second.entity_id()])
        );
    }
    map.read(&third);
    let outer_reads = FxHashSet::from_iter([first.entity_id(), third.entity_id()]);
    assert_eq!(outer.finish(), outer_reads);
    assert_eq!(map.take_accessed(), outer_reads);
    drop(outer_frame);
    assert_eq!(map.accessed_entities.borrow().active_frame, 0);
}

#[test]
fn nested_draw_unwind_restores_outer_reads_and_balances_scopes() {
    let mut map = EntityMap::new();
    let first = insert(&mut map, 1);
    let second = insert(&mut map, 2);
    let _outer_frame = map.frame_access_scope();
    let outer = map.access_scope();
    map.read(&first);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _inner_frame = map.frame_access_scope();
            let _inner_scope = map.access_scope();
            map.read(&second);
            panic!("abandoned nested draw");
        }))
        .is_err()
    );
    map.read(&first);
    let expected = FxHashSet::from_iter([first.entity_id()]);
    assert_eq!(outer.finish(), expected);
    assert_eq!(map.take_accessed(), expected);
    let next = map.access_scope();
    map.read(&second);
    assert_eq!(next.finish(), FxHashSet::from_iter([second.entity_id()]));
}

#[test]
fn releasing_an_entity_in_a_nested_draw_removes_saved_outer_reads() {
    let mut map = EntityMap::new();
    let value = insert(&mut map, 42);
    let _outer_frame = map.frame_access_scope();
    let outer = map.access_scope();
    map.read(&value);
    {
        let _inner_frame = map.frame_access_scope();
        drop(value);
        map.take_dropped();
    }
    assert!(outer.finish().is_empty());
    assert!(map.take_accessed().is_empty());
}

#[test]
fn frame_context_storage_is_reused_after_completed_draws() {
    let mut map = EntityMap::new();
    let value = insert(&mut map, 42);
    for _ in 0..64 {
        let _outer_frame = map.frame_access_scope();
        let _inner_frame = map.frame_access_scope();
        map.read(&value);
        assert_eq!(
            map.take_accessed(),
            FxHashSet::from_iter([value.entity_id()])
        );
    }
    let accesses = map.accessed_entities.borrow();
    assert_eq!(accesses.active_frame, 0);
    assert_eq!(accesses.frames.len(), 3);
}
