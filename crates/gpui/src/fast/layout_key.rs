use crate::{ElementId, collections::FxHasher};
use smallvec::SmallVec;
use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) struct LayoutKey(u64);

impl LayoutKey {
    const ROOT: Self = Self(0);

    fn child(self, step: impl Hash) -> Self {
        let mut hasher = FxHasher::default();
        (self, step).hash(&mut hasher);
        Self(hasher.finish())
    }
}

#[derive(Hash)]
enum KeyStep<'a> {
    Identified(&'a ElementId),
    Positional(usize),
    Request(usize),
    Prepaint,
}

struct Scope {
    key: LayoutKey,
    next_positional: usize,
    next_request: usize,
}

impl Scope {
    fn new(key: LayoutKey) -> Self {
        Self {
            key,
            next_positional: 0,
            next_request: 0,
        }
    }
}

pub(crate) struct LayoutKeys {
    root: Scope,
    stack: SmallVec<[Scope; 32]>,
}

impl Default for LayoutKeys {
    fn default() -> Self {
        Self {
            root: Scope::new(LayoutKey::ROOT),
            stack: SmallVec::new(),
        }
    }
}

impl LayoutKeys {
    pub(crate) fn begin_frame(&mut self) {
        self.root = Scope::new(LayoutKey::ROOT);
        self.stack.clear();
    }

    pub(crate) fn end_frame(&mut self) {
        debug_assert!(self.stack.is_empty());
        self.begin_frame();
    }

    pub(crate) fn push(&mut self, id: Option<&ElementId>) -> LayoutKey {
        let parent = self.stack.last_mut().unwrap_or(&mut self.root);
        let step = match id {
            Some(id) => KeyStep::Identified(id),
            None => {
                let index = parent.next_positional;
                parent.next_positional += 1;
                KeyStep::Positional(index)
            }
        };
        let key = parent.key.child(step);
        self.push_existing(key);
        key
    }

    pub(crate) fn push_existing(&mut self, key: LayoutKey) {
        self.stack.push(Scope::new(key));
    }

    pub(crate) fn push_prepaint(&mut self, element: LayoutKey) {
        self.push_existing(element.child(KeyStep::Prepaint));
    }

    pub(crate) fn pop(&mut self) {
        let scope = self.stack.pop();
        debug_assert!(scope.is_some());
    }

    pub(crate) fn current(&self) -> Option<LayoutKey> {
        self.stack.last().map(|scope| scope.key)
    }

    pub(crate) fn next_request(&mut self) -> Option<LayoutKey> {
        let key = self.current()?;
        let scope = self.stack.last_mut().expect("layout key has a scope");
        let index = scope.next_request;
        scope.next_request += 1;
        Some(key.child(KeyStep::Request(index)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identified_children_keep_keys_when_siblings_move() {
        let mut keys = LayoutKeys::default();
        let first = ElementId::from("first");
        let second = ElementId::from("second");
        let parent = keys.push(None);
        keys.push(Some(&first));
        let first_key = keys.next_request();
        keys.pop();
        keys.push(None);
        let positional = keys.next_request();
        keys.pop();
        keys.pop();
        keys.end_frame();

        assert_eq!(keys.push(None), parent);
        keys.push(Some(&second));
        assert_ne!(keys.next_request(), first_key);
        keys.pop();
        keys.push(None);
        assert_eq!(keys.next_request(), positional);
        keys.pop();
        keys.push(Some(&first));
        assert_eq!(keys.next_request(), first_key);
        keys.pop();
        keys.pop();

        keys.push(None);
        keys.push(Some(&first));
        assert_ne!(keys.next_request(), first_key);
    }

    #[test]
    fn request_slots_and_prepaint_children_use_distinct_keys() {
        let mut keys = LayoutKeys::default();
        let element = keys.push(None);
        let first = keys.next_request();
        let second = keys.next_request();
        assert_ne!(first, second);
        keys.push(None);
        let child = keys.next_request();
        keys.pop();
        keys.pop();

        keys.push_prepaint(element);
        keys.push(None);
        let prepaint_child = keys.next_request();
        assert_ne!(prepaint_child, child);
        keys.pop();
        keys.pop();

        assert_eq!(keys.next_request(), None);
        keys.push_existing(element);
        assert_eq!(keys.next_request(), first);
        keys.push(None);
        assert_eq!(keys.next_request(), child);
        keys.pop();
        keys.pop();

        keys.begin_frame();
        assert_eq!(keys.push(None), element);
        assert_eq!(keys.next_request(), first);
        assert_eq!(keys.next_request(), second);
    }
}
