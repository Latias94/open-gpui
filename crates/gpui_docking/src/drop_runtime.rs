#[cfg(test)]
use crate::drop_target::DockDropPreviewIntent;
use crate::{
    DockNodeId, DockPolicy, DropZone,
    drop_target::{
        self, DockDropResolution, DockDropResolverInput, DockEmptySpaceDropTarget,
        DockFloatingTitleBarDropTarget, DockLeafDropTarget, DockResolvedDropTarget,
        DockRootDropTarget, DockTabLabelDropTarget, DockTearOffCandidateDropTarget,
    },
};
use open_gpui::{Bounds, Pixels, Point};

#[derive(Debug, Default)]
pub(crate) struct DockDropRuntime {
    target: Option<DockResolvedDropTarget>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropTargetUpdate {
    pub(crate) position: Point<Pixels>,
    pub(crate) tab_labels: Vec<DockTabLabelDropTarget>,
    pub(crate) leaves: Vec<DockLeafDropTarget>,
    pub(crate) root: Option<DockRootDropTarget>,
    pub(crate) floating_title_bars: Vec<DockFloatingTitleBarDropTarget>,
    pub(crate) empty_spaces: Vec<DockEmptySpaceDropTarget>,
    pub(crate) known_viewport: Option<crate::DockViewportHit>,
    pub(crate) tear_off_candidate: Option<DockTearOffCandidateDropTarget>,
    pub(crate) clear_on_miss: bool,
}

impl DockDropTargetUpdate {
    pub(crate) fn new(position: Point<Pixels>) -> Self {
        Self {
            position,
            tab_labels: Vec::new(),
            leaves: Vec::new(),
            root: None,
            floating_title_bars: Vec::new(),
            empty_spaces: Vec::new(),
            known_viewport: None,
            tear_off_candidate: None,
            clear_on_miss: true,
        }
    }

    pub(crate) fn with_tab_label(mut self, target: DockTabLabelDropTarget) -> Self {
        self.tab_labels.push(target);
        self
    }

    pub(crate) fn with_leaf(mut self, target: DockLeafDropTarget) -> Self {
        self.leaves.push(target);
        self
    }

    pub(crate) fn preserve_on_miss(mut self) -> Self {
        self.clear_on_miss = false;
        self
    }

    fn resolve(&self, policy: &DockPolicy) -> Option<DockDropResolution> {
        drop_target::resolve_layout_drop(DockDropResolverInput {
            position: self.position,
            policy,
            tab_labels: &self.tab_labels,
            leaves: &self.leaves,
            root: self.root,
            floating_title_bars: &self.floating_title_bars,
            empty_spaces: &self.empty_spaces,
            known_viewport: self.known_viewport.clone(),
            tear_off_candidate: self.tear_off_candidate.clone(),
        })
    }
}

impl DockDropRuntime {
    pub(crate) fn update_target(
        &mut self,
        update: DockDropTargetUpdate,
        policy: &DockPolicy,
    ) -> bool {
        let mut target = match update.resolve(policy).and_then(DockDropResolution::target) {
            Some(target) => Some(target),
            None if update.clear_on_miss => None,
            None => return false,
        };
        if let Some(existing) = self.target.as_ref()
            && let Some(existing_intent) = existing.preview_intent()
            && existing_intent.insert_index.is_some()
            && existing_intent.preview_bounds.contains(&update.position)
            && target
                .as_ref()
                .and_then(DockResolvedDropTarget::preview_intent)
                .is_some_and(|intent| {
                    intent.target_tabs == existing_intent.target_tabs
                        && intent.zone == DropZone::Center
                })
        {
            target = Some(existing.clone());
        }
        self.replace_target(target)
    }

    pub(crate) fn take_resolved_target(
        &mut self,
        receiver_tabs: DockNodeId,
    ) -> Option<DockResolvedDropTarget> {
        let target = self.target.take()?;
        if target.matches_drop_receiver(receiver_tabs) {
            Some(target)
        } else {
            None
        }
    }

    pub(crate) fn preview_bounds(&self, receiver_tabs: DockNodeId) -> Option<Bounds<Pixels>> {
        let target = self.target.as_ref()?;
        if !target.matches_drop_receiver(receiver_tabs) {
            return None;
        }
        let intent = target.preview_intent()?;
        if intent.insert_index.is_some() {
            return None;
        }
        Some(intent.preview_bounds)
    }

    fn replace_target(&mut self, target: Option<DockResolvedDropTarget>) -> bool {
        if self.target == target {
            return false;
        }
        self.target = target;
        true
    }

    #[cfg(test)]
    pub(crate) fn preview_intent(&self) -> Option<DockDropPreviewIntent> {
        self.target
            .as_ref()
            .and_then(DockResolvedDropTarget::preview_intent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drop_target::DockResolvedDropTargetKind;
    use open_gpui::{point, px, size};
    use slotmap::Key;

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    #[test]
    fn tab_reorder_drop_updates_target_only_inside_tab_bounds() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();

        assert!(runtime.update_target(
            DockDropTargetUpdate::new(point(px(95.0), px(28.0))).with_tab_label(
                DockTabLabelDropTarget {
                    target_tabs: tabs,
                    target_index: 2,
                    bounds: bounds(10.0, 20.0, 100.0, 24.0),
                    is_central: false,
                },
            ),
            &DockPolicy::default()
        ));
        assert_eq!(
            runtime.preview_intent().map(|intent| intent.insert_index),
            Some(Some(3))
        );

        assert!(
            !runtime.update_target(
                DockDropTargetUpdate::new(point(px(200.0), px(28.0)))
                    .with_tab_label(DockTabLabelDropTarget {
                        target_tabs: tabs,
                        target_index: 1,
                        bounds: bounds(10.0, 20.0, 100.0, 24.0),
                        is_central: false,
                    })
                    .preserve_on_miss(),
                &DockPolicy::default()
            )
        );
        assert_eq!(
            runtime.preview_intent().map(|intent| intent.insert_index),
            Some(Some(3))
        );
    }

    #[test]
    fn tabs_drop_preserves_reorder_target_while_pointer_stays_inside_tab() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();

        assert!(runtime.update_target(
            DockDropTargetUpdate::new(point(px(95.0), px(108.0))).with_tab_label(
                DockTabLabelDropTarget {
                    target_tabs: tabs,
                    target_index: 2,
                    bounds: bounds(10.0, 100.0, 100.0, 24.0),
                    is_central: false,
                },
            ),
            &DockPolicy::default()
        ));
        assert!(!runtime.update_target(
            DockDropTargetUpdate::new(point(px(95.0), px(108.0))).with_leaf(DockLeafDropTarget {
                root: tabs,
                target_tabs: tabs,
                bounds: bounds(0.0, 0.0, 400.0, 400.0),
                is_central: false,
            }),
            &DockPolicy::default()
        ));

        let target = runtime
            .take_resolved_target(tabs)
            .expect("reorder target should remain available");
        let intent = target
            .preview_intent()
            .expect("tab drop target should project");
        assert_eq!(intent.zone, DropZone::Center);
        assert_eq!(intent.insert_index, Some(3));
        assert!(runtime.preview_intent().is_none());
    }

    #[test]
    fn reorder_target_does_not_render_drop_preview() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();

        assert!(runtime.update_target(
            DockDropTargetUpdate::new(point(px(20.0), px(28.0))).with_tab_label(
                DockTabLabelDropTarget {
                    target_tabs: tabs,
                    target_index: 0,
                    bounds: bounds(10.0, 20.0, 100.0, 24.0),
                    is_central: false,
                },
            ),
            &DockPolicy::default()
        ));

        assert_eq!(runtime.preview_bounds(tabs), None);
    }

    #[test]
    fn runtime_resolves_multi_fact_update_through_layout_resolver() {
        let tabs = DockNodeId::null();
        let mut runtime = DockDropRuntime::default();

        assert!(
            runtime.update_target(
                DockDropTargetUpdate::new(point(px(95.0), px(28.0)))
                    .with_leaf(DockLeafDropTarget {
                        root: tabs,
                        target_tabs: tabs,
                        bounds: bounds(0.0, 0.0, 400.0, 400.0),
                        is_central: false,
                    })
                    .with_tab_label(DockTabLabelDropTarget {
                        target_tabs: tabs,
                        target_index: 2,
                        bounds: bounds(10.0, 20.0, 100.0, 24.0),
                        is_central: false,
                    }),
                &DockPolicy::default()
            )
        );

        let target = runtime
            .take_resolved_target(tabs)
            .expect("multi-fact update should resolve");
        assert_eq!(
            target.kind,
            DockResolvedDropTargetKind::TabBar {
                target_tabs: tabs,
                insert_index: 3,
            }
        );
    }

    #[test]
    fn root_edge_target_can_be_taken_by_leaf_receiver() {
        let root = DockNodeId::null();
        let mut graph = crate::DockGraph::new();
        let leaf = graph.insert_node(crate::DockNode::Tabs {
            items: vec![crate::DockItemId::from("a")],
            active: 0,
        });
        let mut runtime = DockDropRuntime::default();

        assert!(runtime.update_target(
            DockDropTargetUpdate {
                root: Some(crate::drop_target::DockRootDropTarget {
                    root,
                    bounds: bounds(0.0, 0.0, 400.0, 240.0),
                }),
                leaves: vec![DockLeafDropTarget {
                    root,
                    target_tabs: leaf,
                    bounds: bounds(0.0, 20.0, 160.0, 180.0),
                    is_central: false,
                }],
                ..DockDropTargetUpdate::new(point(px(2.0), px(100.0)))
            },
            &DockPolicy::default()
        ));

        let target = runtime
            .take_resolved_target(leaf)
            .expect("root-edge target should still belong to the leaf receiver");
        assert!(matches!(
            target.kind,
            DockResolvedDropTargetKind::RootEdge {
                root: matched_root,
                leaf_tabs,
                zone: DropZone::Left,
            } if matched_root == root && leaf_tabs == leaf
        ));
    }
}
