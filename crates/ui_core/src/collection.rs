//! Renderer-neutral collection metadata for composite component surfaces.

/// Position metadata for a collection item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CollectionPosition {
    index: usize,
    count: usize,
}

impl CollectionPosition {
    /// Creates position metadata from a zero-based index and total item count.
    pub const fn new(index: usize, count: usize) -> Self {
        Self { index, count }
    }

    /// Returns the zero-based index.
    pub const fn index(self) -> usize {
        self.index
    }

    /// Returns the total item count.
    pub const fn count(self) -> usize {
        self.count
    }

    /// Returns the 1-based position announced by assistive technology.
    pub const fn pos_in_set(self) -> usize {
        self.index.saturating_add(1)
    }

    /// Returns the collection size announced by assistive technology.
    pub const fn set_size(self) -> usize {
        self.count
    }

    /// Returns whether the index is inside the declared set size.
    pub const fn is_valid(self) -> bool {
        self.index < self.count
    }
}

#[cfg(test)]
mod tests {
    use super::CollectionPosition;

    #[test]
    fn collection_position_uses_one_based_set_metadata() {
        let position = CollectionPosition::new(2, 4);

        assert_eq!(position.index(), 2);
        assert_eq!(position.count(), 4);
        assert_eq!(position.pos_in_set(), 3);
        assert_eq!(position.set_size(), 4);
        assert!(position.is_valid());
    }
}
