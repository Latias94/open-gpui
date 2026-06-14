use crate::{
    DockActionApplyError, DockActionOutcome, DockGraph, DockGraphMutationError, DockNode,
    DockSpaceId, DockViewportDropPayload, DockViewportTearOffPending, DockViewportTearOffRequest,
    DockWorkspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportTearOffSourceStatus {
    Ready,
    Missing,
    Moved,
}

pub(crate) fn validate_tear_off_request(
    graph: &DockGraph,
    request: &DockViewportTearOffRequest,
) -> Result<(), DockActionApplyError> {
    match request.payload() {
        DockViewportDropPayload::Item(item) => {
            let source_tabs = request.source_node();
            if graph
                .find_item_in_space(request.source_space(), item)
                .is_none_or(|(tabs, _)| tabs != source_tabs)
            {
                return Err(DockActionApplyError::ItemNotInTabs {
                    tabs: source_tabs,
                    item: item.clone(),
                });
            }
        }
        DockViewportDropPayload::Tabs => {
            let source_tabs = request.source_node();
            if graph
                .root_for_node_in_space(request.source_space(), source_tabs)
                .is_none()
                || !matches!(
                    graph.node(source_tabs),
                    Some(DockNode::Tabs { items, .. }) if !items.is_empty()
                )
            {
                return Err(tear_off_payload_mismatch(
                    request.source_space(),
                    source_tabs,
                ));
            }
        }
        DockViewportDropPayload::Floating(floating) => {
            let source_floating = request.source_node();
            if source_floating != *floating {
                return Err(tear_off_payload_mismatch(
                    request.source_space(),
                    source_floating,
                ));
            }
            if graph
                .floating_containers(request.source_space())
                .iter()
                .all(|container| container.node != *floating)
            {
                return Err(DockGraphMutationError::FloatingContainerNotFound {
                    space: request.source_space().clone(),
                    floating: *floating,
                }
                .into());
            }
            if graph.collect_items_in_subtree(*floating).is_empty() {
                return Err(tear_off_payload_mismatch(
                    request.source_space(),
                    source_floating,
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn tear_off_source_status(
    graph: &DockGraph,
    pending: &DockViewportTearOffPending,
) -> DockViewportTearOffSourceStatus {
    let request = pending.request();
    match request.payload() {
        DockViewportDropPayload::Item(item) => graph
            .find_item_in_space(request.source_space(), item)
            .map(|(tabs, _)| {
                if tabs == request.source_node() {
                    DockViewportTearOffSourceStatus::Ready
                } else {
                    DockViewportTearOffSourceStatus::Moved
                }
            })
            .unwrap_or_else(|| {
                if graph.contains_item(item) {
                    DockViewportTearOffSourceStatus::Moved
                } else {
                    DockViewportTearOffSourceStatus::Missing
                }
            }),
        DockViewportDropPayload::Tabs => {
            let source_tabs = request.source_node();
            let Some(DockNode::Tabs { items, .. }) = graph.node(source_tabs) else {
                return DockViewportTearOffSourceStatus::Missing;
            };
            if graph
                .root_for_node_in_space(request.source_space(), source_tabs)
                .is_some()
                && !items.is_empty()
            {
                DockViewportTearOffSourceStatus::Ready
            } else {
                DockViewportTearOffSourceStatus::Moved
            }
        }
        DockViewportDropPayload::Floating(floating) => {
            if request.source_node() != *floating {
                return DockViewportTearOffSourceStatus::Missing;
            }
            if graph
                .floating_containers(request.source_space())
                .iter()
                .all(|container| container.node != *floating)
            {
                return DockViewportTearOffSourceStatus::Missing;
            }
            if !graph.collect_items_in_subtree(*floating).is_empty() {
                DockViewportTearOffSourceStatus::Ready
            } else {
                DockViewportTearOffSourceStatus::Moved
            }
        }
    }
}

pub(crate) fn commit_tear_off_move(
    workspace: &mut DockWorkspace,
    pending: &DockViewportTearOffPending,
) -> Result<DockActionOutcome, DockActionApplyError> {
    let request = pending.request();
    match request.payload() {
        DockViewportDropPayload::Item(item) => workspace.commit_item_to_empty_dock_space(
            request.source_space(),
            item,
            pending.target_space(),
        ),
        DockViewportDropPayload::Tabs => workspace.commit_tabs_to_empty_dock_space(
            request.source_space(),
            request.source_node(),
            pending.target_space(),
        ),
        DockViewportDropPayload::Floating(floating) => workspace
            .commit_floating_to_empty_dock_space(
                request.source_space(),
                *floating,
                pending.target_space(),
            ),
    }
}

fn tear_off_payload_mismatch(
    source_space: &DockSpaceId,
    source_tabs: crate::DockNodeId,
) -> DockActionApplyError {
    DockActionApplyError::DropPayloadMismatch {
        space: source_space.clone(),
        tabs: source_tabs,
    }
}
