use open_gpui::accesskit;

#[allow(dead_code)]
pub(crate) fn assert_exact_actions(node: &accesskit::Node, expected: &[accesskit::Action]) {
    // All published semantic nodes expose the framework-owned geometry action. Callers specify
    // only role-specific activation and editing actions.
    for &action in open_gpui::test::ACCESSKIT_ACTIONS {
        assert_eq!(
            node.supports_action(action),
            expected.contains(&action) || action == accesskit::Action::ScrollIntoView,
            "unexpected support for {action:?} on {:?}",
            node.role()
        );
    }
}

pub(crate) fn node_with_label<'a>(
    update: &'a accesskit::TreeUpdate,
    label: &str,
) -> (accesskit::NodeId, &'a accesskit::Node) {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(label))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| {
            let labels = update
                .nodes
                .iter()
                .filter_map(|(_, node)| node.label())
                .collect::<Vec<_>>();
            panic!("missing accessibility node labelled {label:?}; published labels: {labels:?}")
        })
}
