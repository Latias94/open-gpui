//! Ordinary consumers of fixed-target text input, viewport and safe-area contracts.

use open_gpui::{
    Autocapitalize, Bounds, Corners, Edges, FocusHandle, Pixels, PlatformInputHandler,
    PlatformWindow, TextInputAction, TextInputConfiguration, TextInputStateChange, Window,
    WindowInsets, point, px, size,
};
use std::ops::Range;

#[test]
fn ordinary_public_text_session_and_viewport_contracts_require_no_support_features() {
    let _: fn(&Window) -> Bounds<Pixels> = Window::visual_viewport_bounds;
    let _: fn(&Window) -> Bounds<Pixels> = Window::fully_visible_bounds;
    let _: fn(&Window) -> WindowInsets = Window::insets;
    let _: fn(&Window) = Window::request_virtual_keyboard;
    let _: fn(&Window) = Window::dismiss_virtual_keyboard;
    let _: fn(&Window, &FocusHandle) = Window::notify_text_input_changed;
    let _: fn(&Window, &FocusHandle) = Window::notify_text_selection_changed;
    let _: fn(&mut PlatformInputHandler) -> Option<Range<usize>> =
        PlatformInputHandler::text_input_editable_range;
    let _: fn(&mut PlatformInputHandler) -> Option<usize> = PlatformInputHandler::text_length_utf16;
    let _: fn(&mut PlatformInputHandler, Range<usize>) =
        PlatformInputHandler::set_selected_text_range;
    let _: fn(&mut PlatformInputHandler, Range<usize>) -> Vec<Bounds<Pixels>> =
        PlatformInputHandler::selection_bounds_for_range;
    let _backend_type_only = |backend: &mut dyn PlatformWindow,
                              configuration: TextInputConfiguration,
                              change: TextInputStateChange| {
        backend.set_text_input_configuration(configuration);
        backend.text_input_state_changed(change);
        backend.show_soft_keyboard();
        backend.hide_soft_keyboard();
        backend.on_visual_viewport_changed(Box::new(|| {}));
        backend.on_insets_changed(Box::new(|_: WindowInsets| {}));
        let _: bool = backend.prepare_frame();
        let _: Bounds<Pixels> = backend.visual_viewport_bounds();
        let _: WindowInsets = backend.insets();
    };
    let _configuration_query_type_only =
        |handler: &mut PlatformInputHandler, window: &mut Window, app: &mut open_gpui::App| {
            let _: TextInputConfiguration = handler.text_input_configuration(window, app);
        };
    let _: [Autocapitalize; 4] = [
        Autocapitalize::None,
        Autocapitalize::Words,
        Autocapitalize::Sentences,
        Autocapitalize::Characters,
    ];
    let _: [TextInputAction; 8] = [
        TextInputAction::Unspecified,
        TextInputAction::Enter,
        TextInputAction::Done,
        TextInputAction::Go,
        TextInputAction::Next,
        TextInputAction::Previous,
        TextInputAction::Search,
        TextInputAction::Send,
    ];
    let _: TextInputConfiguration = TextInputConfiguration {
        autocorrect: true,
        autocapitalize: Autocapitalize::Sentences,
        suggestions: false,
        input_action: TextInputAction::Send,
    };
    let insets = WindowInsets {
        safe_area: Edges {
            top: px(24.0),
            right: px(8.0),
            bottom: px(16.0),
            left: px(6.0),
        },
        ime: Edges {
            bottom: px(120.0),
            ..Default::default()
        },
        corner_radii: Corners {
            top_left: px(28.0),
            top_right: px(26.0),
            bottom_right: px(18.0),
            bottom_left: px(16.0),
        },
    };
    assert_eq!(
        insets.effective(),
        Edges {
            bottom: px(120.0),
            ..insets.safe_area.clone()
        }
    );
    assert_eq!(insets.corner_radii.top_right, px(26.0));
    let _: [TextInputStateChange; 4] = [
        TextInputStateChange::FocusGained,
        TextInputStateChange::FocusLost,
        TextInputStateChange::SelectionChanged,
        TextInputStateChange::ContentChanged,
    ];
    let _: Bounds<Pixels> = Bounds::new(point(px(4.0), px(8.0)), size(px(300.0), px(180.0)));
}

#[cfg(feature = "bench-support")]
mod ordinary_runtime {
    use super::*;
    use open_gpui::{
        AppContext as _, Application, Context, ElementInputHandler, EntityInputHandler,
        InputHandler as _, InteractiveElement as _, IntoElement, NoopTextSystem,
        ParentElement as _, Point, QuitMode, Render, Styled as _, UTF16Selection, WindowOptions,
        bench_platform, canvas, div,
    };
    use std::{cell::Cell, rc::Rc, sync::Arc};

    struct Field {
        focus: FocusHandle,
        configuration: TextInputConfiguration,
        queries: Rc<Cell<usize>>,
        text: String,
        selected: Range<usize>,
    }

    impl EntityInputHandler for Field {
        fn text_for_range(
            &mut self,
            range: Range<usize>,
            adjusted: &mut Option<Range<usize>>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<String> {
            let text: Vec<_> = self.text.encode_utf16().collect();
            *adjusted = Some(range.clone());
            String::from_utf16(text.get(range)?).ok()
        }

        fn selected_text_range(
            &mut self,
            _: bool,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<UTF16Selection> {
            Some(UTF16Selection {
                range: self.selected.clone(),
                reversed: false,
            })
        }

        fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
            None
        }

        fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

        fn replace_text_in_range(
            &mut self,
            range: Option<Range<usize>>,
            text: &str,
            _: &mut Window,
            _: &mut Context<Self>,
        ) {
            let range = range.unwrap_or_else(|| self.selected.clone());
            let mut encoded: Vec<_> = self.text.encode_utf16().collect();
            encoded.splice(range, text.encode_utf16());
            self.text = String::from_utf16(&encoded).expect("valid consumer-owned UTF-16 edits");
        }

        fn replace_and_mark_text_in_range(
            &mut self,
            range: Option<Range<usize>>,
            text: &str,
            _: Option<Range<usize>>,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.replace_text_in_range(range, text, window, cx);
        }

        fn bounds_for_range(
            &mut self,
            _: Range<usize>,
            bounds: Bounds<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<Bounds<Pixels>> {
            Some(bounds)
        }

        fn selection_bounds_for_range(
            &mut self,
            _: Range<usize>,
            bounds: Bounds<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Vec<Bounds<Pixels>> {
            vec![bounds]
        }

        fn character_index_for_point(
            &mut self,
            _: Point<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<usize> {
            Some(self.selected.start)
        }

        fn set_selected_text_range(
            &mut self,
            range: Range<usize>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) {
            self.selected = range;
        }

        fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
            Some(self.text.encode_utf16().count())
        }

        fn text_input_configuration(
            &mut self,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> TextInputConfiguration {
            self.queries.set(self.queries.get() + 1);
            self.configuration.clone()
        }

        fn text_input_editable_range(
            &mut self,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<Range<usize>> {
            Some(1..self.text.encode_utf16().count())
        }
    }

    impl Render for Field {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let entity = cx.entity();
            let focus = self.focus.clone();
            div().track_focus(&self.focus).size_full().child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, app| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), app);
                    },
                )
                .size_full(),
            )
        }
    }

    #[test]
    fn ordinary_embedded_app_renders_and_consumes_text_sessions_without_test_support() {
        let queries = Rc::new(Cell::new(0));
        let configuration = TextInputConfiguration {
            autocorrect: true,
            autocapitalize: Autocapitalize::Sentences,
            suggestions: true,
            input_action: TextInputAction::Send,
        };
        let owner = Application::with_platform(bench_platform(None, Arc::new(NoopTextSystem)))
            .with_quit_mode(QuitMode::Explicit)
            .run_embedded(|_| {});
        let handle = owner.update(|app| {
            assert!(!app.is_test());
            assert!(!app.background_executor().is_test());
            app.open_window(
                WindowOptions {
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |window, app| {
                    app.new(|cx| {
                        let focus = cx.focus_handle();
                        window.focus(&focus, cx);
                        Field {
                            focus,
                            configuration: configuration.clone(),
                            queries: queries.clone(),
                            text: "a🦀é".into(),
                            selected: 1..3,
                        }
                    })
                },
            )
            .expect("an ordinary headless App must open its input consumer")
        });
        let entity = owner.update(|app| handle.entity(app).expect("ordinary root entity"));
        owner.update(|app| {
            app.update_window(handle.into(), |_, window, app| {
                let generation = window.rendered_frame_revision();
                window.refresh();
                window.draw(app).clear(app);
                assert!(window.rendered_frame_revision() > generation);
                let focus = entity.read(app).focus.clone();
                assert_eq!(window.committed_focus(app), Some(focus.clone()));
                assert!(queries.get() > 0);
                let original = (window.bounds(), window.viewport_size());
                assert_eq!(
                    window.visual_viewport_bounds(),
                    Bounds::new(Point::default(), original.1)
                );
                assert_eq!(window.insets(), WindowInsets::default());
                assert_eq!(
                    window.fully_visible_bounds(),
                    window.visual_viewport_bounds()
                );
                window.request_virtual_keyboard();
                window.dismiss_virtual_keyboard();
                window.notify_text_input_changed(&focus);
                window.notify_text_selection_changed(&focus);

                let bounds = Bounds::new(point(px(6.0), px(8.0)), size(px(80.0), px(20.0)));
                let mut input = ElementInputHandler::new(bounds, entity.clone());
                assert_eq!(input.text_input_configuration(window, app), configuration);
                assert_eq!(input.text_input_editable_range(window, app), Some(1..4));
                assert_eq!(input.text_length_utf16(window, app), Some(4));
                let mut adjusted = None;
                assert_eq!(
                    input.text_for_range(1..3, &mut adjusted, window, app),
                    Some("🦀".into())
                );
                assert_eq!(adjusted, Some(1..3));
                input.set_selected_text_range(3..4, window, app);
                assert_eq!(
                    input.selected_text_range(false, window, app).unwrap().range,
                    3..4
                );
                assert_eq!(
                    input.selection_bounds_for_range(1..4, window, app),
                    [bounds]
                );
                input.replace_text_in_range(Some(1..3), "語", window, app);
                assert_eq!(entity.read(app).text, "a語é");
                assert_eq!(input.text_length_utf16(window, app), Some(3));
                assert_eq!((window.bounds(), window.viewport_size()), original);
                window.blur(app);
                window.refresh();
                window.draw(app).clear(app);
                assert!(window.committed_focus(app).is_none());
                window.remove_window(app);
            })
            .expect("ordinary render/input calls must preserve normal window removal");
            assert!(app.windows().is_empty());
            app.shutdown();
        });
    }
}
