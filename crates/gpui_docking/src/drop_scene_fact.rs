use crate::{
    DockNodeId, DockSpaceId,
    drop_runtime::DockHostDropSceneFact,
    drop_target::{
        DockEmptySpaceDropTarget, DockFloatingTitleBarDropTarget, DockLeafDropTarget,
        DockRootDropTarget, DockTabBarDropTarget, DockTabLabelDropTarget,
    },
    host_render_session::{DockFloatingChromeTarget, DockHostPresentationSession},
    presentation_scene::{DockPresentationPaneKind, DockPresentationScene},
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

pub(crate) fn tab_bar(
    target_tabs: DockNodeId,
    insert_index: usize,
    bounds: Bounds<Pixels>,
    is_central: bool,
) -> DockHostDropSceneFact {
    DockHostDropSceneFact::TabBar(DockTabBarDropTarget {
        target_tabs,
        insert_index,
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

pub(crate) fn presentation_scene_drop_facts(
    scene: &DockPresentationScene,
    session: &DockHostPresentationSession,
) -> Vec<DockHostDropSceneFact> {
    let mut facts = Vec::new();

    if let Some(root) = scene.root {
        facts.push(self::root(root, scene.bounds));
    } else if session.has_empty_central_region() {
        facts.push(empty_central_space(scene.space.clone(), scene.bounds));
    } else {
        facts.push(empty_space(scene.space.clone(), scene.bounds));
    }

    for pane in &scene.panes {
        if pane.kind != DockPresentationPaneKind::Tabs {
            continue;
        }
        let Some(target_tabs) = pane.node else {
            continue;
        };
        let Some(root) = session.drop_root_for_tabs(target_tabs) else {
            continue;
        };
        facts.push(leaf(root, target_tabs, pane.bounds, pane.is_central));
    }

    for tab_bar in &scene.tab_bars {
        let insert_index = scene
            .tab_labels
            .iter()
            .filter(|label| label.tabs == tab_bar.tabs)
            .count();
        facts.push(self::tab_bar(
            tab_bar.tabs,
            insert_index,
            tab_bar.bounds,
            tab_bar.is_central,
        ));
    }

    for container in &scene.floating_containers {
        if let Some(DockFloatingChromeTarget::SingleTabs(target_tabs)) =
            session.floating_chrome_target(container.node)
        {
            facts.push(floating_title_bar(
                container.node,
                target_tabs,
                container.title_bar_bounds,
                container.bounds,
            ));
        }
    }

    facts
}
