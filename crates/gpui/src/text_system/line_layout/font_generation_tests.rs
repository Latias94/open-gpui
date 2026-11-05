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
    cache.layout_line_by_hash(1, 3, px(16.), &runs, None, || "old".into(), 0);
    let end = cache.layout_index();
    cache.finish_frame();
    text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
    cache.layout_line("new", px(16.), &runs, None, 1);
    cache.layout_wrapped_line("new", px(16.), &runs, Some(px(20.)), None, 1);
    cache.layout_line_by_hash(2, 3, px(16.), &runs, None, || "new".into(), 1);
    cache.reuse_layouts(start.clone()..end);
    cache.truncate_layouts(start);
    let state = cache.state();
    let current = &state.current_frame;
    assert_eq!(current.used_lines.len(), 1);
    assert_eq!(current.used_wrapped_lines.len(), 1);
    assert_eq!(current.used_lines_by_hash.len(), 1);
    assert!(current.lines.keys().all(|key| key.text.as_ref() == "new"));
    assert!(state.previous_frame.lines.is_empty());
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
fn stale_resolved_runs_cannot_populate_new_generation_plain_wrapped_or_hash_caches() {
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
    cache.layout_line_by_hash(1, 4, px(16.), &runs, None, || "same".into(), 0);
    assert!(
        cache
            .try_layout_line_by_hash(1, 4, px(16.), &runs, None, 0)
            .is_none()
    );
    {
        let state = cache.state();
        assert_eq!(state.font_generation, 1);
        assert!(state.current_frame.lines.is_empty());
        assert!(state.current_frame.wrapped_lines.is_empty());
        assert!(state.current_frame.lines_by_hash.is_empty());
    }
    let current = cache.layout_line("same", px(16.), &runs, None, 1);
    assert_eq!(current.width, old.width * 2.);
    assert!(Arc::ptr_eq(
        &current,
        &cache.layout_line("same", px(16.), &runs, None, 1)
    ));
}

#[test]
fn range_operations_waiting_on_cache_state_observe_the_latest_font_generation() {
    let (_, text, window) = fixture();
    let cache = &window.line_layout_cache;
    let runs = [crate::FontRun {
        len: 4,
        font_id: FontId(1),
    }];
    let start = cache.layout_index();
    cache.layout_line("same", px(16.), &runs, None, 0);
    let end = cache.layout_index();
    cache.finish_frame();
    let (started, waiting) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let state = cache.state.lock();
        let worker = scope.spawn(|| {
            started.send(()).unwrap();
            cache.reuse_layouts(start.clone()..end);
            cache.layout_line("same", px(16.), &runs, None, 1);
            cache.truncate_layouts(start);
            cache.layout_index()
        });
        waiting
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
        drop(state);
        let index = worker.join().unwrap();
        assert_eq!(index.font_generation, 1);
        assert_eq!(index.lines_index, 1);
    });
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
    let (ready, waiting) = std::sync::mpsc::channel();
    let (publish, allowed) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let old = old.clone();
        let worker = scope.spawn(move || {
            ready.send(()).unwrap();
            allowed
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap();
            cache.publish_wrapped_layout(key, old, 0)
        });
        waiting
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        text.add_fonts(vec![Cow::Borrowed(b"font")]).unwrap();
        let new = cache.layout_wrapped_line("same text", px(16.), &runs, Some(px(40.)), None, 1);
        publish.send(()).unwrap();
        let in_flight = worker.join().unwrap();
        assert_eq!(
            new.unwrapped_layout.width,
            in_flight.unwrapped_layout.width * 2.
        );
        let reused = cache.layout_wrapped_line("same text", px(16.), &runs, Some(px(40.)), None, 1);
        assert!(Arc::ptr_eq(&new, &reused));
        assert!(!Arc::ptr_eq(&in_flight, &reused));
    });
}
