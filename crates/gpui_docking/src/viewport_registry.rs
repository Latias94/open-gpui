use crate::DockSpaceId;
use crate::DockViewportIdentity;
use open_gpui::{AnyWindowHandle, Bounds, DisplayId, Pixels, WindowBounds, WindowId};
use std::collections::{BTreeMap, HashMap};

/// Runtime snapshot for one rendered dock viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockViewportSnapshot {
    /// GPUI window currently rendering the logical dock space.
    pub(crate) window: AnyWindowHandle,
    /// Display containing the window, when the application has recorded one.
    pub(crate) display_id: Option<DisplayId>,
    /// Last known platform window bounds in screen coordinates.
    pub(crate) window_bounds: Option<WindowBounds>,
    /// Last known dock host bounds in window-local coordinates.
    pub(crate) host_bounds: Option<Bounds<Pixels>>,
}

impl DockViewportSnapshot {
    /// Creates a snapshot for a newly registered viewport window.
    pub(crate) fn new(window: AnyWindowHandle) -> Self {
        Self {
            window,
            display_id: None,
            window_bounds: None,
            host_bounds: None,
        }
    }

    fn identity(&self, space: &DockSpaceId) -> DockViewportIdentity {
        DockViewportIdentity::new(space.clone(), self.window.window_id())
    }
}

/// Internal one-to-one mapping between logical dock spaces and GPUI windows.
#[derive(Debug, Default)]
pub(crate) struct DockViewportRegistry {
    viewports: BTreeMap<DockSpaceId, DockViewportSnapshot>,
    windows: HashMap<WindowId, DockSpaceId>,
}

impl DockViewportRegistry {
    pub(crate) fn register(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Option<DockViewportSnapshot> {
        self.register_with_replacements(space.clone(), window)
            .into_iter()
            .find(|(removed_space, _)| *removed_space == space)
            .map(|(_, snapshot)| snapshot)
    }

    pub(crate) fn register_with_replacements(
        &mut self,
        space: DockSpaceId,
        window: AnyWindowHandle,
    ) -> Vec<(DockSpaceId, DockViewportSnapshot)> {
        let window_id = window.window_id();
        if let Some(snapshot) = self.viewports.get(&space)
            && snapshot.identity(&space).matches(&space, window_id)
            && self
                .windows
                .get(&window_id)
                .is_none_or(|registered_space| registered_space == &space)
        {
            self.windows.insert(window_id, space);
            return Vec::new();
        }

        let mut replaced = Vec::new();

        if let Some(previous) = self.viewports.remove(&space) {
            self.windows.remove(&previous.window.window_id());
            replaced.push((space.clone(), previous));
        }
        if let Some(previous_space) = self.windows.remove(&window_id)
            && previous_space != space
            && let Some(previous) = self.viewports.remove(&previous_space)
        {
            replaced.push((previous_space, previous));
        }

        self.windows.insert(window_id, space.clone());
        self.viewports
            .insert(space, DockViewportSnapshot::new(window));
        replaced
    }

    pub(crate) fn unregister_space(&mut self, space: &DockSpaceId) -> Option<DockViewportSnapshot> {
        let snapshot = self.viewports.remove(space)?;
        self.windows.remove(&snapshot.window.window_id());
        Some(snapshot)
    }

    pub(crate) fn unregister_window_id(
        &mut self,
        window_id: WindowId,
    ) -> Option<(DockSpaceId, DockViewportSnapshot)> {
        let space = self.windows.remove(&window_id)?;
        if !self
            .viewports
            .get(&space)
            .is_some_and(|snapshot| snapshot.identity(&space).matches(&space, window_id))
        {
            return None;
        }
        let snapshot = self.viewports.remove(&space)?;
        Some((space, snapshot))
    }

    pub(crate) fn snapshot(&self, space: &DockSpaceId) -> Option<&DockViewportSnapshot> {
        self.viewports.get(space)
    }

    pub(crate) fn snapshot_mut(
        &mut self,
        space: &DockSpaceId,
    ) -> Option<&mut DockViewportSnapshot> {
        self.viewports.get_mut(space)
    }

    pub(crate) fn window_for_space(&self, space: &DockSpaceId) -> Option<AnyWindowHandle> {
        self.snapshot(space).map(|snapshot| snapshot.window)
    }

    pub(crate) fn space_for_window_id(&self, window_id: WindowId) -> Option<&DockSpaceId> {
        let space = self.windows.get(&window_id)?;
        let (space, snapshot) = self.viewports.get_key_value(space)?;
        snapshot
            .identity(space)
            .matches(space, window_id)
            .then_some(space)
    }

    pub(crate) fn spaces(&self) -> Vec<DockSpaceId> {
        self.viewports.keys().cloned().collect()
    }

    #[cfg(test)]
    pub(crate) fn insert_stale_window_index_for_test(
        &mut self,
        window_id: WindowId,
        space: DockSpaceId,
    ) {
        self.windows.insert(window_id, space);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport_test_support::{handle, space};

    #[test]
    fn register_keeps_space_and_window_indexes_one_to_one() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let secondary = space("secondary");
        let first = handle(1);
        let second = handle(2);

        assert!(registry.register(main.clone(), first).is_none());
        assert_eq!(registry.window_for_space(&main), Some(first));
        assert_eq!(registry.space_for_window_id(first.window_id()), Some(&main));

        let previous = registry
            .register(main.clone(), second)
            .expect("replacing a space should return the previous snapshot");
        assert_eq!(previous.window, first);
        assert_eq!(registry.window_for_space(&main), Some(second));
        assert_eq!(registry.space_for_window_id(first.window_id()), None);

        registry.register(secondary.clone(), second);
        assert_eq!(registry.window_for_space(&main), None);
        assert_eq!(registry.window_for_space(&secondary), Some(second));
        assert_eq!(registry.spaces(), vec![secondary]);
    }

    #[test]
    fn valid_window_lookup_ignores_and_cleanup_discards_stale_indexes() {
        let mut registry = DockViewportRegistry::default();
        let window_id = WindowId::from(7);
        registry.insert_stale_window_index_for_test(window_id, space("missing"));

        assert_eq!(registry.space_for_window_id(window_id), None);
        assert_eq!(registry.unregister_window_id(window_id), None);
        assert_eq!(registry.space_for_window_id(window_id), None);
    }

    #[test]
    fn valid_window_lookup_rejects_stale_index_to_rebound_space() {
        let mut registry = DockViewportRegistry::default();
        let main = space("main");
        let stale_window_id = WindowId::from(7);
        let current_window = handle(8);
        registry.register(main.clone(), current_window);
        registry.insert_stale_window_index_for_test(stale_window_id, main.clone());

        assert_eq!(registry.space_for_window_id(stale_window_id), None);
        assert_eq!(registry.unregister_window_id(stale_window_id), None);
        assert_eq!(registry.window_for_space(&main), Some(current_window));
        assert_eq!(
            registry.space_for_window_id(current_window.window_id()),
            Some(&main)
        );
    }
}
