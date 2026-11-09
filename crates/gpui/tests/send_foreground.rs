//! Crate-external App/Window consumers of Zed's sendable foreground submission mechanism
//! (2c2f737ab2f3db793b7a313bc8e5307f1708d2e8, fixed at 635026fabc38440fce5e1767d125026a17b12012).
//! Real worker threads submit and wake tasks on the public TestAppContext's existing scheduler.
//! A local consumer awaits the Send task before updating its real root entity and notifying it.
//! TestApp performs dirty draws automatically, after the typed root update releases its borrow.
//! These CPU TestPlatform workflows qualify neither a native event loop nor native Submitted.
//! The ordinary independently locked target covers the production PlatformScheduler bridge.

#![cfg(all(feature = "test-support", not(target_family = "wasm")))]

use futures::{FutureExt as _, channel::oneshot};
use open_gpui::{
    AppContext as _, Context, IntoElement, QuitMode, Render, Styled as _, TestAppContext, Window,
    WindowHandle, div, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Sender, TryRecvError},
    },
    thread::{self, ThreadId},
};

#[derive(Default)]
struct ViewObservations {
    renders: Cell<usize>,
    drops: Cell<usize>,
}

struct ForegroundInbox {
    values: Vec<usize>,
    observed: Rc<ViewObservations>,
}

impl Render for ForegroundInbox {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.observed.renders.set(self.observed.renders.get() + 1);
        div().size_full().bg(if self.values.is_empty() {
            rgb(0x183048)
        } else {
            rgb(0x306030)
        })
    }
}

impl Drop for ForegroundInbox {
    fn drop(&mut self) {
        self.observed.drops.set(self.observed.drops.get() + 1);
    }
}

struct DropProbe(Sender<ThreadId>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        let _ = self.0.send(thread::current().id());
    }
}

fn open_inbox(cx: &mut TestAppContext) -> (WindowHandle<ForegroundInbox>, Rc<ViewObservations>) {
    let observed = Rc::new(ViewObservations::default());
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let window = cx.open_window(size(px(160.), px(100.)), {
        let observed = observed.clone();
        move |_, _| ForegroundInbox {
            values: Vec::new(),
            observed,
        }
    });
    cx.run_until_parked();
    assert!(observed.renders.get() > 0);
    (window, observed)
}

#[test]
fn worker_submission_and_wakeup_notify_and_redraw_only_the_original_app() {
    let owner = thread::current().id();
    let mut source = TestAppContext::single();
    source.executor().allow_parking();
    let mut unrelated = TestAppContext::single();
    unrelated.executor().allow_parking();
    let weak_source = Rc::downgrade(&source.app);
    let weak_unrelated = Rc::downgrade(&unrelated.app);
    let (window, observed) = open_inbox(&mut source);
    let (unrelated_window, unrelated_observed) = open_inbox(&mut unrelated);
    let root = window
        .entity(&source)
        .expect("the source window must own its inbox");
    let weak_root = root.downgrade();
    let weak_unrelated_root = unrelated_window
        .entity(&unrelated)
        .expect("the unrelated window must own its inbox")
        .downgrade();
    let notifications = Rc::new(RefCell::new(Vec::new()));
    let subscription = source.update(|app| {
        let notifications = notifications.clone();
        app.observe(&root, move |entity, app| {
            notifications
                .borrow_mut()
                .push(entity.read(app).values.clone());
        })
    });
    let initial_renders = observed.renders.get();
    let unrelated_renders = unrelated_observed.renders.get();
    let polls = Arc::new(AtomicUsize::new(0));
    let observed_polls = polls.clone();
    let (release, gate) = oneshot::channel::<usize>();
    let (dropped, drops) = mpsc::channel();
    let probe = DropProbe(dropped);
    let send = source.read(|app| app.foreground_executor().to_send());
    let clone = send.clone();
    drop(send);
    let submitted = thread::spawn(move || {
        assert_ne!(thread::current().id(), owner);
        let value = Cell::new(40);
        clone.spawn(async move {
            let _capture = probe;
            assert_eq!(thread::current().id(), owner);
            observed_polls.fetch_add(1, Ordering::SeqCst);
            let amount = gate
                .await
                .expect("the worker must release the pending task");
            assert_eq!(thread::current().id(), owner);
            observed_polls.fetch_add(1, Ordering::SeqCst);
            value.set(value.get() + amount);
            value
        })
    })
    .join()
    .expect("the worker must submit the Send future without polling it");
    let receipt = Rc::new(Cell::new(0));
    let local_receipt = receipt.clone();
    let consumer = source.spawn(move |mut app| async move {
        let value: Cell<usize> = submitted.await;
        assert_eq!(thread::current().id(), owner);
        window
            .update(&mut app, |view, _, cx| {
                view.values.push(value.get());
                cx.notify();
            })
            .expect("the original window must consume the foreground result");
        local_receipt.set(value.get());
        local_receipt
    });

    unrelated.run_until_parked();
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    source.run_until_parked();
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(!consumer.is_ready());
    assert!(root.read_with(&source, |view, _| view.values.is_empty()));
    assert!(notifications.borrow().is_empty());
    assert_eq!(observed.renders.get(), initial_renders);
    assert!(matches!(drops.try_recv(), Err(TryRecvError::Empty)));

    thread::spawn(move || {
        assert_ne!(thread::current().id(), owner);
        release
            .send(2)
            .expect("the original pending future must retain its receiver");
    })
    .join()
    .expect("the worker must wake the original foreground future");
    unrelated.run_until_parked();
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(notifications.borrow().is_empty());
    source.run_until_parked();
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    let returned = consumer
        .now_or_never()
        .expect("the local consumer must finish");
    assert!(Rc::ptr_eq(&returned, &receipt));
    assert_eq!(receipt.get(), 42);
    assert_eq!(root.read_with(&source, |view, _| view.values.clone()), [42]);
    assert_eq!(notifications.borrow().as_slice(), [vec![42]]);
    assert!(observed.renders.get() > initial_renders);
    assert_eq!(unrelated_observed.renders.get(), unrelated_renders);
    assert!(
        unrelated_window
            .read_with(&unrelated, |view, _| view.values.is_empty())
            .expect("the unrelated App must still own its unchanged window")
    );
    assert_eq!(
        drops
            .try_recv()
            .expect("completion must release the Send capture"),
        owner
    );
    assert!(matches!(drops.try_recv(), Err(TryRecvError::Disconnected)));

    drop((returned, receipt, subscription, root));
    source
        .update_window(window.into(), |_, window, app| window.remove_window(app))
        .expect("the source consumer window must close");
    unrelated
        .update_window(unrelated_window.into(), |_, window, app| {
            window.remove_window(app)
        })
        .expect("the unrelated consumer window must close");
    source.run_until_parked();
    unrelated.run_until_parked();
    source.update(|_| ());
    unrelated.update(|_| ());
    assert!(weak_root.upgrade().is_none());
    assert!(weak_unrelated_root.upgrade().is_none());
    assert_eq!(observed.drops.get(), 1);
    assert_eq!(unrelated_observed.drops.get(), 1);
    source.quit();
    unrelated.quit();
    drop((source, unrelated));
    assert!(weak_source.upgrade().is_none());
    assert!(weak_unrelated.upgrade().is_none());
}

#[test]
fn worker_cancellation_before_or_after_first_poll_leaves_the_window_unchanged() {
    let owner = thread::current().id();
    for poll_before_cancelling in [false, true] {
        let mut cx = TestAppContext::single();
        cx.executor().allow_parking();
        let weak_app = Rc::downgrade(&cx.app);
        let (window, observed) = open_inbox(&mut cx);
        let weak_root = window
            .entity(&cx)
            .expect("the window must own its root")
            .downgrade();
        let initial_renders = observed.renders.get();
        let polls = Arc::new(AtomicUsize::new(0));
        let observed_polls = polls.clone();
        let (release, gate) = oneshot::channel::<()>();
        let (dropped, drops) = mpsc::channel();
        let probe = DropProbe(dropped);
        let send = cx.foreground_executor().to_send();
        let task = thread::spawn(move || {
            send.spawn(async move {
                let _capture = probe;
                assert_eq!(thread::current().id(), owner);
                observed_polls.fetch_add(1, Ordering::SeqCst);
                gate.await
                    .expect("cancelled work must not reach completion");
                observed_polls.fetch_add(1, Ordering::SeqCst);
                Cell::new(42)
            })
        })
        .join()
        .expect("the worker must return the task's cancellation handle");
        if poll_before_cancelling {
            cx.run_until_parked();
            assert_eq!(polls.load(Ordering::SeqCst), 1);
            assert!(!task.is_ready());
        }
        assert!(matches!(drops.try_recv(), Err(TryRecvError::Empty)));
        thread::spawn(move || drop(task))
            .join()
            .expect("the worker must drop the task to request cancellation");
        cx.run_until_parked();
        assert_eq!(
            polls.load(Ordering::SeqCst),
            usize::from(poll_before_cancelling)
        );
        assert_eq!(
            drops
                .try_recv()
                .expect("owner cleanup must release the capture"),
            owner
        );
        assert!(matches!(drops.try_recv(), Err(TryRecvError::Disconnected)));
        assert!(release.send(()).is_err());
        cx.run_until_parked();
        assert_eq!(
            polls.load(Ordering::SeqCst),
            usize::from(poll_before_cancelling)
        );
        assert_eq!(observed.renders.get(), initial_renders);
        assert!(
            window
                .read_with(&cx, |view, _| view.values.is_empty())
                .expect("cancellation must leave the live root unchanged")
        );
        cx.update_window(window.into(), |_, window, app| window.remove_window(app))
            .expect("the unchanged window must close");
        cx.run_until_parked();
        cx.update(|_| ());
        assert!(weak_root.upgrade().is_none());
        assert_eq!(observed.drops.get(), 1);
        cx.quit();
        drop(cx);
        assert!(weak_app.upgrade().is_none());
    }
}

#[test]
fn surviving_send_handles_retain_the_scheduler_without_retaining_an_app_or_window() {
    let owner = thread::current().id();
    let mut cx = TestAppContext::single();
    cx.executor().allow_parking();
    let weak_app = Rc::downgrade(&cx.app);
    let (window, observed) = open_inbox(&mut cx);
    let weak_root = window
        .entity(&cx)
        .expect("the window must own its root")
        .downgrade();
    let scheduler_lifetime = Arc::downgrade(cx.dispatcher.scheduler());
    let send = cx.read(|app| app.foreground_executor().to_send());
    let clone = send.clone();

    cx.update_window(window.into(), |_, window, app| window.remove_window(app))
        .expect("the consumer must close its window before releasing the App");
    cx.run_until_parked();
    cx.update(|_| ());
    assert!(weak_root.upgrade().is_none());
    assert_eq!(observed.drops.get(), 1);
    cx.quit();
    drop(cx);
    assert!(weak_app.upgrade().is_none());
    assert!(scheduler_lifetime.upgrade().is_some());

    let task = thread::spawn(move || {
        clone.spawn(async move {
            assert_eq!(thread::current().id(), owner);
            Cell::new(42)
        })
    })
    .join()
    .expect("the surviving handle must still submit to its captured scheduler");
    assert!(!task.is_ready());
    let scheduler = scheduler_lifetime
        .upgrade()
        .expect("the Send handle must retain the originating scheduler");
    scheduler.run();
    assert_eq!(
        task.now_or_never()
            .expect("the original scheduler must poll the task")
            .get(),
        42
    );
    assert!(weak_app.upgrade().is_none());
    assert!(weak_root.upgrade().is_none());
    assert_eq!(observed.drops.get(), 1);
    drop(scheduler);
    assert!(scheduler_lifetime.upgrade().is_some());
    drop(send);
    assert!(scheduler_lifetime.upgrade().is_none());
}
