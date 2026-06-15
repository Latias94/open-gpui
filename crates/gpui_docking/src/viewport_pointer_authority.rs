use crate::{DockViewportPlatformSignals, DockViewportTargetContext};

/// Pointer authority available for one viewport drop route request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockViewportPointerAuthority {
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

impl DockViewportPointerCoordinateSpace {
    pub(crate) fn for_hovered_host(has_global_window_bounds: bool) -> Self {
        if has_global_window_bounds {
            Self::GlobalScreen
        } else {
            Self::ReceiverLocal
        }
    }

    pub(crate) fn for_source_only(has_global_window_bounds: bool) -> Self {
        if has_global_window_bounds {
            Self::GlobalScreen
        } else {
            Self::SourceLocalOnly
        }
    }
}

impl DockViewportPointerAuthority {
    pub(crate) fn from_platform_signals(
        coordinate_space: DockViewportPointerCoordinateSpace,
        platform_signals: DockViewportPlatformSignals,
    ) -> Self {
        Self {
            coordinate_space,
            target_context: platform_signals.into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_target_context(target_context: DockViewportTargetContext) -> Self {
        Self {
            coordinate_space: DockViewportPointerCoordinateSpace::GlobalScreen,
            target_context,
        }
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
            DockViewportPointerCoordinateSpace::GlobalScreen,
            signals,
        );

        assert_eq!(
            authority.coordinate_space(),
            DockViewportPointerCoordinateSpace::GlobalScreen
        );
        assert_eq!(
            authority.target_context().hovered_window(),
            Some(handle(7).window_id())
        );
    }

    #[test]
    fn coordinate_space_helpers_preserve_release_origin_without_storing_it() {
        assert_eq!(
            DockViewportPointerCoordinateSpace::for_hovered_host(true),
            DockViewportPointerCoordinateSpace::GlobalScreen
        );
        assert_eq!(
            DockViewportPointerCoordinateSpace::for_source_only(true),
            DockViewportPointerCoordinateSpace::GlobalScreen
        );
        assert_eq!(
            DockViewportPointerCoordinateSpace::for_hovered_host(false),
            DockViewportPointerCoordinateSpace::ReceiverLocal
        );
        assert_eq!(
            DockViewportPointerCoordinateSpace::for_source_only(false),
            DockViewportPointerCoordinateSpace::SourceLocalOnly
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
            DockViewportPointerCoordinateSpace::ReceiverLocal,
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
            DockViewportPointerCoordinateSpace::SourceLocalOnly,
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
