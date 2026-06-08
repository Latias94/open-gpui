use crate::{
    DockAction, DockActionApplyError, DockActionOutcome, DockNode, DockPolicyError,
    host_test_support::*,
};
use open_gpui::TestAppContext;

#[open_gpui::test]
fn floating_action_respects_workspace_policy(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b"], 0);
    let mut workspace =
        workspace_with_panels(cx, graph, &[("a", "Panel A", "A"), ("b", "Panel B", "B")]);

    let result = workspace.apply_action(&DockAction::FloatItemInWindow {
        source_space: space(),
        item: item("a"),
        target_space: space(),
        bounds: floating_bounds(20.0, 30.0, 200.0, 120.0),
    });

    assert_eq!(
        result,
        Err(DockActionApplyError::Policy(
            DockPolicyError::FloatingDisabled
        ))
    );
    assert!(workspace.graph().floating_containers(&space()).is_empty());
    let DockNode::Tabs { items, .. } = workspace
        .graph()
        .node(root)
        .expect("root tabs should still exist")
    else {
        panic!("root should remain tabs");
    };
    assert_eq!(items, &vec![item("a"), item("b")]);
}

#[open_gpui::test]
fn floating_actions_create_move_raise_and_merge_containers(cx: &mut TestAppContext) {
    let (graph, root) = tabs_graph(&["a", "b", "c"], 0);
    let mut workspace = workspace_with_panels(
        cx,
        graph,
        &[
            ("a", "Panel A", "A"),
            ("b", "Panel B", "B"),
            ("c", "Panel C", "C"),
        ],
    );
    workspace.policy_mut().set_allow_floating(true);

    let first_bounds = floating_bounds(20.0, 30.0, 200.0, 120.0);
    let second_bounds = floating_bounds(60.0, 80.0, 180.0, 100.0);
    assert_eq!(
        workspace
            .apply_action(&DockAction::FloatItemInWindow {
                source_space: space(),
                item: item("a"),
                target_space: space(),
                bounds: first_bounds,
            })
            .expect("floating should be enabled"),
        DockActionOutcome::Changed
    );
    workspace
        .apply_action(&DockAction::FloatItemInWindow {
            source_space: space(),
            item: item("b"),
            target_space: space(),
            bounds: second_bounds,
        })
        .expect("second floating should be valid");

    let first = workspace.graph().floating_containers(&space())[0].node;
    let second = workspace.graph().floating_containers(&space())[1].node;
    assert_eq!(
        workspace
            .graph()
            .floating_containers(&space())
            .iter()
            .map(|container| container.node)
            .collect::<Vec<_>>(),
        vec![first, second]
    );

    workspace
        .apply_action(&DockAction::RaiseFloating {
            space: space(),
            floating: first,
        })
        .expect("raising a floating container should be valid");
    assert_eq!(
        workspace
            .graph()
            .floating_containers(&space())
            .iter()
            .map(|container| container.node)
            .collect::<Vec<_>>(),
        vec![second, first]
    );
    assert_eq!(
        workspace
            .apply_action(&DockAction::RaiseFloating {
                space: space(),
                floating: first,
            })
            .expect("raising the top floating container should be a valid no-op"),
        DockActionOutcome::Unchanged
    );

    let moved_bounds = floating_bounds(90.0, 100.0, 200.0, 120.0);
    workspace
        .apply_action(&DockAction::SetFloatingBounds {
            space: space(),
            floating: first,
            bounds: moved_bounds,
        })
        .expect("floating bounds update should be valid");
    assert_eq!(
        workspace
            .graph()
            .floating_containers(&space())
            .iter()
            .find(|container| container.node == first)
            .expect("first floating should remain present")
            .bounds,
        moved_bounds
    );
    assert_eq!(
        workspace
            .apply_action(&DockAction::SetFloatingBounds {
                space: space(),
                floating: first,
                bounds: moved_bounds,
            })
            .expect("setting identical floating bounds should be a valid no-op"),
        DockActionOutcome::Unchanged
    );

    workspace
        .apply_action(&DockAction::MergeFloatingInto {
            space: space(),
            floating: first,
            target_tabs: root,
        })
        .expect("floating merge should be valid");
    assert_eq!(workspace.graph().floating_containers(&space()).len(), 1);
    let DockNode::Tabs { items, .. } = workspace
        .graph()
        .node(root)
        .expect("root tabs should remain present")
    else {
        panic!("root should remain tabs");
    };
    assert_eq!(items, &vec![item("c"), item("a")]);
}
