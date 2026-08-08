use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockGraph, DockNode, DockPanel,
    DockPanelAttachError, DockPanelDescriptor, DockPanelOpenPlacementSource, DockPanelPlacement,
    DockPanelPlacementTarget, DockPanelRegistry, DockWorkspace, host_test_support::*,
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
fn workspace_close_item_transaction_removes_empty_tabs_without_instantiating_lazy_panel(
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
    assert_eq!(workspace.graph().root(&space()), None);
    assert!(
        workspace.graph().node(root).is_none(),
        "an unreachable runtime node must not survive canonicalization"
    );
}

#[open_gpui::test]
fn workspace_actions_can_use_descriptor_only_panel_metadata(_cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["anchor", "restored"]);
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_descriptor(
        item("restored"),
        DockPanelDescriptor::new("Restored")
            .closable(false)
            .dirty(true)
            .with_close_veto_reason("unsaved changes"),
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
    let descriptor = workspace
        .panels()
        .descriptor(&item("restored"))
        .expect("descriptor-only metadata should stay readable");
    assert!(descriptor.is_dirty());
    assert_eq!(descriptor.close_veto_reason(), Some("unsaved changes"));

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
fn workspace_open_item_at_placement_uses_product_target_resolution() {
    let graph = DockGraph::from_panel_placements(
        space(),
        [
            DockPanelPlacement::center("editor"),
            DockPanelPlacement::right_rail("inspector"),
        ],
    );
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_descriptor(item("terminal"), DockPanelDescriptor::new("Terminal"));

    let outcome = workspace
        .open_item_at_placement(
            space(),
            DockPanelPlacement::stacked_with("terminal", "inspector"),
        )
        .expect("registered panel should open beside its placement anchor");

    assert_eq!(outcome, DockActionOutcome::Changed);
    let graph = workspace.graph();
    let (terminal_tabs, terminal_index) = graph
        .find_item_in_space(&space(), &item("terminal"))
        .expect("terminal should open");
    let (inspector_tabs, inspector_index) = graph
        .find_item_in_space(&space(), &item("inspector"))
        .expect("inspector should remain in the right rail");
    assert_eq!(terminal_tabs, inspector_tabs);
    assert_eq!((inspector_index, terminal_index), (0, 1));
}

#[test]
fn workspace_close_panel_records_last_product_placement_without_instantiating_view() {
    let graph = DockGraph::from_panel_placements(
        space(),
        [
            DockPanelPlacement::center("editor"),
            DockPanelPlacement::bottom_rail("terminal").fraction(0.30),
        ],
    );
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_factory("terminal", "Terminal", |_| unreachable!());

    let outcome = workspace
        .close_panel(space(), item("terminal"))
        .expect("registered lazy panel should close from descriptor metadata");

    assert_eq!(outcome.action(), DockActionOutcome::Changed);
    assert_eq!(
        outcome.placement().map(DockPanelPlacement::target),
        Some(&DockPanelPlacementTarget::bottom_rail().fraction(0.30))
    );
    assert_eq!(
        workspace
            .panels()
            .descriptor(&item("terminal"))
            .and_then(DockPanelDescriptor::last_known_placement),
        Some(&DockPanelPlacementTarget::bottom_rail().fraction(0.30))
    );
    assert!(
        workspace.panels().has_view_lifecycle(&item("terminal")),
        "closing should preserve lazy lifecycle without instantiating the view"
    );
}

#[test]
fn workspace_reopen_panel_prefers_recorded_product_placement() {
    let graph = DockGraph::from_panel_placements(
        space(),
        [
            DockPanelPlacement::center("editor"),
            DockPanelPlacement::right_rail("inspector"),
            DockPanelPlacement::stacked_with("terminal", "inspector"),
        ],
    );
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_descriptor(
        item("terminal"),
        DockPanelDescriptor::new("Terminal")
            .with_default_placement(DockPanelPlacementTarget::bottom_rail()),
    );

    workspace
        .close_panel(space(), item("terminal"))
        .expect("terminal should close");
    let outcome = workspace
        .reopen_panel(space(), item("terminal"))
        .expect("terminal should reopen from recorded placement");

    assert_eq!(outcome.action(), DockActionOutcome::Changed);
    assert_eq!(
        outcome.placement_source(),
        DockPanelOpenPlacementSource::LastKnown
    );
    let (terminal_tabs, terminal_index) = workspace
        .graph()
        .find_item_in_space(&space(), &item("terminal"))
        .expect("terminal should reopen");
    let (inspector_tabs, inspector_index) = workspace
        .graph()
        .find_item_in_space(&space(), &item("inspector"))
        .expect("inspector should remain");
    assert_eq!(terminal_tabs, inspector_tabs);
    assert_eq!((inspector_index, terminal_index), (0, 1));
}

#[test]
fn workspace_reopen_panel_falls_back_to_descriptor_default_when_recorded_target_is_invalid() {
    let graph = DockGraph::from_panel_placements(
        space(),
        [
            DockPanelPlacement::center("editor"),
            DockPanelPlacement::left_rail("explorer"),
            DockPanelPlacement::right_rail("inspector"),
            DockPanelPlacement::stacked_with("terminal", "inspector"),
        ],
    );
    let mut workspace = DockWorkspace::new(space(), graph);
    workspace.register_panel_descriptor(item("inspector"), DockPanelDescriptor::new("Inspector"));
    workspace.register_panel_descriptor(
        item("terminal"),
        DockPanelDescriptor::new("Terminal")
            .with_default_placement(DockPanelPlacementTarget::left_rail()),
    );

    workspace
        .close_panel(space(), item("terminal"))
        .expect("terminal should close");
    workspace
        .close_item(space(), item("inspector"))
        .expect("closing the recorded anchor should make the last placement invalid");
    let outcome = workspace
        .reopen_panel(space(), item("terminal"))
        .expect("terminal should reopen from descriptor default");

    assert_eq!(outcome.action(), DockActionOutcome::Changed);
    assert_eq!(
        outcome.placement_source(),
        DockPanelOpenPlacementSource::DescriptorDefault
    );
    let (terminal_tabs, _) = workspace
        .graph()
        .find_item_in_space(&space(), &item("terminal"))
        .expect("terminal should reopen");
    let (explorer_tabs, _) = workspace
        .graph()
        .find_item_in_space(&space(), &item("explorer"))
        .expect("default left rail should still exist");
    assert_eq!(terminal_tabs, explorer_tabs);
}

#[test]
fn panel_registry_attach_view_handle_preserves_restored_metadata() {
    let (_graph, _root) = tabs_graph(&["restored"]);
    let mut registry = DockPanelRegistry::new();
    registry.register_descriptor(
        item("restored"),
        DockPanelDescriptor::new("Restored").closable(false),
    );

    let previous = registry
        .attach_view_handle(
            item("restored"),
            crate::panel_view::DockPanelViewHandle::lazy(|_| unreachable!()),
        )
        .expect("descriptor-backed attach should succeed");

    assert!(previous.is_none());
    let registration = registry
        .get(&item("restored"))
        .expect("attached view lifecycle should complete registration");
    assert_eq!(registration.title(), "Restored");
    assert!(!registration.is_closable());

    assert!(matches!(
        registry.attach_view_handle(item("missing"), crate::panel_view::DockPanelViewHandle::lazy(|_| unreachable!())),
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
