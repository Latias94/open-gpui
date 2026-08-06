use crate::{
    DockItemId, DockNodeId, DockSpaceId,
    locked_drop_identity::DockLockedPayloadIdentity,
    presentation_scene::{DockPresentationPaneKind, DockPresentationScene},
};
use open_gpui::{Bounds, EntityId, Pixels, retained_visual::SourceId};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockLivePayloadCarrier {
    pub(crate) kind: DockLivePayloadCarrierKind,
    pub(crate) bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockLivePayloadCarrierKind {
    Item {
        source_tabs: DockNodeId,
        item: DockItemId,
    },
    Tabs {
        source_tabs: DockNodeId,
    },
    Floating {
        floating: DockNodeId,
    },
}

impl DockLivePayloadCarrierKind {
    pub(crate) fn retained_source_id(
        &self,
        host: EntityId,
        binding_generation: u64,
        space: &DockSpaceId,
    ) -> SourceId {
        match self {
            Self::Item { source_tabs, .. } | Self::Tabs { source_tabs } => {
                tabs_retained_source_id(host, binding_generation, space, *source_tabs)
            }
            Self::Floating { floating } => {
                floating_retained_source_id(host, binding_generation, space, *floating)
            }
        }
    }
}

pub(crate) fn tabs_retained_source_id(
    host: EntityId,
    binding_generation: u64,
    space: &DockSpaceId,
    tabs: DockNodeId,
) -> SourceId {
    SourceId::new(format!(
        "dock-live-payload:{host:?}:{binding_generation}:{space}:tabs:{}",
        tabs.as_u64()
    ))
}

pub(crate) fn floating_retained_source_id(
    host: EntityId,
    binding_generation: u64,
    space: &DockSpaceId,
    floating: DockNodeId,
) -> SourceId {
    SourceId::new(format!(
        "dock-live-payload:{host:?}:{binding_generation}:{space}:floating:{}",
        floating.as_u64()
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum DockLivePayloadCarrierError {
    #[error("presentation scene belongs to dock space {actual}, expected {expected}")]
    SceneSpaceMismatch {
        expected: DockSpaceId,
        actual: DockSpaceId,
    },
    #[error("tabs pane {tabs:?} is missing from presentation scene for dock space {space}")]
    TabsPaneMissing {
        space: DockSpaceId,
        tabs: DockNodeId,
    },
    #[error("item {item} is not selected in tabs {tabs:?} for dock space {space}")]
    ItemNotSelected {
        space: DockSpaceId,
        tabs: DockNodeId,
        item: DockItemId,
    },
    #[error(
        "floating container {floating:?} is missing from presentation scene for dock space {space}"
    )]
    FloatingContainerMissing {
        space: DockSpaceId,
        floating: DockNodeId,
    },
}

pub(crate) fn resolve_live_payload_carrier(
    scene: &DockPresentationScene,
    identity: &DockLockedPayloadIdentity,
) -> Result<DockLivePayloadCarrier, DockLivePayloadCarrierError> {
    let source_space = identity.source_space();
    if scene.space != *source_space {
        return Err(DockLivePayloadCarrierError::SceneSpaceMismatch {
            expected: source_space.clone(),
            actual: scene.space.clone(),
        });
    }

    match identity {
        DockLockedPayloadIdentity::Item {
            source_tabs, item, ..
        } => {
            let bounds = tabs_pane_bounds(scene, *source_tabs)?;
            let selected = scene
                .focus_regions
                .iter()
                .any(|region| region.tabs == *source_tabs && region.item == *item);
            if !selected {
                return Err(DockLivePayloadCarrierError::ItemNotSelected {
                    space: scene.space.clone(),
                    tabs: *source_tabs,
                    item: item.clone(),
                });
            }

            Ok(DockLivePayloadCarrier {
                kind: DockLivePayloadCarrierKind::Item {
                    source_tabs: *source_tabs,
                    item: item.clone(),
                },
                bounds,
            })
        }
        DockLockedPayloadIdentity::Tabs { source_tabs, .. } => Ok(DockLivePayloadCarrier {
            kind: DockLivePayloadCarrierKind::Tabs {
                source_tabs: *source_tabs,
            },
            bounds: tabs_pane_bounds(scene, *source_tabs)?,
        }),
        DockLockedPayloadIdentity::Floating { floating, .. } => {
            let Some(container) = scene
                .floating_containers
                .iter()
                .find(|container| container.node == *floating)
            else {
                return Err(DockLivePayloadCarrierError::FloatingContainerMissing {
                    space: scene.space.clone(),
                    floating: *floating,
                });
            };

            Ok(DockLivePayloadCarrier {
                kind: DockLivePayloadCarrierKind::Floating {
                    floating: *floating,
                },
                bounds: container.bounds,
            })
        }
    }
}

fn tabs_pane_bounds(
    scene: &DockPresentationScene,
    tabs: DockNodeId,
) -> Result<Bounds<Pixels>, DockLivePayloadCarrierError> {
    scene
        .panes
        .iter()
        .find(|pane| pane.node == Some(tabs) && pane.kind == DockPresentationPaneKind::Tabs)
        .map(|pane| pane.bounds)
        .ok_or_else(|| DockLivePayloadCarrierError::TabsPaneMissing {
            space: scene.space.clone(),
            tabs,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockGraph, DockNode,
        presentation_scene::{
            DockPresentationFloatingContainer, DockPresentationFocusRegion, DockPresentationPane,
        },
    };
    use open_gpui::{point, px, size};

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    fn node_ids(count: usize) -> Vec<DockNodeId> {
        let mut graph = DockGraph::new();
        (0..count)
            .map(|index| {
                graph.insert_node(DockNode::Tabs {
                    items: vec![DockItemId::from(format!("item-{index}"))],
                    selected: Some(DockItemId::from(format!("item-{index}"))),
                })
            })
            .collect()
    }

    fn empty_scene(space: DockSpaceId, scene_bounds: Bounds<Pixels>) -> DockPresentationScene {
        DockPresentationScene {
            space,
            bounds: scene_bounds,
            root: None,
            panes: Vec::new(),
            tab_bars: Vec::new(),
            tab_labels: Vec::new(),
            splitters: Vec::new(),
            floating_containers: Vec::new(),
            focus_regions: Vec::new(),
            overlay_anchors: Vec::new(),
        }
    }

    fn tabs_pane(tabs: DockNodeId, pane_bounds: Bounds<Pixels>) -> DockPresentationPane {
        DockPresentationPane {
            node: Some(tabs),
            kind: DockPresentationPaneKind::Tabs,
            bounds: pane_bounds,
            floating: None,
            is_central: false,
        }
    }

    #[test]
    fn item_carrier_requires_exact_selected_focus_region_and_uses_full_pane_bounds() {
        let tabs = node_ids(1)[0];
        let source_space = DockSpaceId::from("source");
        let item = DockItemId::from("selected");
        let pane_bounds = bounds(20.0, 30.0, 420.0, 260.0);
        let mut scene = empty_scene(source_space.clone(), bounds(0.0, 0.0, 800.0, 600.0));
        scene.panes.push(tabs_pane(tabs, pane_bounds));
        scene.focus_regions.push(DockPresentationFocusRegion {
            tabs,
            item: item.clone(),
            bounds: bounds(20.0, 62.0, 420.0, 228.0),
        });
        let identity = DockLockedPayloadIdentity::Item {
            source_space,
            source_tabs: tabs,
            item: item.clone(),
        };

        let carrier = resolve_live_payload_carrier(&scene, &identity)
            .expect("selected item should resolve its pane carrier");

        assert_eq!(carrier.bounds, pane_bounds);
        assert_eq!(
            carrier.kind,
            DockLivePayloadCarrierKind::Item {
                source_tabs: tabs,
                item,
            }
        );
    }

    #[test]
    fn tabs_carrier_uses_corresponding_pane_bounds() {
        let tabs = node_ids(1)[0];
        let source_space = DockSpaceId::from("source");
        let pane_bounds = bounds(48.0, 36.0, 360.0, 240.0);
        let mut scene = empty_scene(source_space.clone(), bounds(0.0, 0.0, 800.0, 600.0));
        scene.panes.push(tabs_pane(tabs, pane_bounds));
        let identity = DockLockedPayloadIdentity::Tabs {
            source_space,
            source_tabs: tabs,
            ordered_items: vec![DockItemId::from("a"), DockItemId::from("b")],
        };

        let carrier = resolve_live_payload_carrier(&scene, &identity)
            .expect("tabs payload should resolve its pane carrier");

        assert_eq!(carrier.bounds, pane_bounds);
        assert_eq!(
            carrier.kind,
            DockLivePayloadCarrierKind::Tabs { source_tabs: tabs }
        );
    }

    #[test]
    fn item_and_tabs_share_the_pane_visual_source_but_floating_uses_its_container() {
        let ids = node_ids(2);
        let tabs = ids[0];
        let floating = ids[1];
        let space = DockSpaceId::from("source");
        let item = DockLivePayloadCarrierKind::Item {
            source_tabs: tabs,
            item: DockItemId::from("selected"),
        };
        let stack = DockLivePayloadCarrierKind::Tabs { source_tabs: tabs };
        let floating = DockLivePayloadCarrierKind::Floating { floating };
        let host = EntityId::from(7);
        let binding_generation = 11;

        assert_eq!(
            item.retained_source_id(host, binding_generation, &space),
            stack.retained_source_id(host, binding_generation, &space)
        );
        assert_ne!(
            stack.retained_source_id(host, binding_generation, &space),
            floating.retained_source_id(host, binding_generation, &space)
        );
        assert_ne!(
            item.retained_source_id(host, binding_generation, &space),
            item.retained_source_id(EntityId::from(8), binding_generation, &space)
        );
        assert_ne!(
            item.retained_source_id(host, binding_generation, &space),
            item.retained_source_id(host, binding_generation + 1, &space)
        );
    }

    #[test]
    fn floating_carrier_uses_full_container_including_chrome() {
        let ids = node_ids(2);
        let floating = ids[0];
        let child_root = ids[1];
        let source_space = DockSpaceId::from("source");
        let container_bounds = bounds(70.0, 55.0, 500.0, 340.0);
        let content_bounds = bounds(70.0, 87.0, 500.0, 308.0);
        let mut scene = empty_scene(source_space.clone(), bounds(0.0, 0.0, 900.0, 700.0));
        scene
            .floating_containers
            .push(DockPresentationFloatingContainer {
                node: floating,
                bounds: container_bounds,
                title_bar_bounds: bounds(70.0, 55.0, 500.0, 32.0),
                content_bounds,
            });
        let identity = DockLockedPayloadIdentity::Floating {
            source_space,
            floating,
            child_root,
            ordered_items: vec![DockItemId::from("floating-item")],
        };

        let carrier = resolve_live_payload_carrier(&scene, &identity)
            .expect("floating payload should resolve its full container");

        assert_ne!(container_bounds, content_bounds);
        assert_eq!(carrier.bounds, container_bounds);
        assert_eq!(
            carrier.kind,
            DockLivePayloadCarrierKind::Floating { floating }
        );
    }

    #[test]
    fn item_carrier_rejects_an_item_that_is_not_selected() {
        let tabs = node_ids(1)[0];
        let source_space = DockSpaceId::from("source");
        let requested = DockItemId::from("requested");
        let mut scene = empty_scene(source_space.clone(), bounds(0.0, 0.0, 800.0, 600.0));
        scene
            .panes
            .push(tabs_pane(tabs, bounds(20.0, 30.0, 420.0, 260.0)));
        scene.focus_regions.push(DockPresentationFocusRegion {
            tabs,
            item: DockItemId::from("other"),
            bounds: bounds(20.0, 30.0, 420.0, 260.0),
        });
        let identity = DockLockedPayloadIdentity::Item {
            source_space: source_space.clone(),
            source_tabs: tabs,
            item: requested.clone(),
        };

        assert_eq!(
            resolve_live_payload_carrier(&scene, &identity),
            Err(DockLivePayloadCarrierError::ItemNotSelected {
                space: source_space,
                tabs,
                item: requested,
            })
        );
    }

    #[test]
    fn carrier_rejects_a_scene_from_another_space() {
        let tabs = node_ids(1)[0];
        let expected = DockSpaceId::from("source");
        let actual = DockSpaceId::from("other");
        let scene = empty_scene(actual.clone(), bounds(0.0, 0.0, 800.0, 600.0));
        let identity = DockLockedPayloadIdentity::Tabs {
            source_space: expected.clone(),
            source_tabs: tabs,
            ordered_items: vec![DockItemId::from("a")],
        };

        assert_eq!(
            resolve_live_payload_carrier(&scene, &identity),
            Err(DockLivePayloadCarrierError::SceneSpaceMismatch { expected, actual })
        );
    }

    #[test]
    fn tabs_carrier_rejects_a_missing_pane() {
        let tabs = node_ids(1)[0];
        let source_space = DockSpaceId::from("source");
        let scene = empty_scene(source_space.clone(), bounds(0.0, 0.0, 800.0, 600.0));
        let identity = DockLockedPayloadIdentity::Tabs {
            source_space: source_space.clone(),
            source_tabs: tabs,
            ordered_items: vec![DockItemId::from("a")],
        };

        assert_eq!(
            resolve_live_payload_carrier(&scene, &identity),
            Err(DockLivePayloadCarrierError::TabsPaneMissing {
                space: source_space,
                tabs,
            })
        );
    }

    #[test]
    fn floating_carrier_rejects_a_missing_container() {
        let ids = node_ids(2);
        let floating = ids[0];
        let source_space = DockSpaceId::from("source");
        let scene = empty_scene(source_space.clone(), bounds(0.0, 0.0, 800.0, 600.0));
        let identity = DockLockedPayloadIdentity::Floating {
            source_space: source_space.clone(),
            floating,
            child_root: ids[1],
            ordered_items: vec![DockItemId::from("floating-item")],
        };

        assert_eq!(
            resolve_live_payload_carrier(&scene, &identity),
            Err(DockLivePayloadCarrierError::FloatingContainerMissing {
                space: source_space,
                floating,
            })
        );
    }
}
