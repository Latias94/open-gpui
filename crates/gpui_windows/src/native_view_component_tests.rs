//! Generic views composed through an actual Windows Application, native input and submissions.
//!
//! Synthetic pointer/key values enter the production native input callback outside App borrows.
//! Paint receipts identify the source accepted by the renderer; this does not qualify OS input
//! injection, physical foreground focus, compositor scanout or the complete Windows gate.

use super::{discard_stale_quit_messages, pump_messages_until_with_delay};
use crate::{
    RegisteredWindow, WindowsPlatform, WindowsWindowInner,
    native_test_observation::{
        NativeWindowTestEventKind, NativeWindowTestMessageDisposition, NativeWindowTestObservation,
        begin_native_window_test_observation,
    },
    with_windows_shared_callback,
};
use open_gpui::{
    AnyElement, AnyView, AnyWeakView, App, AppContext as _, Application, Bounds, Context,
    DispatchEventResult, Entity, EntityId, FocusHandle, Global, InteractiveElement as _,
    IntoElement, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, MouseUpEvent,
    NativeBoundaryDiagnosticCursor, NativeBoundaryDisposition, NativeBoundaryKind,
    NativeBoundaryTarget, NativeCallbackKind, NativeInputDeliveryResult, ParentElement, Pixels,
    PlatformInput, PlatformWindowPresentOutcome, QuitMode, Render, RenderOnce, StyleRefinement,
    Styled, View, ViewElement, WeakEntity, Window, WindowBounds, WindowHandle, WindowId,
    WindowInitialPresentationStatus, WindowOptions, canvas, div, fill, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Duration,
};
use windows::Win32::UI::WindowsAndMessaging::IsWindow;

struct NativeViewTheme {
    color: u32,
    _lifetime: Rc<()>,
}

impl Global for NativeViewTheme {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct NativeViewSource {
    props: u32,
    pointer_inputs: u32,
    key_inputs: u32,
    updates: u32,
    local_inputs: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeViewComposition {
    props: u32,
    weak_route: bool,
    cached: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct NativeViewPaint {
    window_id: WindowId,
    owner: EntityId,
    local: EntityId,
    source: NativeViewSource,
    bounds: Bounds<Pixels>,
    previous_revision: u64,
}

#[derive(Clone, Copy)]
struct NativeViewColumnPaint {
    window_id: WindowId,
    source: NativeViewComposition,
    previous_revision: u64,
}

struct NativeViewLocal {
    inputs: u32,
}

struct NativeViewProbe {
    selector: &'static str,
    renders: Cell<usize>,
    paints: Cell<usize>,
    paint: RefCell<Option<NativeViewPaint>>,
    local: RefCell<Option<WeakEntity<NativeViewLocal>>>,
    callbacks: RefCell<Vec<Weak<()>>>,
}

impl NativeViewProbe {
    fn new(selector: &'static str) -> Self {
        Self {
            selector,
            renders: Cell::new(0),
            paints: Cell::new(0),
            paint: RefCell::new(None),
            local: RefCell::new(None),
            callbacks: RefCell::new(Vec::new()),
        }
    }

    fn counts(&self) -> (usize, usize) {
        (self.renders.get(), self.paints.get())
    }
}

struct NativeViewState {
    focus: FocusHandle,
    pointer_inputs: u32,
    key_inputs: u32,
    updates: u32,
    probe: Rc<NativeViewProbe>,
}

impl NativeViewState {
    fn body(&mut self, props: u32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.probe.renders.set(self.probe.renders.get() + 1);
        let local = window.use_keyed_state("native-view-local", cx, |_, _| NativeViewLocal {
            inputs: 0,
        });
        *self.probe.local.borrow_mut() = Some(local.downgrade());
        let source = NativeViewSource {
            props,
            pointer_inputs: self.pointer_inputs,
            key_inputs: self.key_inputs,
            updates: self.updates,
            local_inputs: local.read(cx).inputs,
        };
        let owner = cx.entity_id();
        let local_id = local.entity_id();
        let pointer_local = local.downgrade();
        let key_local = local.downgrade();
        let callback = Rc::new(());
        self.probe
            .callbacks
            .borrow_mut()
            .push(Rc::downgrade(&callback));
        let pointer_capture = callback.clone();
        let probe = self.probe.clone();
        let color = cx.global::<NativeViewTheme>().color
            + props * 0x100
            + source.pointer_inputs * 0x10000
            + source.key_inputs * 0x1000
            + source.updates;
        div()
            .id("native-view-panel")
            .debug_selector(|| self.probe.selector.to_owned())
            .track_focus(&self.focus)
            .w_full()
            .h(px(64.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |state, _, window, cx| {
                    let _keep_alive = &pointer_capture;
                    state.pointer_inputs += 1;
                    pointer_local
                        .update(cx, |local, cx| {
                            local.inputs += 1;
                            cx.notify();
                        })
                        .expect("the pointer callback must retain its rendered local state");
                    window.focus(&state.focus, cx);
                    cx.notify();
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(move |state, event: &KeyDownEvent, _, cx| {
                let _keep_alive = &callback;
                if event.keystroke.key != "v" {
                    return;
                }
                state.key_inputs += 1;
                key_local
                    .update(cx, |local, cx| {
                        local.inputs += 1;
                        cx.notify();
                    })
                    .expect("the key callback must retain the same rendered local state");
                cx.notify();
                cx.stop_propagation();
            }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        window.paint_quad(fill(bounds, rgb(color)));
                        probe.paints.set(probe.paints.get() + 1);
                        *probe.paint.borrow_mut() = Some(NativeViewPaint {
                            window_id: window.window_handle().window_id(),
                            owner,
                            local: local_id,
                            source,
                            bounds,
                            previous_revision: window.rendered_frame_revision(),
                        });
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }
}

impl Render for NativeViewState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.body(0, window, cx)
    }
}

#[derive(IntoElement)]
struct NativePropsView {
    owner: Entity<NativeViewState>,
    props: u32,
}

impl View for NativePropsView {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.owner.entity_id())
    }

    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.owner
            .update(cx, |state, cx| state.body(self.props, window, cx))
    }
}

#[derive(IntoElement)]
struct NativeViewColumn {
    source: NativeViewComposition,
    paint: Rc<RefCell<Option<NativeViewColumnPaint>>>,
    children: Vec<AnyElement>,
}

impl NativeViewColumn {
    fn new(paint: Rc<RefCell<Option<NativeViewColumnPaint>>>) -> Self {
        Self {
            source: NativeViewComposition {
                props: 0,
                weak_route: false,
                cached: false,
            },
            paint,
            children: Vec::new(),
        }
    }

    fn source(mut self, source: NativeViewComposition) -> Self {
        self.source = source;
        self
    }
}

impl ParentElement for NativeViewColumn {
    fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(children);
    }
}

impl RenderOnce for NativeViewColumn {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let source = self.source;
        let paint = self.paint;
        div()
            .id("native-view-composition")
            .flex()
            .flex_col()
            .size_full()
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        window.paint_quad(fill(bounds, rgb(0x204060 + source.props * 0x100)));
                        *paint.borrow_mut() = Some(NativeViewColumnPaint {
                            window_id: window.window_handle().window_id(),
                            source,
                            previous_revision: window.rendered_frame_revision(),
                        });
                    },
                )
                .w_full()
                .h(px(24.)),
            )
            .children(self.children)
    }
}

struct NativeViewRoot {
    typed: Entity<NativeViewState>,
    erased: AnyView,
    weak: AnyWeakView,
    props_owner: Entity<NativeViewState>,
    composition: NativeViewComposition,
    column_paint: Rc<RefCell<Option<NativeViewColumnPaint>>>,
    renders: Rc<Cell<usize>>,
}

impl Render for NativeViewRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let erased = if self.composition.weak_route {
            self.weak
                .upgrade()
                .expect("the root must own its weak-route backing entity")
        } else {
            self.erased.clone()
        };
        assert_eq!(erased.entity_id(), self.erased.entity_id());
        let (typed, erased) = if self.composition.cached {
            let style = || StyleRefinement::default().w_full().h(px(64.));
            (
                self.typed.clone().cached(style()).into_any_element(),
                erased.cached(style()).into_any_element(),
            )
        } else {
            (
                self.typed.clone().into_any_element(),
                erased.into_any_element(),
            )
        };
        let props = NativePropsView {
            owner: self.props_owner.clone(),
            props: self.composition.props,
        };
        let props = if self.composition.weak_route {
            ViewElement::new(props).into_any_element()
        } else {
            props.into_any_element()
        };
        NativeViewColumn::new(self.column_paint.clone())
            .source(self.composition)
            .child(typed)
            .child(erased)
            .child(props)
    }
}

struct NativeViewWindow {
    handle: WindowHandle<NativeViewRoot>,
    registration: RegisteredWindow,
    native: Rc<WindowsWindowInner>,
    root: WeakEntity<NativeViewRoot>,
    owners: [WeakEntity<NativeViewState>; 3],
    erased: AnyWeakView,
    probes: [Rc<NativeViewProbe>; 3],
    column_paint: Rc<RefCell<Option<NativeViewColumnPaint>>>,
    root_renders: Rc<Cell<usize>>,
}

impl NativeViewWindow {
    fn open(app: &mut Application, platform: &WindowsPlatform) -> Self {
        let probes = [
            "native-view-typed",
            "native-view-erased",
            "native-view-props",
        ]
        .map(|selector| Rc::new(NativeViewProbe::new(selector)));
        let column_paint = Rc::new(RefCell::new(None));
        let root_renders = Rc::new(Cell::new(0));
        let (handle, root, owners, erased_lifetime) = app.update_for_test(|cx| {
            let [typed, erased, props_owner] = probes.each_ref().map(|probe| {
                cx.new(|cx| NativeViewState {
                    focus: cx.focus_handle(),
                    pointer_inputs: 0,
                    key_inputs: 0,
                    updates: 0,
                    probe: probe.clone(),
                })
            });
            let owners = [
                typed.downgrade(),
                erased.downgrade(),
                props_owner.downgrade(),
            ];
            let erased = AnyView::from(erased);
            let downcast = erased
                .clone()
                .downcast::<NativeViewState>()
                .expect("the erased consumer must preserve its concrete render type");
            assert_eq!(downcast.entity_id(), owners[1].entity_id());
            drop(downcast);
            let erased_lifetime = erased.downgrade();
            let weak = AnyWeakView::from(owners[1].clone());
            let handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(320.), px(260.)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        inactive_frame_interval: None,
                        low_power_frame_interval: None,
                        thermal_frame_interval: None,
                        ..WindowOptions::default()
                    },
                    |_, cx| {
                        cx.new(|_| NativeViewRoot {
                            typed,
                            erased,
                            weak,
                            props_owner,
                            composition: NativeViewComposition {
                                props: 1,
                                weak_route: false,
                                cached: false,
                            },
                            column_paint: column_paint.clone(),
                            renders: root_renders.clone(),
                        })
                    },
                )
                .expect("the native generic-view consumer must open");
            let root = handle
                .root(cx)
                .expect("the native root must exist")
                .downgrade();
            (handle, root, owners, erased_lifetime)
        });
        let registration = platform
            .raw_window_handles
            .read()
            .iter()
            .copied()
            .find(|entry| entry.window_id() == handle.window_id())
            .expect("the generic-view consumer must own its exact HWND registration");
        let native = platform
            .window_from_hwnd(registration.as_raw())
            .expect("the generic-view consumer must have a native window owner");
        assert_eq!(native.registration, registration);
        Self {
            handle,
            registration,
            native,
            root,
            owners,
            erased: erased_lifetime,
            probes,
            column_paint,
            root_renders,
        }
    }

    fn wait_for_submission(
        &self,
        app: &mut Application,
        composition: NativeViewComposition,
        sources: [NativeViewSource; 3],
        after: u64,
    ) -> (u64, [NativeViewPaint; 3]) {
        let mut submitted = None;
        let description = format!(
            "generic views' exact nonempty Submitted frame after {after}: {composition:?}, {sources:?}"
        );
        pump_messages_until_with_delay(&description, Duration::from_millis(5), || {
            submitted = app
                .update_for_test(|cx| {
                    self.handle.update(cx, |_, window, _| {
                        let facts = window.presentation_facts();
                        let attempt = facts.latest_present_attempt?;
                        let column = (*self.column_paint.borrow())?;
                        let [Some(typed), Some(erased), Some(props)] =
                            self.probes.each_ref().map(|probe| *probe.paint.borrow())
                        else {
                            return None;
                        };
                        let paints = [typed, erased, props];
                        let diagnostics = window.submitted_frame_diagnostics_for_test()?;
                        let exact_source = paints.iter().enumerate().all(|(index, paint)| {
                            paint.window_id == self.handle.window_id()
                                && paint.owner == self.owners[index].entity_id()
                                && paint.source == sources[index]
                                && paint.bounds.size.width > px(0.)
                                && paint.bounds.size.height > px(0.)
                                && paint.previous_revision.checked_add(1).is_some_and(
                                    |generation| {
                                        if composition.cached && index < 2 {
                                            generation <= attempt.generation
                                        } else {
                                            generation == attempt.generation
                                        }
                                    },
                                )
                                && diagnostics.debug_bounds().iter().any(|(selector, bounds)| {
                                    selector == self.probes[index].selector
                                        && *bounds == paint.bounds
                                })
                        });
                        (exact_source
                            && column.window_id == self.handle.window_id()
                            && column.source == composition
                            && column.previous_revision.checked_add(1) == Some(attempt.generation)
                            && attempt.generation > after
                            && attempt.outcome == PlatformWindowPresentOutcome::Submitted
                            && attempt.contained_valid_primitives
                            && facts.initial_presentation
                                == WindowInitialPresentationStatus::Completed
                            && facts.native_visible
                            && facts.frame_accepted_generation == Some(attempt.generation)
                            && facts.present_submitted_generation == Some(attempt.generation)
                            && facts.non_empty_presented_generation == Some(attempt.generation)
                            && window.rendered_frame_revision() == attempt.generation
                            && diagnostics.generation() == attempt.generation)
                            .then_some((attempt.generation, paints))
                    })
                })
                .expect("the composition must remain live through native submission");
            submitted.is_some()
        });
        submitted.expect("the actual composed source must belong to the exact Submitted generation")
    }

    fn input(&self, app: &mut Application, input: PlatformInput) -> DispatchEventResult {
        let cursor = app.update_for_test(|cx| {
            cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
                .cursor
        });
        let result = self.native.dispatch_input(input);
        let diagnostics = app.update_for_test(|cx| cx.native_boundary_diagnostics(cursor));
        assert_eq!(diagnostics.omitted_before_cursor, 0);
        assert!(diagnostics.terminal.iter().any(|entry| {
            entry.target == NativeBoundaryTarget::Window(self.handle.window_id())
                && entry.kind == NativeBoundaryKind::Callback(NativeCallbackKind::PlatformInput)
                && entry.disposition
                    == NativeBoundaryDisposition::Delivered {
                        input_result: Some(NativeInputDeliveryResult {
                            propagate: result.propagate,
                            default_prevented: result.default_prevented,
                        }),
                    }
        }));
        assert!(diagnostics.terminal.iter().all(|entry| !matches!(
            entry.disposition,
            NativeBoundaryDisposition::InvariantFailure(_)
        )));
        result
    }

    fn retire(
        self,
        app: &mut Application,
        platform: &WindowsPlatform,
        observation: &NativeWindowTestObservation,
    ) -> Rc<dyn Fn()> {
        assert!(self.root.upgrade().is_some());
        assert!(self.owners.iter().all(|owner| owner.upgrade().is_some()));
        assert!(self.erased.upgrade().is_some());
        assert!(self.probes.iter().all(|probe| {
            probe
                .local
                .borrow()
                .as_ref()
                .is_some_and(|local| local.upgrade().is_some())
                && probe
                    .callbacks
                    .borrow()
                    .iter()
                    .any(|callback| callback.upgrade().is_some())
        }));
        let stale_callback =
            with_windows_shared_callback(&self.native.state.callbacks.moved, Rc::clone)
                .expect("the retiring HWND must still own its production moved callback");
        let native_lifetime = Rc::downgrade(&self.native);
        let pending = Rc::new(());
        let pending_lifetime = Rc::downgrade(&pending);
        let pending_calls = Rc::new(Cell::new(0));
        app.update_for_test(|cx| {
            self.handle.update(cx, |_, window, cx| {
                let callback_calls = pending_calls.clone();
                window.on_next_frame(move |_, _| {
                    let _keep_alive = &pending;
                    callback_calls.set(callback_calls.get() + 1);
                });
                assert!(pending_lifetime.upgrade().is_some());
                assert_eq!(pending_calls.get(), 0);
                window.remove_window(cx);
            })
        })
        .expect("the exact generic-view window must close through its public owner");
        pump_messages_until_with_delay(
            "generic-view HWND, root, local state and native retirement",
            Duration::from_millis(5),
            || {
                app.update_for_test(|cx| cx.windows().is_empty())
                    && !unsafe { IsWindow(Some(self.registration.as_raw())).as_bool() }
                    && !platform
                        .raw_window_handles
                        .read()
                        .contains(&self.registration)
                    && platform.inner.native_retirement_is_settled()
                    && self.root.upgrade().is_none()
                    && self.owners.iter().all(|owner| owner.upgrade().is_none())
                    && self.erased.upgrade().is_none()
                    && pending_lifetime.upgrade().is_none()
                    && self.probes.iter().all(|probe| {
                        probe
                            .local
                            .borrow()
                            .as_ref()
                            .is_some_and(|local| local.upgrade().is_none())
                            && probe
                                .callbacks
                                .borrow()
                                .iter()
                                .all(|callback| callback.upgrade().is_none())
                    })
            },
        );
        assert_eq!(pending_calls.get(), 0);
        assert!(app.update_for_test(|cx| {
            assert!(self.handle.update(cx, |_, _, _| ()).is_err());
            cx.windows().is_empty()
        }));
        assert!(platform.raw_window_handles.read().is_empty());
        let lifecycle = observation
            .events()
            .into_iter()
            .filter(|event| {
                event.window().window_id() == self.registration.window_id()
                    && event.window().native_generation() == self.registration.generation()
            })
            .filter_map(|event| match event.kind() {
                NativeWindowTestEventKind::WindowMessage { disposition, .. } => {
                    assert_ne!(disposition, NativeWindowTestMessageDisposition::Panicked);
                    None
                }
                lifecycle => Some(lifecycle),
            })
            .collect::<Vec<_>>();
        let [
            NativeWindowTestEventKind::PresentationQuiesced {
                ticket_generation: quiesced,
            },
            NativeWindowTestEventKind::DestroyEntered {
                ticket_generation: destroyed,
            },
            NativeWindowTestEventKind::NativeTerminal {
                ticket_generation: terminal,
            },
        ] = lifecycle.as_slice()
        else {
            panic!("unexpected generic-view retirement sequence: {lifecycle:?}");
        };
        assert_eq!(quiesced, destroyed);
        assert_eq!(destroyed, terminal);
        assert_closed_view_callback(app, self.handle.window_id(), stale_callback.as_ref());
        assert_eq!(pending_calls.get(), 0);
        drop(self);
        assert!(native_lifetime.upgrade().is_none());
        stale_callback
    }
}

fn assert_closed_view_callback(app: &mut Application, window_id: WindowId, callback: &dyn Fn()) {
    let cursor = app.update_for_test(|cx| {
        cx.native_boundary_diagnostics(NativeBoundaryDiagnosticCursor::default())
            .cursor
    });
    callback();
    pump_messages_until_with_delay(
        "closed generic-view native callback rejection",
        Duration::from_millis(5),
        || {
            app.update_for_test(|cx| {
                let diagnostics = cx.native_boundary_diagnostics(cursor);
                assert_eq!(diagnostics.omitted_before_cursor, 0);
                assert!(!diagnostics.terminal.iter().any(|entry| {
                    entry.target == NativeBoundaryTarget::Window(window_id)
                        && matches!(
                            entry.disposition,
                            NativeBoundaryDisposition::Delivered { .. }
                        )
                }));
                diagnostics.terminal.iter().any(|entry| {
                    entry.target == NativeBoundaryTarget::Window(window_id)
                        && entry.kind == NativeBoundaryKind::Callback(NativeCallbackKind::Moved)
                        && matches!(
                            entry.disposition,
                            NativeBoundaryDisposition::Stale | NativeBoundaryDisposition::Closed
                        )
                })
            })
        },
    );
}

#[test]
fn native_generic_views_compose_rebuild_cache_notify_and_release_exact_owners() {
    discard_stale_quit_messages();
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let platform =
        Rc::new(WindowsPlatform::new(false).expect("native generic-view Windows backend"));
    let platform_hwnd = platform.handle;
    let platform_lifetime = Rc::downgrade(&platform);
    let lifecycle = platform.lifecycle_test_probe.clone();
    let mut app = Application::with_platform(platform.clone()).with_quit_mode(QuitMode::Explicit);
    let global = Rc::new(());
    let global_lifetime = Rc::downgrade(&global);
    app.update_for_test(|cx| {
        cx.set_global(NativeViewTheme {
            color: 0x204060,
            _lifetime: global,
        })
    });
    platform.begin_vsync_thread();

    let consumer = NativeViewWindow::open(&mut app, &platform);
    let mut composition = NativeViewComposition {
        props: 1,
        weak_route: false,
        cached: false,
    };
    let mut sources = [NativeViewSource::default(); 3];
    sources[2].props = composition.props;
    let (mut generation, mut paints) =
        consumer.wait_for_submission(&mut app, composition, sources, 0);
    let identities = paints.map(|paint| (paint.owner, paint.local));
    for left in 0..3 {
        for right in left + 1..3 {
            assert_ne!(identities[left].0, identities[right].0);
            assert_ne!(identities[left].1, identities[right].1);
        }
    }

    for props in [1, 7] {
        if props != composition.props {
            composition.props = props;
            composition.weak_route = true;
            sources[2].props = props;
            app.update_for_test(|cx| {
                consumer.handle.update(cx, |root, _, cx| {
                    root.composition = composition;
                    cx.notify();
                })
            })
            .expect("the parent must replace props without replacing any backing entity");
            (generation, paints) =
                consumer.wait_for_submission(&mut app, composition, sources, generation);
            assert_eq!(paints.map(|paint| (paint.owner, paint.local)), identities);
        }
        for index in 0..3 {
            let position = paints[index].bounds.center();
            assert!(
                !consumer
                    .input(
                        &mut app,
                        PlatformInput::MouseDown(MouseDownEvent {
                            button: MouseButton::Left,
                            position,
                            click_count: 1,
                            ..Default::default()
                        })
                    )
                    .propagate
            );
            consumer.input(
                &mut app,
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position,
                    click_count: 1,
                    ..Default::default()
                }),
            );
            sources[index].pointer_inputs += 1;
            sources[index].local_inputs += 1;
            generation = consumer
                .wait_for_submission(&mut app, composition, sources, generation)
                .0;
            assert!(
                !consumer
                    .input(
                        &mut app,
                        PlatformInput::KeyDown(KeyDownEvent {
                            keystroke: Keystroke::parse("v")
                                .expect("native view key")
                                .with_simulated_ime(),
                            is_held: false,
                            prefer_character_input: false,
                        })
                    )
                    .propagate
            );
            sources[index].key_inputs += 1;
            sources[index].local_inputs += 1;
            (generation, paints) =
                consumer.wait_for_submission(&mut app, composition, sources, generation);
            assert_eq!(paints.map(|paint| (paint.owner, paint.local)), identities);
        }
    }

    composition.cached = true;
    app.update_for_test(|cx| {
        consumer.handle.update(cx, |root, window, cx| {
            root.composition = composition;
            window.refresh();
            cx.notify();
        })
    })
    .expect("the native consumer must initialize both public entity cache routes");
    (generation, paints) = consumer.wait_for_submission(&mut app, composition, sources, generation);
    assert!(
        paints
            .iter()
            .all(|paint| paint.previous_revision.checked_add(1) == Some(generation))
    );
    let cached_paints = paints;
    let cached_counts = consumer.probes.each_ref().map(|probe| probe.counts());
    for props in [8, 9] {
        let renders_before = consumer.root_renders.get();
        composition.props = props;
        sources[2].props = props;
        app.update_for_test(|cx| {
            consumer.handle.update(cx, |root, _, cx| {
                root.composition = composition;
                cx.notify();
            })
        })
        .expect("parent notification must rebuild props while preserving cached siblings");
        (generation, paints) =
            consumer.wait_for_submission(&mut app, composition, sources, generation);
        assert!(consumer.root_renders.get() > renders_before);
        assert_eq!(&paints[..2], &cached_paints[..2]);
        assert_eq!(
            &consumer.probes.each_ref().map(|probe| probe.counts())[..2],
            &cached_counts[..2]
        );
        assert_eq!(paints.map(|paint| (paint.owner, paint.local)), identities);
    }
    for index in 0..2 {
        let before = consumer.probes.each_ref().map(|probe| probe.counts());
        app.update_for_test(|cx| {
            consumer.owners[index].update(cx, |owner, cx| {
                owner.updates += 1;
                cx.notify();
            })
        })
        .expect("the same cached entity must remain publicly updateable after two reuses");
        sources[index].updates += 1;
        (generation, paints) =
            consumer.wait_for_submission(&mut app, composition, sources, generation);
        assert_eq!(
            paints[index].previous_revision.checked_add(1),
            Some(generation)
        );
        let after = consumer.probes[index].counts();
        assert!(after.0 > before[index].0 && after.1 > before[index].1);
        assert_eq!(consumer.probes[1 - index].counts(), before[1 - index]);
        assert_eq!(paints.map(|paint| (paint.owner, paint.local)), identities);
    }
    let position = paints[0].bounds.center();
    assert!(
        !consumer
            .input(
                &mut app,
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position,
                    click_count: 1,
                    ..Default::default()
                })
            )
            .propagate
    );
    consumer.input(
        &mut app,
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }),
    );
    sources[0].pointer_inputs += 1;
    sources[0].local_inputs += 1;
    (generation, paints) = consumer.wait_for_submission(&mut app, composition, sources, generation);
    assert_eq!(paints[0].previous_revision.checked_add(1), Some(generation));
    assert_eq!(paints.map(|paint| (paint.owner, paint.local)), identities);
    assert!(
        !consumer
            .input(
                &mut app,
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: Keystroke::parse("v")
                        .expect("replayed view key")
                        .with_simulated_ime(),
                    is_held: false,
                    prefer_character_input: false,
                })
            )
            .propagate
    );
    sources[0].key_inputs += 1;
    sources[0].local_inputs += 1;
    (generation, paints) = consumer.wait_for_submission(&mut app, composition, sources, generation);
    assert_eq!(paints[0].previous_revision.checked_add(1), Some(generation));
    assert_eq!(paints.map(|paint| (paint.owner, paint.local)), identities);

    let retired_registration = consumer.registration;
    let retired_root = consumer.root.entity_id();
    let stale_callback = consumer.retire(&mut app, &platform, &observation);
    let rebuilt = NativeViewWindow::open(&mut app, &platform);
    assert_ne!(
        rebuilt.registration.generation(),
        retired_registration.generation()
    );
    assert_ne!(rebuilt.handle.window_id(), retired_registration.window_id());
    assert_ne!(rebuilt.root.entity_id(), retired_root);
    composition = NativeViewComposition {
        props: 1,
        weak_route: false,
        cached: false,
    };
    sources = [NativeViewSource::default(); 3];
    sources[2].props = 1;
    (generation, paints) = rebuilt.wait_for_submission(&mut app, composition, sources, 0);
    let rebuilt_identities = paints.map(|paint| (paint.owner, paint.local));
    for index in 0..3 {
        assert_ne!(rebuilt_identities[index].0, identities[index].0);
        assert_ne!(rebuilt_identities[index].1, identities[index].1);
    }
    let renders_before_stale = rebuilt.root_renders.get();
    assert_closed_view_callback(
        &mut app,
        retired_registration.window_id(),
        stale_callback.as_ref(),
    );
    assert_eq!(rebuilt.root_renders.get(), renders_before_stale);
    assert_eq!(
        rebuilt.probes.each_ref().map(|probe| *probe.paint.borrow()),
        paints.map(Some)
    );
    let position = paints[2].bounds.center();
    assert!(
        !rebuilt
            .input(
                &mut app,
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position,
                    click_count: 1,
                    ..Default::default()
                })
            )
            .propagate
    );
    rebuilt.input(
        &mut app,
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            ..Default::default()
        }),
    );
    assert!(
        !rebuilt
            .input(
                &mut app,
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: Keystroke::parse("v")
                        .expect("rebuilt view key")
                        .with_simulated_ime(),
                    is_held: false,
                    prefer_character_input: false,
                })
            )
            .propagate
    );
    sources[2].pointer_inputs = 1;
    sources[2].key_inputs = 1;
    sources[2].local_inputs = 2;
    let (_, rebuilt_paints) =
        rebuilt.wait_for_submission(&mut app, composition, sources, generation);
    assert_eq!(
        rebuilt_paints.map(|paint| (paint.owner, paint.local)),
        rebuilt_identities
    );
    drop(rebuilt.retire(&mut app, &platform, &observation));
    drop(stale_callback);

    app.update_for_test(|cx| cx.shutdown());
    drop(app);
    drop(platform);
    pump_messages_until_with_delay(
        "generic-view global, platform HWND and OLE release",
        Duration::from_millis(5),
        || {
            global_lifetime.upgrade().is_none()
                && platform_lifetime.upgrade().is_none()
                && !unsafe { IsWindow(Some(platform_hwnd)).as_bool() }
        },
    );
    assert_eq!(lifecycle.platform_destroy_attempts(), 1);
    assert_eq!(lifecycle.ole_uninitialize_count(), 1);
}
