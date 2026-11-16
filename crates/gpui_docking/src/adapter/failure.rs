//! Diagnostic copies never retain an affine output, effect result or native owner.

use std::error::Error;

use dockspace::runtime::{
    DockspaceRuntimeError, DockspaceRuntimeErrorKind, NativeHostErrorKind,
    SurfacePresentationReportError,
};

use super::native::NativeBridgeError;

/// An owned diagnostic of a background host failure.
///
/// This observation does not acknowledge queued input, settle outputs or authorize
/// replay. Capacity and operation conflicts require their original owner's retry
/// path. A permanently rejected prefix can prevent semantic drain even after
/// native windows close. Retain that complete failed generation until every old
/// backend, callback, output and asynchronous owner terminates; an empty window
/// inventory alone permits neither disposal nor reuse of the old session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockHostFailure {
    /// Original diagnostic, including error context supplied by the adapter.
    pub message: String,
    /// Core's stable failure category, when present in the original source chain.
    /// Absence means unclassified, not success or permission to retry.
    pub runtime_kind: Option<DockspaceRuntimeErrorKind>,
    /// Core's native-host category, when the failure belongs to that lane.
    pub native_kind: Option<NativeHostErrorKind>,
}

/// Terminal disposition of one native workspace generation.
///
/// Both variants require actual backend, renderer and callback termination.
/// Only `Drained` also proves successful semantic cleanup. A failed session must
/// be discarded as a whole and replaced with fresh session/provider authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DockNativeTermination {
    /// The original native protocol and all of its physical owners drained.
    Drained,
    /// Physical owners terminated while the original failed protocol is retained.
    Failed(DockHostFailure),
}

impl DockHostFailure {
    pub(super) fn from_error(error: &(dyn Error + 'static)) -> Self {
        let mut failure = Self {
            message: std::iter::successors(Some(error), |source| (*source).source())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(": "),
            runtime_kind: None,
            native_kind: None,
        };
        let mut source = Some(error);
        while let Some(error) = source {
            if let Some(error) = error.downcast_ref::<DockspaceRuntimeError>() {
                failure.runtime_kind = Some(error.kind());
                failure.native_kind = error.native_kind();
                break;
            }
            if let Some(error) = error.downcast_ref::<NativeBridgeError>() {
                match error {
                    NativeBridgeError::Core(error) => {
                        failure.runtime_kind = Some(error.kind());
                        failure.native_kind = error.native_kind();
                        break;
                    }
                    NativeBridgeError::EffectReport(kind) => {
                        failure.runtime_kind = Some(kind.runtime_error_kind());
                        failure.native_kind = Some(*kind);
                        break;
                    }
                    _ => {}
                }
            }
            if error.is::<SurfacePresentationReportError>() {
                failure.runtime_kind = Some(DockspaceRuntimeErrorKind::HostProtocol);
                break;
            }
            source = error.source();
        }
        failure
    }

    pub(super) fn is_primary_protocol_failure(&self) -> bool {
        matches!(
            self.runtime_kind,
            Some(
                DockspaceRuntimeErrorKind::HostProtocol
                    | DockspaceRuntimeErrorKind::Internal
                    | DockspaceRuntimeErrorKind::ForeignUpdate
            )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contextual_capacity_failure_keeps_its_retry_category() {
        let error = anyhow::Error::new(NativeBridgeError::EffectReport(
            NativeHostErrorKind::InputCapacity,
        ))
        .context("collecting an original native result");
        let failure = DockHostFailure::from_error(error.as_ref());
        assert_eq!(
            failure.runtime_kind,
            Some(DockspaceRuntimeErrorKind::OperationConflict)
        );
        assert_eq!(
            failure.native_kind,
            Some(NativeHostErrorKind::InputCapacity)
        );
        assert!(!failure.is_primary_protocol_failure());
        assert!(
            failure
                .message
                .contains("collecting an original native result")
        );
    }
}
