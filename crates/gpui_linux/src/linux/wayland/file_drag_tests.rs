use super::*;
use open_gpui::{Context, IntoElement, Render, Window, WindowHandle, WindowId};
use std::cell::Cell;

struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        open_gpui::Empty
    }
}

fn handle(index: u32, generation: u32) -> AnyWindowHandle {
    WindowHandle::<Blank>::new(WindowId::from(
        (u64::from(generation) << 32) | u64::from(index),
    ))
    .into()
}

#[derive(Clone, Default)]
struct FakeDataOffer {
    finished: Rc<Cell<usize>>,
    destroyed: Rc<Cell<usize>>,
}

impl FileDragDataOffer for FakeDataOffer {
    fn finish(&self) {
        self.finished.set(self.finished.get() + 1);
    }

    fn destroy(&self) {
        self.destroyed.set(self.destroyed.get() + 1);
    }
}

#[derive(Clone)]
struct FakeWindow(Rc<FakeWindowState>);

struct FakeWindowState {
    handle: AnyWindowHandle,
    quiesced: Cell<bool>,
    _connection_lease: Rc<()>,
}

impl FakeWindow {
    fn new(handle: AnyWindowHandle, lease: &Rc<()>) -> Self {
        Self(Rc::new(FakeWindowState {
            handle,
            quiesced: Cell::new(false),
            _connection_lease: lease.clone(),
        }))
    }
}

impl FileDragWindow for FakeWindow {
    fn handle(&self) -> AnyWindowHandle {
        self.0.handle
    }

    fn interaction_is_quiesced(&self) -> bool {
        self.0.quiesced.get()
    }
}

fn drag_state() -> DragState<FakeDataOffer, FakeWindow> {
    DragState {
        data_offer: None,
        window: None,
        position: Point::default(),
        uri_read_generation: 0,
        uri_read_window: None,
    }
}

fn complete(
    drag: &mut DragState<FakeDataOffer, FakeWindow>,
    generation: u64,
    offer: &FakeDataOffer,
    window: Option<FakeWindow>,
) -> Option<(FakeWindow, PlatformInput)> {
    drag.complete_uri_read(
        generation,
        offer.clone(),
        window,
        point(px(17.0), px(29.0)),
        open_gpui::ExternalPaths(SmallVec::from_vec(vec![PathBuf::from("/tmp/file")])),
    )
}

#[test]
fn leave_before_completion_discards_the_pending_destination() {
    let mut drag = drag_state();
    let window = FakeWindow::new(handle(7, 1), &Rc::new(()));
    let offer = FakeDataOffer::default();
    let generation = drag.begin_uri_read(Some(window.handle()));

    assert!(drag.handle_leave().is_none());
    assert!(complete(&mut drag, generation, &offer, Some(window)).is_none());

    assert_eq!(offer.destroyed.get(), 1);
    assert_eq!(offer.finished.get(), 0);
    assert!(drag.window.is_none());
    assert!(drag.data_offer.is_none());
    assert!(drag.uri_read_window.is_none());
}

#[test]
fn drop_before_completion_invalidates_the_read_without_finishing_it() {
    let mut drag = drag_state();
    let window = FakeWindow::new(handle(7, 1), &Rc::new(()));
    let offer = FakeDataOffer::default();
    let generation = drag.begin_uri_read(Some(window.handle()));

    assert!(drag.handle_drop().is_none());
    assert!(complete(&mut drag, generation, &offer, Some(window)).is_none());
    assert!(drag.handle_leave().is_none());

    assert_eq!(offer.finished.get(), 0);
    assert_eq!(offer.destroyed.get(), 1);
    assert!(drag.window.is_none());
}

#[test]
fn older_completion_cannot_replace_a_newer_enter_destination() {
    let mut drag = drag_state();
    let old_window = FakeWindow::new(handle(7, 1), &Rc::new(()));
    let new_window = FakeWindow::new(handle(8, 1), &Rc::new(()));
    let old_offer = FakeDataOffer::default();
    let new_offer = FakeDataOffer::default();
    let old_generation = drag.begin_uri_read(Some(old_window.handle()));
    let new_generation = drag.begin_uri_read(Some(new_window.handle()));

    let (_, input) = complete(
        &mut drag,
        new_generation,
        &new_offer,
        Some(new_window.clone()),
    )
    .expect("the current drag should enter");
    assert!(matches!(
        input,
        PlatformInput::FileDrop(FileDropEvent::Entered { .. })
    ));
    assert!(complete(&mut drag, old_generation, &old_offer, Some(old_window)).is_none());

    assert_eq!(
        drag.window.as_ref().map(FileDragWindow::handle),
        Some(new_window.handle()),
    );
    assert!(Rc::ptr_eq(
        &drag.data_offer.as_ref().unwrap().destroyed,
        &new_offer.destroyed,
    ));
    assert_eq!(old_offer.destroyed.get(), 1);
    assert_eq!(new_offer.destroyed.get(), 0);
}

#[test]
fn enter_without_a_live_destination_also_invalidates_the_old_read() {
    let mut drag = drag_state();
    let window = FakeWindow::new(handle(7, 1), &Rc::new(()));
    let offer = FakeDataOffer::default();
    let generation = drag.begin_uri_read(Some(window.handle()));

    drag.begin_uri_read(None);

    assert!(complete(&mut drag, generation, &offer, Some(window)).is_none());
    assert_eq!(offer.destroyed.get(), 1);
    assert!(drag.window.is_none());
}

#[test]
fn closing_before_completion_does_not_retain_or_reopen_the_native_window() {
    let mut drag = drag_state();
    let lease = Rc::new(());
    let window = FakeWindow::new(handle(7, 1), &lease);
    let target = window.handle();
    let offer = FakeDataOffer::default();
    let generation = drag.begin_uri_read(Some(target));
    drop(window);
    assert_eq!(Rc::strong_count(&lease), 1);

    drag.handle_window_closed(target);

    assert!(!drag.is_uri_read_current(generation));
    assert!(complete(&mut drag, generation, &offer, None).is_none());
    assert_eq!(offer.destroyed.get(), 1);
    assert!(drag.window.is_none());
    assert_eq!(Rc::strong_count(&lease), 1);
}

#[test]
fn closing_a_drag_window_releases_its_lease_and_invalidates_its_pending_read() {
    let mut drag = drag_state();
    let lease = Rc::new(());
    let window = FakeWindow::new(handle(7, 1), &lease);
    let target = window.handle();
    let active_offer = FakeDataOffer::default();
    let active_generation = drag.begin_uri_read(Some(target));
    drop(
        complete(
            &mut drag,
            active_generation,
            &active_offer,
            Some(window.clone()),
        )
        .unwrap(),
    );
    let pending_offer = FakeDataOffer::default();
    let pending_generation = drag.begin_uri_read(Some(target));
    drop(window);
    assert_eq!(Rc::strong_count(&lease), 2);

    drag.handle_window_closed(target);

    assert_eq!(Rc::strong_count(&lease), 1);
    assert_eq!(active_offer.destroyed.get(), 1);
    assert_eq!(active_offer.finished.get(), 0);
    assert!(drag.window.is_none());
    assert!(complete(&mut drag, pending_generation, &pending_offer, None).is_none());
    assert_eq!(pending_offer.destroyed.get(), 1);
}

#[test]
fn closing_the_previous_destination_preserves_another_windows_pending_read() {
    let mut drag = drag_state();
    let old_window = FakeWindow::new(handle(7, 1), &Rc::new(()));
    let new_window = FakeWindow::new(handle(8, 1), &Rc::new(()));
    let old_offer = FakeDataOffer::default();
    let new_offer = FakeDataOffer::default();
    let old_generation = drag.begin_uri_read(Some(old_window.handle()));
    drop(
        complete(
            &mut drag,
            old_generation,
            &old_offer,
            Some(old_window.clone()),
        )
        .unwrap(),
    );
    let new_generation = drag.begin_uri_read(Some(new_window.handle()));

    drag.handle_window_closed(old_window.handle());

    assert!(
        complete(
            &mut drag,
            new_generation,
            &new_offer,
            Some(new_window.clone()),
        )
        .is_some()
    );
    assert_eq!(old_offer.destroyed.get(), 1);
    assert_eq!(new_offer.destroyed.get(), 0);
    assert_eq!(
        drag.window.as_ref().map(FileDragWindow::handle),
        Some(new_window.handle()),
    );
}

#[test]
fn completion_rejects_missing_quiesced_and_reused_window_generations() {
    let original = handle(7, 1);
    let replacement = FakeWindow::new(handle(7, 3), &Rc::new(()));
    assert_ne!(original, replacement.handle());
    let quiesced = FakeWindow::new(original, &Rc::new(()));
    quiesced.0.quiesced.set(true);

    for window in [None, Some(quiesced), Some(replacement)] {
        let mut drag = drag_state();
        let offer = FakeDataOffer::default();
        let generation = drag.begin_uri_read(Some(original));

        assert!(complete(&mut drag, generation, &offer, window).is_none());
        assert_eq!(offer.destroyed.get(), 1);
        assert!(drag.window.is_none());
        assert!(drag.data_offer.is_none());
        assert!(drag.uri_read_window.is_none());
    }
}

#[test]
fn an_old_window_close_cannot_cancel_a_reused_slots_new_generation() {
    let mut drag = drag_state();
    let old = handle(7, 1);
    let current = FakeWindow::new(handle(7, 3), &Rc::new(()));
    let offer = FakeDataOffer::default();
    let generation = drag.begin_uri_read(Some(current.handle()));

    drag.handle_window_closed(old);

    assert!(complete(&mut drag, generation, &offer, Some(current)).is_some());
    assert_eq!(offer.destroyed.get(), 0);
}

#[test]
fn completed_drags_keep_leave_and_drop_protocol_semantics() {
    for submit in [false, true] {
        let mut drag = drag_state();
        let window = FakeWindow::new(handle(7, 1), &Rc::new(()));
        let offer = FakeDataOffer::default();
        let generation = drag.begin_uri_read(Some(window.handle()));
        drop(complete(&mut drag, generation, &offer, Some(window)).unwrap());

        let (_, input) = if submit {
            drag.handle_drop()
        } else {
            drag.handle_leave()
        }
        .expect("a completed drag should deliver its terminal event");

        if submit {
            let PlatformInput::FileDrop(FileDropEvent::Submit { position }) = input else {
                panic!("drop should submit the drag");
            };
            assert_eq!(position, point(px(17.0), px(29.0)));
        } else {
            assert!(matches!(
                input,
                PlatformInput::FileDrop(FileDropEvent::Exited {})
            ));
        }
        assert_eq!(offer.finished.get(), if submit { 1 } else { 0 });
        assert_eq!(offer.destroyed.get(), 1);
        assert!(drag.window.is_none());
        assert!(drag.data_offer.is_none());
        assert!(drag.handle_leave().is_none());
        assert!(drag.handle_drop().is_none());
        assert_eq!(offer.destroyed.get(), 1);
    }
}
