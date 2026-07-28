use std::{any::TypeId, cell::RefCell};

use open_gpui_collections::FxHashMap;

use crate::{WindowControlArea, WindowId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeWindowLifecycle {
    Reserved,
    Committed,
}

#[derive(Default)]
pub(super) struct NativeQuerySnapshots {
    windows: RefCell<FxHashMap<WindowId, NativeWindowQuerySnapshot>>,
    app_menu_actions: RefCell<FxHashMap<TypeId, bool>>,
}

impl NativeQuerySnapshots {
    pub(super) fn reserve_window(&self, window_id: WindowId) {
        let previous = self
            .windows
            .borrow_mut()
            .insert(window_id, NativeWindowQuerySnapshot::reserved());
        assert!(
            previous.is_none(),
            "native window {window_id:?} was reserved more than once"
        );
    }

    pub(super) fn commit_window(&self, window_id: WindowId) {
        let mut windows = self.windows.borrow_mut();
        let snapshot = windows
            .get_mut(&window_id)
            .unwrap_or_else(|| panic!("native window {window_id:?} was committed without reserve"));
        assert_eq!(
            snapshot.lifecycle,
            NativeWindowLifecycle::Reserved,
            "native window {window_id:?} was committed more than once"
        );
        snapshot.lifecycle = NativeWindowLifecycle::Committed;
    }

    pub(super) fn remove_window(&self, window_id: WindowId) {
        self.windows.borrow_mut().remove(&window_id);
    }

    pub(super) fn clear(&self) {
        self.windows.borrow_mut().clear();
    }

    pub(super) fn lookup(&self, window_id: WindowId) -> Option<NativeWindowQuerySnapshot> {
        self.windows.borrow().get(&window_id).copied()
    }

    pub(super) fn committed(&self, window_id: WindowId) -> Option<NativeWindowQuerySnapshot> {
        self.lookup(window_id)
            .filter(|snapshot| snapshot.lifecycle == NativeWindowLifecycle::Committed)
    }

    pub(super) fn set_window_control_area(
        &self,
        window_id: WindowId,
        area: Option<WindowControlArea>,
    ) {
        if let Some(snapshot) = self.windows.borrow_mut().get_mut(&window_id) {
            snapshot.window_control_area = area;
        }
    }

    pub(super) fn window_control_area(&self, window_id: WindowId) -> Option<WindowControlArea> {
        self.committed(window_id)
            .and_then(NativeWindowQuerySnapshot::window_control_area)
    }

    pub(super) fn commit_app_menu_action_availability(&self, action_type: TypeId, available: bool) {
        self.app_menu_actions
            .borrow_mut()
            .insert(action_type, available);
    }

    pub(super) fn app_menu_action_available(&self, action_type: TypeId) -> bool {
        self.app_menu_actions
            .borrow()
            .get(&action_type)
            .copied()
            .unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeWindowQuerySnapshot {
    lifecycle: NativeWindowLifecycle,
    window_control_area: Option<WindowControlArea>,
}

impl NativeWindowQuerySnapshot {
    fn reserved() -> Self {
        Self {
            lifecycle: NativeWindowLifecycle::Reserved,
            window_control_area: None,
        }
    }

    pub(super) fn lifecycle(self) -> NativeWindowLifecycle {
        self.lifecycle
    }

    pub(super) fn window_control_area(self) -> Option<WindowControlArea> {
        self.window_control_area
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_id() -> WindowId {
        let mut ids = slotmap::SlotMap::<WindowId, ()>::with_key();
        ids.insert(())
    }

    #[test]
    fn reserved_window_is_visible_to_lookup_but_not_committed_queries() {
        let snapshots = NativeQuerySnapshots::default();
        let window_id = window_id();

        snapshots.reserve_window(window_id);
        snapshots.set_window_control_area(window_id, Some(WindowControlArea::Drag));

        assert_eq!(
            snapshots
                .lookup(window_id)
                .map(|snapshot| snapshot.lifecycle()),
            Some(NativeWindowLifecycle::Reserved)
        );
        assert_eq!(snapshots.committed(window_id), None);
        assert_eq!(snapshots.window_control_area(window_id), None);
    }

    #[test]
    fn commit_preserves_reserved_snapshot_and_enables_committed_queries() {
        let snapshots = NativeQuerySnapshots::default();
        let window_id = window_id();

        snapshots.reserve_window(window_id);
        snapshots.set_window_control_area(window_id, Some(WindowControlArea::Close));
        snapshots.commit_window(window_id);

        let committed = snapshots
            .committed(window_id)
            .expect("committed window must have a query snapshot");
        assert_eq!(committed.lifecycle(), NativeWindowLifecycle::Committed);
        assert_eq!(
            committed.window_control_area(),
            Some(WindowControlArea::Close)
        );
        assert_eq!(
            snapshots.window_control_area(window_id),
            Some(WindowControlArea::Close)
        );
    }

    #[test]
    fn remove_and_clear_make_windows_absent() {
        let snapshots = NativeQuerySnapshots::default();
        let mut ids = slotmap::SlotMap::<WindowId, ()>::with_key();
        let removed = ids.insert(());
        let cleared = ids.insert(());

        snapshots.reserve_window(removed);
        snapshots.commit_window(removed);
        snapshots.reserve_window(cleared);
        snapshots.commit_window(cleared);

        snapshots.remove_window(removed);
        assert_eq!(snapshots.lookup(removed), None);
        assert!(snapshots.committed(cleared).is_some());

        snapshots.clear();
        assert_eq!(snapshots.lookup(cleared), None);
    }

    #[test]
    #[should_panic(expected = "was committed without reserve")]
    fn commit_without_reserve_is_rejected() {
        NativeQuerySnapshots::default().commit_window(window_id());
    }

    #[test]
    #[should_panic(expected = "was reserved more than once")]
    fn duplicate_reserve_is_rejected() {
        let snapshots = NativeQuerySnapshots::default();
        let window_id = window_id();
        snapshots.reserve_window(window_id);
        snapshots.reserve_window(window_id);
    }

    #[test]
    #[should_panic(expected = "was committed more than once")]
    fn duplicate_commit_is_rejected() {
        let snapshots = NativeQuerySnapshots::default();
        let window_id = window_id();
        snapshots.reserve_window(window_id);
        snapshots.commit_window(window_id);
        snapshots.commit_window(window_id);
    }

    #[test]
    fn app_menu_action_queries_use_the_last_committed_value() {
        struct Available;
        struct Unavailable;

        let snapshots = NativeQuerySnapshots::default();
        assert!(!snapshots.app_menu_action_available(TypeId::of::<Available>()));

        snapshots.commit_app_menu_action_availability(TypeId::of::<Available>(), true);
        snapshots.commit_app_menu_action_availability(TypeId::of::<Unavailable>(), false);

        assert!(snapshots.app_menu_action_available(TypeId::of::<Available>()));
        assert!(!snapshots.app_menu_action_available(TypeId::of::<Unavailable>()));
    }
}
