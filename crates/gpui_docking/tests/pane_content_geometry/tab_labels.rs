use super::*;

use open_gpui::{App, RenderOnce};
use std::cell::{OnceCell, RefCell};

fn label_test_session() -> DockspaceSession {
    let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
        SURFACE,
        DockspaceRootLayout::new(RootId::new(37), DockspaceNode::tabs([CENTER, FLOATING])),
    )])
    .unwrap();
    DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap()
}

fn show_labels(cx: &mut TestAppContext, dock: &DockSurface) -> VisualTestContext {
    let handle = cx.open_window(size(px(900.0), px(640.0)), |_, _| Mounted(dock.clone()));
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, _| {
        let _ = window.activate_window();
    });
    present(&mut visual, dock);
    visual
}

#[open_gpui::test]
fn pure_tab_factory_can_read_the_committed_selection(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let mounted = Rc::new(OnceCell::<DockSurface>::new());
    let dock = cx.update(|app| {
        let first = DockPane::new(CENTER, "Editor", app.new(|_| Document(CENTER)));
        let weak = Rc::downgrade(&mounted);
        let second = DockPane::new(FLOATING, "Preview", app.new(|_| Document(FLOATING)))
            .with_tab_content(move |_, _| {
                let owner = weak.upgrade().unwrap();
                let active = owner
                    .get()
                    .unwrap()
                    .with_view(|view| view.item(FLOATING).unwrap().is_active());
                div().child(if active {
                    "Selected preview"
                } else {
                    "Preview"
                })
            });
        DockSurface::new(
            label_test_session(),
            [first, second],
            Default::default(),
            app,
        )
        .unwrap()
    });
    assert!(mounted.set(dock.clone()).is_ok());
    let mut visual = show_labels(cx, &dock);
    assert!(visual.debug_bounds("dockspace:tab:509").is_some());
}

#[derive(IntoElement)]
struct LabelWithState {
    width: u32,
    observed: Rc<RefCell<Vec<(u32, u32)>>>,
}

impl RenderOnce for LabelWithState {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let width = window.use_keyed_state("tab-label-local-width", cx, |_, _| self.width);
        let actual = *width.read(cx);
        self.observed.borrow_mut().push((self.width, actual));
        div().w(px(actual as f32)).h(px(12.0))
    }
}

fn labels_with_state(cx: &mut TestAppContext, explicit_scope: bool) -> (Vec<(u32, u32)>, [f32; 2]) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let dock = cx.update(|app| {
        let panes = [(CENTER, 80), (FLOATING, 152)].map(|(item, width)| {
            let observed = observed.clone();
            DockPane::new(item, "Editor", app.new(|_| Document(item))).with_tab_content(
                move |_, _| {
                    let child = LabelWithState {
                        width,
                        observed: observed.clone(),
                    };
                    if explicit_scope {
                        div()
                            .id(("user-explicit-pane-scope", item.get() as usize))
                            .child(child)
                            .into_any_element()
                    } else {
                        div().child(child).into_any_element()
                    }
                },
            )
        });
        DockSurface::new(label_test_session(), panes, Default::default(), app).unwrap()
    });
    let mut visual = show_labels(cx, &dock);
    let widths = ["dockspace:tab:201", "dockspace:tab:509"]
        .map(|id| f32::from(visual.debug_bounds(id).unwrap().size.width));
    let actual = observed.borrow().clone();
    (actual, widths)
}

#[open_gpui::test]
fn two_independent_tab_instances_do_not_share_local_gpui_state(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let (control, explicit_widths) = labels_with_state(cx, true);
    assert!(!control.is_empty());
    assert!(
        control.iter().all(|(declared, actual)| declared == actual),
        "{control:?}"
    );
    let (default, actual_widths) = labels_with_state(cx, false);
    assert!(!default.is_empty());
    assert_eq!(actual_widths, explicit_widths);
    assert!(
        default.iter().all(|(declared, actual)| declared == actual),
        "{default:?}"
    );
}

#[open_gpui::test]
fn height_relative_icon_fits_the_measured_label_without_clipping(cx: &mut TestAppContext) {
    cx.defer_next_window_frame_requests();
    let dock = cx.update(|app| {
        let first = DockPane::new(CENTER, "Editor", app.new(|_| Document(CENTER)));
        let second = DockPane::new(FLOATING, "Preview", app.new(|_| Document(FLOATING)))
            .with_tab_content(|_, _| {
                div()
                    .flex()
                    .h_full()
                    .flex_none()
                    .child(div().flex_none().w(px(95.0)).h(px(12.0)))
                    .child(
                        div()
                            .debug_selector(|| "trailing-icon".into())
                            .flex_none()
                            .h_full()
                            .aspect_square(),
                    )
            });
        DockSurface::new(
            label_test_session(),
            [first, second],
            Default::default(),
            app,
        )
        .unwrap()
    });
    let mut visual = show_labels(cx, &dock);
    let slot = visual.debug_bounds("dockspace:tab:509").unwrap();
    let icon = visual.debug_bounds("trailing-icon").unwrap();
    assert!(icon.size.width > px(0.0));
    assert!(
        icon.right() <= slot.right(),
        "icon should not be cut off in an unconstrained strip"
    );
}

#[open_gpui::test]
fn end_on_an_already_focused_last_tab_selects_it(cx: &mut TestAppContext) {
    use open_gpui::accesskit::{Action, ActionRequest, TreeId};
    cx.defer_next_window_frame_requests();
    let dock = cx.update(|app| {
        let panes = [CENTER, FLOATING].map(|item| {
            DockPane::new(
                item,
                format!("Tab {}", item.get()),
                app.new(|_| Document(item)),
            )
        });
        DockSurface::new(label_test_session(), panes, Default::default(), app).unwrap()
    });
    let handle = cx.open_window(size(px(900.0), px(640.0)), |_, _| Mounted(dock.clone()));
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, _| {
        let _ = window.activate_window();
    });
    assert!(visual.activate_accessibility());
    present(&mut visual, &dock);
    let tree = visual.latest_accessibility_tree_update().unwrap();
    let (tab, _) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Tab && node.label() == Some("Tab 509"))
        .unwrap();
    assert!(visual.dispatch_accessibility_action(ActionRequest {
        action: Action::Focus,
        target_tree: TreeId::ROOT,
        target_node: *tab,
        data: None
    }));
    assert!(dock.with_view(|view| view.item(CENTER).unwrap().is_active()));
    let handled = visual.update(|window, cx| {
        window.dispatch_keystroke(open_gpui::Keystroke::parse("end").unwrap(), cx)
    });
    present(&mut visual, &dock);
    let selected = dock.with_view(|view| view.item(FLOATING).unwrap().is_active());
    assert!(selected, "End handled={handled}");
}

struct FocusableEditor(open_gpui::FocusHandle);

impl Render for FocusableEditor {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("document-content")
            .track_focus(&self.0)
            .size_full()
            .child("Document")
    }
}

#[open_gpui::test]
fn body_shortcut_skips_a_last_header_whose_only_live_button_is_close(cx: &mut TestAppContext) {
    use dockspace::runtime::{SurfacePaintProgramOperation, SurfacePaintProgramStep};
    cx.defer_next_window_frame_requests();
    let third = ItemId::new(774);
    let floating = ItemId::new(775);
    let mut cover: Option<LogicalRect> = None;
    for should_cover in [false, true] {
        let mut surface = DockspaceSurfaceLayout::new(
            SURFACE,
            DockspaceRootLayout::new(
                RootId::new(37),
                DockspaceNode::central_tabs([CENTER, FLOATING, third]),
            ),
        );
        if should_cover {
            surface = surface.with_contained(DockspaceContainedLayout::new(
                FloatingPresentationId::new(9),
                DockspaceRootLayout::new(RootId::new(38), DockspaceNode::tabs([floating])),
                cover.unwrap(),
            ));
        }
        let layout = DockspaceLayout::new([surface]).unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let (dock, focus) = cx.update(|app| {
            let initial = app.focus_handle();
            let panes = [CENTER, FLOATING, third, floating].map(|item| {
                let focus = if item == CENTER {
                    initial.clone()
                } else {
                    app.focus_handle()
                };
                DockPane::new(
                    item,
                    format!("Tab {}", item.get()),
                    app.new(|_| FocusableEditor(focus.clone())),
                )
                .with_focus_handle(focus)
                .with_tab_content(|_, _| div().w(px(120.0)).h(px(12.0)))
            });
            (
                DockSurface::new(session, panes, Default::default(), app).unwrap(),
                initial,
            )
        });
        let mut visual = show_labels(cx, &dock);
        let (body, selectable, closable) = cx
            .update(|app| {
                dock.update(
                    |frame| {
                        let plan = frame.paint_plan(SURFACE)?.unwrap();
                        let mut row = None;
                        plan.visit_surface_program(|step| {
                            if let SurfacePaintProgramStep::Operation {
                                operation: SurfacePaintProgramOperation::Tab(tab),
                                ..
                            } = step
                                && tab.item() == Some(third)
                            {
                                row = Some((
                                    tab.drag_bounds(),
                                    plan.prepare_tab_select(third).is_some(),
                                    plan.prepare_tab_close(third).is_some(),
                                ));
                            }
                            Ok::<(), std::convert::Infallible>(())
                        })?;
                        Ok(row.unwrap())
                    },
                    app,
                )
            })
            .unwrap();
        if !should_cover {
            cover = Some(LogicalRect::new(body.x(), body.y(), body.width(), 140.0).unwrap());
        }
        assert_eq!(selectable, !should_cover);
        assert!(closable);
        present(&mut visual, &dock);
        visual.update(|window, app| window.focus(&focus, app));
        let handled = visual.update(|window, app| {
            window.dispatch_keystroke(open_gpui::Keystroke::parse("ctrl-tab").unwrap(), app)
        });
        present(&mut visual, &dock);
        let selected = dock.with_view(|view| view.item(FLOATING).unwrap().is_active());
        assert!(
            selected,
            "Ctrl+Tab should select the second tab with covered={should_cover}, handled={handled}"
        );
    }
}
