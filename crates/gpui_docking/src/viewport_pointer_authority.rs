use crate::{
    DockViewportPlatformSignals, DockViewportTargetContext,
    interaction::DockPayloadDropReleaseOrigin,
};

/// Pointer authority available for one viewport drop route request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportPointerAuthority {
    origin: DockPayloadDropReleaseOrigin,
    coordinate_space: DockViewportPointerCoordinateSpace,
    target_context: DockViewportTargetContext,
}

/// Coordinate space used by `DockViewportDropRouteRequest::release_position`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockViewportPointerCoordinateSpace {
    /// `release_position` is a screen-space point and may be geometry hit-tested globally.
    GlobalScreen,
    /// `release_position` is local to the event-receiver window.
    ReceiverLocal,
    /// `release_position` is local to the source host only.
    SourceLocalOnly,
}

impl DockViewportPointerAuthority {
    pub(crate) fn from_platform_signals(
        origin: DockPayloadDropReleaseOrigin,
        platform_signals: DockViewportPlatformSignals,
    ) -> Self {
        let coordinate_space = if platform_signals.has_global_window_bounds() {
            DockViewportPointerCoordinateSpace::GlobalScreen
        } else {
            match origin {
                DockPayloadDropReleaseOrigin::HoveredHost => {
                    DockViewportPointerCoordinateSpace::ReceiverLocal
                }
                DockPayloadDropReleaseOrigin::SourceOnly => {
                    DockViewportPointerCoordinateSpace::SourceLocalOnly
                }
            }
        };
        Self {
            origin,
            coordinate_space,
            target_context: platform_signals.into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_target_context(target_context: DockViewportTargetContext) -> Self {
        Self {
            origin: DockPayloadDropReleaseOrigin::HoveredHost,
            coordinate_space: DockViewportPointerCoordinateSpace::GlobalScreen,
            target_context,
        }
    }

    #[cfg(test)]
    pub(crate) fn origin(&self) -> DockPayloadDropReleaseOrigin {
        self.origin
    }

    pub(crate) fn coordinate_space(&self) -> DockViewportPointerCoordinateSpace {
        self.coordinate_space
    }

    pub(crate) fn target_context(&self) -> &DockViewportTargetContext {
        &self.target_context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport_test_support::handle;

    #[test]
    fn pointer_authority_keeps_global_screen_coordinates_when_backend_supports_global_bounds() {
        let signals = DockViewportPlatformSignals::default()
            .with_hovered_window(handle(7))
            .with_global_window_bounds(true);

        let authority = DockViewportPointerAuthority::from_platform_signals(
            DockPayloadDropReleaseOrigin::SourceOnly,
            signals,
        );

        assert_eq!(
            authority.coordinate_space(),
            DockViewportPointerCoordinateSpace::GlobalScreen
        );
        assert_eq!(authority.origin(), DockPayloadDropReleaseOrigin::SourceOnly);
        assert_eq!(
            authority.target_context().hovered_window(),
            Some(handle(7).window_id())
        );
    }

    #[test]
    fn pointer_authority_splits_receiver_local_from_source_local_without_global_bounds() {
        let hovered = handle(7);
        let receiver = handle(8);
        let signals = DockViewportPlatformSignals::from_target_context(
            DockViewportTargetContext::new()
                .with_hovered_window(hovered)
                .with_event_receiver_window(receiver),
        )
        .with_global_window_bounds(false);

        let receiver_authority = DockViewportPointerAuthority::from_platform_signals(
            DockPayloadDropReleaseOrigin::HoveredHost,
            signals.clone(),
        );
        assert_eq!(
            receiver_authority.coordinate_space(),
            DockViewportPointerCoordinateSpace::ReceiverLocal
        );
        assert_eq!(
            receiver_authority.target_context().event_receiver_window(),
            Some(receiver.window_id())
        );

        let source_authority = DockViewportPointerAuthority::from_platform_signals(
            DockPayloadDropReleaseOrigin::SourceOnly,
            signals,
        );
        assert_eq!(
            source_authority.coordinate_space(),
            DockViewportPointerCoordinateSpace::SourceLocalOnly
        );
        assert_eq!(
            source_authority.target_context().event_receiver_window(),
            Some(receiver.window_id()),
            "source-only keeps backend diagnostics but must not inherit receiver-local coordinates"
        );
    }
}
