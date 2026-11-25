use std::cell::Cell;

use super::*;

#[derive(Default)]
struct FakeImeCursorRectangleSink {
    cursor_rectangles: RefCell<Vec<(i32, i32, i32, i32)>>,
    commit_count: Cell<usize>,
}

impl ImeCursorRectangleSink for FakeImeCursorRectangleSink {
    fn set_ime_cursor_rectangle(&self, x: i32, y: i32, width: i32, height: i32) {
        self.cursor_rectangles
            .borrow_mut()
            .push((x, y, width, height));
    }

    fn commit_ime_state(&self) {
        self.commit_count.set(self.commit_count.get() + 1);
    }
}

fn ime_cursor_bounds(x: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(20.25)), size(px(1.0), px(18.75)))
}

#[test]
fn caches_cursor_rectangle_committed_after_done() {
    let text_input = FakeImeCursorRectangleSink::default();
    let mut last_ime_cursor_rectangle = None;
    let initial_bounds = ime_cursor_bounds(10.0);
    let updated_bounds = ime_cursor_bounds(20.0);

    update_ime_cursor_rectangle(&text_input, &mut last_ime_cursor_rectangle, initial_bounds);
    set_ime_cursor_rectangle_after_done(
        &text_input,
        &mut last_ime_cursor_rectangle,
        updated_bounds,
        true,
    );
    update_ime_cursor_rectangle(&text_input, &mut last_ime_cursor_rectangle, updated_bounds);

    assert_eq!(text_input.commit_count.get(), 2);
    assert_eq!(text_input.cursor_rectangles.borrow().len(), 2);
}

#[test]
fn skips_unchanged_cursor_rectangle_after_done() {
    let text_input = FakeImeCursorRectangleSink::default();
    let mut last_ime_cursor_rectangle = None;
    let bounds = ime_cursor_bounds(10.0);

    update_ime_cursor_rectangle(&text_input, &mut last_ime_cursor_rectangle, bounds);
    set_ime_cursor_rectangle_after_done(&text_input, &mut last_ime_cursor_rectangle, bounds, true);

    assert_eq!(text_input.commit_count.get(), 1);
    assert_eq!(text_input.cursor_rectangles.borrow().len(), 1);
}

#[test]
fn skips_cursor_rectangles_with_unchanged_protocol_coordinates() {
    let text_input = FakeImeCursorRectangleSink::default();
    let mut last_ime_cursor_rectangle = None;

    update_ime_cursor_rectangle(
        &text_input,
        &mut last_ime_cursor_rectangle,
        ime_cursor_bounds(10.25),
    );
    update_ime_cursor_rectangle(
        &text_input,
        &mut last_ime_cursor_rectangle,
        ime_cursor_bounds(10.75),
    );

    assert_eq!(text_input.commit_count.get(), 1);
    assert_eq!(
        text_input.cursor_rectangles.borrow().as_slice(),
        &[(10, 20, 1, 18)]
    );
}

#[test]
fn changed_done_serial_does_not_commit_or_cache_pending_coordinates() {
    let text_input = FakeImeCursorRectangleSink::default();
    let mut cache = ImeCursorRectangleCache::default();
    let mut serial_tracker = SerialTracker::new();
    let initial_bounds = ime_cursor_bounds(10.0);
    let updated_bounds = ime_cursor_bounds(20.0);
    update_ime_cursor_rectangle(&text_input, &mut cache.last, initial_bounds);
    serial_tracker.update(SerialKind::InputMethod, 41);

    let last_serial = serial_tracker.get(SerialKind::InputMethod);
    let serial = 42;
    serial_tracker.update(SerialKind::InputMethod, serial);
    let generation = cache.advance();
    cache.set_after_done(
        &text_input,
        generation,
        updated_bounds,
        last_serial.as_raw() == serial,
    );

    assert_eq!(text_input.commit_count.get(), 1);
    assert_eq!(cache.last, Some(ImeCursorRectangle::from(initial_bounds)));
    assert_eq!(
        text_input.cursor_rectangles.borrow().as_slice(),
        &[(10, 20, 1, 18), (20, 20, 1, 18)]
    );

    let last_serial = serial_tracker.get(SerialKind::InputMethod);
    serial_tracker.update(SerialKind::InputMethod, serial);
    let generation = cache.advance();
    cache.set_after_done(
        &text_input,
        generation,
        updated_bounds,
        last_serial.as_raw() == serial,
    );
    update_ime_cursor_rectangle(&text_input, &mut cache.last, updated_bounds);

    assert_eq!(text_input.commit_count.get(), 2);
    assert_eq!(text_input.cursor_rectangles.borrow().len(), 3);
    assert_eq!(cache.last, Some(ImeCursorRectangle::from(updated_bounds)));
}

#[test]
fn reset_resubmits_identical_coordinates_to_the_new_protocol_owner() {
    let original = FakeImeCursorRectangleSink::default();
    let replacement = FakeImeCursorRectangleSink::default();
    let mut cache = ImeCursorRectangleCache::default();
    let bounds = ime_cursor_bounds(10.0);
    update_ime_cursor_rectangle(&original, &mut cache.last, bounds);
    let original_generation = cache.generation;

    cache.reset();
    assert_eq!(cache.last, None);
    cache.set_after_done(&replacement, cache.generation, bounds, true);
    cache.set_after_done(&replacement, cache.generation, bounds, true);
    cache.set_after_done(
        &original,
        original_generation,
        ime_cursor_bounds(30.0),
        false,
    );

    assert_eq!(original.commit_count.get(), 1);
    assert_eq!(original.cursor_rectangles.borrow().len(), 1);
    assert_eq!(replacement.commit_count.get(), 1);
    assert_eq!(
        replacement.cursor_rectangles.borrow().as_slice(),
        &[(10, 20, 1, 18)]
    );
    assert_eq!(cache.last, Some(ImeCursorRectangle::from(bounds)));
}

#[test]
fn nested_ime_restart_preserves_the_replacement_cursor_cache() {
    let text_input = FakeImeCursorRectangleSink::default();
    let mut cache = ImeCursorRectangleCache::default();
    let retired_generation = cache.generation;

    cache.reset();
    cache.reset();
    let bounds = ime_cursor_bounds(20.0);
    cache.set_after_done(&text_input, cache.generation, bounds, true);
    cache.set_after_done(
        &text_input,
        retired_generation,
        ime_cursor_bounds(30.0),
        true,
    );
    cache.set_after_done(
        &text_input,
        retired_generation,
        ime_cursor_bounds(40.0),
        false,
    );

    assert_eq!(text_input.commit_count.get(), 1);
    assert_eq!(
        text_input.cursor_rectangles.borrow().as_slice(),
        &[(20, 20, 1, 18)]
    );
    assert_eq!(cache.last, Some(ImeCursorRectangle::from(bounds)));
}

#[test]
fn nested_done_retires_older_results_without_clearing_the_committed_cache() {
    let text_input = FakeImeCursorRectangleSink::default();
    let mut cache = ImeCursorRectangleCache::default();
    update_ime_cursor_rectangle(&text_input, &mut cache.last, ime_cursor_bounds(10.0));
    let outer_generation = cache.advance();
    let inner_generation = cache.advance();
    let bounds = ime_cursor_bounds(20.0);

    cache.set_after_done(&text_input, inner_generation, bounds, true);
    cache.set_after_done(&text_input, outer_generation, ime_cursor_bounds(30.0), true);
    cache.set_after_done(&text_input, inner_generation, bounds, true);

    assert_eq!(text_input.commit_count.get(), 2);
    assert_eq!(
        text_input.cursor_rectangles.borrow().as_slice(),
        &[(10, 20, 1, 18), (20, 20, 1, 18)]
    );
    assert_eq!(cache.last, Some(ImeCursorRectangle::from(bounds)));
}
