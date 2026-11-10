use super::*;
use std::hash::{Hash, Hasher};

fn path(depth: usize, leaf: u64) -> Vec<ElementId> {
    let mut ids: Vec<ElementId> = (0..depth as u64)
        .map(|level| ElementId::NamedInteger("some-container-element".into(), level))
        .collect();
    ids.push(ElementId::NamedInteger("leaf".into(), leaf));
    ids
}

fn fx_hash(value: &impl Hash) -> u64 {
    let mut hasher = open_gpui_collections::FxHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn equal_paths_are_equal_and_hash_alike() {
    let a = GlobalElementId::new(&path(8, 1));
    let b = GlobalElementId::new(&path(8, 1));
    let c = GlobalElementId::new(&path(8, 2));
    let shallower = GlobalElementId::new(&path(7, 1));
    assert_eq!(a, b);
    assert_eq!(fx_hash(&a), fx_hash(&b));
    assert_eq!(a.accesskit_node_id(), b.accesskit_node_id());
    assert_ne!(a, c);
    assert_ne!(a, shallower);
    assert_ne!(fx_hash(&a), fx_hash(&c));
    assert_eq!(&*a, path(8, 1).as_slice());
    assert_eq!(a.to_string(), b.to_string());
    assert_eq!(GlobalElementId::default(), GlobalElementId::new(&[]));
}
