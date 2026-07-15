use open_gpui::accesskit;

#[allow(dead_code)]
pub(crate) fn assert_exact_actions(node: &accesskit::Node, expected: &[accesskit::Action]) {
    for &action in open_gpui::test::ACCESSKIT_ACTIONS {
        assert_eq!(
            node.supports_action(action),
            expected.contains(&action),
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
        .unwrap_or_else(|| panic!("missing accessibility node labelled {label:?}"))
}
