//! Browser touch identifiers and terminal cleanup independent of DOM ownership.

use std::collections::HashMap;

use open_gpui::{Pixels, Point, TouchEvent, TouchId, TouchPhase};

#[derive(Default)]
struct TouchIds {
    next: u64,
    active: HashMap<i32, TouchId>,
}

impl TouchIds {
    fn start(&mut self, pointer_id: i32) -> Option<TouchId> {
        let next = self.next.checked_add(1)?;
        let touch_id = TouchId(self.next);
        self.next = next;
        self.active.insert(pointer_id, touch_id);
        Some(touch_id)
    }

    fn active(&self, pointer_id: i32) -> Option<TouchId> {
        self.active.get(&pointer_id).copied()
    }

    fn end(&mut self, pointer_id: i32) -> Option<TouchId> {
        self.active.remove(&pointer_id)
    }
}

#[derive(Default)]
pub(crate) struct WebTouchState {
    ids: TouchIds,
    positions: HashMap<i32, Point<Pixels>>,
}

impl WebTouchState {
    pub(crate) fn start(&mut self, pointer_id: i32, position: Point<Pixels>) -> Option<TouchId> {
        let id = self.ids.start(pointer_id)?;
        self.positions.insert(pointer_id, position);
        Some(id)
    }

    pub(crate) fn moved(&mut self, pointer_id: i32, position: Point<Pixels>) -> Option<TouchId> {
        let id = self.ids.active(pointer_id)?;
        self.positions.insert(pointer_id, position);
        Some(id)
    }

    pub(crate) fn end(&mut self, pointer_id: i32) -> Option<TouchId> {
        self.positions.remove(&pointer_id);
        self.ids.end(pointer_id)
    }

    pub(crate) fn cancel(&mut self, pointer_id: i32) -> Option<TouchEvent> {
        let position = self.positions.remove(&pointer_id)?;
        let id = self.ids.end(pointer_id)?;
        Some(TouchEvent {
            id,
            phase: TouchPhase::Cancelled,
            position,
            predicted_position: None,
            force: None,
        })
    }

    pub(crate) fn cancel_all(&mut self) -> Vec<(i32, TouchEvent)> {
        let positions = std::mem::take(&mut self.positions);
        positions
            .into_iter()
            .filter_map(|(pointer_id, position)| {
                let id = self.ids.end(pointer_id)?;
                Some((
                    pointer_id,
                    TouchEvent {
                        id,
                        phase: TouchPhase::Cancelled,
                        position,
                        predicted_position: None,
                        force: None,
                    },
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{point, px};

    #[test]
    fn browser_pointer_id_reuse_gets_a_new_touch_id() {
        let mut touch_ids = TouchIds::default();
        let first = touch_ids.start(7).expect("first touch id");
        let concurrent = touch_ids.start(8).expect("concurrent touch id");

        assert_ne!(first, concurrent);
        assert_eq!(touch_ids.active(7), Some(first));
        assert_eq!(touch_ids.end(7), Some(first));
        assert_eq!(touch_ids.active(7), None);

        let reused = touch_ids.start(7).expect("reused pointer touch id");
        assert_ne!(reused, first);
        assert_ne!(reused, concurrent);
    }

    #[test]
    fn lost_capture_and_window_cleanup_cancel_each_touch_once() {
        let mut state = WebTouchState::default();
        let first = state.start(7, point(px(1.), px(2.))).unwrap();
        let second = state.start(8, point(px(3.), px(4.))).unwrap();
        state.moved(7, point(px(5.), px(6.)));

        let canceled = state.cancel(7).unwrap();
        assert_eq!(canceled.id, first);
        assert_eq!(canceled.position, point(px(5.), px(6.)));
        assert!(matches!(canceled.phase, TouchPhase::Cancelled));
        assert!(state.cancel(7).is_none());

        let cleanup = state.cancel_all();
        assert_eq!(cleanup.len(), 1);
        assert_eq!(cleanup[0].0, 8);
        assert_eq!(cleanup[0].1.id, second);
        assert_eq!(cleanup[0].1.position, point(px(3.), px(4.)));
        assert!(matches!(cleanup[0].1.phase, TouchPhase::Cancelled));
        assert!(state.cancel_all().is_empty());
        assert!(state.moved(8, Point::default()).is_none());
        assert!(state.end(8).is_none());
        assert_ne!(state.start(7, Point::default()).unwrap(), first);
    }

    #[test]
    fn pointer_up_retires_touch_before_implicit_capture_loss() {
        let mut state = WebTouchState::default();
        let touch = state.start(7, Point::default()).unwrap();
        assert_eq!(state.end(7), Some(touch));
        assert!(state.cancel(7).is_none());
        assert!(state.cancel_all().is_empty());
    }
}
