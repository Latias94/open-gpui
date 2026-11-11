#![cfg(feature = "test-support")]

//! Public variable-height collection consumers using the original window and owned ListState.
//! Callback bounds cover visible rendering, not total reconciliation cost: authored rows, filtered
//! order, key lookup and ListState retain O(logical rows) data and reconciliation visits that data.
//! Paint probes and accepted, nonempty Submitted test-platform scenes establish CPU publication;
//! they make no GPU, latency, allocation or throughput claim.

use open_gpui::{
    App, AppContext as _, Bounds, Context, Entity, EntityId, FocusHandle, InteractiveElement as _,
    IntoElement, ListAlignment, ListOffset, ListState, MouseButton, MouseDownEvent, MouseUpEvent,
    ParentElement as _, Pixels, PlatformInput, PlatformWindowPresentOutcome, Point, QuitMode,
    Render, RequestFrameOptions, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, TestAppContext, VisualTestContext, Window, WindowBounds, WindowHandle,
    WindowOptions, canvas, div, fill, list, point, px, rgb, size,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, HashMap},
    rc::Rc,
};

const VIEWPORT_HEIGHT: f32 = 96.;
const MIN_ROW_HEIGHT: f32 = 16.;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    key: u64,
    text: SharedString,
    detail_height: u32,
    hidden: bool,
}

impl Row {
    fn new(key: u64) -> Self {
        Self {
            key,
            text: format!(
                "row {key}: {}",
                "alpha beta ".repeat(1 + (key % 3) as usize)
            )
            .into(),
            detail_height: (key % 3) as u32 * 8,
            hidden: false,
        }
    }
}

/// Row publications advance revision; the activation log records input without changing row data.
#[derive(Clone)]
struct Source {
    revision: u64,
    rows: Vec<Row>,
    reversed: bool,
    activated: Vec<u64>,
}

impl Source {
    fn new(count: usize) -> Self {
        Self {
            revision: 0,
            rows: (0..count).map(|key| Row::new(key as u64)).collect(),
            reversed: false,
            activated: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Publication {
    source: EntityId,
    revision: u64,
}

/// Mouse handlers capture the rendered publication. Keyboard input retains the original selection
/// even when keyboard dispatch redraws a dirty window before delivering the event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Target {
    publication: Publication,
    key: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Input {
    Pointer,
    Keyboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Attempt {
    target: Target,
    input: Input,
    accepted: bool,
}

#[derive(Clone)]
struct PaintedRow {
    index: usize,
    target: Target,
    text: SharedString,
    bounds: Bounds<Pixels>,
    clip: Bounds<Pixels>,
}

#[derive(Default)]
struct Trace {
    filter_visits: Cell<usize>,
    order_visits: Cell<usize>,
    identity_visits: Cell<usize>,
    renders: Cell<usize>,
    callbacks: Cell<usize>,
    callback_keys: RefCell<BTreeSet<u64>>,
    painted: RefCell<BTreeMap<u64, PaintedRow>>,
    presses: RefCell<Vec<Target>>,
    attempts: RefCell<Vec<Attempt>>,
}

struct Collection {
    source: Entity<Source>,
    publication: Publication,
    rows: Rc<[Row]>,
    positions: HashMap<u64, usize>,
    state: ListState,
    width: Pixels,
    font_size: Pixels,
    line_height: Pixels,
    focus: FocusHandle,
    selected: Option<Target>,
    activate_on_click: bool,
    trace: Rc<Trace>,
    _subscription: Subscription,
}

impl Collection {
    fn project(source: &Source, trace: &Trace) -> (Rc<[Row]>, HashMap<u64, usize>) {
        let mut order: Vec<_> = source
            .rows
            .iter()
            .filter(|row| {
                trace.filter_visits.set(trace.filter_visits.get() + 1);
                !row.hidden
            })
            .cloned()
            .collect();
        if source.reversed {
            order.reverse();
        }
        let rows: Rc<[Row]> = order
            .into_iter()
            .inspect(|_| {
                trace.order_visits.set(trace.order_visits.get() + 1);
            })
            .collect();
        let mut positions = HashMap::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            trace.identity_visits.set(trace.identity_visits.get() + 1);
            assert!(positions.insert(row.key, index).is_none());
        }
        (rows, positions)
    }

    fn new(source: Entity<Source>, trace: Rc<Trace>, cx: &mut Context<Self>) -> Self {
        let document = source.read(cx);
        let publication = Publication {
            source: source.entity_id(),
            revision: document.revision,
        };
        let (rows, positions) = Self::project(document, &trace);
        let state = ListState::new(rows.len(), ListAlignment::Top, px(0.));
        let subscription = cx.observe(&source, |collection, _, cx| collection.synchronize(cx));
        Self {
            source,
            publication,
            rows,
            positions,
            state,
            width: px(200.),
            font_size: px(12.),
            line_height: px(16.),
            focus: cx.focus_handle(),
            selected: None,
            activate_on_click: false,
            trace,
            _subscription: subscription,
        }
    }

    /// Surviving keys retain their absolute intra-row offset. A removed anchor falls forward
    /// through the old published order, then backward, with zero offset; an empty result starts
    /// at zero. This is consumer identity reconciliation; ListState owns all layout and indexing.
    fn reconciled_anchor(&self, positions: &HashMap<u64, usize>) -> ListOffset {
        let old = self.state.logical_scroll_top();
        if let Some(row) = self.rows.get(old.item_ix)
            && let Some(&item_ix) = positions.get(&row.key)
        {
            return ListOffset {
                item_ix,
                offset_in_item: old.offset_in_item,
            };
        }
        let next = self.rows.iter().skip(old.item_ix.saturating_add(1));
        let previous = self.rows.iter().take(old.item_ix).rev();
        ListOffset {
            item_ix: next
                .chain(previous)
                .find_map(|row| positions.get(&row.key).copied())
                .unwrap_or(0),
            offset_in_item: px(0.),
        }
    }

    fn synchronize(&mut self, cx: &mut Context<Self>) {
        let source = self.source.read(cx);
        let publication = Publication {
            source: self.source.entity_id(),
            revision: source.revision,
        };
        let (rows, positions) = Self::project(source, &self.trace);
        if publication.source != self.publication.source {
            self.state.reset(rows.len());
        } else {
            let prefix = self
                .rows
                .iter()
                .zip(rows.iter())
                .take_while(|(old, new)| old.key == new.key)
                .count();
            let suffix = self.rows[prefix..]
                .iter()
                .rev()
                .zip(rows[prefix..].iter().rev())
                .take_while(|(old, new)| old.key == new.key)
                .count();
            if prefix + suffix != self.rows.len() || prefix + suffix != rows.len() {
                let anchor = self.reconciled_anchor(&positions);
                self.state.splice(
                    prefix..self.rows.len() - suffix,
                    rows.len() - prefix - suffix,
                );
                self.state.scroll_to(anchor);
            }
            for (index, row) in rows.iter().enumerate() {
                if let Some(&old_index) = self.positions.get(&row.key)
                    && self.rows[old_index] != *row
                {
                    self.state.remeasure_items(index..index + 1);
                }
            }
        }
        self.rows = rows;
        self.positions = positions;
        self.publication = publication;
        cx.notify();
    }

    fn rebind(&mut self, source: Entity<Source>, cx: &mut Context<Self>) {
        self._subscription = cx.observe(&source, |collection, _, cx| collection.synchronize(cx));
        self.source = source;
        self.synchronize(cx);
    }

    fn activate(&mut self, target: Target, input: Input, cx: &mut Context<Self>) -> bool {
        let source = self.source.read(cx);
        let accepted = target.publication.source == self.source.entity_id()
            && target.publication.revision == source.revision
            && source
                .rows
                .iter()
                .any(|row| row.key == target.key && !row.hidden);
        self.trace.attempts.borrow_mut().push(Attempt {
            target,
            input,
            accepted,
        });
        if accepted {
            self.source
                .update(cx, |source, _| source.activated.push(target.key));
        }
        accepted
    }
}

impl Render for Collection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.trace.renders.set(self.trace.renders.get() + 1);
        self.trace.callbacks.set(0);
        self.trace.callback_keys.borrow_mut().clear();
        self.trace.painted.borrow_mut().clear();
        let rows = Rc::clone(&self.rows);
        let publication = self.publication;
        let owner = cx.entity().downgrade();
        let trace = Rc::clone(&self.trace);
        let font_size = self.font_size;
        let line_height = self.line_height;
        let activate_on_click = self.activate_on_click;
        div()
            .id("keyed-collection")
            .size_full()
            .p(px(16.))
            .flex()
            .flex_col()
            .items_start()
            .bg(rgb(0x101820))
            .track_focus(&self.focus)
            .on_key_down(
                cx.listener(|collection, event: &open_gpui::KeyDownEvent, _, cx| {
                    if event.keystroke.key == "enter"
                        && let Some(original_selection) = collection.selected
                    {
                        collection.activate(original_selection, Input::Keyboard, cx);
                    }
                }),
            )
            .child(div().h(px(16.)).flex_shrink_0())
            .child(
                list(self.state.clone(), move |index, _, _| {
                    let row = &rows[index];
                    let target = Target {
                        publication,
                        key: row.key,
                    };
                    trace.callbacks.set(trace.callbacks.get() + 1);
                    trace.callback_keys.borrow_mut().insert(row.key);
                    let painted_trace = Rc::clone(&trace);
                    let text = row.text.clone();
                    let pointer_owner = owner.clone();
                    let activate = move |window: &mut Window, app: &mut App| {
                        pointer_owner
                            .update(app, |collection, cx| {
                                if collection.activate(target, Input::Pointer, cx) {
                                    collection.selected = Some(target);
                                    window.focus(&collection.focus, cx);
                                }
                            })
                            .unwrap();
                    };
                    let element = div()
                        .id(("row", row.key))
                        .relative()
                        .w_full()
                        .min_h(px(MIN_ROW_HEIGHT))
                        .flex()
                        .flex_col()
                        .text_size(font_size)
                        .line_height(line_height)
                        .whitespace_normal()
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, (), window, _| {
                                    painted_trace.painted.borrow_mut().insert(
                                        target.key,
                                        PaintedRow {
                                            index,
                                            target,
                                            text,
                                            bounds,
                                            clip: window.clip_bounds(),
                                        },
                                    );
                                    window.paint_quad(fill(
                                        bounds,
                                        rgb(0x305080 + (target.key % 32) as u32),
                                    ));
                                },
                            )
                            .absolute()
                            .left(px(0.))
                            .top(px(0.))
                            .size_full(),
                        )
                        .child(row.text.clone())
                        .child(div().h(px(row.detail_height as f32)).flex_shrink_0());
                    if activate_on_click {
                        let press_trace = Rc::clone(&trace);
                        element
                            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                press_trace.presses.borrow_mut().push(target);
                            })
                            .on_click(move |_, window, app| activate(window, app))
                            .into_any_element()
                    } else {
                        element
                            .on_mouse_down(MouseButton::Left, move |_, window, app| {
                                activate(window, app);
                            })
                            .into_any_element()
                    }
                })
                .w(self.width)
                .h(px(VIEWPORT_HEIGHT))
                .flex_shrink_0(),
            )
    }
}

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
                    size(px(280.), px(200.)),
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

fn dispatch_click(window: &mut Window, app: &mut App, position: Point<Pixels>) {
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }),
        app,
    );
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            position,
            button: MouseButton::Left,
            modifiers: Default::default(),
            click_count: 1,
        }),
        app,
    );
}

struct Fixture {
    source: Entity<Source>,
    owner: Entity<Collection>,
    state: ListState,
    trace: Rc<Trace>,
    visual: VisualTestContext,
}

impl Fixture {
    fn new(cx: &mut TestAppContext, count: usize) -> Self {
        let source = cx.new(|_| Source::new(count));
        let trace = Rc::new(Trace::default());
        let (handle, visual) = open_root(cx, |_, app| {
            app.new(|cx| Collection::new(source.clone(), Rc::clone(&trace), cx))
        });
        let owner = handle.entity(&visual).unwrap();
        let state = owner.read_with(&visual, |collection, _| collection.state.clone());
        Self {
            source,
            owner,
            state,
            trace,
            visual,
        }
    }

    fn change(&mut self, change: impl FnOnce(&mut Source)) {
        self.source.update(&mut self.visual, |source, cx| {
            change(source);
            source.revision += 1;
            cx.notify();
        });
    }

    fn scroll_to(&mut self, key: u64, offset: Pixels) {
        self.owner.update(&mut self.visual, |collection, cx| {
            collection.state.scroll_to(ListOffset {
                item_ix: collection.positions[&key],
                offset_in_item: offset,
            });
            cx.notify();
        });
        submit_scene(&mut self.visual);
    }

    fn index(&self, key: u64) -> usize {
        self.owner
            .read_with(&self.visual, |collection, _| collection.positions[&key])
    }

    fn assert_anchor(&self, key: u64, offset: Pixels) {
        let anchor = self.state.logical_scroll_top();
        assert_eq!(anchor.item_ix, self.index(key));
        assert!((anchor.offset_in_item - offset).abs() < px(0.01));
        let bounds = self.state.bounds_for_item(anchor.item_ix).unwrap();
        assert!((bounds.top() - (self.state.viewport_bounds().top() - offset)).abs() < px(0.01));
    }

    fn visible_rows(&self) -> Vec<PaintedRow> {
        let viewport = self.state.viewport_bounds();
        assert_eq!(viewport.origin, point(px(16.), px(32.)));
        assert_eq!(viewport.size.height, px(VIEWPORT_HEIGHT));
        let mut painted: Vec<_> = self.trace.painted.borrow().values().cloned().collect();
        painted.sort_by_key(|row| row.index);
        assert!(!painted.is_empty());
        for row in &painted {
            assert_eq!(row.clip, viewport);
            assert!(!row.bounds.intersect(&row.clip).is_empty());
            assert_eq!(row.bounds.size.width, viewport.size.width);
            assert_eq!(self.state.bounds_for_item(row.index), Some(row.bounds));
        }
        for adjacent in painted.windows(2) {
            assert_eq!(adjacent[0].index + 1, adjacent[1].index);
            assert_eq!(adjacent[0].bounds.bottom(), adjacent[1].bounds.top());
        }
        assert!(painted[0].bounds.top() <= viewport.top());
        assert!(painted.last().unwrap().bounds.bottom() >= viewport.bottom());
        painted
    }

    fn point_in_first_row(&self) -> Point<Pixels> {
        self.state.viewport_bounds().origin + point(px(4.), px(2.))
    }

    fn activations(&self) -> Vec<u64> {
        self.source
            .read_with(&self.visual, |source, _| source.activated.clone())
    }

    fn last_attempt(&self) -> Attempt {
        *self.trace.attempts.borrow().last().unwrap()
    }
}

#[open_gpui::test]
fn bounded_keyed_rows_clip_and_dispatch_at_ten_and_one_hundred_thousand(cx: &mut TestAppContext) {
    for count in [10_000, 100_000] {
        let mut fixture = Fixture::new(cx, count);
        assert_eq!(fixture.state.item_count(), count);
        assert_eq!(fixture.trace.filter_visits.get(), count);
        assert_eq!(fixture.trace.order_visits.get(), count);
        assert_eq!(fixture.trace.identity_visits.get(), count);
        let key = (count / 2) as u64;
        fixture.scroll_to(key, px(9.));
        fixture.assert_anchor(key, px(9.));
        let rows = fixture.visible_rows();
        assert_eq!(rows[0].target.key, key);
        assert!(
            rows.windows(2)
                .any(|rows| rows[0].bounds.size.height != rows[1].bounds.size.height)
        );
        assert!(rows[0].bounds.top() < rows[0].clip.top());
        let last = rows.last().unwrap();
        assert!(last.bounds.bottom() > last.clip.bottom());
        let callback_bound = 2 * (VIEWPORT_HEIGHT / MIN_ROW_HEIGHT).ceil() as usize + 2;
        assert!(fixture.trace.callbacks.get() > 0);
        assert!(fixture.trace.callbacks.get() <= callback_bound);
        assert!(fixture.trace.callback_keys.borrow().len() <= callback_bound);
        assert!(
            !fixture
                .trace
                .callback_keys
                .borrow()
                .contains(&(count as u64 - 1))
        );
        assert_eq!(fixture.trace.filter_visits.get(), count);
        let viewport = fixture.state.viewport_bounds();
        fixture.visual.simulate_click(
            point(viewport.left() + px(4.), viewport.top() - px(2.)),
            Default::default(),
        );
        fixture.visual.simulate_click(
            point(viewport.left() + px(4.), viewport.bottom() + px(2.)),
            Default::default(),
        );
        assert!(fixture.trace.attempts.borrow().is_empty());
        let position = fixture.point_in_first_row();
        fixture.visual.simulate_click(position, Default::default());
        submit_scene(&mut fixture.visual);
        fixture.visual.simulate_keystrokes("enter");
        submit_scene(&mut fixture.visual);
        assert_eq!(fixture.activations(), [key, key]);
        assert_eq!(
            fixture.last_attempt(),
            Attempt {
                target: rows[0].target,
                input: Input::Keyboard,
                accepted: true,
            }
        );
        assert!(fixture.visual.simulate_close());
    }
}

#[open_gpui::test]
fn keyed_reconciliation_preserves_anchor_and_falls_forward_then_backward(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx, 10_000);
    let owner = fixture.owner.entity_id();
    fixture.scroll_to(5_000, px(9.));
    fixture.change(|source| {
        for key in [20_000, 20_001] {
            let mut row = Row::new(key);
            row.detail_height = 256;
            source.rows.insert(0, row);
        }
    });
    submit_scene(&mut fixture.visual);
    assert_eq!(fixture.state.item_count(), 10_002);
    fixture.assert_anchor(5_000, px(9.));
    fixture.change(|source| source.rows.retain(|row| row.key != 7));
    submit_scene(&mut fixture.visual);
    fixture.assert_anchor(5_000, px(9.));
    fixture.change(|source| source.reversed = true);
    submit_scene(&mut fixture.visual);
    fixture.assert_anchor(5_000, px(9.));
    fixture.visible_rows();

    fixture.change(|source| source.rows.retain(|row| row.key != 5_000));
    submit_scene(&mut fixture.visual);
    fixture.assert_anchor(4_999, px(0.));
    fixture.change(|source| {
        source
            .rows
            .iter_mut()
            .find(|row| row.key == 4_999)
            .unwrap()
            .hidden = true;
    });
    submit_scene(&mut fixture.visual);
    fixture.assert_anchor(4_998, px(0.));
    fixture.scroll_to(20_001, px(9.));
    fixture.change(|source| source.rows.retain(|row| row.key != 20_001));
    submit_scene(&mut fixture.visual);
    fixture.assert_anchor(20_000, px(0.));
    fixture.visible_rows();
    assert_eq!(fixture.state.item_count(), 9_998);
    assert_eq!(fixture.owner.entity_id(), owner);
    assert!(fixture.trace.identity_visits.get() > 10_000);
    assert!(fixture.visual.simulate_close());
}

#[open_gpui::test]
fn offscreen_content_width_and_typography_remeasure_the_original_list(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx, 10_000);
    let owner = fixture.owner.entity_id();
    let remote = 8_000;
    fixture.scroll_to(remote, px(0.));
    let old = fixture
        .state
        .bounds_for_item(fixture.index(remote))
        .unwrap();
    fixture.scroll_to(500, px(8.));
    assert!(
        fixture
            .state
            .bounds_for_item(fixture.index(remote))
            .is_some()
    );
    fixture.change(|source| {
        let row = source
            .rows
            .iter_mut()
            .find(|row| row.key == remote)
            .unwrap();
        row.text = "remote content wraps across many narrow columns "
            .repeat(3)
            .into();
        row.detail_height = 96;
    });
    submit_scene(&mut fixture.visual);
    fixture.assert_anchor(500, px(8.));
    assert!(
        fixture
            .state
            .bounds_for_item(fixture.index(remote))
            .is_none()
    );
    assert!(!fixture.trace.callback_keys.borrow().contains(&remote));
    fixture.scroll_to(remote, px(0.));
    let wide = fixture
        .state
        .bounds_for_item(fixture.index(remote))
        .unwrap();
    assert!(wide.size.height > old.size.height);
    assert!(
        fixture.trace.painted.borrow()[&remote]
            .text
            .starts_with("remote content")
    );
    fixture.visible_rows();

    fixture.scroll_to(500, px(8.));
    fixture.owner.update(&mut fixture.visual, |collection, cx| {
        collection.width = px(96.);
        cx.notify();
    });
    submit_scene(&mut fixture.visual);
    assert!(
        fixture
            .state
            .bounds_for_item(fixture.index(remote))
            .is_none()
    );
    fixture.scroll_to(remote, px(0.));
    let narrow = fixture
        .state
        .bounds_for_item(fixture.index(remote))
        .unwrap();
    assert_eq!(narrow.size.width, px(96.));
    assert!(narrow.size.height > wide.size.height);
    fixture.visible_rows();

    fixture.scroll_to(500, px(8.));
    let anchor_index = fixture.index(500);
    let before = fixture.state.bounds_for_item(anchor_index).unwrap();
    fixture.owner.update(&mut fixture.visual, |collection, cx| {
        collection.font_size = px(18.);
        collection.line_height = px(24.);
        collection.state.remeasure();
        cx.notify();
    });
    submit_scene(&mut fixture.visual);
    let after = fixture.state.bounds_for_item(anchor_index).unwrap();
    assert!(after.size.height > before.size.height);
    fixture.assert_anchor(500, after.size.height * (px(8.) / before.size.height));
    assert!(
        fixture
            .state
            .bounds_for_item(fixture.index(remote))
            .is_none()
    );
    assert!(!fixture.trace.callback_keys.borrow().contains(&remote));
    fixture.scroll_to(remote, px(0.));
    let larger_type = fixture
        .state
        .bounds_for_item(fixture.index(remote))
        .unwrap();
    assert!(larger_type.size.height > narrow.size.height);
    fixture.visible_rows();
    assert_eq!(fixture.state.item_count(), 10_000);
    assert_eq!(fixture.owner.entity_id(), owner);
    assert!(fixture.visual.simulate_close());
}

#[open_gpui::test]
fn stale_pointer_and_keyboard_keep_the_original_key_and_source_basis(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx, 10_000);
    fixture.scroll_to(5_000, px(9.));
    let position = fixture.point_in_first_row();
    fixture.visual.simulate_click(position, Default::default());
    submit_scene(&mut fixture.visual);
    fixture.visual.simulate_keystrokes("enter");
    let original = fixture.last_attempt().target;
    assert_eq!(fixture.activations(), [5_000, 5_000]);
    let generation = submit_scene(&mut fixture.visual);
    let source = fixture.source.clone();
    let trace = Rc::clone(&fixture.trace);
    let renders = trace.renders.get();
    fixture.visual.update(|window, app| {
        source.update(app, |source, cx| {
            source.rows[5_000] = Row::new(50_000);
            source.revision += 1;
            cx.notify();
        });
        dispatch_click(window, app, position);
        assert_eq!(trace.renders.get(), renders);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(generation)
        );
    });
    assert_eq!(
        fixture.last_attempt(),
        Attempt {
            target: original,
            input: Input::Pointer,
            accepted: false,
        }
    );
    fixture.visual.simulate_keystrokes("enter");
    assert_eq!(
        fixture.last_attempt(),
        Attempt {
            target: original,
            input: Input::Keyboard,
            accepted: false,
        }
    );
    submit_scene(&mut fixture.visual);
    assert_eq!(fixture.activations(), [5_000, 5_000]);
    fixture.visual.simulate_click(position, Default::default());
    submit_scene(&mut fixture.visual);
    fixture.visual.simulate_keystrokes("enter");
    let current = fixture.last_attempt().target;
    assert_eq!(current.key, 5_001);
    assert_eq!(current.publication.source, original.publication.source);
    assert_eq!(
        fixture.activations(),
        [5_000, 5_000, current.key, current.key]
    );

    let mut replacement = fixture
        .source
        .read_with(&fixture.visual, |source, _| source.clone());
    replacement.activated.clear();
    assert_eq!(replacement.revision, current.publication.revision);
    assert!(replacement.rows.iter().any(|row| row.key == current.key));
    let replacement = fixture.visual.new(|_| replacement);
    let old_activations = fixture.activations();
    let owner = fixture.owner.clone();
    let generation = submit_scene(&mut fixture.visual);
    let renders = fixture.trace.renders.get();
    fixture.visual.update(|window, app| {
        owner.update(app, |collection, cx| {
            collection.rebind(replacement.clone(), cx)
        });
        dispatch_click(window, app, position);
        assert_eq!(trace.renders.get(), renders);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(generation)
        );
    });
    assert_eq!(
        fixture.last_attempt(),
        Attempt {
            target: current,
            input: Input::Pointer,
            accepted: false,
        }
    );
    fixture.visual.simulate_keystrokes("enter");
    assert_eq!(
        fixture.last_attempt(),
        Attempt {
            target: current,
            input: Input::Keyboard,
            accepted: false,
        }
    );
    submit_scene(&mut fixture.visual);
    assert_eq!(fixture.activations(), old_activations);
    assert!(replacement.read_with(&fixture.visual, |source, _| source.activated.is_empty()));
    fixture.visual.simulate_click(position, Default::default());
    submit_scene(&mut fixture.visual);
    fixture.visual.simulate_keystrokes("enter");
    submit_scene(&mut fixture.visual);
    let accepted = fixture.last_attempt();
    assert!(accepted.accepted);
    assert_eq!(accepted.target.publication.source, replacement.entity_id());
    assert_eq!(
        accepted.target.publication.revision,
        current.publication.revision
    );
    assert_eq!(
        replacement.read_with(&fixture.visual, |source, _| source.activated.clone()),
        [accepted.target.key, accepted.target.key]
    );
    assert_eq!(fixture.state.item_count(), 10_000);
    assert!(fixture.visual.simulate_close());
}

#[open_gpui::test]
fn list_row_click_activates_on_release_without_an_intervening_frame(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx, 10_000);
    fixture.owner.update(&mut fixture.visual, |collection, cx| {
        collection.activate_on_click = true;
        cx.notify();
    });
    fixture.scroll_to(5_000, px(9.));
    let target = fixture.visible_rows()[0].target;
    assert_eq!(target.key, 5_000);
    let position = fixture.point_in_first_row();
    let generation = submit_scene(&mut fixture.visual);
    let trace = Rc::clone(&fixture.trace);
    let renders = trace.renders.get();
    let source = fixture.source.clone();
    fixture.visual.update(|window, app| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            app,
        );
        assert_eq!(trace.presses.borrow().as_slice(), &[target]);
        assert!(source.read(app).activated.is_empty());
        assert!(trace.attempts.borrow().is_empty());
        assert_eq!(trace.renders.get(), renders);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(generation)
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            app,
        );
        assert_eq!(source.read(app).activated.as_slice(), &[target.key]);
        assert_eq!(
            trace.attempts.borrow().as_slice(),
            &[Attempt {
                target,
                input: Input::Pointer,
                accepted: true,
            }]
        );
        assert_eq!(trace.renders.get(), renders);
        assert_eq!(
            window.presentation_facts().frame_accepted_generation,
            Some(generation)
        );
    });
    assert!(submit_scene(&mut fixture.visual) > generation);
    fixture.assert_anchor(5_000, px(9.));
    assert_eq!(fixture.activations(), [5_000]);
    assert_eq!(fixture.trace.attempts.borrow().len(), 1);
    assert!(fixture.visual.simulate_close());
}
