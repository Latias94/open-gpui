use std::{cell::OnceCell, rc::Weak};

use dockspace::geometry::LogicalSize;
use open_gpui::{
    Empty, FocusHandle, KeyDownEvent, QuitMode, SubtreeTransform, SubtreeTransformExt,
    SubtreeTransformOrigin,
};

use super::*;

fn tabs_layout(items: impl IntoIterator<Item = ItemId>) -> DockspaceSession {
    let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
        SURFACE,
        DockspaceRootLayout::new(RootId::new(37), DockspaceNode::tabs(items)),
    )])
    .unwrap();
    DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap()
}

struct TransformedDock {
    dock: DockSurface,
    transform: Rc<Cell<SubtreeTransform>>,
}

impl Render for TransformedDock {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .w(px(570.0))
                .h(px(270.0))
                .child(self.dock.element(SURFACE))
                .with_subtree_transform(self.transform.get()),
        )
    }
}

#[open_gpui::test]
fn clicking_displayed_tabs_uses_the_committed_subtree_transform(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_quit_mode(QuitMode::Explicit));
    let items = [1, 2, 3].map(ItemId::new);
    for custom_label in [false, true] {
        let dock = cx.update(|cx| {
            let panes = items.map(|item| {
                let pane =
                    DockPane::new(item, format!("Document {}", item.get()), cx.new(|_| Empty));
                if custom_label {
                    pane.with_tab_content(move |_, _| div().w(px(100.0)).child("Document"))
                } else {
                    pane
                }
            });
            DockSurface::new(tabs_layout(items), panes, Default::default(), cx).unwrap()
        });
        let transform = Rc::new(Cell::new(SubtreeTransform::identity()));
        cx.defer_next_window_frame_requests();
        let handle = cx.open_window(size(px(940.0), px(490.0)), |_, _| TransformedDock {
            dock: dock.clone(),
            transform: transform.clone(),
        });
        let mut visual = VisualTestContext::from_window(handle.into(), cx);
        visual.update(|window, _| {
            let _ = window.activate_window();
        });
        cx.set_platform_hovered_window(Some(handle.into()));
        assert!(visual.activate_accessibility());
        present(&mut visual, &dock);
        let second = visual.debug_bounds("dockspace:tab:2").unwrap().center();
        let third = visual.debug_bounds("dockspace:tab:3").unwrap().center();
        let close = visual
            .latest_accessibility_tree_update()
            .unwrap()
            .nodes
            .iter()
            .find_map(|(_, node)| {
                (node.label() == Some("Close Document 3"))
                    .then(|| node.bounds())
                    .flatten()
            })
            .unwrap();
        let dpr = visual.update(|window, _| f64::from(window.scale_factor()));
        let close_x = px(((close.x0 + close.x1) / (2.0 * dpr)) as f32);
        for (translation, scale) in [
            (third.x - second.x, 1.0),
            (close_x - second.x, 1.0),
            (px(0.0), 1.5),
        ] {
            transform.set(
                SubtreeTransform::try_new(
                    size(scale, scale),
                    point(translation, px(0.0)),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            );
            present(&mut visual, &dock);
            let tree = visual.latest_accessibility_tree_update().unwrap();
            let displayed = tree
                .nodes
                .iter()
                .find_map(|(_, node)| {
                    (node.role() == Role::Tab && node.label() == Some("Document 2"))
                        .then(|| node.bounds())
                        .flatten()
                })
                .unwrap();
            let click = point(
                px(((displayed.x0 + displayed.x1) / (2.0 * dpr)) as f32),
                px(((displayed.y0 + displayed.y1) / (2.0 * dpr)) as f32),
            );
            assert!(!dock.with_view(|view| view.item(items[1]).unwrap().is_active()));
            visual.simulate_click(click, Modifiers::none());
            present(&mut visual, &dock);
            assert!(
                dock.with_view(|view| view.item(items[1]).unwrap().is_active()),
                "custom label: {custom_label}, translation: {translation}, scale: {scale}"
            );
            assert!(dock.pending_content_close_plans().unwrap().is_empty());
            visual.update(|_, cx| {
                dock.update(
                    |frame| {
                        frame.select_item_current(items[0])?;
                        Ok(())
                    },
                    cx,
                )
                .unwrap();
            });
        }
        visual.update(|window, cx| window.remove_window(cx));
    }
}

struct Editor {
    focus: FocusHandle,
    text: String,
    modified: Rc<Cell<bool>>,
    dock: Weak<OnceCell<DockSurface>>,
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("typing-editor")
            .size_full()
            .track_focus(&self.focus)
            .debug_selector(|| "typing-editor".to_owned())
            .on_key_down(cx.listener(|editor, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.len() == 1 {
                    editor.text.push_str(&event.keystroke.key);
                    if !editor.modified.replace(true) {
                        let dock = editor.dock.upgrade().unwrap();
                        dock.get().unwrap().refresh_tab_content(CENTER, cx).unwrap();
                    }
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .child(self.text.clone())
    }
}

#[open_gpui::test]
fn dirty_marker_and_other_tab_rename_preserve_editor_input(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_quit_mode(QuitMode::Explicit));
    for managed in [false, true] {
        let owner = Rc::new(OnceCell::new());
        let modified = Rc::new(Cell::new(false));
        let (dock, editor, focus) = cx.update(|cx| {
            let focus = cx.focus_handle();
            let editor = cx.new(|_| Editor {
                focus: focus.clone(),
                text: String::new(),
                modified: modified.clone(),
                dock: Rc::downgrade(&owner),
            });
            let pane = DockPane::new(CENTER, "Notes", editor.clone())
                .with_focus_handle(focus.clone())
                .with_tab_content(move |_, _| {
                    let (width, title) = if modified.get() {
                        (104.0, "Notes •")
                    } else {
                        (80.0, "Notes")
                    };
                    div().w(px(width)).child(title)
                });
            let panes = [pane, DockPane::new(FLOATING, "Other", cx.new(|_| Empty))];
            let session = tabs_layout([CENTER, FLOATING]);
            let dock = if managed {
                DockSurface::new_managed(session, panes, Default::default(), cx)
            } else {
                DockSurface::new(session, panes, Default::default(), cx)
            }
            .unwrap();
            (dock, editor, focus)
        });
        assert!(owner.set(dock.clone()).is_ok());
        cx.defer_next_window_frame_requests();
        let handle = cx.open_window(size(px(700.0), px(490.0)), |window, cx| {
            if managed {
                dock.bind_native_root(SURFACE, true, window, cx).unwrap();
            }
            Mounted(dock.clone())
        });
        let mut visual = VisualTestContext::from_window(handle.into(), cx);
        visual.update(|window, _| {
            let _ = window.activate_window();
        });
        present(&mut visual, &dock);
        visual.update(|window, cx| window.focus(&focus, cx));
        present(&mut visual, &dock);
        let tab = format!("dockspace:tab:{}", CENTER.get());
        let label = visual.debug_bounds(&tab).unwrap();
        let pane = visual.debug_bounds("typing-editor").unwrap();
        visual.simulate_keystrokes("x");
        present(&mut visual, &dock);
        assert_eq!(
            visual.debug_bounds(&tab).unwrap().size.width - label.size.width,
            px(24.0)
        );
        assert_eq!(visual.debug_bounds("typing-editor"), Some(pane));
        visual.simulate_keystrokes("y");
        assert_eq!(
            editor.read_with(&visual, |editor, _| editor.text.clone()),
            "xy",
            "managed: {managed}"
        );
        visual.update(|_, cx| {
            dock.update_pane_metadata(
                FLOATING,
                "Renamed",
                LogicalSize::new(120.0, 90.0).unwrap(),
                cx,
            )
            .unwrap();
        });
        present(&mut visual, &dock);
        visual.simulate_keystrokes("z");
        assert_eq!(
            editor.read_with(&visual, |editor, _| editor.text.clone()),
            "xyz"
        );
        assert!(visual.update(|window, _| focus.is_focused(window)));
        if managed {
            cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
            cx.run_until_parked();
        } else {
            visual.update(|window, cx| window.remove_window(cx));
        }
    }
}
