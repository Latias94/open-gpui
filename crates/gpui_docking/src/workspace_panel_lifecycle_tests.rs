use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockNode, DockPanel, DockPanelAttachError,
    DockPanelDescriptor, DockWorkspace, host_test_support::*,
};
use open_gpui::{AppContext as _, TestAppContext};
use std::{cell::Cell, rc::Rc};

#[open_gpui::test]
fn workspace_close_item_transaction_respects_panel_policy(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel(
        item("a"),
        DockPanel::new("A", test_view(cx, "A")).closable(false),
    );
    workspace.register_panel_view(item("b"), "B", test_view(cx, "B"));

    let err = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("a"),
        })
        .expect_err("non-closable panel should block close");
    assert_eq!(
        err,
        DockActionApplyError::PanelNotClosable { item: item("a") }
    );

    let outcome = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("b"),
        })
        .expect("closable panel should close");
    assert_eq!(outcome, DockActionOutcome::Changed);
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(selected.as_ref(), items.get(0));
    assert!(workspace.panels().contains(&item("b")));
}

#[open_gpui::test]
fn workspace_close_item_transaction_uses_metadata_without_instantiating_lazy_panel(
    _cx: &mut TestAppContext,
) {
    let (graph, root) = tabs_graph(&["lazy"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    let instantiations = Rc::new(Cell::new(0));
    let observed_instantiations = instantiations.clone();
    workspace.register_panel_factory(item("lazy"), "Lazy", move |cx| {
        instantiations.set(instantiations.get() + 1);
        cx.new(|cx| TestPanel::new("Lazy", cx)).into()
    });

    let outcome = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("lazy"),
        })
        .expect("closable lazy panel should close from metadata");

    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(observed_instantiations.get(), 0);
    assert!(
        workspace.panels().has_view_lifecycle(&item("lazy")),
        "closing from metadata should keep lazy view lifecycle available"
    );
    let DockNode::Tabs { items, .. } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert!(items.is_empty());
}

#[open_gpui::test]
fn workspace_actions_can_use_descriptor_only_panel_metadata(_cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["anchor", "restored"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_descriptor(
        item("restored"),
        DockPanelDescriptor::new("Restored").closable(false),
    );

    let err = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("restored"),
        })
        .expect_err("descriptor-only close policy should still apply");
    assert_eq!(
        err,
        DockActionApplyError::PanelNotClosable {
            item: item("restored")
        }
    );

    workspace.register_panel_descriptor(item("restored"), DockPanelDescriptor::new("Restored"));
    let outcome = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("restored"),
        })
        .expect("closable descriptor-only panel should close");
    assert_eq!(outcome, DockActionOutcome::Changed);
    assert!(
        !workspace.panels().has_view_lifecycle(&item("restored")),
        "descriptor-only metadata should not create view lifecycle state"
    );

    let outcome = workspace
        .apply_action(&DockAction::OpenItem {
            space: space(),
            target_tabs: Some(root),
            item: item("restored"),
            insert_index: Some(0),
        })
        .expect("registered descriptor-only panel should reopen in graph state");
    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(
        workspace
            .panels()
            .descriptor(&item("restored"))
            .expect("metadata should stay registered")
            .title(),
        "Restored"
    );
}

#[open_gpui::test]
fn workspace_open_item_transaction_reopens_registered_lazy_panel_without_instantiating_view(
    _cx: &mut TestAppContext,
) {
    let (graph, root) = tabs_graph(&["a", "b"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel("a", DockPanel::lazy("A", |_| unreachable!()));
    let instantiations = Rc::new(Cell::new(0));
    let observed_instantiations = instantiations.clone();
    workspace.register_panel_factory("b", "B", move |cx| {
        instantiations.set(instantiations.get() + 1);
        cx.new(|cx| TestPanel::new("B", cx)).into()
    });

    workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("b"),
        })
        .expect("registered panel should close");
    assert!(
        workspace.panels().has_view_lifecycle(&item("b")),
        "closed panel should keep lazy view lifecycle available"
    );

    let outcome = workspace
        .apply_action(&DockAction::OpenItem {
            space: space(),
            target_tabs: Some(root),
            item: item("b"),
            insert_index: Some(1),
        })
        .expect("registered closed panel should reopen");

    assert_eq!(outcome, DockActionOutcome::Changed);
    assert_eq!(observed_instantiations.get(), 0);
    assert!(
        workspace.panels().has_view_lifecycle(&item("b")),
        "reopened panel registration should remain lazy without instantiating"
    );
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
    assert_eq!(selected.as_ref(), items.get(1));
}

#[test]
fn workspace_attach_panel_factory_preserves_restored_metadata() {
    let (graph, _root) = tabs_graph(&["restored"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_descriptor(
        item("restored"),
        DockPanelDescriptor::new("Restored").closable(false),
    );

    let previous = workspace
        .attach_panel_factory(item("restored"), |_| unreachable!())
        .expect("descriptor-backed attach should succeed");

    assert!(previous.is_none());
    let registration = workspace
        .panels()
        .get(&item("restored"))
        .expect("attached view lifecycle should complete registration");
    assert_eq!(registration.title(), "Restored");
    assert!(!registration.is_closable());

    assert!(matches!(
        workspace.attach_panel_factory(item("missing"), |_| unreachable!()),
        Err(DockPanelAttachError::MissingDescriptor { item }) if item == self::item("missing")
    ));
}

#[open_gpui::test]
fn workspace_open_item_transaction_requires_registered_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a"]);
    let mut workspace = workspace_with_panels(cx, graph, &[("a", "A", "A")]);

    let err = workspace
        .apply_action(&DockAction::OpenItem {
            space: space(),
            target_tabs: Some(root),
            item: item("missing"),
            insert_index: None,
        })
        .expect_err("missing panel metadata should block open policy");

    assert_eq!(
        err,
        DockActionApplyError::PanelNotRegistered {
            item: item("missing")
        }
    );
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(selected.as_ref(), items.get(0));
}

#[open_gpui::test]
fn workspace_close_item_transaction_requires_registered_panel(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a"]);
    let mut workspace = workspace_with_panels(cx, graph, &[]);

    let err = workspace
        .apply_action(&DockAction::CloseItem {
            space: space(),
            item: item("a"),
        })
        .expect_err("missing panel metadata should block close policy");

    assert_eq!(
        err,
        DockActionApplyError::PanelNotRegistered { item: item("a") }
    );
    let DockNode::Tabs { items, selected } = workspace
        .graph()
        .node(root)
        .expect("source tabs should remain")
    else {
        panic!("source should be tabs");
    };
    assert_eq!(items, &vec![item("a")]);
    assert_eq!(selected.as_ref(), items.get(0));
}
