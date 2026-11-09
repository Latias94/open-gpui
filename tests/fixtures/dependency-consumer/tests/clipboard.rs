//! Ordinary public consumers of Zed d70c45e5bb1d8b7b3926fba5a6d1c69c23bbc405,
//! within target 635026fabc38440fce5e1767d125026a17b12012. App and Window cases
//! use the existing bench-support feature without enabling GPUI test-support.

use open_gpui::{
    ClipboardEntry, ClipboardItem, ClipboardReadError, ClipboardString, ExternalPaths, Image,
    ImageFormat,
};
use std::path::PathBuf;

fn mixed_item() -> ClipboardItem {
    ClipboardItem {
        entries: vec![
            ClipboardEntry::String(ClipboardString {
                text: "first λ".into(),
                metadata: Some("application-owned metadata".into()),
            }),
            ClipboardEntry::Image(Image::from_bytes(
                ImageFormat::Svg,
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#.to_vec(),
            )),
            ClipboardEntry::ExternalPaths(ExternalPaths(
                [PathBuf::from("clipboard-file.txt")].into_iter().collect(),
            )),
            ClipboardEntry::String(ClipboardString::new(" and 🦀".into())),
        ],
    }
}

#[test]
fn downstream_clipboard_errors_preserve_exact_variants_messages_and_error_contract() {
    let cases = [
        (
            ClipboardReadError::Unavailable,
            "the clipboard is unavailable",
        ),
        (
            ClipboardReadError::Denied("permission prompt rejected: λ".into()),
            "clipboard access was denied: permission prompt rejected: λ",
        ),
        (
            ClipboardReadError::UnsupportedContent,
            "the clipboard contents are unsupported",
        ),
    ];

    for (error, message) in cases {
        match &error {
            ClipboardReadError::Unavailable => {
                assert_eq!(error, ClipboardReadError::Unavailable);
            }
            ClipboardReadError::Denied(reason) => {
                assert_eq!(reason, "permission prompt rejected: λ");
            }
            ClipboardReadError::UnsupportedContent => {
                assert_eq!(error, ClipboardReadError::UnsupportedContent);
            }
        }
        assert_eq!(error.clone(), error);
        let public_error: &dyn std::error::Error = &error;
        assert_eq!(public_error.to_string(), message);
        assert!(public_error.source().is_none());
    }
}

#[test]
fn downstream_mixed_items_keep_order_metadata_image_bytes_and_text_projection() {
    let item = mixed_item();
    assert_eq!(item.text().as_deref(), Some("first λ and 🦀"));
    assert_eq!(item.entries().len(), 4);
    assert_eq!(
        item.clone().into_entries().collect::<Vec<_>>(),
        item.entries
    );
    match item.entries() {
        [
            ClipboardEntry::String(text),
            ClipboardEntry::Image(image),
            ClipboardEntry::ExternalPaths(paths),
            ClipboardEntry::String(tail),
        ] => {
            assert_eq!(text.text, "first λ");
            assert_eq!(text.metadata.as_deref(), Some("application-owned metadata"));
            assert_eq!(image.format, ImageFormat::Svg);
            assert_eq!(
                image.bytes.as_slice(),
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#
            );
            assert_eq!(paths.paths(), [PathBuf::from("clipboard-file.txt")]);
            assert_eq!(tail.text, " and 🦀");
        }
        other => panic!("the mixed clipboard entries must retain their order: {other:?}"),
    }
}

#[cfg(feature = "bench-support")]
mod runtime {
    use super::*;
    use open_gpui::{
        App, AppContext as _, Application, ApplicationHandle, Bounds, Context, ElementInputHandler,
        Empty, EntityInputHandler, Global, InputHandler, NoopTextSystem, Pixels, Point, QuitMode,
        Task, UTF16Selection, Window, WindowOptions, bench_platform,
    };
    use std::{
        cell::Cell,
        future::Future,
        ops::Range,
        pin::pin,
        rc::Rc,
        sync::Arc,
        task::{Context as TaskContext, Poll, Waker},
    };

    fn application() -> ApplicationHandle {
        Application::with_platform(bench_platform(None, Arc::new(NoopTextSystem)))
            .with_quit_mode(QuitMode::Explicit)
            .run_embedded(|_| {})
    }

    fn window_options() -> WindowOptions {
        WindowOptions {
            show: false,
            focus_on_appearing: false,
            ..Default::default()
        }
    }

    fn ready<T: 'static>(task: Task<T>) -> T {
        let mut task = pin!(task);
        match task
            .as_mut()
            .poll(&mut TaskContext::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => {
                panic!("the synchronous platform fallback must return a ready snapshot")
            }
        }
    }

    #[test]
    fn downstream_async_reads_are_ready_snapshots_including_empty_clipboards() {
        let app = application();
        let empty: Task<Result<Option<ClipboardItem>, ClipboardReadError>> =
            app.update(|app| App::read_from_clipboard_async(app));
        assert!(empty.is_ready());

        let expected = mixed_item();
        let snapshot = app.update(|app| {
            app.write_to_clipboard(expected.clone());
            app.read_from_clipboard_async()
        });
        assert!(snapshot.is_ready());

        let later = ClipboardItem::new_string("later clipboard write".into());
        app.update(|app| app.write_to_clipboard(later.clone()));

        assert_eq!(ready(empty), Ok(None));
        assert_eq!(ready(snapshot), Ok(Some(expected)));
        app.update(|app| {
            assert_eq!(app.read_from_clipboard(), Some(later.clone()));
            assert_eq!(ready(app.read_from_clipboard_async()), Ok(Some(later)));
            app.shutdown();
        });
    }

    #[derive(Default)]
    struct TextInput {
        replacements: Vec<(Option<Range<usize>>, String)>,
    }

    impl EntityInputHandler for TextInput {
        fn text_for_range(
            &mut self,
            _: Range<usize>,
            _: &mut Option<Range<usize>>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<String> {
            None
        }

        fn selected_text_range(
            &mut self,
            _: bool,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<UTF16Selection> {
            None
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
            self.replacements.push((range, text.to_owned()));
        }

        fn replace_and_mark_text_in_range(
            &mut self,
            _: Option<Range<usize>>,
            _: &str,
            _: Option<Range<usize>>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) {
            panic!("paste must not introduce marked text");
        }

        fn bounds_for_range(
            &mut self,
            _: Range<usize>,
            _: Bounds<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<Bounds<Pixels>> {
            None
        }

        fn character_index_for_point(
            &mut self,
            _: Point<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<usize> {
            None
        }
    }

    #[derive(Default)]
    struct RichInput {
        items: Vec<ClipboardItem>,
    }

    impl EntityInputHandler for RichInput {
        fn text_for_range(
            &mut self,
            _: Range<usize>,
            _: &mut Option<Range<usize>>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<String> {
            None
        }

        fn selected_text_range(
            &mut self,
            _: bool,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<UTF16Selection> {
            None
        }

        fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
            None
        }

        fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

        fn paste(&mut self, item: ClipboardItem, _: &mut Window, _: &mut Context<Self>) {
            self.items.push(item);
        }

        fn replace_text_in_range(
            &mut self,
            _: Option<Range<usize>>,
            _: &str,
            _: &mut Window,
            _: &mut Context<Self>,
        ) {
            panic!("the full-item override must receive paste before text fallback");
        }

        fn replace_and_mark_text_in_range(
            &mut self,
            _: Option<Range<usize>>,
            _: &str,
            _: Option<Range<usize>>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) {
            panic!("paste must not introduce marked text");
        }

        fn bounds_for_range(
            &mut self,
            _: Range<usize>,
            _: Bounds<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<Bounds<Pixels>> {
            None
        }

        fn character_index_for_point(
            &mut self,
            _: Point<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<usize> {
            None
        }
    }

    #[test]
    fn downstream_entity_paste_executes_text_fallback_and_preserves_full_item_overrides() {
        let app = application();
        let window = app
            .update(|app| app.open_window(window_options(), |_, app| app.new(|_| Empty)))
            .expect("the ordinary headless consumer must open a Window");
        let text = app.update(|app| app.new(|_| TextInput::default()));
        let rich = app.update(|app| app.new(|_| RichInput::default()));
        let mixed = mixed_item();
        let image = ClipboardItem::new_image(&Image::empty());
        let paths = ClipboardItem {
            entries: vec![ClipboardEntry::ExternalPaths(ExternalPaths(
                [PathBuf::from("only-path.txt")].into_iter().collect(),
            ))],
        };
        let empty = ClipboardItem {
            entries: Vec::new(),
        };

        app.update(|app| {
            app.update_window(window.into(), |_, window, app| {
                let mut text_handler = ElementInputHandler::new(Bounds::default(), text.clone());
                let mut rich_handler = ElementInputHandler::new(Bounds::default(), rich.clone());
                for item in [&mixed, &image, &paths, &empty] {
                    InputHandler::paste(&mut text_handler, item.clone(), window, app);
                    InputHandler::paste(&mut rich_handler, item.clone(), window, app);
                }
            })
        })
        .expect("public input handlers must execute against the live Window");

        app.update(|app| {
            let text = text.read(app);
            assert_eq!(
                text.replacements,
                [
                    (None, "first λ and 🦀".to_owned()),
                    (None, "only-path.txt".to_owned()),
                ]
            );
        });
        app.update(|app| {
            let rich = rich.read(app);
            assert_eq!(rich.items, [mixed, image, paths, empty]);
        });
        drop((text, rich));
        app.update(|app| app.shutdown());
    }

    struct AppOwner(Rc<Cell<usize>>);

    impl Global for AppOwner {}

    impl Drop for AppOwner {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn downstream_ready_read_and_async_contexts_do_not_retain_the_window_or_app() {
        let app = application();
        let app_drops = Rc::new(Cell::new(0));
        app.update(|app| app.set_global(AppOwner(Rc::clone(&app_drops))));
        let window = app
            .update(|app| app.open_window(window_options(), |_, app| app.new(|_| Empty)))
            .expect("the ordinary headless consumer must open a Window");
        let weak_root = app.update(|app| window.entity(app).unwrap().downgrade());
        let mut async_app = app.to_async();
        let mut async_window = app
            .update(|app| app.update_window(window.into(), |_, window, app| window.to_async(app)))
            .expect("the live Window must supply an AsyncWindowContext");
        let expected = mixed_item();
        let snapshot = async_app.update(|app| {
            app.write_to_clipboard(expected.clone());
            app.read_from_clipboard_async()
        });

        app.update(|app| {
            app.update_window(window.into(), |_, window, app| window.remove_window(app))
        })
        .expect("the headless Window must close");
        assert!(async_window.update(|_, _| ()).is_err());
        assert!(weak_root.upgrade().is_none());
        assert_eq!(app_drops.get(), 0);
        app.update(|app| app.shutdown());
        drop(app);

        assert_eq!(app_drops.get(), 1);
        assert!(
            async_app
                .update_window(window.into(), |_, _, _| ())
                .is_err()
        );
        assert!(async_window.update(|_, _| ()).is_err());
        assert_eq!(ready(snapshot), Ok(Some(expected)));
    }
}
