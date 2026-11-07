use std::{cell::RefCell, mem, rc::Rc};

use open_gpui_collections::FxHashSet;

use crate::{App, Effect, EntityId, WindowId};

use super::DrawPhase;

#[cfg(feature = "profiler")]
use open_gpui_scheduler::Instant;

#[cfg(feature = "profiler")]
#[derive(Clone, Copy, Default)]
pub(super) struct FrameDirtyAccumulator {
    pub(super) dirty_at: Option<Instant>,
    pub(super) invalidations: u64,
}

struct WindowInvalidatorInner {
    #[cfg(feature = "profiler")]
    window_id: WindowId,
    dirty: bool,
    focus_only_dirty: bool,
    draw_phase: DrawPhase,
    dirty_views: FxHashSet<EntityId>,
    update_count: usize,
    #[cfg(feature = "profiler")]
    frame_dirty: FrameDirtyAccumulator,
    platform_waker: Option<Rc<dyn Fn()>>,
}

#[derive(Clone)]
pub(crate) struct WindowInvalidator {
    inner: Rc<RefCell<WindowInvalidatorInner>>,
}

impl WindowInvalidator {
    pub fn new(window_id: WindowId) -> Self {
        #[cfg(not(feature = "profiler"))]
        let _ = window_id;
        WindowInvalidator {
            inner: Rc::new(RefCell::new(WindowInvalidatorInner {
                #[cfg(feature = "profiler")]
                window_id,
                dirty: true,
                focus_only_dirty: false,
                draw_phase: DrawPhase::None,
                dirty_views: FxHashSet::default(),
                update_count: 0,
                #[cfg(feature = "profiler")]
                frame_dirty: FrameDirtyAccumulator::default(),
                platform_waker: None,
            })),
        }
    }

    pub fn invalidate_view(&self, entity: EntityId, cx: &mut App) -> bool {
        let mut inner = self.inner.borrow_mut();
        inner.update_count += 1;
        inner.dirty_views.insert(entity);
        if inner.draw_phase == DrawPhase::None {
            #[cfg(feature = "profiler")]
            Self::record_frame_dirty(&mut inner);
            let became_dirty = !inner.dirty;
            inner.dirty = true;
            inner.focus_only_dirty = false;
            let waker = became_dirty.then(|| inner.platform_waker.clone()).flatten();
            drop(inner);
            cx.push_effect(Effect::Notify { emitter: entity });
            if let Some(waker) = waker {
                waker();
            }
            true
        } else {
            if inner.draw_phase == DrawPhase::Focus {
                #[cfg(feature = "profiler")]
                Self::record_frame_dirty(&mut inner);
                inner.dirty = true;
                inner.focus_only_dirty = false;
                cx.push_effect(Effect::Notify { emitter: entity });
            }
            false
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.inner.borrow().dirty
    }

    pub fn set_dirty(&self, dirty: bool) {
        let mut inner = self.inner.borrow_mut();
        let became_dirty = dirty && !inner.dirty;
        inner.dirty = dirty;
        inner.focus_only_dirty = false;
        if dirty {
            inner.update_count += 1;
            #[cfg(feature = "profiler")]
            Self::record_frame_dirty(&mut inner);
        }
        let waker = became_dirty.then(|| inner.platform_waker.clone()).flatten();
        drop(inner);
        if let Some(waker) = waker {
            waker();
        }
    }

    pub fn set_focus_only_dirty(&self) {
        let mut inner = self.inner.borrow_mut();
        let became_dirty = !inner.dirty;
        if !inner.dirty {
            inner.dirty = true;
            inner.focus_only_dirty = inner.dirty_views.is_empty();
        } else if !inner.dirty_views.is_empty() {
            inner.focus_only_dirty = false;
        }
        inner.update_count += 1;
        #[cfg(feature = "profiler")]
        Self::record_frame_dirty(&mut inner);
        let waker = became_dirty.then(|| inner.platform_waker.clone()).flatten();
        drop(inner);
        if let Some(waker) = waker {
            waker();
        }
    }

    pub fn set_platform_waker(&self, waker: Option<Rc<dyn Fn()>>) {
        let mut inner = self.inner.borrow_mut();
        inner.platform_waker = waker;
        let waker = inner.dirty.then(|| inner.platform_waker.clone()).flatten();
        drop(inner);
        if let Some(waker) = waker {
            waker();
        }
    }

    pub fn wake_platform(&self) {
        let waker = self.inner.borrow().platform_waker.clone();
        if let Some(waker) = waker {
            waker();
        }
    }

    #[cfg(feature = "profiler")]
    fn record_frame_dirty(inner: &mut WindowInvalidatorInner) {
        inner.frame_dirty.dirty_at.get_or_insert_with(Instant::now);
        inner.frame_dirty.invalidations = inner.frame_dirty.invalidations.saturating_add(1);
        if let Some(dirty_at) = inner.frame_dirty.dirty_at {
            crate::profiler::journal::record_frame_pending(inner.window_id, dirty_at);
        }
    }

    #[cfg(feature = "profiler")]
    pub(super) fn take_frame_dirty(&self) -> FrameDirtyAccumulator {
        mem::take(&mut self.inner.borrow_mut().frame_dirty)
    }

    #[cfg(feature = "profiler")]
    pub(super) fn pending_dirty_at(&self) -> Option<Instant> {
        self.inner.borrow().frame_dirty.dirty_at
    }

    #[cfg(feature = "profiler")]
    pub(super) fn restore_frame_dirty(&self, previous: FrameDirtyAccumulator) {
        let mut inner = self.inner.borrow_mut();
        if let Some(previous_at) = previous.dirty_at {
            inner.frame_dirty.dirty_at = Some(
                inner
                    .frame_dirty
                    .dirty_at
                    .map_or(previous_at, |current| current.min(previous_at)),
            );
        }
        inner.frame_dirty.invalidations = inner
            .frame_dirty
            .invalidations
            .saturating_add(previous.invalidations);
    }

    pub fn clear_focus_only_dirty(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        if !inner.focus_only_dirty {
            return false;
        }
        if !inner.dirty_views.is_empty() {
            inner.focus_only_dirty = false;
            return false;
        }
        inner.dirty = false;
        inner.focus_only_dirty = false;
        #[cfg(feature = "profiler")]
        {
            inner.frame_dirty = FrameDirtyAccumulator::default();
        }
        true
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn is_focus_only_dirty(&self) -> bool {
        self.inner.borrow().focus_only_dirty
    }

    pub fn set_phase(&self, phase: DrawPhase) {
        self.inner.borrow_mut().draw_phase = phase
    }

    pub fn update_count(&self) -> usize {
        self.inner.borrow().update_count
    }

    pub fn take_views(&self) -> FxHashSet<EntityId> {
        mem::take(&mut self.inner.borrow_mut().dirty_views)
    }

    pub fn replace_views(&self, views: FxHashSet<EntityId>) {
        self.inner.borrow_mut().dirty_views = views;
    }

    pub fn can_schedule_refresh(&self) -> bool {
        matches!(
            self.inner.borrow().draw_phase,
            DrawPhase::None | DrawPhase::Focus
        )
    }

    pub fn is_focus_phase(&self) -> bool {
        self.inner.borrow().draw_phase == DrawPhase::Focus
    }

    pub fn is_building_frame(&self) -> bool {
        matches!(
            self.inner.borrow().draw_phase,
            DrawPhase::Prepaint | DrawPhase::Paint
        )
    }

    pub fn is_prepaint(&self) -> bool {
        self.inner.borrow().draw_phase == DrawPhase::Prepaint
    }

    pub fn is_paint(&self) -> bool {
        self.inner.borrow().draw_phase == DrawPhase::Paint
    }

    #[track_caller]
    pub fn debug_assert_paint(&self) {
        debug_assert!(
            matches!(self.inner.borrow().draw_phase, DrawPhase::Paint),
            "this method can only be called during paint"
        );
    }

    #[track_caller]
    pub fn debug_assert_prepaint(&self) {
        debug_assert!(
            matches!(self.inner.borrow().draw_phase, DrawPhase::Prepaint),
            "this method can only be called during request_layout, or prepaint"
        );
    }

    #[track_caller]
    pub fn debug_assert_paint_or_prepaint(&self) {
        debug_assert!(
            matches!(
                self.inner.borrow().draw_phase,
                DrawPhase::Paint | DrawPhase::Prepaint
            ),
            "this method can only be called during request_layout, prepaint, or paint"
        );
    }
}

#[cfg(all(test, feature = "profiler"))]
mod profiler_tests {
    use super::*;

    #[test]
    fn canceled_focus_only_work_does_not_age_a_later_unrelated_frame() {
        let invalidator = WindowInvalidator::new(WindowId::from(1));
        invalidator.set_dirty(false);
        invalidator.set_focus_only_dirty();
        assert!(invalidator.clear_focus_only_dirty());
        let canceled = invalidator.take_frame_dirty();
        assert_eq!(canceled.dirty_at, None);
        assert_eq!(canceled.invalidations, 0);
        let started = Instant::now();
        invalidator.set_dirty(true);
        let next = invalidator.take_frame_dirty();
        assert!(next.dirty_at.is_some_and(|at| at >= started));
        assert_eq!(next.invalidations, 1);
    }

    #[test]
    fn refused_focus_only_cancellation_preserves_unrelated_dirty_work() {
        let invalidator = WindowInvalidator::new(WindowId::from(1));
        invalidator.set_dirty(false);
        invalidator.set_dirty(true);
        invalidator.set_focus_only_dirty();
        assert!(!invalidator.clear_focus_only_dirty());
        let pending = invalidator.take_frame_dirty();
        assert!(pending.dirty_at.is_some());
        assert_eq!(pending.invalidations, 2);
    }
}
