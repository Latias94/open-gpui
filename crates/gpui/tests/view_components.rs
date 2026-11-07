#![cfg(feature = "test-support")]

use open_gpui::{
    AnyView, AnyWeakView, App, AppContext as _, Bounds, Context, Element, Empty, Entity, EntityId,
    FocusHandle, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    PlatformWindowPresentOutcome, QuitMode, Render, RenderOnce, RequestFrameOptions,
    StyleRefinement, Styled as _, TestAppContext, View, VisualTestContext, WeakEntity, Window,
    WindowBounds, WindowHandle, WindowOptions, canvas, div, fill, point, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn open_root<V: Render>(
    cx: &mut TestAppContext,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> (WindowHandle<V>, VisualTestContext) {
    cx.defer_next_window_frame_requests();
    let handle = cx.update(|app| {
        app.set_quit_mode(QuitMode::Explicit);
        app.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(120.), px(120.)),
                ))),
                focus_on_appearing: false,
                inactive_frame_interval: None,
                low_power_frame_interval: None,
                thermal_frame_interval: None,
                ..Default::default()
            },
            build,
        )
        .unwrap()
    });
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    let _activation = visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    visual.update(|window, app| {
        assert!(window.is_window_active());
        window.draw(app).clear(app);
    });
    submit_scene(&mut visual);
    (handle, visual)
}

fn submit_scene(visual: &mut VisualTestContext) -> u64 {
    assert!(visual.simulate_frame(RequestFrameOptions {
        require_presentation: true,
        ..Default::default()
    }));
    visual.update(|window, _| {
        let facts = window.presentation_facts();
        let generation = facts.frame_accepted_generation.unwrap();
        assert_eq!(facts.present_submitted_generation, Some(generation));
        assert_eq!(facts.non_empty_presented_generation, Some(generation));
        let attempt = facts.latest_present_attempt.unwrap();
        assert_eq!(attempt.generation, generation);
        assert_eq!(attempt.outcome, PlatformWindowPresentOutcome::Submitted);
        assert!(attempt.contained_valid_primitives);
        generation
    })
}

#[derive(Default)]
struct CounterObservation {
    state: RefCell<Option<WeakEntity<CounterState>>>,
    initializations: Cell<usize>,
    releases: Rc<Cell<usize>>,
    paints: Cell<usize>,
    painted_value: Cell<usize>,
}

struct CounterState {
    value: usize,
    pointer_calls: usize,
    keyboard_calls: usize,
    focus: FocusHandle,
}

#[derive(IntoElement)]
struct GenericCounter<T, const STEP: usize = 1>
where
    T: IntoElement + 'static,
{
    child: T,
    initial: usize,
    observed: Rc<CounterObservation>,
}

impl<T, const STEP: usize> RenderOnce for GenericCounter<T, STEP>
where
    T: IntoElement + 'static,
{
    fn render(self, window: &mut Window, app: &mut App) -> impl IntoElement {
        let state = window.use_state(app, |_, cx| {
            self.observed
                .initializations
                .set(self.observed.initializations.get() + 1);
            let releases = Rc::clone(&self.observed.releases);
            cx.on_release(move |_, _| releases.set(releases.get() + 1))
                .detach();
            CounterState {
                value: self.initial,
                pointer_calls: 0,
                keyboard_calls: 0,
                focus: cx.focus_handle(),
            }
        });
        *self.observed.state.borrow_mut() = Some(state.downgrade());
        let (focus, value) = state.read_with(app, |state, _| (state.focus.clone(), state.value));
        let pointer_state = state.clone();
        let observed = self.observed;
        div()
            .id("counter")
            .w(px(96.))
            .h(px(48.))
            .flex_shrink_0()
            .track_focus(&focus)
            .child(self.child)
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        observed.paints.set(observed.paints.get() + 1);
                        observed.painted_value.set(value);
                        window.paint_quad(fill(bounds, rgb(0x305080 + value as u32)));
                    },
                )
                .size_full(),
            )
            .on_mouse_down(MouseButton::Left, move |_, window, app| {
                window.focus(&focus, app);
                pointer_state.update(app, |state, cx| {
                    state.value += STEP;
                    state.pointer_calls += 1;
                    cx.notify();
                });
            })
            .on_key_down(move |event, _, app| {
                if event.keystroke.key == "enter" {
                    state.update(app, |state, cx| {
                        state.value += STEP;
                        state.keyboard_calls += 1;
                        cx.notify();
                    });
                }
            })
    }
}

struct GenericRoot {
    visible: bool,
    initial: [usize; 2],
    observed: [Rc<CounterObservation>; 2],
}

impl Render for GenericRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full().flex().flex_col().bg(rgb(0x101820));
        if self.visible {
            root.child(GenericCounter::<Empty> {
                child: Empty,
                initial: self.initial[0],
                observed: Rc::clone(&self.observed[0]),
            })
            .child(GenericCounter::<_, 10> {
                child: div().size(px(0.)),
                initial: self.initial[1],
                observed: Rc::clone(&self.observed[1]),
            })
        } else {
            root
        }
    }
}

fn counter_snapshot(
    observed: &CounterObservation,
    visual: &VisualTestContext,
) -> (EntityId, usize, usize, usize) {
    let state = observed.state.borrow().as_ref().unwrap().upgrade().unwrap();
    visual.read(|app| {
        let value = state.read(app);
        (
            state.entity_id(),
            value.value,
            value.pointer_calls,
            value.keyboard_calls,
        )
    })
}

#[open_gpui::test]
fn generic_components_scope_local_state_by_type_and_dispatch_pointer_keyboard_and_canvas(
    cx: &mut TestAppContext,
) {
    let observed = [
        Rc::new(CounterObservation::default()),
        Rc::new(CounterObservation::default()),
    ];
    let (handle, mut visual) = open_root(cx, |_, app| {
        app.new(|_| GenericRoot {
            visible: true,
            initial: [3, 70],
            observed: observed.clone(),
        })
    });
    let first = counter_snapshot(&observed[0], &visual);
    let second = counter_snapshot(&observed[1], &visual);
    assert_ne!(first.0, second.0);
    assert_eq!((first.1, first.2, first.3), (3, 0, 0));
    assert_eq!((second.1, second.2, second.3), (70, 0, 0));
    for observation in &observed {
        assert_eq!(observation.initializations.get(), 1);
        assert!(observation.paints.get() > 0);
    }
    let before = submit_scene(&mut visual);
    handle
        .update(&mut visual, |root, _, cx| {
            root.initial = [100, 900];
            cx.notify();
        })
        .unwrap();
    assert!(submit_scene(&mut visual) > before);
    assert_eq!(counter_snapshot(&observed[0], &visual), first);
    assert_eq!(counter_snapshot(&observed[1], &visual), second);

    visual.simulate_click(point(px(12.), px(12.)), Default::default());
    visual.simulate_keystrokes("enter");
    visual.simulate_click(point(px(12.), px(60.)), Default::default());
    visual.simulate_keystrokes("enter enter");
    submit_scene(&mut visual);
    assert_eq!(counter_snapshot(&observed[0], &visual), (first.0, 5, 1, 1));
    assert_eq!(
        counter_snapshot(&observed[1], &visual),
        (second.0, 100, 1, 2)
    );
    assert_eq!(observed[0].painted_value.get(), 5);
    assert_eq!(observed[1].painted_value.get(), 100);

    let old_states = observed
        .each_ref()
        .map(|observation| observation.state.borrow().as_ref().unwrap().clone());
    handle
        .update(&mut visual, |root, _, cx| {
            root.visible = false;
            cx.notify();
        })
        .unwrap();
    submit_scene(&mut visual);
    for (state, observation) in old_states.iter().zip(&observed) {
        assert!(state.upgrade().is_none());
        assert_eq!(observation.releases.get(), 1);
    }
    handle
        .update(&mut visual, |root, _, cx| {
            root.visible = true;
            cx.notify();
        })
        .unwrap();
    submit_scene(&mut visual);
    let reconstructed_first = counter_snapshot(&observed[0], &visual);
    let reconstructed_second = counter_snapshot(&observed[1], &visual);
    assert_ne!(reconstructed_first.0, first.0);
    assert_ne!(reconstructed_second.0, second.0);
    assert_eq!(
        (
            reconstructed_first.1,
            reconstructed_first.2,
            reconstructed_first.3,
        ),
        (100, 0, 0)
    );
    assert_eq!(
        (
            reconstructed_second.1,
            reconstructed_second.2,
            reconstructed_second.3,
        ),
        (900, 0, 0)
    );
    assert!(visual.simulate_close());
    for observation in &observed {
        assert_eq!(observation.initializations.get(), 2);
        assert_eq!(observation.releases.get(), 2);
        assert!(
            observation
                .state
                .borrow()
                .as_ref()
                .unwrap()
                .upgrade()
                .is_none()
        );
    }
}

struct BackingState {
    value: usize,
    pointer_calls: usize,
}

#[derive(Default)]
struct PropsObservation {
    local: RefCell<Option<WeakEntity<usize>>>,
    initializations: Cell<usize>,
    releases: Rc<Cell<usize>>,
    painted: Cell<(usize, usize)>,
}

#[derive(IntoElement)]
struct PropsView<T>
where
    T: IntoElement + 'static,
{
    state: Entity<BackingState>,
    adjustment: usize,
    observed: Rc<PropsObservation>,
    child: T,
}

impl<T> View for PropsView<T>
where
    T: IntoElement + 'static,
{
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(self, window: &mut Window, app: &mut App) -> impl IntoElement {
        let local = window.use_state(app, |_, cx| {
            self.observed
                .initializations
                .set(self.observed.initializations.get() + 1);
            let releases = Rc::clone(&self.observed.releases);
            cx.on_release(move |_, _| releases.set(releases.get() + 1))
                .detach();
            0_usize
        });
        *self.observed.local.borrow_mut() = Some(local.downgrade());
        let value = self.state.read(app).value;
        let adjustment = self.adjustment;
        let observed = self.observed;
        div()
            .id("props-counter")
            .w(px(96.))
            .h(px(48.))
            .flex_shrink_0()
            .child(self.child)
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        observed.painted.set((value, adjustment));
                        window.paint_quad(fill(bounds, rgb(0x507030 + value as u32)));
                    },
                )
                .size_full(),
            )
            .on_mouse_down(MouseButton::Left, move |_, _, app| {
                self.state.update(app, |state, cx| {
                    state.value += adjustment;
                    state.pointer_calls += 1;
                    cx.notify();
                });
                local.update(app, |count, cx| {
                    *count += 1;
                    cx.notify();
                });
            })
    }
}

struct PropsRoot {
    states: [Entity<BackingState>; 2],
    adjustments: [usize; 2],
    observed: [Rc<PropsObservation>; 2],
    reversed: bool,
    visible: bool,
}

impl Render for PropsRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut root = div().size_full().flex().flex_col().bg(rgb(0x101820));
        if self.visible {
            for index in if self.reversed { [1, 0] } else { [0, 1] } {
                root = root.child(
                    PropsView {
                        state: self.states[index].clone(),
                        adjustment: self.adjustments[index],
                        observed: Rc::clone(&self.observed[index]),
                        child: Empty,
                    }
                    .into_any_element(),
                );
            }
        }
        root
    }
}

#[open_gpui::test]
fn manual_view_props_follow_backing_identity_across_reorder_removal_and_reconstruction(
    cx: &mut TestAppContext,
) {
    let releases = Rc::new(Cell::new(0));
    let states = [10, 20].map(|value| {
        cx.new(|cx| {
            let releases = Rc::clone(&releases);
            cx.on_release(move |_, _| releases.set(releases.get() + 1))
                .detach();
            BackingState {
                value,
                pointer_calls: 0,
            }
        })
    });
    let weak_states = states.each_ref().map(Entity::downgrade);
    let observed = [
        Rc::new(PropsObservation::default()),
        Rc::new(PropsObservation::default()),
    ];
    let (handle, mut visual) = open_root(cx, |_, app| {
        app.new(|_| PropsRoot {
            states: states.clone(),
            adjustments: [2, 7],
            observed: observed.clone(),
            reversed: false,
            visible: true,
        })
    });
    let root_weak = handle.entity(&visual).unwrap().downgrade();
    let locals = observed
        .each_ref()
        .map(|observation| observation.local.borrow().as_ref().unwrap().clone());
    assert_ne!(
        locals[0].upgrade().unwrap().entity_id(),
        locals[1].upgrade().unwrap().entity_id()
    );
    assert_eq!(observed[0].painted.get(), (10, 2));
    assert_eq!(observed[1].painted.get(), (20, 7));
    visual.simulate_click(point(px(12.), px(12.)), Default::default());
    submit_scene(&mut visual);
    assert_eq!(observed[0].painted.get(), (12, 2));

    handle
        .update(&mut visual, |root, _, cx| {
            root.adjustments = [5, 11];
            root.reversed = true;
            cx.notify();
        })
        .unwrap();
    submit_scene(&mut visual);
    assert_eq!(observed[0].painted.get(), (12, 5));
    assert_eq!(observed[1].painted.get(), (20, 11));
    for (local, observation) in locals.iter().zip(&observed) {
        assert_eq!(
            local.upgrade().unwrap().entity_id(),
            observation
                .local
                .borrow()
                .as_ref()
                .unwrap()
                .upgrade()
                .unwrap()
                .entity_id()
        );
        assert_eq!(observation.initializations.get(), 1);
    }
    visual.simulate_click(point(px(12.), px(12.)), Default::default());
    visual.simulate_click(point(px(12.), px(60.)), Default::default());
    submit_scene(&mut visual);
    visual.read(|app| {
        assert_eq!(
            (states[0].read(app).value, states[0].read(app).pointer_calls),
            (17, 2)
        );
        assert_eq!(
            (states[1].read(app).value, states[1].read(app).pointer_calls),
            (31, 1)
        );
        assert_eq!(*locals[0].upgrade().unwrap().read(app), 2);
        assert_eq!(*locals[1].upgrade().unwrap().read(app), 1);
    });
    assert_eq!(observed[0].painted.get(), (17, 5));
    assert_eq!(observed[1].painted.get(), (31, 11));

    handle
        .update(&mut visual, |root, _, cx| {
            root.visible = false;
            cx.notify();
        })
        .unwrap();
    submit_scene(&mut visual);
    for (local, observation) in locals.iter().zip(&observed) {
        assert!(local.upgrade().is_none());
        assert_eq!(observation.releases.get(), 1);
    }
    assert_eq!(releases.get(), 0);
    handle
        .update(&mut visual, |root, _, cx| {
            root.visible = true;
            cx.notify();
        })
        .unwrap();
    submit_scene(&mut visual);
    visual.read(|app| {
        for observation in &observed {
            assert_eq!(
                *observation
                    .local
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .upgrade()
                    .unwrap()
                    .read(app),
                0
            );
        }
    });
    assert_eq!(observed[0].painted.get(), (17, 5));
    assert_eq!(observed[1].painted.get(), (31, 11));
    assert!(visual.simulate_close());
    assert!(root_weak.upgrade().is_none());
    for observation in &observed {
        assert_eq!(observation.initializations.get(), 2);
        assert_eq!(observation.releases.get(), 2);
    }
    assert!(weak_states.iter().all(|state| state.upgrade().is_some()));
    drop(states);
    visual.cx.update(|_| ());
    assert!(weak_states.iter().all(|state| state.upgrade().is_none()));
    assert_eq!(releases.get(), 2);
}

struct RenderedLeaf {
    value: usize,
    renders: Rc<Cell<usize>>,
    painted: Rc<Cell<usize>>,
}

impl Render for RenderedLeaf {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let value = self.value;
        let painted = Rc::clone(&self.painted);
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                painted.set(value);
                window.paint_quad(fill(bounds, rgb(0x305080 + value as u32)));
            },
        )
        .size(px(48.))
    }
}

enum ChildRoute {
    Typed(Entity<RenderedLeaf>),
    Erased(AnyView),
    Weak(AnyWeakView),
}

struct RouteRoot(ChildRoute);

impl Render for RouteRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let child = match &self.0 {
            ChildRoute::Typed(view) => view.clone().into_any_element(),
            ChildRoute::Erased(view) => view.clone().into_any_element(),
            ChildRoute::Weak(view) => view
                .upgrade()
                .map(IntoElement::into_any_element)
                .unwrap_or_else(|| Empty.into_any_element()),
        };
        div().size_full().bg(rgb(0x101820)).child(child)
    }
}

#[open_gpui::test]
fn typed_erased_and_weak_consumers_submit_notify_and_release_across_root_replacement(
    cx: &mut TestAppContext,
) {
    let renders = Rc::new(Cell::new(0));
    let painted = Rc::new(Cell::new(0));
    let releases = Rc::new(Cell::new(0));
    let leaf = cx.new(|cx| {
        let releases = Rc::clone(&releases);
        cx.on_release(move |_, _| releases.set(releases.get() + 1))
            .detach();
        RenderedLeaf {
            value: 1,
            renders: Rc::clone(&renders),
            painted: Rc::clone(&painted),
        }
    });
    let weak_leaf = leaf.downgrade();
    let erased_weak = AnyView::from(leaf.clone()).downgrade();
    {
        let at = line!() + 1;
        let element = leaf.clone().into_element();
        assert_conversion_source(element, at);
        let at = line!() + 1;
        let element = AnyView::from(leaf.clone()).into_element();
        assert_conversion_source(element, at);
        let at = line!() + 1;
        let element = leaf.clone().cached(StyleRefinement::default());
        assert_conversion_source(element, at);
        let at = line!() + 1;
        let element = AnyView::from(leaf.clone()).cached(StyleRefinement::default());
        assert_conversion_source(element, at);
        let at = line!() + 1;
        let element = weak_leaf.upgrade().unwrap().into_element();
        assert_conversion_source(element, at);
        let at = line!() + 1;
        let element = erased_weak.upgrade().unwrap().into_element();
        assert_conversion_source(element, at);
    }
    let (handle, mut visual) = open_root(cx, |_, app| {
        app.new(|_| RouteRoot(ChildRoute::Typed(leaf.clone())))
    });
    let root = handle.entity(&visual).unwrap();
    let root_id = root.entity_id();
    let weak_root = root.downgrade();
    let erased_root = AnyView::from(root);
    assert_eq!(
        erased_root.entity_type(),
        std::any::TypeId::of::<RouteRoot>()
    );
    let erased_root = erased_root.downcast::<RenderedLeaf>().unwrap_err();
    assert_eq!(erased_root.entity_id(), root_id);
    let erased_weak_root = erased_root.downgrade();
    assert_eq!(
        erased_root.downcast::<RouteRoot>().unwrap().entity_id(),
        root_id
    );
    assert_eq!(AnyWeakView::from(weak_root.clone()), erased_weak_root);
    assert_eq!(painted.get(), 1);
    let mut generation = submit_scene(&mut visual);

    for route in [
        ChildRoute::Erased(leaf.clone().into()),
        ChildRoute::Weak(leaf.downgrade().into()),
    ] {
        handle
            .update(&mut visual, |root, _, cx| {
                root.0 = route;
                cx.notify();
            })
            .unwrap();
        let next_generation = submit_scene(&mut visual);
        assert!(next_generation > generation);
        generation = next_generation;
        let before = renders.get();
        handle.update(&mut visual, |_, _, cx| cx.notify()).unwrap();
        submit_scene(&mut visual);
        assert_eq!(renders.get(), before + 1);
        let before = renders.get();
        let value = leaf.update(&mut visual, |leaf, cx| {
            leaf.value += 1;
            cx.notify();
            leaf.value
        });
        submit_scene(&mut visual);
        assert_eq!(renders.get(), before + 1);
        assert_eq!(painted.get(), value);
        assert_eq!(erased_weak.upgrade().unwrap().entity_id(), leaf.entity_id());
    }

    handle
        .update(&mut visual, |root, _, cx| {
            root.0 = ChildRoute::Weak(leaf.downgrade().into());
            cx.notify();
        })
        .unwrap();
    submit_scene(&mut visual);
    drop(leaf);
    visual.cx.update(|_| ());
    assert!(weak_leaf.upgrade().is_none());
    assert!(erased_weak.upgrade().is_none());
    assert_eq!(releases.get(), 1);
    handle.update(&mut visual, |_, _, cx| cx.notify()).unwrap();
    let before_replacement = submit_scene(&mut visual);
    let replacement = visual.update(|window, app| {
        window.replace_root(app, |_, cx| {
            let releases = Rc::clone(&releases);
            cx.on_release(move |_, _| releases.set(releases.get() + 1))
                .detach();
            RenderedLeaf {
                value: 99,
                renders: Rc::clone(&renders),
                painted: Rc::clone(&painted),
            }
        })
    });
    assert!(submit_scene(&mut visual) > before_replacement);
    assert_eq!(painted.get(), 99);
    assert!(weak_root.upgrade().is_none());
    assert!(erased_weak_root.upgrade().is_none());
    assert!(handle.update(&mut visual, |_, _, _| ()).is_err());
    let replacement_weak = replacement.downgrade();
    drop(replacement);
    assert!(visual.simulate_close());
    assert!(replacement_weak.upgrade().is_none());
    assert_eq!(releases.get(), 2);
}

struct CachedPairRoot {
    typed: Entity<RenderedLeaf>,
    erased: AnyView,
}

impl Render for CachedPairRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x101820))
            .child(
                self.typed
                    .clone()
                    .cached(StyleRefinement::default().size(px(48.))),
            )
            .child(
                self.erased
                    .clone()
                    .cached(StyleRefinement::default().size(px(48.))),
            )
    }
}

fn assert_conversion_source(element: impl Element, line: u32) {
    #[cfg(debug_assertions)]
    {
        let source = element.source_location().unwrap();
        assert_eq!(source.file(), file!());
        assert_eq!(source.line(), line);
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = line;
        assert!(element.source_location().is_none());
    }
}

#[open_gpui::test]
fn typed_entity_and_erased_view_cache_entries_reuse_and_notify_independently(
    cx: &mut TestAppContext,
) {
    let renders = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    let painted = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    let releases = Rc::new(Cell::new(0));
    let leaves = [0, 1].map(|index| {
        cx.new(|cx| {
            let releases = Rc::clone(&releases);
            cx.on_release(move |_, _| releases.set(releases.get() + 1))
                .detach();
            RenderedLeaf {
                value: [2, 7][index],
                renders: Rc::clone(&renders[index]),
                painted: Rc::clone(&painted[index]),
            }
        })
    });
    let typed_weak = leaves[0].downgrade();
    let erased = AnyView::from(leaves[1].clone());
    let erased_weak = erased.downgrade();
    let (handle, mut visual) = open_root(cx, |_, app| {
        app.new(|_| CachedPairRoot {
            typed: leaves[0].clone(),
            erased: erased.clone(),
        })
    });
    assert_eq!(painted.each_ref().map(|value| value.get()), [2, 7]);
    let initial = renders.each_ref().map(|count| count.get());
    assert!(initial.iter().all(|count| *count > 0));
    let mut generation = submit_scene(&mut visual);
    handle.update(&mut visual, |_, _, cx| cx.notify()).unwrap();
    assert!(submit_scene(&mut visual) > generation);
    assert_eq!(renders.each_ref().map(|count| count.get()), initial);

    generation = submit_scene(&mut visual);
    leaves[0].update(&mut visual, |leaf, cx| {
        leaf.value = 4;
        cx.notify();
    });
    assert!(submit_scene(&mut visual) > generation);
    assert_eq!(
        renders.each_ref().map(|count| count.get()),
        [initial[0] + 1, initial[1]]
    );
    assert_eq!(painted.each_ref().map(|value| value.get()), [4, 7]);

    generation = submit_scene(&mut visual);
    erased
        .clone()
        .downcast::<RenderedLeaf>()
        .unwrap()
        .update(&mut visual, |leaf, cx| {
            leaf.value = 12;
            cx.notify();
        });
    assert!(submit_scene(&mut visual) > generation);
    assert_eq!(
        renders.each_ref().map(|count| count.get()),
        [initial[0] + 1, initial[1] + 1]
    );
    assert_eq!(painted.each_ref().map(|value| value.get()), [4, 12]);

    handle.update(&mut visual, |_, _, cx| cx.notify()).unwrap();
    submit_scene(&mut visual);
    assert_eq!(
        renders.each_ref().map(|count| count.get()),
        [initial[0] + 1, initial[1] + 1]
    );
    handle
        .update(&mut visual, |_, window, _| window.refresh())
        .unwrap();
    submit_scene(&mut visual);
    assert_eq!(
        renders.each_ref().map(|count| count.get()),
        [initial[0] + 2, initial[1] + 2]
    );
    assert_eq!(painted.each_ref().map(|value| value.get()), [4, 12]);

    drop(leaves);
    drop(erased);
    assert!(typed_weak.upgrade().is_some());
    assert!(erased_weak.upgrade().is_some());
    assert_eq!(releases.get(), 0);
    assert!(visual.simulate_close());
    assert!(typed_weak.upgrade().is_none());
    assert!(erased_weak.upgrade().is_none());
    assert_eq!(releases.get(), 2);
}

#[open_gpui::test]
fn typed_erased_and_upgraded_weak_roots_submit_and_release_after_window_reconstruction(
    cx: &mut TestAppContext,
) {
    let renders = Rc::new(Cell::new(0));
    let painted = Rc::new(Cell::new(0));
    let releases = Rc::new(Cell::new(0));
    let leaf = cx.new(|cx| {
        let releases = Rc::clone(&releases);
        cx.on_release(move |_, _| releases.set(releases.get() + 1))
            .detach();
        RenderedLeaf {
            value: 10,
            renders: Rc::clone(&renders),
            painted: Rc::clone(&painted),
        }
    });
    let identity = leaf.entity_id();
    let weak = leaf.downgrade();
    let erased_weak = AnyView::from(leaf.clone()).downgrade();
    let roots = [
        leaf.clone(),
        AnyView::from(leaf.clone())
            .downcast::<RenderedLeaf>()
            .unwrap(),
        AnyWeakView::from(leaf.downgrade())
            .upgrade()
            .unwrap()
            .downcast::<RenderedLeaf>()
            .unwrap(),
    ];
    for root in roots {
        let (handle, mut visual) = open_root(cx, |_, _| root);
        assert_eq!(handle.entity(&visual).unwrap().entity_id(), identity);
        let generation = submit_scene(&mut visual);
        let before = renders.get();
        let value = handle
            .update(&mut visual, |root, _, cx| {
                root.value += 1;
                cx.notify();
                root.value
            })
            .unwrap();
        assert!(submit_scene(&mut visual) > generation);
        assert_eq!(painted.get(), value);
        assert_eq!(renders.get(), before + 1);
        assert!(visual.simulate_close());
        assert!(weak.upgrade().is_some());
        assert!(erased_weak.upgrade().is_some());
        assert_eq!(releases.get(), 0);
    }
    assert_eq!(leaf.read_with(cx, |leaf, _| leaf.value), 13);
    drop(leaf);
    cx.update(|_| ());
    assert!(weak.upgrade().is_none());
    assert!(erased_weak.upgrade().is_none());
    assert_eq!(releases.get(), 1);

    let rebuilt = cx.new(|cx| {
        let releases = Rc::clone(&releases);
        cx.on_release(move |_, _| releases.set(releases.get() + 1))
            .detach();
        RenderedLeaf {
            value: 40,
            renders: Rc::clone(&renders),
            painted: Rc::clone(&painted),
        }
    });
    let rebuilt_weak = AnyView::from(rebuilt.clone()).downgrade();
    assert_ne!(rebuilt.entity_id(), identity);
    let (_, mut visual) = open_root(cx, |_, _| {
        rebuilt_weak
            .upgrade()
            .unwrap()
            .downcast::<RenderedLeaf>()
            .unwrap()
    });
    drop(rebuilt);
    submit_scene(&mut visual);
    assert_eq!(painted.get(), 40);
    assert!(rebuilt_weak.upgrade().is_some());
    assert!(visual.simulate_close());
    assert!(rebuilt_weak.upgrade().is_none());
    assert_eq!(releases.get(), 2);
}
