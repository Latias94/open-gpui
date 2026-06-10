use crate::{
    DockPolicyError, DockViewportDropRoute,
    drop_runtime::resolution_target,
    drop_target::{DockDropResolution, DockResolvedDropTargetKind},
};
use open_gpui::{Bounds, Pixels, Point, point, px, size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockDropPreviewKind {
    Local,
    KnownViewportRoute,
    TearOffRoute,
    RejectedRoute,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropPreview {
    pub(crate) kind: DockDropPreviewKind,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) rejected: bool,
}

impl DockDropPreview {
    pub(crate) fn from_resolution(resolution: &DockDropResolution) -> Option<Self> {
        let target = resolution_target(resolution)?;
        let rejected = matches!(resolution, DockDropResolution::Rejected(_));
        let (kind, bounds) = match &target.kind {
            DockResolvedDropTargetKind::KnownViewport { target } => (
                DockDropPreviewKind::KnownViewportRoute,
                route_bounds(target.host_position()),
            ),
            DockResolvedDropTargetKind::TearOffCandidate {
                release_position, ..
            } => (
                DockDropPreviewKind::TearOffRoute,
                route_bounds(*release_position),
            ),
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. }
            | DockResolvedDropTargetKind::EmptyDockSpace { .. } => {
                (DockDropPreviewKind::Local, target.preview_bounds?)
            }
        };

        Some(Self {
            kind,
            bounds,
            rejected,
        })
    }

    pub(crate) fn from_viewport_route(
        route: &DockViewportDropRoute,
        host_position: Point<Pixels>,
    ) -> Option<Self> {
        let (kind, rejected) = match route {
            DockViewportDropRoute::Local { .. } => return None,
            DockViewportDropRoute::KnownViewport { .. } => {
                (DockDropPreviewKind::KnownViewportRoute, false)
            }
            DockViewportDropRoute::TearOff(_) => (DockDropPreviewKind::TearOffRoute, false),
            DockViewportDropRoute::Rejected(DockPolicyError::PlatformViewportsDisabled) => {
                (DockDropPreviewKind::RejectedRoute, true)
            }
            DockViewportDropRoute::Rejected(_) => (DockDropPreviewKind::RejectedRoute, true),
        };

        Some(Self {
            kind,
            bounds: route_bounds(host_position),
            rejected,
        })
    }

    pub(crate) fn is_route(&self) -> bool {
        matches!(
            self.kind,
            DockDropPreviewKind::KnownViewportRoute
                | DockDropPreviewKind::TearOffRoute
                | DockDropPreviewKind::RejectedRoute
        )
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
        DockNodeId, DockViewportDropPayload, DockViewportTargetHit, DockViewportTearOffRequest,
        drop_target::{DockDropResolveSource, DockResolvedDropTarget},
        viewport_test_support::{handle, item, space},
    };
    use open_gpui::{WindowBounds, point, px};
    use slotmap::Key;

    #[test]
    fn known_viewport_route_preview_uses_host_pointer_anchor() {
        let preview = DockDropPreview::from_viewport_route(
            &DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::new(
                    space("target"),
                    handle(7),
                    point(px(300.0), px(20.0)),
                ),
            },
            point(px(40.0), px(50.0)),
        )
        .expect("known viewport route should produce a preview");

        assert_eq!(preview.kind, DockDropPreviewKind::KnownViewportRoute);
        assert!(!preview.rejected);
        assert!(preview.bounds.contains(&point(px(40.0), px(50.0))));
    }

    #[test]
    fn tear_off_route_preview_is_visible_without_receiver_bounds() {
        let source_space = space("source");
        let source_tabs = DockNodeId::null();
        let release_position = point(px(900.0), px(700.0));
        let preview = DockDropPreview::from_viewport_route(
            &DockViewportDropRoute::TearOff(DockViewportTearOffRequest {
                source_space,
                source_tabs,
                payload: DockViewportDropPayload::Item(item("a")),
                release_position,
                suggested_window_bounds: None,
            }),
            point(px(100.0), px(120.0)),
        )
        .expect("tear-off route should produce a preview");

        assert_eq!(preview.kind, DockDropPreviewKind::TearOffRoute);
        assert!(!preview.rejected);
        assert!(preview.bounds.contains(&point(px(100.0), px(120.0))));
    }

    #[test]
    fn rejected_route_preview_is_marked_rejected() {
        let preview = DockDropPreview::from_viewport_route(
            &DockViewportDropRoute::Rejected(DockPolicyError::PlatformViewportsDisabled),
            point(px(12.0), px(34.0)),
        )
        .expect("rejected route should produce a preview");

        assert_eq!(preview.kind, DockDropPreviewKind::RejectedRoute);
        assert!(preview.rejected);
        assert!(preview.bounds.contains(&point(px(12.0), px(34.0))));
    }

    #[test]
    fn resolved_tear_off_candidate_preview_does_not_need_receiver_bounds() {
        let release_position = point(px(320.0), px(180.0));
        let resolution = DockDropResolution::Valid(DockResolvedDropTarget {
            kind: DockResolvedDropTargetKind::TearOffCandidate {
                release_position,
                suggested_window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    release_position,
                    size(px(360.0), px(240.0)),
                ))),
            },
            source: DockDropResolveSource::TearOffCandidate,
            preview_bounds: None,
            is_central_region: false,
        });

        let preview = DockDropPreview::from_resolution(&resolution)
            .expect("tear-off candidate should produce route preview");

        assert_eq!(preview.kind, DockDropPreviewKind::TearOffRoute);
        assert!(preview.bounds.contains(&release_position));
    }
}
