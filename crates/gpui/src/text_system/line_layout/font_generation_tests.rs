use super::*;
use crate::text_system::font_installation_tests::fixture;
use std::borrow::Cow;

#[test]
fn old_font_markers_cannot_restore_or_truncate_new_generation_entries() {
    let (_, text, window) = fixture();
    let cache = &window.line_layout_cache;
    let runs = [crate::FontRun {
        len: 3,
        font_id: FontId(1),
    }];
    let start = cache.layout_index();
    cache.layout_line("old", px(16.), &runs, None, 0);
    cache.layout_wrapped_line("old", px(16.), &runs, Some(px(20.)), None, 0);
    let end = cache.layout_index();
    cache.finish_frame();
    text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    cache.layout_line("new", px(16.), &runs, None, 1);
    cache.layout_wrapped_line("new", px(16.), &runs, Some(px(20.)), None, 1);
    cache.reuse_layouts(start.clone()..end);
    cache.truncate_layouts(start);
    let state = cache.state();
    let current = &state.current_frame;
    assert_eq!(current.used_lines.len(), 1);
    assert_eq!(current.used_wrapped_lines.len(), 1);
    assert!(current.lines.keys().all(|key| key.text.as_ref() == "new"));
    assert!(
        current
            .wrapped_lines
            .keys()
            .all(|key| key.text.as_ref() == "new")
    );
    assert!(state.previous_frame.lines.is_empty());
    assert!(state.previous_frame.wrapped_lines.is_empty());
}

#[test]
fn refused_candidate_restores_current_generation_without_reviving_old_layouts() {
    let (_, text, window) = fixture();
    let cache = &window.line_layout_cache;
    let runs = [crate::FontRun {
        len: 4,
        font_id: FontId(1),
    }];
    let old = cache.layout_line("same", px(16.), &runs, None, 0);
    cache.finish_frame();
    let candidate = cache.layout_line("same", px(16.), &runs, None, 0);
    assert!(Arc::ptr_eq(&old, &candidate));
    cache.abort_frame();
    assert!(Arc::ptr_eq(
        &candidate,
        &cache.layout_line("same", px(16.), &runs, None, 0)
    ));
    cache.finish_frame();
    text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    cache.abort_frame();
    let new = cache.layout_line("same", px(16.), &runs, None, 1);
    assert!(!Arc::ptr_eq(&old, &new));
    assert_eq!(new.width, old.width * 2.);
    cache.abort_frame();
    assert!(Arc::ptr_eq(
        &new,
        &cache.layout_line("same", px(16.), &runs, None, 1)
    ));
}

#[test]
fn stale_resolved_runs_cannot_populate_new_generation_plain_or_wrapped_caches() {
    let (_, text, window) = fixture();
    let cache = &window.line_layout_cache;
    let runs = [crate::FontRun {
        len: 4,
        font_id: FontId(1),
    }];
    let old = cache.layout_line("same", px(16.), &runs, None, 0);
    text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    cache.layout_line("same", px(16.), &runs, None, 0);
    cache.layout_wrapped_line("same", px(16.), &runs, Some(px(20.)), None, 0);
    {
        let state = cache.state();
        assert_eq!(state.font_generation, 1);
        assert!(state.current_frame.lines.is_empty());
        assert!(state.current_frame.wrapped_lines.is_empty());
        assert!(state.current_frame.used_lines.is_empty());
        assert!(state.current_frame.used_wrapped_lines.is_empty());
    }
    let current = cache.layout_line("same", px(16.), &runs, None, 1);
    assert_eq!(current.width, old.width * 2.);
    assert!(Arc::ptr_eq(
        &current,
        &cache.layout_line("same", px(16.), &runs, None, 1)
    ));
    let wrapped = cache.layout_wrapped_line("same", px(16.), &runs, Some(px(20.)), None, 1);
    assert!(Arc::ptr_eq(&current, &wrapped.unwrapped_layout));
    assert!(Arc::ptr_eq(
        &wrapped,
        &cache.layout_wrapped_line("same", px(16.), &runs, Some(px(20.)), None, 1)
    ));
}

#[test]
fn owner_thread_range_operations_observe_shared_font_installation_on_a_worker() {
    let (_, text, window) = fixture();
    let cache = &window.line_layout_cache;
    let runs = [crate::FontRun {
        len: 4,
        font_id: FontId(1),
    }];
    let start = cache.layout_index();
    let old = cache.layout_line("same", px(16.), &runs, None, 0);
    let old_wrapped = cache.layout_wrapped_line("same", px(16.), &runs, Some(px(20.)), None, 0);
    let end = cache.layout_index();
    cache.finish_frame();
    let state = cache.state();
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
        });
        worker.join().unwrap();
    });
    assert_eq!(text.font_generation(), 1);
    assert_eq!(state.font_generation, 0);
    assert_eq!(state.previous_frame.lines.len(), 1);
    assert_eq!(state.previous_frame.wrapped_lines.len(), 1);
    drop(state);

    cache.reuse_layouts(start.clone()..end);
    let new = cache.layout_line("same", px(16.), &runs, None, 1);
    let new_wrapped = cache.layout_wrapped_line("same", px(16.), &runs, Some(px(20.)), None, 1);
    cache.truncate_layouts(start);
    let index = cache.layout_index();
    assert_eq!(index.font_generation, 1);
    assert_eq!(index.lines_index, 1);
    assert_eq!(index.wrapped_lines_index, 1);
    assert!(!Arc::ptr_eq(&old, &new));
    assert!(!Arc::ptr_eq(&old_wrapped, &new_wrapped));
    assert!(Arc::ptr_eq(&new, &new_wrapped.unwrapped_layout));
    assert_eq!(new.width, old.width * 2.);
}

#[test]
fn delayed_wrapped_publication_cannot_replace_a_new_generation_layout() {
    let (_, text, window) = fixture();
    let cache = &window.line_layout_cache;
    let runs = [crate::FontRun {
        len: 9,
        font_id: FontId(1),
    }];
    let old = cache.layout_wrapped_line("same text", px(16.), &runs, Some(px(40.)), None, 0);
    let key = cache.state().current_frame.used_wrapped_lines[0].clone();
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
        });
        worker.join().unwrap();
    });
    let new = cache.layout_wrapped_line("same text", px(16.), &runs, Some(px(40.)), None, 1);
    let in_flight = cache.publish_wrapped_layout(key, old.clone(), 0);
    assert!(Arc::ptr_eq(&old, &in_flight));
    assert_eq!(
        new.unwrapped_layout.width,
        in_flight.unwrapped_layout.width * 2.
    );
    let reused = cache.layout_wrapped_line("same text", px(16.), &runs, Some(px(40.)), None, 1);
    assert!(Arc::ptr_eq(&new, &reused));
    assert!(!Arc::ptr_eq(&in_flight, &reused));
    let state = cache.state();
    assert_eq!(state.font_generation, 1);
    assert_eq!(state.current_frame.used_wrapped_lines.len(), 1);
}
