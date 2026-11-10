use std::sync::Arc;

use crate::{AnyDrag, App, AsyncApp, FileDropEvent, NativeCapturedDragGeneration, WindowId};

/// Native source callbacks for one exact platform-owned drag.
///
/// Clones preserve the original window and framework drag generation. Delivery uses native
/// ingress ordering and cannot finish or restore a replacement drag.
#[derive(Clone)]
pub struct PlatformExternalDragSession {
    app: AsyncApp,
    window_id: WindowId,
    generation: NativeCapturedDragGeneration,
}

impl PlatformExternalDragSession {
    pub(crate) fn new(
        app: AsyncApp,
        window_id: WindowId,
        generation: NativeCapturedDragGeneration,
    ) -> Self {
        Self {
            app,
            window_id,
            generation,
        }
    }

    /// Queues a source-window drag update or terminal event for this exact native session.
    pub fn dispatch(&self, event: FileDropEvent) {
        self.app
            .enqueue_external_drag_event(self.window_id, self.generation, event);
    }
}

pub(super) struct PlatformOwnedDrag {
    source_window: WindowId,
    generation: NativeCapturedDragGeneration,
    state: PlatformOwnedDragState,
    destination: Option<(WindowId, Arc<dyn std::any::Any>)>,
}

enum PlatformOwnedDragState {
    Suspended(AnyDrag),
    Restored(Arc<dyn std::any::Any>),
}

impl App {
    /// Returns whether an internal drag is currently owned by a native platform session.
    pub fn has_active_platform_drag(&self) -> bool {
        self.platform_owned_drag.is_some()
    }

    pub(crate) fn active_drag_generation_for_window(
        &self,
        window_id: WindowId,
    ) -> Option<NativeCapturedDragGeneration> {
        let drag = self
            .active_drag
            .as_ref()
            .filter(|drag| drag.window_id == window_id)?;
        self.active_native_captured_drag
            .as_ref()?
            .generation_for(drag)
    }

    pub(crate) fn hand_active_drag_to_platform(
        &mut self,
        source_window: WindowId,
        generation: NativeCapturedDragGeneration,
    ) -> bool {
        if self.active_drag_generation_for_window(source_window) != Some(generation) {
            return false;
        }
        let drag = self
            .active_drag
            .take()
            .expect("the exact drag was validated");
        self.platform_owned_drag = Some(PlatformOwnedDrag {
            source_window,
            generation,
            state: PlatformOwnedDragState::Suspended(drag),
            destination: None,
        });
        true
    }

    pub(crate) fn platform_drag_matches(
        &self,
        window_id: WindowId,
        generation: NativeCapturedDragGeneration,
    ) -> bool {
        self.platform_owned_drag
            .as_ref()
            .is_some_and(|drag| drag.source_window == window_id && drag.generation == generation)
    }

    pub(crate) fn platform_drag_is_current(
        &self,
        window_id: WindowId,
        generation: NativeCapturedDragGeneration,
    ) -> bool {
        self.platform_drag_matches(window_id, generation)
            && self
                .active_native_captured_drag
                .as_ref()
                .is_some_and(|authority| authority.matches_generation(window_id, generation))
    }

    pub(crate) fn restore_platform_drag(
        &mut self,
        source_window: WindowId,
        generation: NativeCapturedDragGeneration,
    ) -> bool {
        if self.active_drag.is_some() || !self.platform_drag_is_current(source_window, generation) {
            return false;
        }
        let platform_drag = self
            .platform_owned_drag
            .as_mut()
            .expect("session was validated");
        let PlatformOwnedDragState::Suspended(drag) = &platform_drag.state else {
            return false;
        };
        let restored = PlatformOwnedDragState::Restored(drag.value.clone());
        let PlatformOwnedDragState::Suspended(drag) =
            std::mem::replace(&mut platform_drag.state, restored)
        else {
            unreachable!("the suspended state was validated");
        };
        self.active_drag = Some(drag);
        true
    }

    pub(crate) fn hand_restored_drag_to_platform(
        &mut self,
        source_window: WindowId,
        generation: NativeCapturedDragGeneration,
    ) -> bool {
        if self.active_drag_generation_for_window(source_window) != Some(generation)
            || !self.platform_drag_matches(source_window, generation)
        {
            return false;
        }
        let platform_drag = self
            .platform_owned_drag
            .as_mut()
            .expect("session was validated");
        let Some(drag) = self.active_drag.as_ref() else {
            return false;
        };
        if !matches!(&platform_drag.state, PlatformOwnedDragState::Restored(value) if Arc::ptr_eq(value, &drag.value))
        {
            return false;
        }
        platform_drag.state = PlatformOwnedDragState::Suspended(
            self.active_drag
                .take()
                .expect("the exact drag was validated"),
        );
        true
    }

    pub(crate) fn end_platform_drag_exact(
        &mut self,
        source_window: WindowId,
        generation: NativeCapturedDragGeneration,
    ) -> bool {
        if !self.platform_drag_matches(source_window, generation) {
            return false;
        }
        let drag = self
            .platform_owned_drag
            .take()
            .expect("session was validated");
        if let Some((window_id, value)) = drag.destination
            && self.active_drag.as_ref().is_some_and(|active| {
                active.window_id == window_id && Arc::ptr_eq(&value, &active.value)
            })
        {
            self.active_drag = None;
        }
        if let PlatformOwnedDragState::Restored(value) = drag.state
            && self.active_drag.as_ref().is_some_and(|active| {
                active.window_id == source_window && Arc::ptr_eq(&value, &active.value)
            })
        {
            self.active_drag = None;
        }
        if self
            .active_native_captured_drag
            .as_ref()
            .is_some_and(|authority| authority.matches_generation(source_window, generation))
        {
            self.retire_native_captured_drag_authority();
        }
        true
    }

    pub(crate) fn end_platform_drag(&mut self, source_window: WindowId) -> bool {
        let Some(generation) = self
            .platform_owned_drag
            .as_ref()
            .filter(|drag| drag.source_window == source_window)
            .map(|drag| drag.generation)
        else {
            return false;
        };
        self.end_platform_drag_exact(source_window, generation)
    }

    pub(crate) fn restore_current_platform_drag(&mut self, source_window: WindowId) -> bool {
        let Some(generation) = self
            .platform_owned_drag
            .as_ref()
            .filter(|drag| drag.source_window == source_window)
            .map(|drag| drag.generation)
        else {
            return false;
        };
        self.restore_platform_drag(source_window, generation)
    }

    pub(crate) fn hand_current_restored_drag_to_platform(
        &mut self,
        source_window: WindowId,
    ) -> bool {
        let Some(generation) = self
            .platform_owned_drag
            .as_ref()
            .filter(|drag| drag.source_window == source_window)
            .map(|drag| drag.generation)
        else {
            return false;
        };
        self.hand_restored_drag_to_platform(source_window, generation)
    }

    pub(crate) fn start_external_drop(&mut self, drag: AnyDrag) {
        if let Some(platform_drag) = self.platform_owned_drag.as_mut() {
            assert!(
                self.active_drag.is_none(),
                "an inbound file wrapper requires an empty slot"
            );
            platform_drag.destination = Some((drag.window_id, drag.value.clone()));
            self.active_drag = Some(drag);
        } else {
            self.start_active_drag(drag);
        }
    }

    pub(crate) fn retire_drag_authority(&mut self, drag: &AnyDrag) {
        if self
            .active_native_captured_drag
            .as_ref()
            .is_some_and(|authority| authority.generation_for(drag).is_some())
        {
            self.retire_native_captured_drag_authority();
        } else if let Some(platform_drag) = self.platform_owned_drag.as_mut()
            && platform_drag
                .destination
                .as_ref()
                .is_some_and(|(window_id, value)| {
                    *window_id == drag.window_id && Arc::ptr_eq(value, &drag.value)
                })
        {
            platform_drag.destination = None;
        }
    }
}
