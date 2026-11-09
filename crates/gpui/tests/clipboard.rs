#![cfg(feature = "test-support")]

//! Crate-external consumers of Zed d70c45e5bb1d8b7b3926fba5a6d1c69c23bbc405,
//! within target 635026fabc38440fce5e1767d125026a17b12012. Platform results use
//! the public TestAppContext fixture; no engine-private clipboard state is read.

use futures::channel::oneshot;
use open_gpui::{
    App, AppContext as _, Bounds, ClipboardEntry, ClipboardItem, ClipboardReadError,
    ClipboardString, Context, ElementInputHandler, Empty, Entity, EntityInputHandler,
    ExternalPaths, Image, ImageFormat, InputHandler, InteractiveElement as _, IntoElement,
    MouseButton, MouseDownEvent, Pixels, Point, QuitMode, Render, RequestFrameOptions, Styled as _,
    Task, TestAppContext, UTF16Selection, VisualTestContext, Window, div, point, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    ops::Range,
    path::PathBuf,
    pin::Pin,
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};

fn poll<T: Unpin + 'static>(task: &mut Task<T>) -> Poll<T> {
    Pin::new(task).poll(&mut TaskContext::from_waker(Waker::noop()))
}

fn ready<T: Unpin + 'static>(mut task: Task<T>) -> T {
    match poll(&mut task) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("the clipboard task must have completed"),
    }
}

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

#[open_gpui::test]
fn public_async_reads_snapshot_empty_and_full_items_before_later_writes(cx: &mut TestAppContext) {
    let empty: Task<Result<Option<ClipboardItem>, ClipboardReadError>> =
        cx.read(App::read_from_clipboard_async);
    assert!(empty.is_ready());
    let expected = mixed_item();
    cx.write_to_clipboard(expected.clone());
    let snapshot = cx.read(App::read_from_clipboard_async);
    assert!(snapshot.is_ready());
    let later = ClipboardItem::new_string("later clipboard write".into());
    cx.write_to_clipboard(later.clone());

    assert_eq!(ready(empty), Ok(None));
    assert_eq!(ready(snapshot), Ok(Some(expected)));
    assert_eq!(
        ready(cx.read(App::read_from_clipboard_async)),
        Ok(Some(later))
    );
}

#[open_gpui::test]
fn public_async_read_propagates_each_exact_platform_error_once(cx: &mut TestAppContext) {
    let fallback = ClipboardItem::new_string("ordinary fallback".into());
    cx.write_to_clipboard(fallback.clone());

    for error in [
        ClipboardReadError::Unavailable,
        ClipboardReadError::Denied("permission prompt rejected: λ".into()),
        ClipboardReadError::UnsupportedContent,
    ] {
        cx.set_next_clipboard_read(Task::ready(Err(error.clone())));
        assert_eq!(ready(cx.read(App::read_from_clipboard_async)), Err(error));
        assert_eq!(
            ready(cx.read(App::read_from_clipboard_async)),
            Ok(Some(fallback.clone()))
        );
    }
}

#[open_gpui::test]
fn public_pending_read_delivers_the_platform_result_without_blocking_later_reads(
    cx: &mut TestAppContext,
) {
    let (sender, receiver) = oneshot::channel();
    cx.set_next_clipboard_read(cx.foreground_executor.spawn(async move {
        receiver
            .await
            .expect("the platform result must be delivered")
    }));
    let mut pending = cx.read(App::read_from_clipboard_async);
    cx.run_until_parked();
    assert!(poll(&mut pending).is_pending());

    let later = ClipboardItem::new_string("clipboard while permission is pending".into());
    cx.write_to_clipboard(later.clone());
    assert_eq!(
        ready(cx.read(App::read_from_clipboard_async)),
        Ok(Some(later))
    );
    let expected = mixed_item();
    sender.send(Ok(Some(expected.clone()))).unwrap();
    cx.run_until_parked();
    assert_eq!(ready(pending), Ok(Some(expected)));
}

struct DropCounter(Rc<Cell<usize>>);

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[open_gpui::test]
fn dropping_a_public_pending_read_cancels_and_releases_the_platform_future(
    cx: &mut TestAppContext,
) {
    let drops = Rc::new(Cell::new(0));
    let capture = DropCounter(Rc::clone(&drops));
    let (sender, receiver) = oneshot::channel();
    cx.set_next_clipboard_read(cx.foreground_executor.spawn(async move {
        let _capture = capture;
        receiver.await.expect("a cancelled read must not complete")
    }));
    let mut pending = cx.read(App::read_from_clipboard_async);
    cx.run_until_parked();
    assert!(poll(&mut pending).is_pending());
    assert!(!sender.is_canceled());
    assert_eq!(drops.get(), 0);

    drop(pending);
    cx.run_until_parked();

    assert!(sender.is_canceled());
    assert_eq!(drops.get(), 1);
    assert_eq!(ready(cx.read(App::read_from_clipboard_async)), Ok(None));
}

#[derive(Default)]
struct TextInput {
    replacements: Vec<(Option<Range<usize>>, String)>,
}

impl InputHandler for TextInput {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        None
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        None
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut App,
    ) {
        self.replacements.push((range, text.to_owned()));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
        panic!("paste must not introduce marked text");
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        None
    }
}

#[derive(Default)]
struct RichInput {
    items: Vec<ClipboardItem>,
}

impl Render for RichInput {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
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
        panic!("ElementInputHandler must dispatch to the full-item EntityInputHandler override");
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

#[open_gpui::test]
fn public_input_paste_defaults_to_text_while_entity_override_receives_every_entry(
    cx: &mut TestAppContext,
) {
    let window = cx.open_window(size(px(80.), px(80.)), |_, _| Empty);
    let rich = cx.new(|_| RichInput::default());
    let mut text = TextInput::default();
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

    cx.update_window(window.into(), |_, window, app| {
        let mut rich_handler = ElementInputHandler::new(Bounds::default(), rich.clone());
        for item in [&mixed, &image, &paths, &empty] {
            InputHandler::paste(&mut text, item.clone(), window, app);
            InputHandler::paste(&mut rich_handler, item.clone(), window, app);
        }
    })
    .unwrap();

    assert_eq!(
        text.replacements,
        [
            (None, "first λ and 🦀".to_owned()),
            (None, "only-path.txt".to_owned()),
        ]
    );
    cx.read_entity(&rich, |rich, _| {
        assert_eq!(rich.items, [mixed, image, paths, empty]);
    });
}

type ClipboardRead = Result<Option<ClipboardItem>, ClipboardReadError>;

struct PasteOnClick {
    target: Entity<RichInput>,
    results: Rc<RefCell<Vec<ClipboardRead>>>,
}

impl Render for PasteOnClick {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let target = self.target.downgrade();
        let results = Rc::clone(&self.results);
        div().size_full().bg(rgb(0x305080)).on_mouse_down(
            MouseButton::Left,
            move |_, window, app| {
                let read = app.read_from_clipboard_async();
                let target = target.clone();
                let results = Rc::clone(&results);
                let mut window = window.to_async(app);
                app.spawn(async move |_| {
                    let result = read.await;
                    if let Ok(Some(item)) = &result {
                        window
                            .update(|window, app| {
                                let target = target.upgrade().expect("the target must be live");
                                let mut handler =
                                    ElementInputHandler::new(Bounds::default(), target);
                                InputHandler::paste(&mut handler, item.clone(), window, app);
                            })
                            .expect("the callback's Window must remain live");
                    }
                    results.borrow_mut().push(result);
                })
                .detach();
            },
        )
    }
}

#[open_gpui::test]
fn public_mouse_callback_awaits_clipboard_and_delivers_full_items_or_exact_errors(
    cx: &mut TestAppContext,
) {
    let target = cx.new(|_| RichInput::default());
    let results = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(80.), px(80.)), {
        let target = target.clone();
        let results = Rc::clone(&results);
        move |_, _| PasteOnClick { target, results }
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let _activation = visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
        ..Default::default()
    }));
    visual.update(|window, _| {
        assert!(
            window
                .presentation_facts()
                .frame_accepted_generation
                .is_some()
        );
    });

    let click = |cx: &mut TestAppContext| {
        cx.simulate_event(
            window.into(),
            MouseDownEvent {
                position: point(px(8.), px(8.)),
                button: MouseButton::Left,
                click_count: 1,
                ..Default::default()
            },
        );
    };
    let (sender, receiver) = oneshot::channel();
    cx.set_next_clipboard_read(cx.foreground_executor.spawn(async move {
        receiver
            .await
            .expect("the platform result must be delivered")
    }));
    click(cx);
    cx.run_until_parked();
    assert!(results.borrow().is_empty());
    cx.read_entity(&target, |target, _| assert!(target.items.is_empty()));

    let expected = mixed_item();
    sender.send(Ok(Some(expected.clone()))).unwrap();
    cx.run_until_parked();
    assert_eq!(*results.borrow(), [Ok(Some(expected.clone()))]);
    cx.read_entity(&target, |target, _| {
        assert_eq!(target.items, [expected.clone()])
    });

    let mut expected_results = vec![Ok(Some(expected.clone()))];
    for error in [
        ClipboardReadError::Unavailable,
        ClipboardReadError::Denied("callback permission denied".into()),
        ClipboardReadError::UnsupportedContent,
    ] {
        cx.set_next_clipboard_read(Task::ready(Err(error.clone())));
        click(cx);
        cx.run_until_parked();
        expected_results.push(Err(error));
        assert_eq!(*results.borrow(), expected_results);
        cx.read_entity(&target, |target, _| {
            assert_eq!(target.items, [expected.clone()])
        });
    }
    click(cx);
    cx.run_until_parked();
    expected_results.push(Ok(None));
    assert_eq!(*results.borrow(), expected_results);
    cx.read_entity(&target, |target, _| assert_eq!(target.items, [expected]));
}

#[open_gpui::test]
fn public_pending_paste_cannot_update_a_released_window_or_retain_its_entity(
    cx: &mut TestAppContext,
) {
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let releases = Rc::new(Cell::new(0));
    let window = cx.open_window(size(px(80.), px(80.)), {
        let releases = Rc::clone(&releases);
        move |_, cx| {
            cx.on_release(move |_: &mut RichInput, _| releases.set(releases.get() + 1))
                .detach();
            RichInput::default()
        }
    });
    let rich = window.entity(cx).unwrap();
    let weak = rich.downgrade();
    let mut async_window = cx
        .update_window(window.into(), |_, window, app| window.to_async(app))
        .unwrap();
    let (sender, receiver) = oneshot::channel();
    cx.set_next_clipboard_read(cx.foreground_executor.spawn(async move {
        receiver
            .await
            .expect("the platform result must be delivered")
    }));
    let read = cx.read(App::read_from_clipboard_async);
    let late_delivery = cx.foreground_executor.spawn({
        let weak = weak.clone();
        async move {
            let item = read.await.unwrap().unwrap();
            async_window.update(|window, app| {
                let entity = weak.upgrade().expect("a live target must own its entity");
                let mut handler = ElementInputHandler::new(Bounds::default(), entity);
                InputHandler::paste(&mut handler, item, window, app);
            })
        }
    });
    cx.run_until_parked();
    assert!(!late_delivery.is_ready());

    cx.update_window(window.into(), |_, window, app| window.remove_window(app))
        .unwrap();
    drop(rich);
    cx.run_until_parked();
    cx.update(|_| ());
    assert!(weak.upgrade().is_none());
    assert_eq!(releases.get(), 1);

    sender.send(Ok(Some(mixed_item()))).unwrap();
    cx.run_until_parked();
    assert!(ready(late_delivery).is_err());
    assert_eq!(releases.get(), 1);
}
