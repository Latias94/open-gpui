use crate::{
    DockViewportPlatformSyncAction, DockViewportPlatformSyncDispatch,
    DockViewportPlatformSyncDomain, DockViewportPlatformSyncRecord,
    DockViewportPlatformSyncRejected, DockViewportPlatformSyncRejectedReason,
    DockViewportPlatformSyncRequest, DockViewportPlatformSyncUnsupported,
    DockViewportPlatformSyncUnsupportedReason, DockViewportRuntime,
    viewport_registry::DockViewportPlatformRequests,
};
use open_gpui::{
    Window, WindowBackgroundAppearance, WindowBounds, WindowId, WindowKind, WindowMutationDispatch,
    WindowMutationRequest, WindowMutationSupport, WindowMutationTicket, WindowPlacementRequest,
    WindowPlacementState, WindowPlatformFacts,
};

fn default_window_background() -> WindowBackgroundAppearance {
    WindowBackgroundAppearance::Opaque
}

fn unsupported(request: DockViewportPlatformSyncRequest) -> DockViewportPlatformSyncUnsupported {
    DockViewportPlatformSyncUnsupported {
        request,
        reason: DockViewportPlatformSyncUnsupportedReason::UnsupportedByWindowApi,
    }
}

fn creation_only(request: DockViewportPlatformSyncRequest) -> DockViewportPlatformSyncUnsupported {
    DockViewportPlatformSyncUnsupported {
        request,
        reason: DockViewportPlatformSyncUnsupportedReason::CreationOnly,
    }
}

/// Internal result of one Dock live-window dispatch pass.
///
/// The public record deliberately retains only ticket metadata. Tickets stay internal so callers
/// cannot mistake a queued request for committed window facts.
#[derive(Debug)]
pub(crate) struct DockViewportPlatformSyncDispatchResult {
    record: DockViewportPlatformSyncRecord,
    tickets: Vec<WindowMutationTicket>,
}

impl DockViewportPlatformSyncDispatchResult {
    fn new(window_id: WindowId) -> Self {
        Self {
            record: DockViewportPlatformSyncRecord {
                window_id,
                dispatches: Vec::new(),
                observations: Vec::new(),
            },
            tickets: Vec::new(),
        }
    }

    pub(crate) fn record(&self) -> &DockViewportPlatformSyncRecord {
        &self.record
    }

    pub(crate) fn into_parts(self) -> (DockViewportPlatformSyncRecord, Vec<WindowMutationTicket>) {
        (self.record, self.tickets)
    }

    fn push_immediate(&mut self, action: DockViewportPlatformSyncAction) {
        self.record
            .dispatches
            .push(DockViewportPlatformSyncDispatch::Immediate { action });
    }

    fn push_unsupported(&mut self, unsupported: DockViewportPlatformSyncUnsupported) {
        self.record
            .dispatches
            .push(DockViewportPlatformSyncDispatch::Unsupported(unsupported));
    }

    fn push_rejected(
        &mut self,
        request: DockViewportPlatformSyncRequest,
        reason: DockViewportPlatformSyncRejectedReason,
    ) {
        self.record
            .dispatches
            .push(DockViewportPlatformSyncDispatch::Rejected(
                DockViewportPlatformSyncRejected { request, reason },
            ));
    }

    fn push_window_dispatch(
        &mut self,
        request: DockViewportPlatformSyncRequest,
        dispatch: WindowMutationDispatch,
        unsupported_reason: DockViewportPlatformSyncUnsupportedReason,
    ) {
        match dispatch {
            WindowMutationDispatch::Queued(ticket) => {
                self.record
                    .dispatches
                    .push(DockViewportPlatformSyncDispatch::Queued {
                        request,
                        domain: DockViewportPlatformSyncDomain::from(ticket.domain()),
                        generation: ticket.generation(),
                    });
                self.tickets.push(ticket);
            }
            WindowMutationDispatch::Unchanged => {
                self.record
                    .dispatches
                    .push(DockViewportPlatformSyncDispatch::Unchanged { request });
            }
            WindowMutationDispatch::Unsupported => {
                self.push_unsupported(DockViewportPlatformSyncUnsupported {
                    request,
                    reason: unsupported_reason,
                });
            }
            WindowMutationDispatch::Rejected => {
                self.push_rejected(
                    request,
                    DockViewportPlatformSyncRejectedReason::RejectedByWindowApi,
                );
            }
            WindowMutationDispatch::WindowClosed => {
                self.record
                    .dispatches
                    .push(DockViewportPlatformSyncDispatch::WindowClosed { request });
            }
        }
    }
}

pub(crate) fn unavailable_reused_viewport_window_sync(
    window_id: WindowId,
) -> DockViewportPlatformSyncRecord {
    let mut result = DockViewportPlatformSyncDispatchResult::new(window_id);
    result
        .record
        .dispatches
        .push(DockViewportPlatformSyncDispatch::WindowClosed {
            request: DockViewportPlatformSyncRequest::WindowUnavailable,
        });
    result.record
}

fn dispatch_pointer_input(
    window: &mut Window,
    accepts_pointer_input: bool,
) -> DockViewportPlatformSyncDispatchResult {
    let window_id = window.window_handle().window_id();
    let support = window.window_capabilities().mutations.pointer_input;
    let mut result = DockViewportPlatformSyncDispatchResult::new(window_id);
    let unsupported_reason = match support {
        WindowMutationSupport::CreationOnly => {
            DockViewportPlatformSyncUnsupportedReason::CreationOnly
        }
        WindowMutationSupport::Unsupported | WindowMutationSupport::Live => {
            DockViewportPlatformSyncUnsupportedReason::UnsupportedByWindowApi
        }
    };
    result.push_window_dispatch(
        DockViewportPlatformSyncRequest::PointerInput {
            requested: accepts_pointer_input,
        },
        window.request_pointer_input(accepts_pointer_input),
        unsupported_reason,
    );
    result
}

fn dispatch_background_appearance(
    window: &mut Window,
    background: WindowBackgroundAppearance,
) -> DockViewportPlatformSyncDispatchResult {
    let window_id = window.window_handle().window_id();
    let support = window.window_capabilities().mutations.alpha;
    let mut result = DockViewportPlatformSyncDispatchResult::new(window_id);
    let unsupported_reason = match support {
        WindowMutationSupport::CreationOnly => {
            DockViewportPlatformSyncUnsupportedReason::CreationOnly
        }
        WindowMutationSupport::Unsupported | WindowMutationSupport::Live => {
            DockViewportPlatformSyncUnsupportedReason::UnsupportedByWindowApi
        }
    };
    result.push_window_dispatch(
        DockViewportPlatformSyncRequest::BackgroundAppearance {
            requested: background,
        },
        window.set_background_appearance(background),
        unsupported_reason,
    );
    result
}

fn placement_unsupported_reason(
    window: &Window,
    requested: WindowBounds,
) -> DockViewportPlatformSyncUnsupportedReason {
    let request = WindowPlacementRequest::from_window_bounds(requested);
    let facts = window.platform_facts();
    let capabilities = window.window_capabilities().mutations;
    let mut required_support = Vec::with_capacity(4);

    if request
        .position
        .is_some_and(|position| position != facts.bounds.origin)
    {
        required_support.push(capabilities.position);
    }
    if request.size.is_some_and(|size| size != facts.bounds.size) {
        required_support.push(capabilities.size);
    }
    if request
        .restore_bounds
        .is_some_and(|restore_bounds| restore_bounds != facts.window_bounds.get_bounds())
    {
        required_support.push(capabilities.restore_bounds);
    }

    let current_state = if facts.is_minimized {
        WindowPlacementState::Minimized
    } else if facts.is_fullscreen {
        WindowPlacementState::Fullscreen
    } else if facts.is_maximized {
        WindowPlacementState::Maximized
    } else {
        WindowPlacementState::Windowed
    };
    if let Some(target_state) = request.state
        && target_state != current_state
    {
        required_support.push(match target_state {
            WindowPlacementState::Windowed => capabilities.windowed,
            WindowPlacementState::Maximized => capabilities.maximized,
            WindowPlacementState::Fullscreen => capabilities.fullscreen,
            WindowPlacementState::Minimized => capabilities.minimized,
        });
    }

    if required_support
        .iter()
        .any(|support| matches!(support, WindowMutationSupport::Unsupported))
    {
        DockViewportPlatformSyncUnsupportedReason::UnsupportedByWindowApi
    } else if required_support
        .iter()
        .any(|support| matches!(support, WindowMutationSupport::CreationOnly))
    {
        DockViewportPlatformSyncUnsupportedReason::CreationOnly
    } else {
        DockViewportPlatformSyncUnsupportedReason::UnsupportedByWindowApi
    }
}

pub(crate) fn sync_pointer_input_window(
    window: &mut Window,
    accepts_pointer_input: bool,
) -> DockViewportPlatformSyncDispatchResult {
    dispatch_pointer_input(window, accepts_pointer_input)
}

pub(crate) struct DockViewportRenderPointerInputResolution {
    pub(crate) target: Option<bool>,
    pub(crate) request: Option<bool>,
}

pub(crate) fn resolve_render_passthrough_pointer_input_request(
    runtime: &mut DockViewportRuntime,
    window_id: WindowId,
    accepts_pointer_input: bool,
    pending_pointer_input: Option<bool>,
    passthrough: bool,
) -> DockViewportRenderPointerInputResolution {
    if passthrough {
        runtime.record_render_passthrough_pointer_input(window_id);
        if !accepts_pointer_input && pending_pointer_input != Some(true) {
            return DockViewportRenderPointerInputResolution {
                target: Some(false),
                request: None,
            };
        }
        return DockViewportRenderPointerInputResolution {
            target: Some(false),
            request: Some(false),
        };
    }

    if !runtime.take_render_passthrough_pointer_input(window_id) {
        return DockViewportRenderPointerInputResolution {
            target: None,
            request: None,
        };
    }
    DockViewportRenderPointerInputResolution {
        target: Some(true),
        request: (!accepts_pointer_input || pending_pointer_input == Some(false)).then_some(true),
    }
}

/// Dispatches supported `WindowOptions` intent to an existing viewport window.
///
/// Placement and pointer input use typed GPUI live mutation APIs. A queued request is never
/// treated as an observed Dock fact; window facts are updated only by the committed-facts path.
#[cfg(test)]
pub(crate) fn sync_reused_viewport_window(
    window: &mut Window,
    existing_kind: &WindowKind,
    options: open_gpui::WindowOptions,
    platform_requests: DockViewportPlatformRequests,
) -> DockViewportPlatformSyncDispatchResult {
    sync_reused_viewport_window_with_request_gate(
        window,
        existing_kind,
        options,
        platform_requests,
        |_, _| true,
    )
}

pub(crate) fn sync_reused_viewport_window_with_request_gate(
    window: &mut Window,
    existing_kind: &WindowKind,
    options: open_gpui::WindowOptions,
    platform_requests: DockViewportPlatformRequests,
    mut should_dispatch: impl FnMut(WindowMutationRequest, &WindowPlatformFacts) -> bool,
) -> DockViewportPlatformSyncDispatchResult {
    let window_id = window.window_handle().window_id();
    let mut result = DockViewportPlatformSyncDispatchResult::new(window_id);

    if !options.show {
        result.push_unsupported(creation_only(DockViewportPlatformSyncRequest::Show {
            requested: options.show,
        }));
    }

    if &options.kind != existing_kind {
        result.push_unsupported(creation_only(DockViewportPlatformSyncRequest::WindowKind));
    }
    if !options.is_movable {
        result.push_unsupported(creation_only(DockViewportPlatformSyncRequest::Movable {
            requested: options.is_movable,
        }));
    }
    if !options.is_resizable {
        result.push_unsupported(creation_only(DockViewportPlatformSyncRequest::Resizable {
            requested: options.is_resizable,
        }));
    }
    if !options.is_minimizable {
        result.push_unsupported(creation_only(
            DockViewportPlatformSyncRequest::Minimizable {
                requested: options.is_minimizable,
            },
        ));
    }

    let activation_request = WindowMutationRequest::ActivationPolicy(options.activation_policy);
    if should_dispatch(activation_request, window.platform_facts()) {
        let support = window.window_capabilities().mutations.activation_policy;
        let unsupported_reason = match support {
            WindowMutationSupport::CreationOnly => {
                DockViewportPlatformSyncUnsupportedReason::CreationOnly
            }
            WindowMutationSupport::Unsupported | WindowMutationSupport::Live => {
                DockViewportPlatformSyncUnsupportedReason::UnsupportedByWindowApi
            }
        };
        result.push_window_dispatch(
            DockViewportPlatformSyncRequest::ActivationPolicy {
                requested: options.activation_policy,
            },
            window.request_activation_policy(options.activation_policy),
            unsupported_reason,
        );
    }

    let pointer_request = WindowMutationRequest::PointerInput(options.accepts_pointer_input);
    if should_dispatch(pointer_request, window.platform_facts()) {
        let pointer_result = dispatch_pointer_input(window, options.accepts_pointer_input);
        let (record, tickets) = pointer_result.into_parts();
        result.record.dispatches.extend(record.dispatches);
        result.tickets.extend(tickets);
    }

    if let Some(display_id) = options.display_id {
        result.push_unsupported(creation_only(DockViewportPlatformSyncRequest::Display {
            requested: display_id,
        }));
    }
    if let Some(size) = options.window_min_size {
        result.push_unsupported(creation_only(
            DockViewportPlatformSyncRequest::WindowMinSize { requested: size },
        ));
    }
    if options.icon.is_some() {
        result.push_unsupported(creation_only(DockViewportPlatformSyncRequest::Icon));
    }
    if let Some(app_id) = options.app_id {
        window.set_app_id(&app_id);
        result.push_immediate(DockViewportPlatformSyncAction::AppId { app_id });
    }
    if let Some(tabbing_identifier) = options.tabbing_identifier {
        result.push_unsupported(creation_only(
            DockViewportPlatformSyncRequest::TabbingIdentifier {
                requested: tabbing_identifier,
            },
        ));
    }

    if let Some(decorations) = options.window_decorations {
        window.request_decorations(decorations);
        result.push_immediate(DockViewportPlatformSyncAction::WindowDecorations { decorations });
    }

    if options.window_background != default_window_background() {
        let alpha_request = WindowMutationRequest::Alpha(options.window_background);
        if should_dispatch(alpha_request, window.platform_facts()) {
            let background_result =
                dispatch_background_appearance(window, options.window_background);
            let (record, tickets) = background_result.into_parts();
            result.record.dispatches.extend(record.dispatches);
            result.tickets.extend(tickets);
        }
    }

    match options.titlebar {
        Some(titlebar) => {
            if let Some(title) = titlebar.title {
                window.set_window_title(title.as_ref());
                result.push_immediate(DockViewportPlatformSyncAction::Title {
                    title: title.to_string(),
                });
            }
            if titlebar.appears_transparent {
                result.push_unsupported(creation_only(
                    DockViewportPlatformSyncRequest::TitlebarTransparency {
                        requested: titlebar.appears_transparent,
                    },
                ));
            }
            if let Some(position) = titlebar.traffic_light_position {
                #[cfg(target_os = "macos")]
                {
                    window.set_traffic_light_position(position);
                    result.push_immediate(DockViewportPlatformSyncAction::TrafficLightPosition {
                        position,
                    });
                }
                #[cfg(not(target_os = "macos"))]
                {
                    result.push_unsupported(unsupported(
                        DockViewportPlatformSyncRequest::TrafficLightPosition {
                            requested: position,
                        },
                    ));
                }
            }
        }
        None => {
            result.push_unsupported(creation_only(
                DockViewportPlatformSyncRequest::TitlebarPresence { requested: false },
            ));
        }
    }

    if let Some(window_bounds) = options.window_bounds {
        let request = DockViewportPlatformSyncRequest::Placement {
            requested: window_bounds,
        };
        let placement_request = WindowMutationRequest::Placement(
            WindowPlacementRequest::from_window_bounds(window_bounds),
        );
        if should_dispatch(placement_request, window.platform_facts()) {
            if platform_requests.resize_requested {
                result.push_rejected(
                    request,
                    DockViewportPlatformSyncRejectedReason::PlatformRequestInProgress,
                );
            } else {
                let unsupported_reason = placement_unsupported_reason(window, window_bounds);
                result.push_window_dispatch(
                    request,
                    window.request_window_placement(window_bounds),
                    unsupported_reason,
                );
            }
        }
    }

    result
}
