use std::{cell::RefCell, mem, rc::Rc};

use open_gpui_collections::FxHashSet;

use crate::{App, Effect, EntityId};

use super::DrawPhase;

struct WindowInvalidatorInner {
    dirty: bool,
    focus_only_dirty: bool,
    draw_phase: DrawPhase,
    dirty_views: FxHashSet<EntityId>,
    update_count: usize,
}

#[derive(Clone)]
pub(crate) struct WindowInvalidator {
    inner: Rc<RefCell<WindowInvalidatorInner>>,
}

impl WindowInvalidator {
    pub fn new() -> Self {
        WindowInvalidator {
            inner: Rc::new(RefCell::new(WindowInvalidatorInner {
                dirty: true,
                focus_only_dirty: false,
                draw_phase: DrawPhase::None,
                dirty_views: FxHashSet::default(),
                update_count: 0,
            })),
        }
    }

    pub fn invalidate_view(&self, entity: EntityId, cx: &mut App) -> bool {
        let mut inner = self.inner.borrow_mut();
        inner.update_count += 1;
        inner.dirty_views.insert(entity);
        if inner.draw_phase == DrawPhase::None {
            inner.dirty = true;
            inner.focus_only_dirty = false;
            cx.push_effect(Effect::Notify { emitter: entity });
            true
        } else {
            if inner.draw_phase == DrawPhase::Focus {
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
        inner.dirty = dirty;
        inner.focus_only_dirty = false;
        if dirty {
            inner.update_count += 1;
        }
    }

    pub fn set_focus_only_dirty(&self) {
        let mut inner = self.inner.borrow_mut();
        if !inner.dirty {
            inner.dirty = true;
            inner.focus_only_dirty = inner.dirty_views.is_empty();
        } else if !inner.dirty_views.is_empty() {
            inner.focus_only_dirty = false;
        }
        inner.update_count += 1;
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
