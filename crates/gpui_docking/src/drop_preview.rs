use crate::{
    DockNodeId, DockViewportDropRoute,
    drop_runtime::resolution_target,
    drop_target::{DockDropResolution, DockResolvedDropTarget, DockResolvedDropTargetKind},
};
use open_gpui::{Bounds, Pixels, Point, point, px, size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockDropRoutePreviewKind {
    KnownViewport,
    TearOff,
    Rejected,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropPreview {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) rejected: bool,
    pub(crate) payload_tab: bool,
    pub(crate) target_tabs: Option<DockNodeId>,
    pub(crate) insert_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropRoutePreview {
    pub(crate) kind: DockDropRoutePreviewKind,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) rejected: bool,
}

impl DockDropPreview {
    pub(crate) fn from_resolution(resolution: &DockDropResolution) -> Option<Self> {
        let target = resolution_target(resolution)?;
        let rejected = matches!(resolution, DockDropResolution::Rejected(_));
        Self::from_target(target, rejected)
    }

    pub(crate) fn from_resolved_target(target: &DockResolvedDropTarget) -> Option<Self> {
        Self::from_target(target, false)
    }

    pub(crate) fn from_rejected_target(target: &DockResolvedDropTarget) -> Option<Self> {
        Self::from_target(target, true)
    }

    fn from_target(target: &DockResolvedDropTarget, rejected: bool) -> Option<Self> {
        let bounds = match &target.kind {
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. }
            | DockResolvedDropTargetKind::EmptyDockSpace { .. } => target.preview_bounds?,
        };
        let payload_tab = !rejected
            && matches!(
                &target.kind,
                DockResolvedDropTargetKind::TabBar { .. }
                    | DockResolvedDropTargetKind::LeafCenter { .. }
                    | DockResolvedDropTargetKind::FloatingTitleBar { .. }
                    | DockResolvedDropTargetKind::EmptyDockSpace { .. }
            );
        let (target_tabs, insert_index) = match target.kind {
            DockResolvedDropTargetKind::TabBar {
                target_tabs,
                insert_index,
            } => (Some(target_tabs), Some(insert_index)),
            DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => {
                (Some(target_tabs), None)
            }
            DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. }
            | DockResolvedDropTargetKind::EmptyDockSpace { .. } => (None, None),
        };

        Some(Self {
            bounds,
            rejected,
            payload_tab,
            target_tabs,
            insert_index,
        })
    }
}

impl DockDropRoutePreview {
    pub(crate) fn from_route(
        route: &DockViewportDropRoute,
        host_position: Point<Pixels>,
    ) -> Option<Self> {
        let (kind, rejected) = match route {
            DockViewportDropRoute::Local { .. } => return None,
            DockViewportDropRoute::KnownViewport { .. } => {
                (DockDropRoutePreviewKind::KnownViewport, false)
            }
            DockViewportDropRoute::TearOff => (DockDropRoutePreviewKind::TearOff, false),
            DockViewportDropRoute::Unavailable => return None,
            DockViewportDropRoute::Rejected(_) => (DockDropRoutePreviewKind::Rejected, true),
        };

        Some(Self {
            kind,
            bounds: route_bounds(host_position),
            rejected,
        })
    }
}

fn route_bounds(anchor: Point<Pixels>) -> Bounds<Pixels> {
    let marker = size(px(56.0), px(40.0));
    Bounds::new(
        point(
            anchor.x - marker.width / 2.0,
            anchor.y - marker.height / 2.0,
        ),
        marker,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockPolicyError, DockViewportRouteSelectionSource, DockViewportTargetHit,
        viewport_test_support::{handle, space},
    };
    use open_gpui::{point, px};

    #[test]
    fn known_viewport_route_preview_uses_host_pointer_anchor() {
        let preview = DockDropRoutePreview::from_route(
            &DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::new(
                    space("target"),
                    handle(7),
                    point(px(300.0), px(20.0)),
                ),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            point(px(40.0), px(50.0)),
        )
        .expect("known viewport route should produce a preview");

        assert_eq!(preview.kind, DockDropRoutePreviewKind::KnownViewport);
        assert!(!preview.rejected);
        assert!(preview.bounds.contains(&point(px(40.0), px(50.0))));
    }

    #[test]
    fn tear_off_route_preview_is_visible_without_receiver_bounds() {
        let preview = DockDropRoutePreview::from_route(
            &DockViewportDropRoute::TearOff,
            point(px(100.0), px(120.0)),
        )
        .expect("tear-off route should produce a preview");

        assert_eq!(preview.kind, DockDropRoutePreviewKind::TearOff);
        assert!(!preview.rejected);
        assert!(preview.bounds.contains(&point(px(100.0), px(120.0))));
    }

    #[test]
    fn rejected_route_preview_is_marked_rejected() {
        let preview = DockDropRoutePreview::from_route(
            &DockViewportDropRoute::Rejected(DockPolicyError::PlatformViewportsDisabled),
            point(px(12.0), px(34.0)),
        )
        .expect("rejected route should produce a preview");

        assert_eq!(preview.kind, DockDropRoutePreviewKind::Rejected);
        assert!(preview.rejected);
        assert!(preview.bounds.contains(&point(px(12.0), px(34.0))));
    }

    #[test]
    fn unavailable_route_preview_is_hidden() {
        assert_eq!(
            DockDropRoutePreview::from_route(
                &DockViewportDropRoute::Unavailable,
                point(px(12.0), px(34.0)),
            ),
            None
        );
    }
}
