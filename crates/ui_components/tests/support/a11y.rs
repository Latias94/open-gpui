use open_gpui::accesskit;

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
