use open_gpui::{
    LineLayout, NoopTextSystem, ShapedLine, TextRun, TextSystem, Window, WindowTextSystem,
    WrappedLineLayout, font, px,
};
use std::{rc::Rc, sync::Arc};

const SOURCE: &str = "downstream café consumes complete source bytes";

fn consume(owner: &Rc<WindowTextSystem>, width: f32) -> (ShapedLine, Arc<WrappedLineLayout>) {
    let runs = [TextRun {
        len: SOURCE.len(),
        font: font("downstream"),
        ..Default::default()
    }];
    let plain = owner.shape_line(SOURCE.into(), px(16.), &runs, None);
    assert_eq!(plain.text.as_ref(), SOURCE);
    assert!(Arc::ptr_eq(
        &*plain,
        &owner.layout_line(SOURCE, px(16.), &runs, None),
    ));
    let wrapped = owner
        .shape_text(SOURCE.into(), px(16.), &runs, Some(px(width)), None)
        .expect("the downstream consumer must shape complete source text");
    assert_eq!(wrapped.len(), 1);
    assert_eq!(wrapped[0].text.as_ref(), SOURCE);
    assert_eq!(wrapped[0].wrap_width, Some(px(width)));
    assert!(!wrapped[0].wrap_boundaries.is_empty());
    assert!(Arc::ptr_eq(&*plain, &wrapped[0].unwrapped_layout));
    (plain, Arc::clone(&wrapped[0]))
}

fn glyph_indices(layout: &LineLayout) -> Vec<usize> {
    assert_eq!(layout.len, SOURCE.len());
    assert!(layout.width > px(0.));
    let indices = layout
        .runs
        .iter()
        .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.index))
        .collect::<Vec<_>>();
    assert_eq!(
        indices,
        SOURCE
            .char_indices()
            .map(|(index, _)| index)
            .collect::<Vec<_>>(),
    );
    indices
}

#[test]
fn downstream_consumes_rc_window_caches_with_shared_global_and_independent_layout_lifetimes() {
    let _: for<'a> fn(&'a Window) -> &'a Rc<WindowTextSystem> = Window::text_system;
    let backend = Arc::new(NoopTextSystem::new());
    let backend_lifetime = Arc::downgrade(&backend);
    let global = Arc::new(TextSystem::new(backend.clone()));
    let global_lifetime = Arc::downgrade(&global);
    let first = Rc::new(WindowTextSystem::new(global.clone()));
    let second = Rc::new(WindowTextSystem::new(global.clone()));
    let first_lifetime = Rc::downgrade(&first);
    let second_lifetime = Rc::downgrade(&second);
    assert!(!Rc::ptr_eq(&first, &second));
    for owner in [&first, &second] {
        let shared: &Arc<TextSystem> = owner.as_ref();
        assert!(Arc::ptr_eq(shared, &global));
    }
    let (plain, wrapped) = consume(&first, 120.);
    let indices = glyph_indices(&plain);
    let (repeated_plain, repeated_wrapped) = consume(&first, 120.);
    assert!(Arc::ptr_eq(&*plain, &*repeated_plain));
    assert!(Arc::ptr_eq(&wrapped, &repeated_wrapped));
    let (other_plain, other_wrapped) = consume(&second, 120.);
    assert!(!Arc::ptr_eq(&*plain, &*other_plain));
    assert!(!Arc::ptr_eq(&wrapped, &other_wrapped));
    let (narrow_plain, narrow_wrapped) = consume(&first, 60.);
    assert!(Arc::ptr_eq(&*plain, &*narrow_plain));
    assert!(!Arc::ptr_eq(&wrapped, &narrow_wrapped));
    assert!(narrow_wrapped.wrap_boundaries.len() > wrapped.wrap_boundaries.len());
    let retained = first.clone();
    drop(first);
    assert!(first_lifetime.upgrade().is_some());
    let rebuilt = Rc::new(WindowTextSystem::new(global.clone()));
    let rebuilt_lifetime = Rc::downgrade(&rebuilt);
    assert!(!Rc::ptr_eq(&retained, &rebuilt));
    let shared: &Arc<TextSystem> = rebuilt.as_ref();
    assert!(Arc::ptr_eq(shared, &global));
    let (rebuilt_plain, rebuilt_wrapped) = consume(&rebuilt, 120.);
    assert!(!Arc::ptr_eq(&*plain, &*rebuilt_plain));
    assert!(!Arc::ptr_eq(&wrapped, &rebuilt_wrapped));
    assert_eq!(glyph_indices(&rebuilt_plain), indices);
    drop(second);
    drop(rebuilt);
    assert!(second_lifetime.upgrade().is_none());
    assert!(rebuilt_lifetime.upgrade().is_none());
    drop(global);
    drop(backend);
    assert!(global_lifetime.upgrade().is_some());
    assert!(backend_lifetime.upgrade().is_some());
    drop(retained);
    assert!(first_lifetime.upgrade().is_none());
    assert!(global_lifetime.upgrade().is_none());
    assert!(backend_lifetime.upgrade().is_none());
    assert_eq!(glyph_indices(&plain), indices);
    assert_eq!(glyph_indices(&wrapped.unwrapped_layout), indices);
    assert_eq!(glyph_indices(&rebuilt_plain), indices);
}
