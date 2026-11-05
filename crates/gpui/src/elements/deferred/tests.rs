use std::{cell::Cell, rc::Rc};

use crate::{
    AnyView, Context, Entity, StyleRefinement, TestAppContext, Window, anchored, deferred, div,
    point, prelude::*, px, size,
};

struct SubscriptionLeaf {
    value: usize,
    output: Rc<Cell<usize>>,
}

struct SubscriptionPanel {
    leaf: Entity<SubscriptionLeaf>,
    deferred_child: bool,
    renders: Rc<Cell<usize>>,
}

impl Render for SubscriptionPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let leaf = self.leaf.clone();
        let child = crate::canvas(
            |_, _, _| {},
            move |_, _, _, cx| {
                let leaf = leaf.read(cx);
                leaf.output.set(leaf.value);
            },
        )
        .size(px(50.));
        if self.deferred_child {
            deferred(child).into_any_element()
        } else {
            child.into_any_element()
        }
    }
}

struct SubscriptionRoot {
    panel: Entity<SubscriptionPanel>,
    deferred_panel: bool,
    renders: Rc<Cell<usize>>,
}

impl Render for SubscriptionRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let panel =
            AnyView::from(self.panel.clone()).cached(StyleRefinement::default().size(px(100.)));
        if self.deferred_panel {
            deferred(panel).into_any_element()
        } else {
            panel.into_any_element()
        }
    }
}

#[open_gpui::test]
fn test_cached_reuse_keeps_entity_subscriptions(cx: &mut TestAppContext) {
    for deferred_child in [true, false] {
        for deferred_panel in [false, true] {
            let output = Rc::new(Cell::new(0));
            let renders = Rc::new(Cell::new(0));
            let root_renders = Rc::new(Cell::new(0));
            let leaf = cx.new(|_| SubscriptionLeaf {
                value: 1,
                output: output.clone(),
            });
            let panel = cx.new(|_| SubscriptionPanel {
                leaf: leaf.clone(),
                deferred_child,
                renders: renders.clone(),
            });
            let window = cx.open_window(size(px(800.), px(600.)), |_, _| SubscriptionRoot {
                panel,
                deferred_panel,
                renders: root_renders.clone(),
            });
            cx.run_until_parked();
            assert_eq!(output.get(), 1);
            let initial_renders = renders.get();
            for _ in 0..2 {
                window
                    .update(cx, |_, _, cx| cx.notify())
                    .expect("notify root");
                cx.run_until_parked();
                assert_eq!(
                    renders.get(),
                    initial_renders,
                    "panel should reuse its cache"
                );
            }

            let renders_before_notify = root_renders.get();
            leaf.update(cx, |leaf, cx| {
                leaf.value = 2;
                cx.notify();
            });
            cx.run_until_parked();
            let context =
                format!("deferred child {deferred_child}, deferred panel {deferred_panel}");
            assert!(
                root_renders.get() > renders_before_notify,
                "leaf notification should redraw: {context}"
            );
            window
                .update(cx, |_, window, _| window.refresh())
                .expect("refresh canvas");
            cx.run_until_parked();
            assert_eq!(output.get(), 2, "{context}");
        }
    }
}

#[open_gpui::test]
fn test_cached_reuse_does_not_restore_released_entity_subscriptions(cx: &mut TestAppContext) {
    let output = Rc::new(Cell::new(0));
    let renders = Rc::new(Cell::new(0));
    let root_renders = Rc::new(Cell::new(0));
    let leaf = cx.new(|_| SubscriptionLeaf {
        value: 1,
        output: output.clone(),
    });
    let leaf_id = leaf.entity_id();
    let weak_leaf = leaf.downgrade();
    let panel = cx.new(|_| SubscriptionPanel {
        leaf: leaf.clone(),
        deferred_child: true,
        renders: renders.clone(),
    });
    let window = cx.open_window(size(px(800.), px(600.)), |_, _| SubscriptionRoot {
        panel: panel.clone(),
        deferred_panel: true,
        renders: root_renders.clone(),
    });
    cx.run_until_parked();
    assert_eq!(output.get(), 1);
    cx.read(|cx| {
        assert!(
            cx.window_invalidators_by_entity
                .get(&leaf_id)
                .is_some_and(|windows| windows.contains_key(&window.window_id())),
            "the initial paint should subscribe the window to the leaf"
        );
    });
    let initial_renders = renders.get();

    panel.update(cx, |panel, cx| {
        panel.leaf = cx.new(|_| SubscriptionLeaf {
            value: 2,
            output: output.clone(),
        });
    });
    drop(leaf);
    cx.update(|_| {});
    cx.run_until_parked();
    assert!(
        weak_leaf.upgrade().is_none(),
        "the old leaf should be released"
    );
    cx.read(|cx| {
        assert!(
            !cx.window_invalidators_by_entity.contains_key(&leaf_id),
            "releasing the leaf should remove its window subscriptions"
        );
    });

    for _ in 0..2 {
        let renders_before_notify = root_renders.get();
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("notify root");
        cx.run_until_parked();
        assert!(
            root_renders.get() > renders_before_notify,
            "notifying the root should replay the cached panel"
        );
        assert_eq!(
            renders.get(),
            initial_renders,
            "panel should reuse the cache that recorded the released leaf"
        );
        cx.read(|cx| {
            assert!(
                !cx.window_invalidators_by_entity.contains_key(&leaf_id),
                "cache reuse must not restore subscriptions to the released leaf"
            );
        });
    }
}

struct NestedMenuPanel {
    renders: Rc<Cell<usize>>,
}

impl Render for NestedMenuPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().key_context("Panel").size_full().child(
            deferred(
                anchored().position(point(px(10.), px(10.))).child(
                    div().key_context("Popover").w(px(200.)).h(px(200.)).child(
                        deferred(
                            anchored().position(point(px(30.), px(30.))).child(
                                div()
                                    .key_context("NestedMenu")
                                    .debug_selector(|| "NESTED_MENU".into())
                                    .size(px(50.)),
                            ),
                        )
                        .with_priority(2),
                    ),
                ),
            )
            .with_priority(1),
        )
    }
}

struct NestedMenuRoot {
    panel: Entity<NestedMenuPanel>,
}

impl Render for NestedMenuRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Root")
            .size_full()
            .child(AnyView::from(self.panel.clone()).cached(StyleRefinement::default().size_full()))
    }
}

#[open_gpui::test]
fn test_nested_deferred_draws_with_reused_views(cx: &mut TestAppContext) {
    let renders = Rc::new(Cell::new(0));
    let window = cx.open_window(size(px(800.), px(600.)), |_, cx| NestedMenuRoot {
        panel: cx.new(|_| NestedMenuPanel {
            renders: renders.clone(),
        }),
    });
    cx.run_until_parked();
    let initial_renders = renders.get();
    let initial_bounds = window
        .update(cx, |_, window, _| {
            assert_eq!(window.rendered_frame.deferred_draws.len(), 2);
            window.rendered_frame.debug_bounds["NESTED_MENU"]
        })
        .expect("read the nested menu bounds");
    assert_eq!(initial_bounds.size, size(px(50.), px(50.)));

    for _ in 0..2 {
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("reuse the panel containing both deferred menus");
        cx.run_until_parked();
        assert_eq!(renders.get(), initial_renders);
        window
            .update(cx, |_, window, _| {
                assert_eq!(window.rendered_frame.deferred_draws.len(), 2);
                assert_eq!(
                    window.rendered_frame.debug_bounds["NESTED_MENU"],
                    initial_bounds
                );
            })
            .expect("verify nested menu geometry after journal replay");
    }

    window
        .update(cx, |root, _, cx| {
            root.panel.update(cx, |_, cx| cx.notify());
        })
        .expect("rebuild the panel containing the nested menus");
    cx.run_until_parked();
    assert_eq!(renders.get(), initial_renders + 1);
    window
        .update(cx, |_, window, _| {
            assert_eq!(window.rendered_frame.deferred_draws.len(), 2);
            assert_eq!(
                window.rendered_frame.debug_bounds["NESTED_MENU"],
                initial_bounds
            );
        })
        .expect("verify nested menus after an explicit panel rebuild");
}
