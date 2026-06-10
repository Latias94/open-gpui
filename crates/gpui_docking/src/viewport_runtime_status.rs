use crate::{
    DockActionApplyError, DockActionOutcome, DockItemId, DockNodeId, DockPolicyError, DockSpaceId,
    DockViewportActivationTarget, DockViewportCloseOutcome, DockViewportDropPayload,
    DockViewportDropRoute, DockViewportDropRouteOutcome, DockViewportShouldCloseOutcome,
    DockViewportTearOffCancelReason, DockViewportTearOffOpenOutcome,
};
use open_gpui::{Pixels, Point, WindowId};

/// Read-only diagnostic snapshot for the viewport runtime.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DockViewportRuntimeStatus {
    /// Most recent viewport route resolved for a rendered drop.
    pub last_route: Option<DockViewportRouteRecord>,
    /// Most recent routed drop outcome.
    pub last_drop_outcome: Option<DockViewportDropOutcomeRecord>,
    /// Most recent viewport activation requested by a routed drop.
    pub last_activation: Option<DockViewportActivationRecord>,
    /// Most recent platform close cleanup outcome.
    pub last_close: Option<DockViewportCloseOutcome>,
    /// Most recent platform should-close query outcome.
    pub last_should_close: Option<DockViewportShouldCloseOutcome>,
    /// Most recent tear-off transaction outcome.
    pub last_tear_off: Option<DockViewportTearOffRecord>,
}

/// Payload shape recorded in viewport runtime diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockViewportPayloadRecord {
    /// One tab item was routed.
    Item {
        /// Routed item id.
        item: DockItemId,
    },
    /// The entire source tabs stack was routed.
    Tabs,
}

/// Route resolution recorded before a rendered drop mutates the workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportRouteRecord {
    /// Source dock space where the payload drag started.
    pub source_space: DockSpaceId,
    /// Source tabs node where the payload drag started.
    pub source_tabs: DockNodeId,
    /// Payload being routed.
    pub payload: DockViewportPayloadRecord,
    /// Runtime route selected for the release point.
    pub target: DockViewportRouteTarget,
}

/// Runtime route selected for a rendered drop release.
#[derive(Debug, Clone, PartialEq)]
pub enum DockViewportRouteTarget {
    /// The release stayed in the source viewport.
    Local {
        /// Source dock space that should commit locally.
        space: DockSpaceId,
        /// Pointer position in source host coordinates.
        host_position: Point<Pixels>,
    },
    /// The release hit another registered viewport.
    KnownViewport {
        /// Destination dock space.
        space: DockSpaceId,
        /// Destination GPUI window id.
        window_id: WindowId,
        /// Pointer position in destination host coordinates.
        host_position: Point<Pixels>,
    },
    /// The release was outside all registered viewports and can become a platform tear-off.
    TearOff {
        /// Screen position where the payload was released.
        release_position: Point<Pixels>,
    },
    /// The release was rejected by policy before mutation.
    Rejected {
        /// Policy reason that rejected the route.
        reason: DockPolicyError,
    },
}

/// Outcome recorded after a routed drop commit attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportDropOutcomeRecord {
    /// High-level outcome kind.
    pub kind: DockViewportDropOutcomeKind,
    /// Workspace action result when one was produced.
    pub action: Option<DockActionOutcome>,
    /// Commit error when the route was rejected or failed.
    pub error: Option<DockActionApplyError>,
}

/// High-level outcome kind for a routed drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportDropOutcomeKind {
    /// The route committed through a normal workspace action.
    Action,
    /// The route completed a tear-off transaction.
    TearOffCompleted,
    /// The route matched an existing pending tear-off.
    TearOffDuplicate,
    /// The route cancelled a pending tear-off before graph mutation.
    TearOffCancelled,
    /// The route opened a viewport but failed graph mutation afterward.
    TearOffCommitFailed,
    /// The route failed before producing a viewport outcome.
    Error,
}

/// Viewport activation requested by a routed drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockViewportActivationRecord {
    /// Logical dock space that should become active.
    pub space: DockSpaceId,
    /// GPUI window id that should become active.
    pub window_id: WindowId,
    /// Panel item that should receive focus after activation, when known.
    pub focus_item: Option<DockItemId>,
}

/// Tear-off transaction outcome recorded by the viewport runtime.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportTearOffRecord {
    /// High-level tear-off outcome kind.
    pub kind: DockViewportTearOffOutcomeKind,
    /// Source dock space where the tear-off started.
    pub source_space: DockSpaceId,
    /// Target dock space opened for the tear-off.
    pub target_space: DockSpaceId,
    /// Payload that was torn off.
    pub payload: DockViewportPayloadRecord,
    /// Cancel reason when the tear-off was cancelled.
    pub cancel_reason: Option<DockViewportTearOffCancelReason>,
    /// Commit error when the viewport opened but graph mutation failed.
    pub error: Option<DockActionApplyError>,
}

/// High-level tear-off transaction outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockViewportTearOffOutcomeKind {
    /// Viewport registration and graph mutation completed.
    Completed,
    /// A duplicate request reused the existing pending tear-off.
    Duplicate,
    /// The request was cancelled before graph mutation.
    Cancelled,
    /// The viewport opened but graph mutation failed.
    CommitFailed,
}

impl DockViewportRuntimeStatus {
    pub(crate) fn record_route(
        &mut self,
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        payload: DockViewportDropPayload,
        route: &DockViewportDropRoute,
    ) {
        self.last_route = Some(DockViewportRouteRecord {
            source_space: source_space.clone(),
            source_tabs,
            payload: DockViewportPayloadRecord::from_payload(&payload),
            target: DockViewportRouteTarget::from_route(&source_space, route),
        });
    }

    pub(crate) fn record_drop_result(
        &mut self,
        result: &Result<DockViewportDropRouteOutcome, DockActionApplyError>,
    ) {
        self.last_drop_outcome = Some(DockViewportDropOutcomeRecord::from_result(result));
        if let Ok(outcome) = result {
            self.last_activation = outcome
                .activation_target()
                .as_ref()
                .map(DockViewportActivationRecord::from);
            if let DockViewportDropRouteOutcome::TearOff(tear_off) = outcome {
                self.record_tear_off(tear_off);
            }
        }
    }

    pub(crate) fn record_tear_off(&mut self, outcome: &DockViewportTearOffOpenOutcome) {
        self.last_tear_off = Some(DockViewportTearOffRecord::from_outcome(outcome));
    }

    pub(crate) fn record_close(&mut self, outcome: &DockViewportCloseOutcome) {
        self.last_close = Some(outcome.clone());
    }

    pub(crate) fn record_should_close(&mut self, outcome: &DockViewportShouldCloseOutcome) {
        self.last_should_close = Some(outcome.clone());
    }
}

impl DockViewportRouteTarget {
    fn from_route(source_space: &DockSpaceId, route: &DockViewportDropRoute) -> Self {
        match route {
            DockViewportDropRoute::Local { host_position } => Self::Local {
                space: source_space.clone(),
                host_position: *host_position,
            },
            DockViewportDropRoute::KnownViewport { hit, window } => Self::KnownViewport {
                space: hit.space.clone(),
                window_id: window.window_id(),
                host_position: hit.host_position,
            },
            DockViewportDropRoute::TearOff(request) => Self::TearOff {
                release_position: request.release_position,
            },
            DockViewportDropRoute::Rejected(reason) => Self::Rejected { reason: *reason },
        }
    }
}

impl DockViewportPayloadRecord {
    fn from_payload(payload: &DockViewportDropPayload) -> Self {
        match payload {
            DockViewportDropPayload::Item(item) => Self::Item { item: item.clone() },
            DockViewportDropPayload::Tabs => Self::Tabs,
        }
    }
}

impl DockViewportDropOutcomeRecord {
    fn from_result(result: &Result<DockViewportDropRouteOutcome, DockActionApplyError>) -> Self {
        match result {
            Ok(DockViewportDropRouteOutcome::Action(outcome)) => Self {
                kind: DockViewportDropOutcomeKind::Action,
                action: Some(outcome.action),
                error: None,
            },
            Ok(DockViewportDropRouteOutcome::TearOff(outcome)) => {
                Self::from_tear_off_outcome(outcome)
            }
            Err(error) => Self {
                kind: DockViewportDropOutcomeKind::Error,
                action: None,
                error: Some(error.clone()),
            },
        }
    }

    fn from_tear_off_outcome(outcome: &DockViewportTearOffOpenOutcome) -> Self {
        match outcome {
            DockViewportTearOffOpenOutcome::Completed(completed) => Self {
                kind: DockViewportDropOutcomeKind::TearOffCompleted,
                action: Some(completed.action),
                error: None,
            },
            DockViewportTearOffOpenOutcome::Duplicate(_pending) => Self {
                kind: DockViewportDropOutcomeKind::TearOffDuplicate,
                action: Some(DockActionOutcome::Unchanged),
                error: None,
            },
            DockViewportTearOffOpenOutcome::Cancelled(_cancelled) => Self {
                kind: DockViewportDropOutcomeKind::TearOffCancelled,
                action: Some(DockActionOutcome::Unchanged),
                error: None,
            },
            DockViewportTearOffOpenOutcome::CommitFailed(failure) => Self {
                kind: DockViewportDropOutcomeKind::TearOffCommitFailed,
                action: None,
                error: Some(failure.error.clone()),
            },
        }
    }
}

impl From<&DockViewportActivationTarget> for DockViewportActivationRecord {
    fn from(target: &DockViewportActivationTarget) -> Self {
        Self {
            space: target.space.clone(),
            window_id: target.window.window_id(),
            focus_item: target.focus_item.clone(),
        }
    }
}

impl DockViewportTearOffRecord {
    fn from_outcome(outcome: &DockViewportTearOffOpenOutcome) -> Self {
        match outcome {
            DockViewportTearOffOpenOutcome::Completed(completed) => Self {
                kind: DockViewportTearOffOutcomeKind::Completed,
                source_space: completed.pending.request.source_space.clone(),
                target_space: completed.pending.target_space.clone(),
                payload: DockViewportPayloadRecord::from_payload(
                    &completed.pending.request.payload,
                ),
                cancel_reason: None,
                error: None,
            },
            DockViewportTearOffOpenOutcome::Duplicate(pending) => Self {
                kind: DockViewportTearOffOutcomeKind::Duplicate,
                source_space: pending.request.source_space.clone(),
                target_space: pending.target_space.clone(),
                payload: DockViewportPayloadRecord::from_payload(&pending.request.payload),
                cancel_reason: None,
                error: None,
            },
            DockViewportTearOffOpenOutcome::Cancelled(cancelled) => Self {
                kind: DockViewportTearOffOutcomeKind::Cancelled,
                source_space: cancelled.pending.request.source_space.clone(),
                target_space: cancelled.pending.target_space.clone(),
                payload: DockViewportPayloadRecord::from_payload(
                    &cancelled.pending.request.payload,
                ),
                cancel_reason: Some(cancelled.reason),
                error: None,
            },
            DockViewportTearOffOpenOutcome::CommitFailed(failure) => Self {
                kind: DockViewportTearOffOutcomeKind::CommitFailed,
                source_space: failure.pending.request.source_space.clone(),
                target_space: failure.pending.target_space.clone(),
                payload: DockViewportPayloadRecord::from_payload(&failure.pending.request.payload),
                cancel_reason: None,
                error: Some(failure.error.clone()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{point, px};
    use slotmap::Key;

    #[test]
    fn route_record_derives_local_target_identity_from_source() {
        let source = DockSpaceId::from("source");
        let host_position = point(px(12.0), px(34.0));
        let mut status = DockViewportRuntimeStatus::default();

        status.record_route(
            source.clone(),
            DockNodeId::null(),
            DockViewportDropPayload::Tabs,
            &DockViewportDropRoute::Local { host_position },
        );

        let route = status
            .last_route
            .as_ref()
            .expect("route record should be captured");
        assert_eq!(route.source_space, source);
        assert!(
            matches!(
                &route.target,
                DockViewportRouteTarget::Local {
                    space,
                    host_position: recorded_position,
                } if space == &source && *recorded_position == host_position
            ),
            "local route target should be the recorded source, got {:?}",
            route.target
        );
    }
}
