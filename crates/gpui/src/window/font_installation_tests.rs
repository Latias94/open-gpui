use super::cached_paint_atlas_tests::RejectOnceAtlas;
use super::*;
use crate::text_system::font_installation_tests::DynamicPlatform;
use crate::{AtlasTextureKind, HeadlessAppContext, TextRun, canvas, div, font};

struct FontCandidateRoot {
    layouts: Rc<RefCell<Vec<CandidateLayouts>>>,
}

#[derive(Clone)]
struct CandidateLayouts {
    plain: Arc<crate::LineLayout>,
    wrapped: Arc<crate::WrappedLineLayout>,
    shaped: crate::ShapedLine,
}

impl Render for FontCandidateRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let layouts = self.layouts.clone();
        div().size_full().child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let source = "candidate layout";
                    let runs = [TextRun {
                        len: source.len(),
                        font: font("primary"),
                        ..Default::default()
                    }];
                    let line = window
                        .text_system()
                        .layout_line(source, px(16.), &runs, None);
                    let wrapped = window
                        .text_system()
                        .shape_text(source.into(), px(16.), &runs, Some(px(40.)), None)
                        .unwrap();
                    let shaped =
                        window
                            .text_system()
                            .shape_line(source.into(), px(16.), &runs, None);
                    assert_eq!(shaped.text.as_ref(), source);
                    layouts.borrow_mut().push(CandidateLayouts {
                        plain: line,
                        wrapped: wrapped[0].layout.clone(),
                        shaped,
                    });
                    window.insert_scene_primitive(MonochromeSprite {
                        order: 0,
                        animation: Default::default(),
                        transformation: Default::default(),
                        bounds: window.device_local_bounds(bounds),
                        clip: Default::default(),
                        color: crate::red(),
                        tile: RejectOnceAtlas::tile(AtlasTextureKind::Monochrome),
                        transform: PrimitiveTransform::IDENTITY,
                    });
                },
            )
            .size_full(),
        )
    }
}

fn candidate_retry(install: bool) {
    let mut app = HeadlessAppContext::new(Arc::new(DynamicPlatform::default()));
    app.update(|cx| cx.set_quit_mode(crate::QuitMode::Explicit));
    let atlas = Arc::new(RejectOnceAtlas::new());
    let layouts = Rc::new(RefCell::new(Vec::new()));
    let handle = app
        .open_window(size(px(160.), px(80.)), {
            let atlas = atlas.clone();
            let layouts = layouts.clone();
            move |window, cx| {
                window.sprite_atlas = atlas;
                cx.new(|_| FontCandidateRoot { layouts })
            }
        })
        .unwrap();
    app.update_window(handle.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let original = layouts.borrow().last().unwrap().clone();
    if install {
        app.update(|cx| {
            cx.text_system()
                .add_fonts(vec![Cow::Borrowed(b"font")])
                .unwrap()
        });
    }
    app.update_window(handle.into(), |_, window, cx| {
        let committed = window.rendered_frame_revision();
        let scene_len = window.rendered_frame.scene.len();
        atlas.fail_next_lease();
        window.refresh();
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), committed);
        assert_eq!(window.rendered_frame.scene.len(), scene_len);
        assert_eq!(
            window
                .last_atlas_frame_rejection
                .as_ref()
                .unwrap()
                .generation,
            committed + 1
        );
        let candidate = layouts.borrow().last().unwrap().clone();
        for (before, after) in [
            (&original.plain, &candidate.plain),
            (&original.shaped.layout, &candidate.shaped.layout),
        ] {
            assert_eq!(Arc::ptr_eq(before, after), !install);
            assert_eq!(after.width, before.width * if install { 2. } else { 1. });
        }
        assert_eq!(Arc::ptr_eq(&original.wrapped, &candidate.wrapped), !install);
        assert_eq!(
            candidate.wrapped.unwrapped_layout.width,
            original.wrapped.unwrapped_layout.width * if install { 2. } else { 1. }
        );
        window.draw(cx).clear(cx);
        assert_eq!(window.rendered_frame_revision(), committed + 1);
        let recorded = layouts.borrow();
        let retried = recorded.last().unwrap();
        assert!(Arc::ptr_eq(&candidate.plain, &retried.plain));
        assert!(Arc::ptr_eq(&candidate.wrapped, &retried.wrapped));
        assert_eq!(retried.shaped.text.as_ref(), "candidate layout");
        assert!(Arc::ptr_eq(
            &candidate.shaped.layout,
            &retried.shaped.layout
        ));
        assert!(!window.refresh_pending_for_test());
        window.remove_window(cx);
    })
    .unwrap();
}

#[test]
fn rejected_font_candidate_reuses_same_generation_layout_on_retry() {
    candidate_retry(false);
}

#[test]
fn rejected_font_candidate_never_restores_preinstallation_layouts() {
    candidate_retry(true);
}
