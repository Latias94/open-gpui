use std::{
    any::TypeId,
    cell::{Cell, RefCell},
    rc::Rc,
};

use crate::{
    App, Capslock, EntityId, GlobalElementId, HitboxId, Modifiers, Pixels, Point, Window,
    collections::FxHashMap,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RevisionKey {
    Entity(EntityId),
    Global(TypeId),
}

#[derive(Default)]
pub(crate) struct Revisions {
    generation: u64,
    values: FxHashMap<RevisionKey, u64>,
}

impl Revisions {
    fn get(&self, key: RevisionKey) -> u64 {
        self.values.get(&key).copied().unwrap_or(0)
    }

    pub(crate) fn changed(&mut self, key: RevisionKey) {
        self.generation = self.generation.wrapping_add(1);
        self.values.insert(key, self.generation);
    }

    pub(crate) fn release(&mut self, entity: EntityId) {
        self.values.remove(&RevisionKey::Entity(entity));
    }

    pub(crate) fn read(&self, key: RevisionKey) {
        record(|| Read::Revision(key, self.get(key)));
    }
}

enum Read {
    Revision(RevisionKey, u64),
    GlobalPresence(TypeId, bool),
    State(Box<dyn Fn() -> bool>),
    Input(InputRead),
    RequiresFreshDraw,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum InputRead {
    Position(Point<Pixels>),
    Modifiers(Modifiers),
    Capslock(Capslock),
    MouseInWindow(bool),
    Hover(HitboxId, bool),
}

#[derive(Default)]
pub(crate) struct Reads(Vec<Read>);

impl Reads {
    pub(crate) fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    pub(crate) fn is_current(&self, window: &Window, cx: &App) -> bool {
        self.0.iter().all(|read| match read {
            Read::Revision(key, revision) => cx.retained_revisions.get(*key) == *revision,
            Read::GlobalPresence(key, present) => cx.globals_by_type.contains_key(key) == *present,
            Read::State(current) => current(),
            Read::Input(input) => window.retained_input_matches(*input),
            Read::RequiresFreshDraw => false,
        })
    }

    pub(crate) fn input_is_current(&self, window: &Window) -> bool {
        self.0.iter().all(|read| match read {
            Read::Input(input) => window.retained_input_matches(*input),
            _ => true,
        })
    }

    pub(crate) fn global_changed(&self, key: TypeId, present: bool) -> bool {
        self.0.iter().any(|read| match read {
            Read::Revision(RevisionKey::Global(read), _) => *read == key,
            Read::GlobalPresence(read, before) => *read == key && *before != present,
            _ => false,
        })
    }
}

pub(crate) type ElementStateKey = (GlobalElementId, TypeId);

#[derive(Default)]
struct Capture {
    reads: Vec<Read>,
    layout_states: Option<Vec<ElementStateKey>>,
}

type ReadLog = Rc<RefCell<Capture>>;

// Some scroll-state APIs have no App argument. Scoped recording attaches those reads
// to the current view while preserving the enclosing view across nested draws.
thread_local! {
    static CURRENT_READS: RefCell<Option<ReadLog>> = const { RefCell::new(None) };
    static RENDER_RECEIVER: Cell<Option<EntityId>> = const { Cell::new(None) };
}

struct ReadScope(Option<ReadLog>);

impl Drop for ReadScope {
    fn drop(&mut self) {
        CURRENT_READS.with(|current| current.replace(self.0.take()));
    }
}

pub(crate) fn capture<R>(f: impl FnOnce() -> R) -> (R, Reads) {
    let (result, reads, _) = capture_reads(f, None);
    (result, reads)
}

pub(crate) fn capture_layout<R>(f: impl FnOnce() -> R) -> (R, Reads, Vec<ElementStateKey>) {
    let (result, reads, layout_states) = capture_reads(f, Some(Vec::new()));
    (result, reads, layout_states.unwrap_or_default())
}

fn capture_reads<R>(
    f: impl FnOnce() -> R,
    layout_states: Option<Vec<ElementStateKey>>,
) -> (R, Reads, Option<Vec<ElementStateKey>>) {
    let log = Rc::new(RefCell::new(Capture {
        reads: Vec::new(),
        layout_states,
    }));
    let scope = ReadScope(CURRENT_READS.with(|current| current.replace(Some(log.clone()))));
    let result = f();
    drop(scope);
    let Capture {
        reads,
        layout_states,
    } = std::mem::take(&mut *log.borrow_mut());
    (result, Reads(reads), layout_states)
}

fn record(read: impl FnOnce() -> Read) {
    CURRENT_READS.with(|current| {
        if let Some(log) = current.borrow().as_ref() {
            log.borrow_mut().reads.push(read());
        }
    });
}

pub(crate) fn element_state(key: &ElementStateKey) -> bool {
    CURRENT_READS.with(|current| {
        if let Some(log) = current.borrow().as_ref()
            && let Some(states) = &mut log.borrow_mut().layout_states
        {
            states.push(key.clone());
            true
        } else {
            false
        }
    })
}

pub(crate) fn requires_fresh_draw() {
    record(|| Read::RequiresFreshDraw);
}

pub(crate) fn global_presence(key: TypeId, present: bool) {
    record(|| Read::GlobalPresence(key, present));
}

pub(crate) fn state(current: impl FnOnce() -> Box<dyn Fn() -> bool>) {
    record(|| Read::State(current()));
}

pub(crate) fn input(input: InputRead) {
    CURRENT_READS.with(|current| {
        if let Some(log) = current.borrow().as_ref() {
            let mut reads = log.borrow_mut();
            if !reads
                .reads
                .iter()
                .any(|read| matches!(read, Read::Input(previous) if *previous == input))
            {
                reads.reads.push(Read::Input(input));
            }
        }
    });
}

pub(crate) fn render_entity<R>(entity: EntityId, f: impl FnOnce() -> R) -> R {
    struct RenderScope(Option<EntityId>);
    impl Drop for RenderScope {
        fn drop(&mut self) {
            RENDER_RECEIVER.with(|receiver| receiver.set(self.0));
        }
    }

    let _scope = RenderScope(RENDER_RECEIVER.with(|receiver| receiver.replace(Some(entity))));
    f()
}

pub(crate) fn takes_render_receiver(entity: EntityId) -> bool {
    RENDER_RECEIVER.with(|receiver| {
        if receiver.get() != Some(entity) {
            return false;
        }
        receiver.set(None);
        true
    })
}
