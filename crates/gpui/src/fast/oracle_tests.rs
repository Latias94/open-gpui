use std::{cell::Cell, rc::Rc};

use accesskit::Role;

use crate::{
    AppContext as _, Background, BorderStyle, Bounds, Context, Corners, Edges, Entity, FocusHandle,
    Global, GpuClipShape, HitboxBehavior, HitboxTag, Hsla, InteractiveElement, IntoElement,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, PrimitiveTransform, Render, ScaledPixels, Scene, StatefulInteractiveElement, Styled,
    SubtreeClip, TestAppContext, Window, WindowHandle, canvas, div, fill, point, px, rgb, size,
    util::FluentBuilder as _,
};

const INITIAL_FILL: u32 = 0x334477;
const CHANGED_FILL: u32 = 0x994455;
const INITIAL_MARKER: u32 = 0x77aadd;
const CHANGED_MARKER: u32 = 0x88cc66;
const POINTER_FILL: u32 = 0xb87322;

const ORIGINAL_TARGET: Point<Pixels> = point(px(20.0), px(20.0));
const GROWN_TARGET: Point<Pixels> = point(px(66.0), px(20.0));
const MOVED_TARGET: Point<Pixels> = point(px(20.0), px(68.0));
const OUTSIDE_TARGET: Point<Pixels> = point(px(160.0), px(100.0));
const PROBES: [Point<Pixels>; 4] = [ORIGINAL_TARGET, GROWN_TARGET, MOVED_TARGET, OUTSIDE_TARGET];

struct Accent(u32);
impl Global for Accent {}

struct MarkerColor(u32);

struct Tile {
    tag: HitboxTag,
    focus: FocusHandle,
    width: Pixels,
    marker: Entity<MarkerColor>,
    clicks: Rc<Cell<usize>>,
    builds: usize,
}

impl Render for Tile {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.builds += 1;
        let mut fill = cx.global::<Accent>().0;
        if window.modifiers().shift && window.mouse_position().y >= MOVED_TARGET.y {
            fill = POINTER_FILL;
        }
        let marker = self.marker.read(cx).0;
        let clicks = self.clicks.clone();
        let focus = self.focus.clone();
        div()
            .id("tile")
            .role(Role::Button)
            .aria_label("Tile")
            .track_focus(&self.focus)
            .tab_stop(true)
            .hitbox_tag(self.tag)
            .block_mouse_except_scroll()
            .on_click(move |_, window, cx| {
                clicks.set(clicks.get() + 1);
                window.focus(&focus, cx);
            })
            .flex_none()
            .w(self.width)
            .h(px(24.0))
            .bg(rgb(fill))
            .child(div().id("marker").size(px(8.0)).bg(rgb(marker)))
    }
}

struct Panel {
    tile: Entity<Tile>,
    clipped: bool,
    builds: usize,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.builds += 1;
        div()
            .id("panel")
            .role(Role::Group)
            .aria_label("Panel")
            .on_mouse_down(MouseButton::Left, |_, _, _| {})
            .flex()
            .flex_none()
            .w_full()
            .h(px(40.0))
            .p(px(8.0))
            .gap(px(8.0))
            .when(self.clipped, |panel| {
                panel.rounded(px(6.0)).overflow_hidden()
            })
            .bg(rgb(0x202b3b))
            .child(self.tile.clone())
            .child(div().flex_none().w(px(24.0)).h(px(24.0)).bg(rgb(0x775522)))
    }
}

#[derive(Default)]
struct Status {
    builds: usize,
}

impl Render for Status {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.builds += 1;
        div()
            .id("status")
            .role(Role::Status)
            .aria_label("Status")
            .flex_none()
            .w_full()
            .h(px(12.0))
            .bg(rgb(0x3b4b20))
    }
}

struct Sheet {
    panel: Entity<Panel>,
    status: Entity<Status>,
    padding: Pixels,
    builds: usize,
}

impl Render for Sheet {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.builds += 1;
        div()
            .id("sheet")
            .role(Role::Group)
            .aria_label("Sheet")
            .flex()
            .flex_col()
            .size_full()
            .pt(self.padding)
            .child(self.panel.clone())
            .child(self.status.clone())
    }
}

struct Windows {
    retained: WindowHandle<Sheet>,
    redraw: WindowHandle<Sheet>,
    marker: Entity<MarkerColor>,
}

impl Windows {
    fn open(cx: &mut TestAppContext, clipped: bool) -> Self {
        let marker = cx.new(|_| MarkerColor(INITIAL_MARKER));
        let tag = HitboxTag::new();
        cx.update(|cx| cx.set_global(Accent(INITIAL_FILL)));
        let mut open = |retain| {
            cx.open_window(size(px(200.0), px(112.0)), |window, cx| {
                window.set_view_retention_for_test(retain);
                let tile = cx.new(|cx| Tile {
                    tag,
                    focus: cx.focus_handle(),
                    width: px(40.0),
                    marker: marker.clone(),
                    clicks: Rc::default(),
                    builds: 0,
                });
                Sheet {
                    panel: cx.new(|_| Panel {
                        tile,
                        clipped,
                        builds: 0,
                    }),
                    status: cx.new(|_| Status::default()),
                    padding: px(0.0),
                    builds: 0,
                }
            })
        };
        Self {
            retained: open(true),
            redraw: open(false),
            marker,
        }
    }

    fn pair(&self) -> [WindowHandle<Sheet>; 2] {
        [self.retained, self.redraw]
    }

    fn draw_and_compare(&self, cx: &mut TestAppContext, step: &str) -> [Drawn; 2] {
        let [retained, redraw] = self.pair().map(|handle| draw(cx, handle));
        assert_eq!(retained.frame, redraw.frame, "{step}");
        [retained, redraw]
    }

    fn grow_tile(&self, cx: &mut TestAppContext) {
        for handle in self.pair() {
            let tile = handle
                .read_with(cx, |sheet, cx| sheet.panel.read(cx).tile.clone())
                .unwrap();
            tile.update(cx, |tile, cx| {
                tile.width = px(80.0);
                cx.notify();
            });
        }
    }
}

#[derive(Debug, PartialEq)]
struct QuadFacts {
    bounds: Bounds<ScaledPixels>,
    clip_bounds: Bounds<ScaledPixels>,
    clips: Vec<GpuClipShape>,
    transform: PrimitiveTransform,
    background: Background,
    border: Option<(Hsla, Edges<ScaledPixels>, BorderStyle)>,
    corner_radii: Corners<ScaledPixels>,
}

fn quad_facts(scene: &Scene) -> Vec<QuadFacts> {
    scene
        .quads
        .iter()
        .map(|quad| {
            let start = quad.clip.first_clip as usize;
            let end = start + quad.clip.clip_count as usize;
            QuadFacts {
                bounds: quad.bounds,
                clip_bounds: quad.clip.conservative_bounds,
                clips: scene.clip_shapes()[start..end].to_vec(),
                transform: quad.transform,
                background: quad.background,
                border: (quad.border_color.a > 0.0 && quad.border_widths != Edges::default())
                    .then_some((quad.border_color, quad.border_widths, quad.border_style)),
                corner_radii: quad.corner_radii,
            }
        })
        .collect()
}

#[derive(Debug, PartialEq)]
struct HitboxFacts {
    bounds: Bounds<Pixels>,
    displayed_bounds: Bounds<Pixels>,
    clip_bounds: Bounds<Pixels>,
    behavior: HitboxBehavior,
    tile: bool,
}

#[derive(Debug, PartialEq)]
struct FrameFacts {
    quads: Vec<QuadFacts>,
    hitboxes: Vec<HitboxFacts>,
    hits: [(Vec<usize>, usize); PROBES.len()],
    tile_focused: bool,
    tab_stops: usize,
    clicks: usize,
}

impl FrameFacts {
    fn tile_hit(&self, position: Point<Pixels>) -> bool {
        let probe = PROBES.iter().position(|probe| *probe == position).unwrap();
        self.hits[probe]
            .0
            .iter()
            .any(|index| self.hitboxes[*index].tile)
    }
}

struct Drawn {
    frame: FrameFacts,
    builds: [usize; 4],
}

fn draw(cx: &mut TestAppContext, handle: WindowHandle<Sheet>) -> Drawn {
    cx.update_window(handle.into(), |root, window, cx| {
        let previous = window.rendered_frame_revision();
        window.draw_scheduled_frame(cx).clear(cx);
        assert!(window.rendered_frame_revision() > previous);

        let root = root.downcast::<Sheet>().unwrap();
        let sheet = root.read(cx);
        let panel = sheet.panel.read(cx);
        let tile = panel.tile.read(cx);
        let frame = &window.rendered_frame;
        let hitboxes = frame
            .hitboxes
            .iter()
            .map(|hitbox| HitboxFacts {
                bounds: hitbox.layout_bounds(),
                displayed_bounds: hitbox.displayed_bounds(),
                clip_bounds: hitbox.displayed_clip_bounds(),
                behavior: hitbox.behavior(),
                tile: hitbox.tag() == Some(tile.tag),
            })
            .collect();
        let hits = PROBES.map(|position| {
            let hit = frame.hit_test(position);
            let indices = hit
                .ids
                .iter()
                .map(|id| {
                    frame
                        .hitboxes
                        .iter()
                        .position(|hitbox| hitbox.id == *id)
                        .unwrap()
                })
                .collect();
            (indices, hit.hover_hitbox_count)
        });
        Drawn {
            frame: FrameFacts {
                quads: quad_facts(&frame.scene),
                hitboxes,
                hits,
                tile_focused: frame.focus_path().last() == Some(&tile.focus.id),
                tab_stops: frame.tab_stops.tab_stop_count(),
                clicks: tile.clicks.get(),
            },
            builds: [
                sheet.builds,
                panel.builds,
                tile.builds,
                sheet.status.read(cx).builds,
            ],
        }
    })
    .unwrap()
}

fn click(cx: &mut TestAppContext, handle: WindowHandle<Sheet>, position: Point<Pixels>) {
    let button = MouseButton::Left;
    let modifiers = Modifiers::none();
    cx.simulate_event(
        handle.into(),
        MouseDownEvent {
            position,
            button,
            modifiers,
            click_count: 1,
            first_mouse: false,
        },
    );
    cx.simulate_event(
        handle.into(),
        MouseUpEvent {
            position,
            button,
            modifiers,
            click_count: 1,
        },
    );
}

#[open_gpui::test]
fn retained_frames_match_full_redraw_through_edits_and_input(cx: &mut TestAppContext) {
    let windows = Windows::open(cx, true);
    let [initial, _] = windows.draw_and_compare(cx, "initial frame");
    assert!(initial.frame.tile_hit(ORIGINAL_TARGET));
    assert!(!initial.frame.tile_hit(GROWN_TARGET));
    assert!(
        initial
            .frame
            .quads
            .iter()
            .flat_map(|quad| &quad.clips)
            .any(|clip| clip.radii_x.top_left > ScaledPixels::default())
    );
    assert_eq!(initial.frame.tab_stops, 1);

    windows.grow_tile(cx);
    let [grown, _] =
        windows.draw_and_compare(cx, "notified nested tile grows and reflows its sibling");
    assert!(grown.frame.tile_hit(GROWN_TARGET));
    assert!(grown.builds[2] > initial.builds[2]);

    cx.update(|cx| cx.set_global(Accent(CHANGED_FILL)));
    let [global, _] =
        windows.draw_and_compare(cx, "tracked global changes without a view observer");
    assert!(
        global
            .frame
            .quads
            .iter()
            .any(|quad| quad.background == Background::from(rgb(CHANGED_FILL)))
    );

    windows
        .marker
        .update(cx, |marker, _| marker.0 = CHANGED_MARKER);
    let [entity, _] = windows.draw_and_compare(cx, "tracked entity changes without notify");
    assert!(
        entity
            .frame
            .quads
            .iter()
            .any(|quad| quad.background == Background::from(rgb(CHANGED_MARKER)))
    );

    for handle in windows.pair() {
        handle
            .update(cx, |sheet, _, cx| {
                sheet.padding = px(48.0);
                cx.notify();
            })
            .unwrap();
    }
    let [moved, _] = windows.draw_and_compare(cx, "ancestor moves the nested subtree");
    assert!(!moved.frame.tile_hit(ORIGINAL_TARGET));
    assert!(moved.frame.tile_hit(MOVED_TARGET));

    for handle in windows.pair() {
        cx.simulate_event(
            handle.into(),
            MouseMoveEvent {
                position: MOVED_TARGET,
                pressed_button: None,
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::none()
                },
            },
        );
    }
    let [pointer, _] = windows.draw_and_compare(cx, "pointer and modifier reads change the fill");
    assert!(
        pointer
            .frame
            .quads
            .iter()
            .any(|quad| quad.background == Background::from(rgb(POINTER_FILL)))
    );

    for handle in windows.pair() {
        click(cx, handle, ORIGINAL_TARGET);
        click(cx, handle, MOVED_TARGET);
    }
    let [clicked, _] =
        windows.draw_and_compare(cx, "only the moved target receives a click and focus");
    assert_eq!(clicked.frame.clicks, 1);
    assert!(clicked.frame.tile_focused);
}

#[open_gpui::test]
fn static_replay_skips_view_builds(cx: &mut TestAppContext) {
    let windows = Windows::open(cx, true);
    let [retained_before, redraw_before] = windows.draw_and_compare(cx, "warm frame");
    let [retained_after, redraw_after] = windows.draw_and_compare(cx, "static frame");
    assert_eq!(retained_before.frame, retained_after.frame);
    assert_eq!(retained_before.builds, retained_after.builds);
    assert_eq!(
        redraw_before.builds.map(|count| count + 1),
        redraw_after.builds
    );

    cx.update_window(windows.retained.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
    })
    .unwrap();
    let [after_explicit_draw, _] = windows.draw_and_compare(cx, "after explicit draw");
    assert_eq!(
        retained_after.builds.map(|count| count + 1),
        after_explicit_draw.builds
    );
}

struct Swatch {
    key: &'static str,
    color: u32,
    width: Pixels,
    children: Vec<Entity<Swatch>>,
    builds: usize,
}

impl Swatch {
    fn new(key: &'static str, color: u32, width: f32, children: Vec<Entity<Self>>) -> Self {
        Self {
            key,
            color,
            width: px(width),
            children,
            builds: 0,
        }
    }
}

impl Render for Swatch {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.builds += 1;
        div()
            .id(self.key)
            .flex()
            .flex_none()
            .w(self.width)
            .h(px(40.0))
            .bg(rgb(self.color))
            .children(self.children.iter().cloned())
    }
}

#[open_gpui::test]
fn dirty_paint_only_leaf_rebuilds_inside_replayed_ancestors(cx: &mut TestAppContext) {
    let windows = [true, false].map(|retain| {
        let leaf = cx.new(|_| Swatch::new("leaf", INITIAL_FILL, 40.0, vec![]));
        let window = cx.open_window(size(px(200.0), px(80.0)), |window, cx| {
            window.set_view_retention_for_test(retain);
            let parent = cx.new(|_| Swatch::new("parent", 0x202b3b, 80.0, vec![leaf.clone()]));
            let sibling = cx.new(|_| Swatch::new("sibling", 0x775522, 56.0, vec![]));
            Swatch::new("root", 0x3b4b20, 200.0, vec![parent, sibling])
        });
        (window, leaf)
    });
    let snapshot = |cx: &mut TestAppContext, handle: WindowHandle<Swatch>| {
        cx.update_window(handle.into(), |root, window, cx| {
            let root = root.downcast::<Swatch>().unwrap();
            let root = root.read(cx);
            let parent = root.children[0].read(cx);
            let sibling = root.children[1].read(cx);
            let leaf = parent.children[0].read(cx);
            (
                window.rendered_frame_revision(),
                quad_facts(&window.rendered_frame.scene),
                [root.builds, parent.builds, leaf.builds, sibling.builds],
            )
        })
        .unwrap()
    };

    let [
        (retained_revision, retained_before, retained_builds),
        (redraw_revision, redraw_before, redraw_builds),
    ] = windows.each_ref().map(|(handle, _)| snapshot(cx, *handle));
    assert_eq!(retained_before, redraw_before);
    // TestAppContext draws invalidated windows when the update flushes effects.
    cx.update(|cx| {
        for (_, leaf) in &windows {
            leaf.update(cx, |leaf, cx| {
                leaf.color = CHANGED_FILL;
                cx.notify();
            });
        }
    });
    let [
        (retained_next, retained_after, retained_counts),
        (redraw_next, redraw_after, redraw_counts),
    ] = windows.each_ref().map(|(handle, _)| snapshot(cx, *handle));
    assert!(retained_next > retained_revision);
    assert!(redraw_next > redraw_revision);
    assert_eq!(retained_after, redraw_after);
    let mut expected = retained_before;
    expected
        .iter_mut()
        .find(|quad| quad.background == Background::from(rgb(INITIAL_FILL)))
        .unwrap()
        .background = Background::from(rgb(CHANGED_FILL));
    assert_eq!(retained_after, expected);
    let [root, parent, leaf, sibling] = retained_builds;
    assert_eq!(
        retained_counts,
        [root, parent, leaf + 1, sibling],
        "the retained frame rebuilds only the dirty leaf",
    );
    assert_eq!(
        redraw_counts,
        redraw_builds.map(|count| count + 1),
        "the full redraw rebuilds each view for the same update",
    );
}

struct DetachedPane {
    panel: Entity<Swatch>,
    builds: usize,
}

impl Render for DetachedPane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.builds += 1;
        let panel = self.panel.clone();
        canvas(
            move |bounds, window, cx| {
                let pane = Bounds::new(
                    bounds.origin + point(px(18.0), px(14.0)),
                    size(px(80.0), px(32.0)),
                );
                let radius = size(px(6.0), px(6.0));
                let clip = SubtreeClip::try_own_rounded_border_box(Corners {
                    top_left: radius,
                    top_right: radius,
                    bottom_left: radius,
                    bottom_right: radius,
                })
                .unwrap();
                let clip = window.prepare_subtree_clip(&clip, pane);
                let mut container = div()
                    .id("detached-pane")
                    .flex()
                    .w(pane.size.width)
                    .h(pane.size.height)
                    .child(panel)
                    .into_any_element();
                window
                    .with_prepared_subtree_clip(&clip, |window| {
                        container.prepaint_as_root(pane.origin, pane.size.into(), window, cx);
                    })
                    .expect("prepaint the detached pane in its clip");
                (container, clip)
            },
            |bounds, (mut container, clip), window, cx| {
                window.paint_quad(fill(bounds, rgb(0x3b4b20)));
                window
                    .with_prepared_subtree_clip(&clip, |window| container.paint(window, cx))
                    .expect("paint the detached pane in the same clip");
                let rail = Bounds::new(bounds.origin, size(px(8.0), bounds.size.height));
                window.paint_quad(fill(rail, rgb(0x775522)));
            },
        )
        .size_full()
    }
}

#[open_gpui::test]
fn clipped_detached_root_splices_repeated_leaf_edits(cx: &mut TestAppContext) {
    struct Frame {
        revision: u64,
        quads: Vec<QuadFacts>,
        builds: [usize; 3],
    }

    let windows = [true, false].map(|retain| {
        let leaf = cx.new(|_| Swatch::new("leaf", INITIAL_FILL, 100.0, vec![]));
        let window = cx.open_window(size(px(200.0), px(80.0)), |window, cx| {
            window.set_view_retention_for_test(retain);
            DetachedPane {
                panel: cx.new(|_| Swatch::new("panel", 0x202b3b, 120.0, vec![leaf.clone()])),
                builds: 0,
            }
        });
        (window, leaf)
    });
    let snapshot = |cx: &mut TestAppContext, handle: WindowHandle<DetachedPane>| {
        cx.update_window(handle.into(), |root, window, cx| {
            let root = root.downcast::<DetachedPane>().unwrap();
            let root = root.read(cx);
            let panel = root.panel.read(cx);
            let leaf = panel.children[0].read(cx);
            Frame {
                revision: window.rendered_frame_revision(),
                quads: quad_facts(&window.rendered_frame.scene),
                builds: [root.builds, panel.builds, leaf.builds],
            }
        })
        .unwrap()
    };
    let [mut retained, mut redraw] = windows.each_ref().map(|(handle, _)| snapshot(cx, *handle));
    assert_eq!(retained.quads, redraw.quads);
    let leaf_index = retained
        .quads
        .iter()
        .position(|quad| quad.background == Background::from(rgb(INITIAL_FILL)))
        .expect("the leaf paints a quad in the detached pane");
    let leaf_quad = &retained.quads[leaf_index];
    let rail = retained
        .quads
        .iter()
        .find(|quad| quad.background == Background::from(rgb(0x775522)))
        .expect("the rail paints outside the pane clip");
    let (clip, inherited) = leaf_quad.clips.split_last().expect("the leaf is clipped");
    assert_eq!(inherited, rail.clips.as_slice());
    assert_eq!(clip.bounds, leaf_quad.clip_bounds);
    assert_eq!(clip.bounds.origin, leaf_quad.bounds.origin);
    assert_ne!(clip.bounds.origin, Point::default());
    assert!(clip.radii_x.top_left > ScaledPixels::default());
    assert!(clip.bounds.size.width < leaf_quad.bounds.size.width);
    assert!(clip.bounds.size.height < leaf_quad.bounds.size.height);

    for color in [CHANGED_FILL, INITIAL_FILL] {
        cx.update(|cx| {
            for (_, leaf) in &windows {
                leaf.update(cx, |leaf, cx| {
                    leaf.color = color;
                    cx.notify();
                });
            }
        });
        let [next_retained, next_redraw] =
            windows.each_ref().map(|(handle, _)| snapshot(cx, *handle));
        assert!(next_retained.revision > retained.revision);
        assert!(next_redraw.revision > redraw.revision);
        assert_eq!(next_retained.quads, next_redraw.quads, "color {color:#08x}");
        retained.quads[leaf_index].background = Background::from(rgb(color));
        assert_eq!(next_retained.quads, retained.quads, "color {color:#08x}");
        let [root, panel, leaf] = retained.builds;
        assert_eq!(
            next_retained.builds,
            [root, panel, leaf + 1],
            "only the leaf builds for color {color:#08x}",
        );
        assert_eq!(
            next_redraw.builds,
            redraw.builds.map(|count| count + 1),
            "all views build on full redraw for color {color:#08x}",
        );
        retained = next_retained;
        redraw = next_redraw;
    }
}

#[open_gpui::test]
fn a_dirty_view_cannot_replace_hitboxes_captured_by_its_ancestor(cx: &mut TestAppContext) {
    struct Leaf(bool);

    impl Render for Leaf {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let active = self.0;
            canvas(
                move |bounds, window, _| {
                    if active {
                        window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    }
                },
                |_, _, _, _| {},
            )
            .size_full()
        }
    }

    struct Root(Entity<Leaf>);

    impl Render for Root {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let leaf = self.0.clone();
            canvas(
                move |bounds, window, cx| {
                    let mut pane = div()
                        .id("capture-pane")
                        .size_full()
                        .child(leaf)
                        .into_any_element();
                    let (_, hitboxes) = window.capture_prepaint_hitboxes(|window| {
                        pane.prepaint_as_root(bounds.origin, bounds.size.into(), window, cx);
                    });
                    (pane, !hitboxes.is_empty())
                },
                |bounds, (mut pane, captured), window, cx| {
                    pane.paint(window, cx);
                    let color = if captured { CHANGED_FILL } else { INITIAL_FILL };
                    window.paint_quad(fill(bounds, rgb(color)));
                },
            )
            .size_full()
        }
    }

    let windows = [true, false].map(|retain| {
        let leaf = cx.new(|_| Leaf(false));
        let window = cx.open_window(size(px(200.), px(80.)), |window, _| {
            window.set_view_retention_for_test(retain);
            Root(leaf.clone())
        });
        (window, leaf)
    });
    let snapshot = |cx: &mut TestAppContext| {
        windows.each_ref().map(|(handle, _)| {
            cx.update_window((*handle).into(), |_, window, _| {
                quad_facts(&window.rendered_frame.scene)
            })
            .unwrap()
        })
    };
    let [retained, redraw] = snapshot(cx);
    assert_eq!(retained, redraw);
    assert_eq!(retained[0].background, Background::from(rgb(INITIAL_FILL)));
    cx.update(|cx| {
        for (_, leaf) in &windows {
            leaf.update(cx, |leaf, cx| {
                leaf.0 = true;
                cx.notify();
            });
        }
    });
    let [retained, redraw] = snapshot(cx);
    assert_eq!(retained, redraw);
    assert_eq!(retained[0].background, Background::from(rgb(CHANGED_FILL)));
}

#[cfg(not(target_family = "wasm"))]
#[derive(Debug, PartialEq)]
struct AccessibilityNode {
    role: Role,
    label: Option<String>,
    bounds: Option<accesskit::Rect>,
    focused: bool,
    clickable: bool,
    children: Vec<Self>,
}

#[cfg(not(target_family = "wasm"))]
fn accessibility_node(update: &accesskit::TreeUpdate, id: accesskit::NodeId) -> AccessibilityNode {
    let (_, node) = update.nodes.iter().find(|(key, _)| *key == id).unwrap();
    AccessibilityNode {
        role: node.role(),
        label: node.label().map(str::to_owned),
        bounds: node.bounds(),
        focused: update.focus == id,
        clickable: node.supports_action(accesskit::Action::Click),
        children: node
            .children()
            .iter()
            .map(|child| accessibility_node(update, *child))
            .collect(),
    }
}

#[cfg(not(target_family = "wasm"))]
#[open_gpui::test]
fn retained_accessibility_tree_matches_full_redraw(cx: &mut TestAppContext) {
    let windows = Windows::open(cx, false);
    for handle in windows.pair() {
        assert!(cx.activate_accessibility(handle.into()));
    }
    let snapshot = |cx: &TestAppContext, handle: WindowHandle<Sheet>| {
        let update = cx.latest_accessibility_tree_update(handle.into()).unwrap();
        let root = accessibility_node(&update, update.tree.as_ref().unwrap().root);
        let (tile_id, tile) = update
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Button)
            .unwrap();
        assert_eq!(tile.label(), Some("Tile"));
        (root, tile.bounds().unwrap(), update.focus == *tile_id)
    };
    let [warm, _] = windows.draw_and_compare(cx, "warm accessibility frame");
    let [retained, _] = windows.draw_and_compare(cx, "static accessibility frame");
    assert_eq!(retained.builds, warm.builds);
    let before = windows.pair().map(|handle| snapshot(cx, handle));
    assert_eq!(before[0], before[1]);
    assert!(!before[0].2);

    windows.grow_tile(cx);
    for handle in windows.pair() {
        click(cx, handle, ORIGINAL_TARGET);
    }
    let [frame, _] = windows.draw_and_compare(cx, "accessibility tile grows and takes focus");
    assert!(frame.frame.tile_focused);
    let after = windows.pair().map(|handle| snapshot(cx, handle));
    assert_eq!(after[0], after[1]);
    assert!(after[0].1.x1 > before[0].1.x1);
    assert!(after[0].2);
}
