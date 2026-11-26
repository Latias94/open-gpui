pub type HashMap<K, V> = FxHashMap<K, V>;
pub type HashSet<T> = FxHashSet<T>;
pub type TypeIdHashMap<V> = std::collections::HashMap<std::any::TypeId, V, TypeIdHashBuilder>;
pub type TypeIdHashSet = std::collections::HashSet<std::any::TypeId, TypeIdHashBuilder>;

pub use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet, FxHasher};
pub use std::collections::{BTreeMap, VecDeque, hash_map};

pub mod vecmap;
#[cfg(test)]
mod vecmap_tests;

#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeIdHashBuilder;

impl std::hash::BuildHasher for TypeIdHashBuilder {
    type Hasher = TypeIdHasher;

    fn build_hasher(&self) -> Self::Hasher {
        TypeIdHasher::default()
    }
}

#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeIdHasher {
    value: u64,
}

impl std::hash::Hasher for TypeIdHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // TypeId should only hash its first 8 bytes
        if let Some(bytes) = bytes.first_chunk::<8>() {
            self.value = u64::from_ne_bytes(*bytes);
        } else {
            crate::util::debug_panic!(
                "expected a 64-bit value, did you use this hasher with something other than a TypeId?"
            );
        }
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.value
    }
}

#[test]
fn type_id_hasher_uses_exactly_the_first_eight_bytes() {
    use core::hash::Hasher;
    let bytes = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99];
    let expected = if cfg!(target_endian = "little") {
        0x8877665544332211
    } else {
        0x1122334455667788
    };
    for len in [8, 9] {
        let mut hasher = TypeIdHasher::default();
        hasher.write(&bytes[..len]);
        assert_eq!(hasher.finish(), expected);
    }
}

#[test]
fn type_id_hasher() {
    use core::any::TypeId;
    use core::hash::{Hash, Hasher};
    fn verify_hashing_with(type_id: TypeId) {
        let mut hasher = TypeIdHasher::default();
        type_id.hash(&mut hasher);
        assert_ne!(hasher.finish(), 0);
    }
    // Pick a variety of types, just to demonstrate it’s all sane. Normal, zero-sized, unsized, &c.
    verify_hashing_with(TypeId::of::<usize>());
    verify_hashing_with(TypeId::of::<()>());
    verify_hashing_with(TypeId::of::<str>());
    verify_hashing_with(TypeId::of::<&str>());
    verify_hashing_with(TypeId::of::<Vec<u8>>());
}
