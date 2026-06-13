use crate::{DockGraph, DockItemId, DockNode, DockNodeId, DockSpaceId};

pub(crate) fn main_space() -> DockSpaceId {
    DockSpaceId::new("main")
}

pub(crate) fn space(id: &str) -> DockSpaceId {
    DockSpaceId::new(id)
}

pub(crate) fn item(id: &str) -> DockItemId {
    DockItemId::new(id)
}

pub(crate) fn root_tabs_graph(items: &[&str]) -> (DockGraph, DockNodeId) {
    let mut graph = DockGraph::new();
    let items: Vec<DockItemId> = items.iter().copied().map(item).collect();
    let selected = items.first().cloned();
    let root = graph.insert_node(DockNode::Tabs { items, selected });
    graph.set_root(main_space(), root);
    (graph, root)
}
