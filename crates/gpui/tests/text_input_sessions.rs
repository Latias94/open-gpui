#![cfg(feature = "test-support")]

//! Public input-session and visible-geometry consumers at fixed Zed target
//! 635026fabc38440fce5e1767d125026a17b12012, including fork candidate admission.

use open_gpui::{
    App, AppContext as _, AtlasKey, AtlasTextureId, AtlasTextureInstanceId, AtlasTextureLeaseEpoch,
    AtlasTextureLeaseError, AtlasTile, Autocapitalize, Bounds, Context, Corners, DevicePixels,
    Edges, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, PlatformAtlas, Point, QuitMode, Render, RenderImage,
    Size, Styled as _, SubtreeTransform, SubtreeTransformExt as _, SubtreeTransformOrigin,
    TestAppContext, TextInputAction, TextInputConfiguration, TextInputStateChange, TileId,
    UTF16Selection, Window, WindowHandle, WindowInsets, canvas, div, point, px, size,
};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};

fn prose_configuration() -> TextInputConfiguration {
    TextInputConfiguration {
        autocorrect: true,
        autocapitalize: Autocapitalize::Sentences,
        suggestions: true,
        input_action: TextInputAction::Done,
    }
}

struct TextField {
    focus: [FocusHandle; 2],
    configurations: [TextInputConfiguration; 2],
    text: String,
    selected: Range<usize>,
    marked: Option<Range<usize>>,
    editable: Option<Range<usize>>,
    accepts: bool,
    registered: bool,
    transform: SubtreeTransform,
    element_bounds: Option<Bounds<Pixels>>,
    point_queries: Vec<Point<Pixels>>,
    range_queries: Vec<Range<usize>>,
    native_selection_updates: Vec<Range<usize>>,
    replacements: Vec<(Option<Range<usize>>, String, Option<Range<usize>>)>,
    on_configuration_query: Option<Box<dyn FnOnce(&mut Window, &mut App)>>,
}

impl TextField {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: [cx.focus_handle(), cx.focus_handle()],
            configurations: [prose_configuration(), prose_configuration()],
            text: "a🦀e\u{301}中".into(),
            selected: 1..3,
            marked: None,
            editable: Some(1..6),
            accepts: true,
            registered: true,
            transform: SubtreeTransform::IDENTITY,
            element_bounds: None,
            point_queries: Vec::new(),
            range_queries: Vec::new(),
            native_selection_updates: Vec::new(),
            replacements: Vec::new(),
            on_configuration_query: None,
        }
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        self.range_queries.push(range.clone());
        let encoded: Vec<_> = self.text.encode_utf16().collect();
        *adjusted = Some(range.clone());
        String::from_utf16(encoded.get(range)?).ok()
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selected.clone(),
            reversed: true,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone()
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.replacements.push((range, text.to_owned(), None));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        let start = range
            .as_ref()
            .map_or(self.selected.start, |range| range.start);
        self.marked = Some(start..start + text.encode_utf16().count());
        self.replacements.push((range, text.to_owned(), selected));
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.range_queries.push(range);
        self.element_bounds = Some(bounds);
        Some(bounds)
    }

    fn selection_bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.range_queries.push(range);
        vec![Bounds::new(bounds.origin, size(px(8.0), px(12.0))), bounds]
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.point_queries.push(point);
        Some(self.selected.start)
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.native_selection_updates.push(range.clone());
        self.selected = range;
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.text.encode_utf16().count())
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        self.accepts
    }

    fn text_input_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TextInputConfiguration {
        let index = usize::from(self.focus[1].is_focused(window));
        let configuration = self.configurations[index].clone();
        if let Some(on_query) = self.on_configuration_query.take() {
            on_query(window, cx);
        }
        configuration
    }

    fn text_input_editable_range(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.editable.clone()
    }
}

impl Render for TextField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut fields = div().flex().flex_col();
        for focus in self.focus.clone() {
            let target = cx.entity();
            let registered = self.registered;
            fields = fields.child(
                div().track_focus(&focus).w(px(160.0)).h(px(24.0)).child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, (), window, app| {
                            if registered {
                                window.handle_input(
                                    &focus,
                                    ElementInputHandler::new(bounds, target),
                                    app,
                                );
                            }
                        },
                    )
                    .size_full(),
                ),
            );
        }
        fields.with_subtree_transform(self.transform)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Atlas,
    Prepaint,
    Paint,
}

#[derive(Default)]
struct CandidateControls {
    focus: RefCell<Option<FocusHandle>>,
    failure: Cell<Option<Failure>>,
    prepaint_text_input: RefCell<Option<FocusHandle>>,
    paint_text_input: RefCell<Option<FocusHandle>>,
}

struct SessionRoot {
    first: Entity<TextField>,
    second: Entity<TextField>,
    controls: Rc<CandidateControls>,
    image: Arc<RenderImage>,
}

impl Render for SessionRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let prepaint = self.controls.clone();
        let paint = self.controls.clone();
        let image = self.image.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                canvas(
                    move |_, window, app| {
                        if let Some(focus) = prepaint.prepaint_text_input.borrow_mut().take() {
                            window.notify_text_input_changed(&focus);
                            window.notify_text_selection_changed(&focus);
                            window.request_virtual_keyboard();
                        }
                        if let Some(focus) = prepaint.focus.borrow().as_ref() {
                            window.focus(focus, app);
                        }
                        assert_ne!(
                            prepaint.failure.get(),
                            Some(Failure::Prepaint),
                            "injected input candidate prepaint unwind"
                        );
                    },
                    |_, (), _, _| {},
                )
                .h(px(1.0)),
            )
            .child(self.first.clone())
            .child(self.second.clone())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        if let Some(focus) = paint.paint_text_input.borrow_mut().take() {
                            window.notify_text_input_changed(&focus);
                            window.notify_text_selection_changed(&focus);
                            window.dismiss_virtual_keyboard();
                        }
                        match paint.failure.get() {
                            Some(Failure::Atlas) => window
                                .paint_image(bounds, bounds, Corners::default(), image, 0, false)
                                .expect("the candidate image must reach atlas lease admission"),
                            Some(Failure::Paint) => {
                                panic!("injected input candidate paint unwind")
                            }
                            _ => {}
                        }
                    },
                )
                .w(px(8.0))
                .h(px(8.0)),
            )
    }
}

struct RefusingAtlas;

impl PlatformAtlas for RefusingAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: AtlasKey,
        _: &mut dyn FnMut() -> anyhow::Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> anyhow::Result<Option<AtlasTile>> {
        Ok(Some(AtlasTile {
            texture_id: AtlasTextureId {
                index: 1,
                kind: key.texture_kind(),
            },
            tile_id: TileId(1),
            padding: 0,
            bounds: Bounds::new(Point::default(), size(DevicePixels(1), DevicePixels(1))),
            texture_generation: 1,
            texture_generation_padding: 0,
        }))
    }

    fn remove(&self, _: &AtlasKey) {}

    fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
        AtlasTextureLeaseEpoch::INITIAL
    }

    unsafe fn acquire_atlas_texture_leases(
        &self,
        textures: &[AtlasTextureInstanceId],
    ) -> Result<AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
        Err(AtlasTextureLeaseError::TextureUnavailable {
            texture: textures[0],
            epoch: AtlasTextureLeaseEpoch::INITIAL,
        })
    }

    unsafe fn release_atlas_texture_leases(
        &self,
        _: AtlasTextureLeaseEpoch,
        _: &[AtlasTextureInstanceId],
    ) {
        panic!("a refused texture acquisition must create no release obligation");
    }
}

fn open(cx: &mut TestAppContext) -> (WindowHandle<SessionRoot>, Entity<SessionRoot>) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    cx.defer_next_window_frame_requests();
    let handle = cx.open_window(size(px(320.0), px(240.0)), |window, cx| {
        let first = cx.new(TextField::new);
        let second = cx.new(TextField::new);
        let focus = first.read(cx).focus[0].clone();
        window.focus(&focus, cx);
        SessionRoot {
            first,
            second,
            controls: Rc::new(CandidateControls::default()),
            image: Arc::new(RenderImage::new([image::Frame::new(
                image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255])),
            )])),
        }
    });
    let root = handle
        .entity(cx)
        .expect("the public window must retain its root");
    draw(cx, handle);
    (handle, root)
}

fn draw(cx: &mut TestAppContext, handle: WindowHandle<SessionRoot>) {
    cx.update_window(handle.into(), |_, window, app| {
        window.refresh();
        window.draw(app).clear(app);
    })
    .expect("a public input-session window must remain available");
}

#[open_gpui::test]
fn accepted_focus_and_configuration_changes_are_deduplicated_across_entities(
    cx: &mut TestAppContext,
) {
    let (handle, root) = open(cx);
    let (first, second) = root.read_with(cx, |root, _| (root.first.clone(), root.second.clone()));
    let focuses = first.read_with(cx, |field, _| field.focus.clone());
    let other = second.read_with(cx, |field, _| field.focus[0].clone());
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        [prose_configuration()]
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [TextInputStateChange::FocusGained]
    );

    for focus in [&focuses[0], &focuses[1], &other] {
        cx.update_window(handle.into(), |_, window, app| window.focus(focus, app))
            .unwrap();
        draw(cx, handle);
    }
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        [prose_configuration()]
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [TextInputStateChange::FocusGained]
    );

    let search = TextInputConfiguration {
        input_action: TextInputAction::Search,
        autocapitalize: Autocapitalize::Words,
        ..prose_configuration()
    };
    second.update(cx, |field, cx| {
        field.configurations[0] = search.clone();
        cx.notify();
    });
    draw(cx, handle);
    draw(cx, handle);
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        [prose_configuration(), search]
    );

    cx.update_window(handle.into(), |_, window, _| {
        window.notify_text_input_changed(&focuses[0]);
        window.notify_text_selection_changed(&focuses[1]);
        window.notify_text_input_changed(&other);
        window.notify_text_selection_changed(&other);
    })
    .unwrap();
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [
            TextInputStateChange::FocusGained,
            TextInputStateChange::ContentChanged,
            TextInputStateChange::SelectionChanged,
        ]
    );
    assert_eq!(cx.window_virtual_keyboard_requests(handle.into()), 0);
    cx.update_window(handle.into(), |_, window, _| {
        window.request_virtual_keyboard();
        window.dismiss_virtual_keyboard();
    })
    .unwrap();
    draw(cx, handle);
    assert_eq!(cx.window_virtual_keyboard_requests(handle.into()), 1);
    assert_eq!(cx.window_virtual_keyboard_dismissals(handle.into()), 1);

    cx.update_window(handle.into(), |_, window, app| window.blur(app))
        .unwrap();
    draw(cx, handle);
    draw(cx, handle);
    assert_eq!(
        cx.window_text_input_configurations(handle.into()).last(),
        Some(&TextInputConfiguration::default())
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [
            TextInputStateChange::FocusGained,
            TextInputStateChange::ContentChanged,
            TextInputStateChange::SelectionChanged,
            TextInputStateChange::FocusLost,
        ]
    );
    cx.update_window(handle.into(), |_, window, _| {
        window.notify_text_input_changed(&other);
        window.notify_text_selection_changed(&other);
    })
    .unwrap();
    assert_eq!(cx.window_text_input_state_changes(handle.into()).len(), 4);
}

#[open_gpui::test]
fn accepted_nonediting_handler_retains_configuration_independently_of_focus_state(
    cx: &mut TestAppContext,
) {
    let (handle, root) = open(cx);
    let field = root.read_with(cx, |root, _| root.first.clone());
    let focus = field.read_with(cx, |field, _| field.focus[0].clone());
    field.update(cx, |field, cx| {
        field.accepts = false;
        cx.notify();
    });
    draw(cx, handle);
    cx.update_window(handle.into(), |_, window, app| {
        assert_eq!(window.committed_focus(app), Some(focus.clone()));
        window.notify_text_input_changed(&focus);
        window.notify_text_selection_changed(&focus);
    })
    .unwrap();
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        [prose_configuration()]
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [
            TextInputStateChange::FocusGained,
            TextInputStateChange::FocusLost
        ]
    );
    assert_eq!(
        cx.with_window_input_handler(handle.into(), |handler| handler.query_accepts_text_input()),
        Some(false)
    );
    field.update(cx, |field, cx| {
        field.accepts = true;
        cx.notify();
    });
    draw(cx, handle);
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        [prose_configuration()]
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [
            TextInputStateChange::FocusGained,
            TextInputStateChange::FocusLost,
            TextInputStateChange::FocusGained,
        ]
    );
}

#[open_gpui::test]
fn refused_and_unwound_candidates_preserve_the_accepted_text_session(cx: &mut TestAppContext) {
    let (handle, root) = open(cx);
    let (first, second, controls) = root.read_with(cx, |root, _| {
        (
            root.first.clone(),
            root.second.clone(),
            root.controls.clone(),
        )
    });
    let accepted = first.read_with(cx, |field, _| field.focus[0].clone());
    let attempted = second.read_with(cx, |field, _| field.focus[0].clone());
    second.update(cx, |field, _| {
        field.selected = 5..6;
        field.configurations[0] = TextInputConfiguration {
            input_action: TextInputAction::Send,
            ..Default::default()
        };
    });
    cx.update_window(handle.into(), |_, window, _| {
        window.set_sprite_atlas_for_test(Arc::new(RefusingAtlas));
    })
    .unwrap();
    let baseline_configuration = cx.window_text_input_configurations(handle.into());
    let baseline_changes = cx.window_text_input_state_changes(handle.into());
    let baseline_keyboard = cx.window_virtual_keyboard_requests(handle.into());

    for failure in [Failure::Atlas, Failure::Prepaint, Failure::Paint] {
        *controls.focus.borrow_mut() = Some(attempted.clone());
        controls.failure.set(Some(failure));
        cx.update_window(handle.into(), |_, window, app| {
            let generation = window.rendered_frame_revision();
            window.refresh();
            let result = catch_unwind(AssertUnwindSafe(|| window.draw(app).clear(app)));
            assert_eq!(result.is_err(), failure != Failure::Atlas);
            assert_eq!(window.rendered_frame_revision(), generation);
            assert_eq!(window.focused(app), Some(accepted.clone()));
            assert_eq!(window.committed_focus(app), Some(accepted.clone()));
            controls.failure.set(Some(Failure::Atlas));
        })
        .expect("a refused or unwound candidate must preserve the live window");
        assert_eq!(
            cx.window_text_input_configurations(handle.into()),
            baseline_configuration
        );
        assert_eq!(
            cx.window_text_input_state_changes(handle.into()),
            baseline_changes
        );
        assert_eq!(
            cx.window_virtual_keyboard_requests(handle.into()),
            baseline_keyboard
        );
        assert_eq!(
            cx.with_window_input_handler(handle.into(), |handler| {
                (
                    handler.query_accepts_focused_text_input(),
                    handler
                        .selected_text_range(false)
                        .map(|selection| (selection.range, selection.reversed)),
                )
            }),
            Some((true, Some((1..3, true))))
        );
    }
    controls.failure.set(None);
    draw(cx, handle);
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        [
            prose_configuration(),
            TextInputConfiguration {
                input_action: TextInputAction::Send,
                ..Default::default()
            },
        ]
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [TextInputStateChange::FocusGained]
    );
}

#[open_gpui::test]
fn candidate_text_input_intents_publish_once_only_after_acceptance(cx: &mut TestAppContext) {
    let (handle, root) = open(cx);
    let (field, controls) =
        root.read_with(cx, |root, _| (root.first.clone(), root.controls.clone()));
    let focus = field.read_with(cx, |field, _| field.focus[0].clone());
    cx.update_window(handle.into(), |_, window, _| {
        window.set_sprite_atlas_for_test(Arc::new(RefusingAtlas));
    })
    .unwrap();
    let baseline_configuration = cx.window_text_input_configurations(handle.into());
    let mut expected_changes = cx.window_text_input_state_changes(handle.into());
    let mut expected_requests = cx.window_virtual_keyboard_requests(handle.into());
    let mut expected_dismissals = cx.window_virtual_keyboard_dismissals(handle.into());

    for (failure, in_paint) in [
        (Failure::Atlas, false),
        (Failure::Prepaint, false),
        (Failure::Atlas, true),
        (Failure::Paint, true),
    ] {
        let intents = if in_paint {
            &controls.paint_text_input
        } else {
            &controls.prepaint_text_input
        };
        *intents.borrow_mut() = Some(focus.clone());
        controls.failure.set(Some(failure));
        cx.update_window(handle.into(), |_, window, app| {
            let revision = window.rendered_frame_revision();
            window.refresh();
            let result = catch_unwind(AssertUnwindSafe(|| window.draw(app).clear(app)));
            assert_eq!(result.is_err(), failure != Failure::Atlas);
            assert_eq!(window.rendered_frame_revision(), revision);
            assert_eq!(window.committed_focus(app), Some(focus.clone()));
            controls.failure.set(Some(Failure::Atlas));
        })
        .unwrap();
        assert!(
            intents.borrow().is_none(),
            "the candidate must emit its intents"
        );
        assert_eq!(
            cx.window_text_input_configurations(handle.into()),
            baseline_configuration
        );
        assert_eq!(
            cx.window_text_input_state_changes(handle.into()),
            expected_changes
        );
        assert_eq!(
            cx.window_virtual_keyboard_requests(handle.into()),
            expected_requests
        );
        assert_eq!(
            cx.window_virtual_keyboard_dismissals(handle.into()),
            expected_dismissals
        );

        *intents.borrow_mut() = Some(focus.clone());
        controls.failure.set(None);
        cx.update_window(handle.into(), |_, window, app| {
            let revision = window.rendered_frame_revision();
            window.refresh();
            window.draw(app).clear(app);
            assert!(window.rendered_frame_revision() > revision);
        })
        .unwrap();
        draw(cx, handle);
        expected_changes.extend([
            TextInputStateChange::ContentChanged,
            TextInputStateChange::SelectionChanged,
        ]);
        if in_paint {
            expected_dismissals += 1;
        } else {
            expected_requests += 1;
        }
        assert_eq!(
            cx.window_text_input_configurations(handle.into()),
            baseline_configuration
        );
        assert_eq!(
            cx.window_text_input_state_changes(handle.into()),
            expected_changes
        );
        assert_eq!(
            cx.window_virtual_keyboard_requests(handle.into()),
            expected_requests
        );
        assert_eq!(
            cx.window_virtual_keyboard_dismissals(handle.into()),
            expected_dismissals
        );
    }
}

#[open_gpui::test]
fn configuration_query_blur_does_not_publish_stale_text_session(cx: &mut TestAppContext) {
    let (handle, root) = open(cx);
    let (field, controls) =
        root.read_with(cx, |root, _| (root.first.clone(), root.controls.clone()));
    let focus = field.read_with(cx, |field, _| field.focus[0].clone());
    cx.update_window(handle.into(), |_, window, app| window.blur(app))
        .unwrap();
    draw(cx, handle);
    let baseline_configuration = cx.window_text_input_configurations(handle.into());
    let baseline_changes = cx.window_text_input_state_changes(handle.into());
    assert_eq!(
        baseline_changes,
        [
            TextInputStateChange::FocusGained,
            TextInputStateChange::FocusLost
        ]
    );
    let queried = Rc::new(Cell::new(false));
    field.update(cx, |field, _| {
        field.configurations[0] = TextInputConfiguration {
            input_action: TextInputAction::Search,
            ..Default::default()
        };
        let queried = queried.clone();
        field.on_configuration_query = Some(Box::new(move |window, app| {
            queried.set(true);
            window.blur(app);
        }));
    });
    cx.update_window(handle.into(), |_, window, app| {
        window.set_sprite_atlas_for_test(Arc::new(RefusingAtlas));
        window.focus(&focus, app);
        let revision = window.rendered_frame_revision();
        window.refresh();
        window.draw(app).clear(app);
        assert!(window.rendered_frame_revision() > revision);
        controls.failure.set(Some(Failure::Atlas));
    })
    .unwrap();
    assert!(
        queried.get(),
        "the accepted handler must run the user configuration query"
    );
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        baseline_configuration
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        baseline_changes
    );
    controls.failure.set(None);
    draw(cx, handle);
    draw(cx, handle);
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        baseline_configuration
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        baseline_changes
    );
    cx.update_window(handle.into(), |_, window, app| {
        assert_eq!(window.focused(app), None);
        assert_eq!(window.committed_focus(app), None);
    })
    .unwrap();
    assert_eq!(cx.with_window_input_handler(handle.into(), |_| ()), None);
}

#[open_gpui::test]
fn configuration_query_focus_redirect_does_not_publish_stale_text_session(cx: &mut TestAppContext) {
    let (handle, root) = open(cx);
    let (first, second, controls) = root.read_with(cx, |root, _| {
        (
            root.first.clone(),
            root.second.clone(),
            root.controls.clone(),
        )
    });
    let original = first.read_with(cx, |field, _| field.focus[0].clone());
    let redirected = second.read_with(cx, |field, _| field.focus[0].clone());
    cx.update_window(handle.into(), |_, window, app| window.blur(app))
        .unwrap();
    draw(cx, handle);
    let mut expected_configurations = cx.window_text_input_configurations(handle.into());
    let mut expected_changes = cx.window_text_input_state_changes(handle.into());
    assert_eq!(
        expected_changes,
        [
            TextInputStateChange::FocusGained,
            TextInputStateChange::FocusLost
        ]
    );
    let target_configuration = TextInputConfiguration {
        input_action: TextInputAction::Send,
        ..Default::default()
    };
    second.update(cx, |field, _| {
        field.text = "new target".into();
        field.selected = 5..6;
        field.configurations[0] = target_configuration.clone();
    });
    let queried = Rc::new(Cell::new(false));
    first.update(cx, |field, _| {
        field.configurations[0] = TextInputConfiguration {
            input_action: TextInputAction::Search,
            ..Default::default()
        };
        let target = redirected.clone();
        let queried = queried.clone();
        field.on_configuration_query = Some(Box::new(move |window, app| {
            queried.set(true);
            window.focus(&target, app);
        }));
    });
    cx.update_window(handle.into(), |_, window, app| {
        window.set_sprite_atlas_for_test(Arc::new(RefusingAtlas));
        window.focus(&original, app);
        let revision = window.rendered_frame_revision();
        window.refresh();
        window.draw(app).clear(app);
        assert!(window.rendered_frame_revision() > revision);
        controls.failure.set(Some(Failure::Atlas));
    })
    .unwrap();
    assert!(
        queried.get(),
        "the accepted handler must redirect focus during its query"
    );
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        expected_configurations
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        expected_changes
    );
    controls.failure.set(None);
    draw(cx, handle);
    draw(cx, handle);
    expected_configurations.push(target_configuration);
    expected_changes.push(TextInputStateChange::FocusGained);
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        expected_configurations
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        expected_changes
    );
    cx.update_window(handle.into(), |_, window, app| {
        assert_eq!(window.focused(app), Some(redirected.clone()));
        assert_eq!(window.committed_focus(app), Some(redirected.clone()));
    })
    .unwrap();
    assert_eq!(
        cx.with_window_input_handler(handle.into(), |handler| {
            let mut adjusted = None;
            let text = handler.text_for_range(0..10, &mut adjusted);
            (text, adjusted)
        }),
        Some((Some("new target".into()), Some(0..10)))
    );
}

#[open_gpui::test]
fn nested_configuration_query_keeps_new_same_focus_handler(cx: &mut TestAppContext) {
    let (handle, root) = open(cx);
    let (original, controls) =
        root.read_with(cx, |root, _| (root.first.clone(), root.controls.clone()));
    let focuses = original.read_with(cx, |field, _| field.focus.clone());
    let replacement = cx.new(TextField::new);
    replacement.update(cx, |field, _| {
        field.focus = focuses.clone();
        field.text = "replacement".into();
        field.selected = 5..6;
    });
    let nested_revision = Rc::new(Cell::new(None));
    original.update(cx, |field, _| {
        let root = root.clone();
        let replacement = replacement.clone();
        let focus = focuses[0].clone();
        let nested_revision = nested_revision.clone();
        field.on_configuration_query = Some(Box::new(move |window, app| {
            root.update(app, |root, _| root.first = replacement);
            let revision = window.rendered_frame_revision();
            window.refresh();
            window.draw(app).clear(app);
            assert!(window.rendered_frame_revision() > revision);
            assert_eq!(window.committed_focus(app), Some(focus));
            nested_revision.set(Some(window.rendered_frame_revision()));
        }));
    });
    cx.update_window(handle.into(), |_, window, app| {
        window.set_sprite_atlas_for_test(Arc::new(RefusingAtlas));
        window.refresh();
        window.draw(app).clear(app);
        assert_eq!(
            nested_revision.get(),
            Some(window.rendered_frame_revision())
        );
        controls.failure.set(Some(Failure::Atlas));
    })
    .unwrap();
    let original_queries = original.read_with(cx, |field, _| field.range_queries.clone());
    let mut replacement_queries = replacement.read_with(cx, |field, _| field.range_queries.clone());
    assert_eq!(
        cx.with_window_input_handler(handle.into(), |handler| {
            let mut adjusted = None;
            let text = handler.text_for_range(0..11, &mut adjusted);
            let selection = handler
                .selected_text_range(false)
                .map(|selection| (selection.range, selection.reversed));
            (text, adjusted, selection)
        }),
        Some((Some("replacement".into()), Some(0..11), Some((5..6, true))))
    );
    assert_eq!(
        original.read_with(cx, |field, _| field.range_queries.clone()),
        original_queries
    );
    replacement_queries.push(0..11);
    assert_eq!(
        replacement.read_with(cx, |field, _| field.range_queries.clone()),
        replacement_queries
    );
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        [prose_configuration()]
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [TextInputStateChange::FocusGained]
    );
}

#[open_gpui::test]
fn platform_session_forwards_utf16_ranges_and_batched_selection_geometry(cx: &mut TestAppContext) {
    let (handle, root) = open(cx);
    let field = root.read_with(cx, |root, _| root.first.clone());
    field.update(cx, |field, cx| {
        field.transform = SubtreeTransform::try_new(
            size(1.5, 2.0),
            point(px(12.0), px(14.0)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .unwrap();
        cx.notify();
    });
    draw(cx, handle);
    field.update(cx, |field, _| field.range_queries.clear());
    let bounds = cx
        .with_window_input_handler(handle.into(), |handler| {
            assert_eq!(handler.text_length_utf16(), Some(6));
            assert_eq!(handler.text_input_editable_range(), Some(1..6));
            let mut adjusted = None;
            assert_eq!(
                handler.text_for_range(1..3, &mut adjusted),
                Some("🦀".into())
            );
            assert_eq!(adjusted, Some(1..3));
            assert_eq!(
                handler.text_for_range(3..5, &mut adjusted),
                Some("e\u{301}".into())
            );
            assert_eq!(adjusted, Some(3..5));
            handler.set_selected_text_range(3..5);
            assert_eq!(
                handler
                    .selected_text_range(false)
                    .map(|selection| (selection.range, selection.reversed)),
                Some((3..5, true))
            );
            handler.replace_and_mark_text_in_range(Some(1..3), "語🦀", Some(1..3));
            assert_eq!(handler.marked_text_range(), Some(1..4));
            handler.unmark_text();
            assert_eq!(handler.marked_text_range(), None);
            handler.replace_text_in_range(Some(3..5), "é");
            let bounds = handler.bounds_for_range(1..3).expect("field bounds");
            assert_eq!(
                handler.selection_bounds_for_range(1..6),
                [Bounds::new(bounds.origin, size(px(12.0), px(24.0))), bounds]
            );
            assert_eq!(
                handler.character_index_for_point(bounds.origin + point(px(6.0), px(12.0))),
                Some(3)
            );
            bounds
        })
        .expect("the accepted frame must install a usable platform input lease");
    assert!(bounds.size.width > px(0.0));
    field.read_with(cx, |field, _| {
        let local = field
            .element_bounds
            .expect("the adapter must receive layout bounds");
        assert_eq!(bounds.origin, local.origin + point(px(12.0), px(14.0)));
        assert_eq!(
            bounds.size,
            size(local.size.width * 1.5, local.size.height * 2.0)
        );
        assert_eq!(
            field.point_queries,
            [local.origin + point(px(4.0), px(6.0))]
        );
        assert_eq!(field.native_selection_updates.len(), 1);
        assert_eq!(field.native_selection_updates[0], 3..5);
        assert_eq!(field.range_queries, [1..3, 3..5, 1..3, 1..6]);
        assert_eq!(
            field.replacements,
            [
                (Some(1..3), "語🦀".into(), Some(1..3)),
                (Some(3..5), "é".into(), None),
            ]
        );
    });
    field.update(cx, |field, _| field.editable = None);
    assert_eq!(
        cx.with_window_input_handler(handle.into(), |handler| handler.text_input_editable_range()),
        Some(None)
    );
}

#[open_gpui::test]
fn handler_loss_quiescence_and_retirement_end_the_session_once(cx: &mut TestAppContext) {
    let (handle, root) = open(cx);
    let field = root.read_with(cx, |root, _| root.first.clone());
    let focus = field.read_with(cx, |field, _| field.focus[0].clone());
    field.update(cx, |field, cx| {
        field.registered = false;
        cx.notify();
    });
    draw(cx, handle);
    draw(cx, handle);
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [
            TextInputStateChange::FocusGained,
            TextInputStateChange::FocusLost
        ]
    );
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        [prose_configuration(), TextInputConfiguration::default()]
    );
    assert_eq!(cx.with_window_input_handler(handle.into(), |_| ()), None);

    field.update(cx, |field, cx| {
        field.registered = true;
        cx.notify();
    });
    draw(cx, handle);
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [
            TextInputStateChange::FocusGained,
            TextInputStateChange::FocusLost,
            TextInputStateChange::FocusGained,
        ]
    );
    cx.update_window(handle.into(), |_, window, app| {
        assert!(window.quiesce_interaction(app));
        assert!(!window.quiesce_interaction(app));
        window.request_virtual_keyboard();
        window.notify_text_input_changed(&focus);
        window.notify_text_selection_changed(&focus);
    })
    .unwrap();
    draw(cx, handle);
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        [
            TextInputStateChange::FocusGained,
            TextInputStateChange::FocusLost,
            TextInputStateChange::FocusGained,
            TextInputStateChange::FocusLost,
        ]
    );
    assert_eq!(cx.window_virtual_keyboard_requests(handle.into()), 0);
    assert_eq!(cx.with_window_input_handler(handle.into(), |_| ()), None);
    assert_eq!(
        cx.window_text_input_configurations(handle.into()).last(),
        Some(&TextInputConfiguration::default())
    );
    let terminal = cx.hold_window_native_terminal(handle.into());
    cx.update_window(handle.into(), |_, window, app| window.remove_window(app))
        .unwrap();
    cx.run_until_parked();
    assert!(cx.windows().is_empty());
    drop(terminal);
    cx.run_until_parked();
    let (replacement, _) = open(cx);
    assert_ne!(replacement.window_id(), handle.window_id());
    assert_eq!(
        cx.window_text_input_state_changes(replacement.into()),
        [TextInputStateChange::FocusGained]
    );
}

#[open_gpui::test]
fn viewport_and_full_insets_update_visible_geometry_without_resizing_layout(
    cx: &mut TestAppContext,
) {
    let (handle, _) = open(cx);
    let original = cx
        .update_window(handle.into(), |_, window, _| {
            assert_eq!(
                window.visual_viewport_bounds(),
                Bounds::new(Point::default(), window.viewport_size())
            );
            assert_eq!(window.insets(), WindowInsets::default());
            (window.bounds(), window.viewport_size())
        })
        .unwrap();
    let viewport = Bounds::new(point(px(4.0), px(10.0)), size(px(290.0), px(170.0)));
    let insets = WindowInsets {
        safe_area: Edges {
            top: px(20.0),
            right: px(12.0),
            bottom: px(16.0),
            left: px(8.0),
        },
        ime: Edges {
            bottom: px(100.0),
            ..Default::default()
        },
        corner_radii: Corners {
            top_left: px(28.0),
            top_right: px(26.0),
            bottom_right: px(18.0),
            bottom_left: px(16.0),
        },
    };
    cx.simulate_window_visual_viewport_change(handle.into(), viewport);
    cx.simulate_window_insets_change(handle.into(), insets.clone());
    draw(cx, handle);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.visual_viewport_bounds(), viewport);
        assert_eq!(window.insets(), insets);
        assert_eq!((window.bounds(), window.viewport_size()), original);
        assert_eq!(
            window.fully_visible_bounds(),
            Bounds::new(point(px(8.0), px(20.0)), size(px(286.0), px(120.0)))
        );
    })
    .unwrap();

    let covered = WindowInsets {
        safe_area: Edges {
            top: px(-10.0),
            left: px(900.0),
            right: px(900.0),
            ..Default::default()
        },
        ime: Edges {
            bottom: px(900.0),
            ..Default::default()
        },
        ..Default::default()
    };
    cx.simulate_window_insets_change(handle.into(), covered);
    draw(cx, handle);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.fully_visible_bounds().size, size(px(0.0), px(0.0)));
        assert_eq!(window.viewport_size(), original.1);
    })
    .unwrap();
    cx.simulate_window_visual_viewport_change(
        handle.into(),
        Bounds::new(Point::default(), original.1),
    );
    cx.simulate_window_insets_change(handle.into(), WindowInsets::default());
    draw(cx, handle);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.fully_visible_bounds(),
            Bounds::new(Point::default(), original.1)
        );
        assert_eq!(window.viewport_size(), original.1);
    })
    .unwrap();
}

#[open_gpui::test]
fn committed_focus_keyboard_request_supersedes_candidate_dismissal(cx: &mut TestAppContext) {
    let (handle, root) = open(cx);
    let (field, controls) =
        root.read_with(cx, |root, _| (root.first.clone(), root.controls.clone()));
    let focus = field.read_with(cx, |field, _| field.focus[1].clone());
    let requested = Rc::new(Cell::new(0));
    let baseline_requests = cx.window_virtual_keyboard_requests(handle.into());
    let baseline_dismissals = cx.window_virtual_keyboard_dismissals(handle.into());
    *controls.paint_text_input.borrow_mut() = Some(focus.clone());

    cx.update_window(handle.into(), |_, window, app| {
        let requested = requested.clone();
        let _subscription = window.on_focus_committed(&focus, app, move |window, _| {
            requested.set(requested.get() + 1);
            window.request_virtual_keyboard();
        });
        window.focus(&focus, app);
        let revision = window.rendered_frame_revision();
        window.refresh();
        window.draw(app).clear(app);
        assert!(window.rendered_frame_revision() > revision);
        assert_eq!(window.committed_focus(app), Some(focus.clone()));
    })
    .unwrap();
    draw(cx, handle);

    assert!(controls.paint_text_input.borrow().is_none());
    assert_eq!(requested.get(), 1);
    assert_eq!(
        cx.window_virtual_keyboard_requests(handle.into()),
        baseline_requests + 1
    );
    assert_eq!(
        cx.window_virtual_keyboard_dismissals(handle.into()),
        baseline_dismissals
    );
}

#[open_gpui::test]
fn configuration_query_panic_discards_candidate_text_input_effects(cx: &mut TestAppContext) {
    let (handle, root) = open(cx);
    let (field, controls) =
        root.read_with(cx, |root, _| (root.first.clone(), root.controls.clone()));
    let focus = field.read_with(cx, |field, _| field.focus[0].clone());
    let baseline_configurations = cx.window_text_input_configurations(handle.into());
    let baseline_changes = cx.window_text_input_state_changes(handle.into());
    let baseline_requests = cx.window_virtual_keyboard_requests(handle.into());
    let baseline_dismissals = cx.window_virtual_keyboard_dismissals(handle.into());
    let queried = Rc::new(Cell::new(false));
    field.update(cx, |field, _| {
        field.configurations[0] = TextInputConfiguration {
            input_action: TextInputAction::Search,
            ..prose_configuration()
        };
        let queried = queried.clone();
        field.on_configuration_query = Some(Box::new(move |_, _| {
            queried.set(true);
            panic!("injected accepted text input configuration query unwind");
        }));
    });
    *controls.prepaint_text_input.borrow_mut() = Some(focus.clone());

    cx.update_window(handle.into(), |_, window, app| {
        window.set_sprite_atlas_for_test(Arc::new(RefusingAtlas));
        let revision = window.rendered_frame_revision();
        window.refresh();
        let result = catch_unwind(AssertUnwindSafe(|| window.draw(app).clear(app)));
        assert!(result.is_err());
        assert!(window.rendered_frame_revision() > revision);
        assert_eq!(window.focused(app), Some(focus.clone()));
        assert_eq!(window.committed_focus(app), Some(focus.clone()));
        controls.failure.set(Some(Failure::Atlas));
    })
    .unwrap();

    assert!(queried.get());
    assert!(controls.prepaint_text_input.borrow().is_none());
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        baseline_configurations
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        baseline_changes
    );
    assert_eq!(
        cx.window_virtual_keyboard_requests(handle.into()),
        baseline_requests
    );
    assert_eq!(
        cx.window_virtual_keyboard_dismissals(handle.into()),
        baseline_dismissals
    );

    controls.failure.set(None);
    draw(cx, handle);
    draw(cx, handle);
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        baseline_changes
    );
    assert_eq!(
        cx.window_virtual_keyboard_requests(handle.into()),
        baseline_requests
    );
}

#[open_gpui::test]
fn configuration_query_same_focus_claim_discards_candidate_text_input_effects(
    cx: &mut TestAppContext,
) {
    let (handle, root) = open(cx);
    let (field, controls) =
        root.read_with(cx, |root, _| (root.first.clone(), root.controls.clone()));
    let focus = field.read_with(cx, |field, _| field.focus[0].clone());
    let mut expected_configurations = cx.window_text_input_configurations(handle.into());
    let baseline_changes = cx.window_text_input_state_changes(handle.into());
    let baseline_requests = cx.window_virtual_keyboard_requests(handle.into());
    let baseline_dismissals = cx.window_virtual_keyboard_dismissals(handle.into());
    let configuration = TextInputConfiguration {
        input_action: TextInputAction::Search,
        ..prose_configuration()
    };
    let queried = Rc::new(Cell::new(false));
    field.update(cx, |field, _| {
        field.configurations[0] = configuration.clone();
        let focus = focus.clone();
        let queried = queried.clone();
        field.on_configuration_query = Some(Box::new(move |window, app| {
            queried.set(true);
            let revision = window.focus_claim_revision();
            window.focus(&focus, app);
            assert_ne!(window.focus_claim_revision(), revision);
            assert_eq!(window.focused(app), Some(focus.clone()));
        }));
    });
    *controls.prepaint_text_input.borrow_mut() = Some(focus.clone());

    cx.update_window(handle.into(), |_, window, app| {
        window.set_sprite_atlas_for_test(Arc::new(RefusingAtlas));
        let revision = window.rendered_frame_revision();
        window.refresh();
        window.draw(app).clear(app);
        assert!(window.rendered_frame_revision() > revision);
        assert_eq!(window.focused(app), Some(focus.clone()));
        assert_eq!(window.committed_focus(app), Some(focus.clone()));
        controls.failure.set(Some(Failure::Atlas));
    })
    .unwrap();

    assert!(queried.get());
    assert!(controls.prepaint_text_input.borrow().is_none());
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        expected_configurations
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        baseline_changes
    );
    assert_eq!(
        cx.window_virtual_keyboard_requests(handle.into()),
        baseline_requests
    );
    assert_eq!(
        cx.window_virtual_keyboard_dismissals(handle.into()),
        baseline_dismissals
    );

    controls.failure.set(None);
    draw(cx, handle);
    draw(cx, handle);
    expected_configurations.push(configuration);
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        expected_configurations
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        baseline_changes
    );
    assert_eq!(
        cx.window_virtual_keyboard_requests(handle.into()),
        baseline_requests
    );
}

#[open_gpui::test]
fn refused_nested_candidate_preserves_outer_accepted_text_input_effects(cx: &mut TestAppContext) {
    let (handle, root) = open(cx);
    let (original, controls) =
        root.read_with(cx, |root, _| (root.first.clone(), root.controls.clone()));
    let focuses = original.read_with(cx, |field, _| field.focus.clone());
    let focus = focuses[0].clone();
    let replacement = cx.new(TextField::new);
    replacement.update(cx, |field, _| field.focus = focuses);
    let baseline_configurations = cx.window_text_input_configurations(handle.into());
    let mut expected_changes = cx.window_text_input_state_changes(handle.into());
    let baseline_requests = cx.window_virtual_keyboard_requests(handle.into());
    let baseline_dismissals = cx.window_virtual_keyboard_dismissals(handle.into());
    let nested_revision = Rc::new(Cell::new(None));
    original.update(cx, |field, _| {
        let root = root.clone();
        let controls = controls.clone();
        let focus = focus.clone();
        let nested_revision = nested_revision.clone();
        field.on_configuration_query = Some(Box::new(move |window, app| {
            root.update(app, |root, _| root.first = replacement);
            controls.failure.set(Some(Failure::Atlas));
            *controls.paint_text_input.borrow_mut() = Some(focus.clone());
            let revision = window.rendered_frame_revision();
            window.refresh();
            window.draw(app).clear(app);
            assert_eq!(window.rendered_frame_revision(), revision);
            assert_eq!(window.committed_focus(app), Some(focus));
            assert!(controls.paint_text_input.borrow().is_none());
            nested_revision.set(Some(revision));
        }));
    });
    *controls.prepaint_text_input.borrow_mut() = Some(focus.clone());

    cx.update_window(handle.into(), |_, window, app| {
        window.set_sprite_atlas_for_test(Arc::new(RefusingAtlas));
        let revision = window.rendered_frame_revision();
        window.refresh();
        window.draw(app).clear(app);
        assert!(window.rendered_frame_revision() > revision);
        assert_eq!(
            nested_revision.get(),
            Some(window.rendered_frame_revision())
        );
        assert_eq!(window.focused(app), Some(focus.clone()));
        assert_eq!(window.committed_focus(app), Some(focus.clone()));
    })
    .unwrap();

    assert!(controls.prepaint_text_input.borrow().is_none());
    expected_changes.extend([
        TextInputStateChange::ContentChanged,
        TextInputStateChange::SelectionChanged,
    ]);
    assert_eq!(
        cx.window_text_input_configurations(handle.into()),
        baseline_configurations
    );
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        expected_changes
    );
    assert_eq!(
        cx.window_virtual_keyboard_requests(handle.into()),
        baseline_requests + 1
    );
    assert_eq!(
        cx.window_virtual_keyboard_dismissals(handle.into()),
        baseline_dismissals
    );

    controls.failure.set(None);
    draw(cx, handle);
    draw(cx, handle);
    assert_eq!(
        cx.window_text_input_state_changes(handle.into()),
        expected_changes
    );
    assert_eq!(
        cx.window_virtual_keyboard_requests(handle.into()),
        baseline_requests + 1
    );
    assert_eq!(
        cx.window_virtual_keyboard_dismissals(handle.into()),
        baseline_dismissals
    );
}
