use crate::{
    DockNodeId, DockSpaceId,
    drop_runtime::DockHostDropSceneFact,
    drop_target::{
        DockEmptySpaceDropTarget, DockFloatingTitleBarDropTarget, DockLeafDropTarget,
        DockRootDropTarget, DockTabLabelDropTarget,
    },
};
use open_gpui::{Bounds, Pixels};

pub(crate) fn tab_label(
    target_tabs: DockNodeId,
    target_index: usize,
    bounds: Bounds<Pixels>,
    is_central: bool,
) -> DockHostDropSceneFact {
    DockHostDropSceneFact::TabLabel(DockTabLabelDropTarget {
        target_tabs,
        target_index,
        bounds,
        is_central,
    })
}

pub(crate) fn leaf(
    root: DockNodeId,
    target_tabs: DockNodeId,
    bounds: Bounds<Pixels>,
    is_central: bool,
) -> DockHostDropSceneFact {
    DockHostDropSceneFact::Leaf(DockLeafDropTarget {
        root,
        target_tabs,
        bounds,
        is_central,
    })
}

pub(crate) fn root(root: DockNodeId, bounds: Bounds<Pixels>) -> DockHostDropSceneFact {
    DockHostDropSceneFact::Root(DockRootDropTarget { root, bounds })
}

pub(crate) fn empty_space(space: DockSpaceId, bounds: Bounds<Pixels>) -> DockHostDropSceneFact {
    DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
        space,
        bounds,
        is_central: false,
    })
}

pub(crate) fn empty_central_space(
    space: DockSpaceId,
    bounds: Bounds<Pixels>,
) -> DockHostDropSceneFact {
    DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
        space,
        bounds,
        is_central: true,
    })
}

pub(crate) fn floating_title_bar(
    floating: DockNodeId,
    target_tabs: DockNodeId,
    title_bounds: Bounds<Pixels>,
    preview_bounds: Bounds<Pixels>,
) -> DockHostDropSceneFact {
    DockHostDropSceneFact::FloatingTitleBar(DockFloatingTitleBarDropTarget {
        floating,
        target_tabs,
        title_bounds,
        preview_bounds,
    })
}
