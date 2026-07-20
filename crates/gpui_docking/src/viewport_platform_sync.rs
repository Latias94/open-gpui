use crate::{
    DockViewportPlatformSyncAction, DockViewportPlatformSyncRecord,
    DockViewportPlatformSyncRequest, DockViewportPlatformSyncSkipped,
    DockViewportPlatformSyncSkippedReason, DockViewportPlatformSyncUnsupported,
    DockViewportPlatformSyncUnsupportedReason, DockViewportPlatformWindowState,
    DockViewportRuntime, viewport_registry::DockViewportPlatformRequests,
};
use open_gpui::WindowId;
use open_gpui::{
    PlatformViewportCapabilities, PlatformViewportFlagCapabilities, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind,
};

fn default_window_kind() -> WindowKind {
    WindowKind::Normal
}

fn default_window_background() -> WindowBackgroundAppearance {
    WindowBackgroundAppearance::Opaque
}

fn unsupported(request: DockViewportPlatformSyncRequest) -> DockViewportPlatformSyncUnsupported {
    DockViewportPlatformSyncUnsupported {
        request,
        reason: DockViewportPlatformSyncUnsupportedReason::UnsupportedByWindowApi,
    }
}

pub(crate) fn unavailable_reused_viewport_window_sync(
    window_id: WindowId,
) -> DockViewportPlatformSyncRecord {
    DockViewportPlatformSyncRecord {
        window_id,
        applied: Vec::new(),
        skipped_requests: Vec::new(),
        unsupported_requests: vec![DockViewportPlatformSyncUnsupported {
            request: DockViewportPlatformSyncRequest::WindowUnavailable,
            reason: DockViewportPlatformSyncUnsupportedReason::WindowUnavailable,
        }],
    }
}

fn unsupported_pointer_input_sync(
    window_id: open_gpui::WindowId,
    accepts_pointer_input: bool,
) -> DockViewportPlatformSyncRecord {
    let no_inputs_requested = !accepts_pointer_input;
    DockViewportPlatformSyncRecord {
        window_id,
        applied: Vec::new(),
        skipped_requests: Vec::new(),
        unsupported_requests: vec![
            unsupported(DockViewportPlatformSyncRequest::PointerInput {
                requested: accepts_pointer_input,
            }),
            unsupported(DockViewportPlatformSyncRequest::ViewportFlagNoInputs {
                requested: no_inputs_requested,
            }),
        ],
    }
}

pub(crate) fn sync_pointer_input_window(
    window: &mut Window,
    accepts_pointer_input: bool,
    capabilities: PlatformViewportCapabilities,
) -> DockViewportPlatformSyncRecord {
    let window_id = window.window_handle().window_id();
    if window.accepts_pointer_input() == accepts_pointer_input {
        return DockViewportPlatformSyncRecord {
            window_id,
            applied: Vec::new(),
            skipped_requests: Vec::new(),
            unsupported_requests: Vec::new(),
        };
    }
    if capabilities.no_input_windows && window.set_accepts_pointer_input(accepts_pointer_input) {
        DockViewportPlatformSyncRecord {
            window_id,
            applied: vec![
                DockViewportPlatformSyncAction::PointerInput {
                    enabled: accepts_pointer_input,
                },
                DockViewportPlatformSyncAction::ViewportFlagNoInputs {
                    enabled: !accepts_pointer_input,
                },
            ],
            skipped_requests: Vec::new(),
            unsupported_requests: Vec::new(),
        }
    } else {
        unsupported_pointer_input_sync(window_id, accepts_pointer_input)
    }
}

pub(crate) fn sync_render_passthrough_pointer_input(
    runtime: &mut DockViewportRuntime,
    window: &mut Window,
    passthrough: bool,
    capabilities: PlatformViewportCapabilities,
) -> bool {
    let window_id = window.window_handle().window_id();
    if passthrough {
        if !window.accepts_pointer_input() {
            return false;
        }
        runtime.record_render_passthrough_pointer_input(window_id);
        return apply_render_pointer_input_sync(runtime, window, false, capabilities);
    }

    if !runtime.take_render_passthrough_pointer_input(window_id) {
        return false;
    }
    if window.accepts_pointer_input() {
        return false;
    }
    apply_render_pointer_input_sync(runtime, window, true, capabilities)
}

fn apply_render_pointer_input_sync(
    runtime: &mut DockViewportRuntime,
    window: &mut Window,
    accepts_pointer_input: bool,
    capabilities: PlatformViewportCapabilities,
) -> bool {
    let window_id = window.window_handle().window_id();
    if runtime
        .runtime_status()
        .last_platform_sync_is_unsupported_pointer_input(window_id, accepts_pointer_input)
    {
        return false;
    }
    let sync_record = sync_pointer_input_window(window, accepts_pointer_input, capabilities);
    let applied = !sync_record.applied.is_empty();
    runtime.record_platform_sync(sync_record);
    applied
}

fn skipped_for_platform_request(
    request: DockViewportPlatformSyncRequest,
) -> DockViewportPlatformSyncSkipped {
    DockViewportPlatformSyncSkipped {
        request,
        reason: DockViewportPlatformSyncSkippedReason::PlatformRequestInProgress,
    }
}

/// Explicit ImGui-style viewport flag requests owned by docking.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct DockViewportPlatformFlagRequests {
    no_inputs: Option<bool>,
    no_focus_on_appearing: bool,
    no_focus_on_click: bool,
    alpha: Option<f32>,
    topmost: bool,
    no_taskbar: bool,
}

impl DockViewportPlatformFlagRequests {
    pub(crate) fn from_reused_window_options(options: &open_gpui::WindowOptions) -> Self {
        Self::default().with_no_inputs(!options.accepts_pointer_input)
    }

    pub(crate) fn with_no_inputs(mut self, requested: bool) -> Self {
        self.no_inputs = Some(requested);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_no_focus_on_appearing(mut self, requested: bool) -> Self {
        self.no_focus_on_appearing = requested;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_no_focus_on_click(mut self, requested: bool) -> Self {
        self.no_focus_on_click = requested;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_alpha(mut self, requested: Option<f32>) -> Self {
        self.alpha = requested;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_topmost(mut self, requested: bool) -> Self {
        self.topmost = requested;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_no_taskbar(mut self, requested: bool) -> Self {
        self.no_taskbar = requested;
        self
    }
}

#[cfg(test)]
pub(crate) fn unsupported_viewport_platform_flag_requests(
    requests: DockViewportPlatformFlagRequests,
    capabilities: PlatformViewportFlagCapabilities,
) -> Vec<DockViewportPlatformSyncUnsupported> {
    let mut unsupported_requests = Vec::new();
    push_unsupported_viewport_flag_requests(&mut unsupported_requests, requests, capabilities);
    unsupported_requests
}

fn push_unsupported_viewport_flag_requests(
    unsupported_requests: &mut Vec<DockViewportPlatformSyncUnsupported>,
    requests: DockViewportPlatformFlagRequests,
    capabilities: PlatformViewportFlagCapabilities,
) {
    if requests.no_focus_on_appearing && !capabilities.no_focus_on_appearing_windows {
        unsupported_requests.push(unsupported(
            DockViewportPlatformSyncRequest::ViewportFlagNoFocusOnAppearing { requested: true },
        ));
    }
    if let Some(alpha) = requests.alpha
        && !capabilities.alpha_windows
    {
        unsupported_requests.push(unsupported(
            DockViewportPlatformSyncRequest::ViewportFlagAlpha { requested: alpha },
        ));
    }
    if requests.topmost && !capabilities.topmost_windows {
        unsupported_requests.push(unsupported(
            DockViewportPlatformSyncRequest::ViewportFlagTopMost { requested: true },
        ));
    }
    if requests.no_taskbar && !capabilities.no_taskbar_windows {
        unsupported_requests.push(unsupported(
            DockViewportPlatformSyncRequest::ViewportFlagNoTaskbar { requested: true },
        ));
    }
    if requests.no_focus_on_click && !capabilities.no_focus_on_click_windows {
        unsupported_requests.push(unsupported(
            DockViewportPlatformSyncRequest::ViewportFlagNoFocusOnClick { requested: true },
        ));
    }
}

fn sync_viewport_platform_flag_requests(
    window: &mut Window,
    requests: DockViewportPlatformFlagRequests,
    viewport_capabilities: PlatformViewportCapabilities,
    flag_capabilities: PlatformViewportFlagCapabilities,
    applied: &mut Vec<DockViewportPlatformSyncAction>,
    unsupported_requests: &mut Vec<DockViewportPlatformSyncUnsupported>,
) {
    if let Some(no_inputs_requested) = requests.no_inputs {
        let requested_accepts_pointer_input = !no_inputs_requested;
        if requested_accepts_pointer_input != window.accepts_pointer_input() {
            if viewport_capabilities.no_input_windows
                && window.set_accepts_pointer_input(requested_accepts_pointer_input)
            {
                applied.push(DockViewportPlatformSyncAction::PointerInput {
                    enabled: requested_accepts_pointer_input,
                });
                applied.push(DockViewportPlatformSyncAction::ViewportFlagNoInputs {
                    enabled: no_inputs_requested,
                });
            } else {
                unsupported_requests.push(unsupported(
                    DockViewportPlatformSyncRequest::PointerInput {
                        requested: requested_accepts_pointer_input,
                    },
                ));
                unsupported_requests.push(unsupported(
                    DockViewportPlatformSyncRequest::ViewportFlagNoInputs {
                        requested: no_inputs_requested,
                    },
                ));
            }
        }
    }
    push_unsupported_viewport_flag_requests(unsupported_requests, requests, flag_capabilities);
}

/// Applies the subset of `WindowOptions` that GPUI exposes as live window mutations.
pub(crate) fn sync_reused_viewport_window(
    window: &mut Window,
    options: open_gpui::WindowOptions,
    platform_requests: DockViewportPlatformRequests,
    viewport_capabilities: PlatformViewportCapabilities,
    flag_capabilities: PlatformViewportFlagCapabilities,
) -> DockViewportPlatformSyncRecord {
    let window_id = window.window_handle().window_id();
    let mut applied = Vec::new();
    let mut skipped_requests = Vec::new();
    let mut unsupported_requests = Vec::new();
    let viewport_flag_requests =
        DockViewportPlatformFlagRequests::from_reused_window_options(&options);

    if options.focus {
        window.activate_window();
        applied.push(DockViewportPlatformSyncAction::Activate);
    }

    if !options.show {
        unsupported_requests.push(unsupported(DockViewportPlatformSyncRequest::Show {
            requested: options.show,
        }));
    }

    if options.kind != default_window_kind() {
        unsupported_requests.push(unsupported(DockViewportPlatformSyncRequest::WindowKind));
    }
    if !options.is_movable {
        unsupported_requests.push(unsupported(DockViewportPlatformSyncRequest::Movable {
            requested: options.is_movable,
        }));
    }
    if !options.is_resizable {
        unsupported_requests.push(unsupported(DockViewportPlatformSyncRequest::Resizable {
            requested: options.is_resizable,
        }));
    }
    if !options.is_minimizable {
        unsupported_requests.push(unsupported(DockViewportPlatformSyncRequest::Minimizable {
            requested: options.is_minimizable,
        }));
    }
    sync_viewport_platform_flag_requests(
        window,
        viewport_flag_requests,
        viewport_capabilities,
        flag_capabilities,
        &mut applied,
        &mut unsupported_requests,
    );
    if let Some(display_id) = options.display_id {
        unsupported_requests.push(unsupported(DockViewportPlatformSyncRequest::Display {
            requested: display_id,
        }));
    }
    if let Some(size) = options.window_min_size {
        unsupported_requests.push(unsupported(
            DockViewportPlatformSyncRequest::WindowMinSize { requested: size },
        ));
    }
    if options.icon.is_some() {
        unsupported_requests.push(unsupported(DockViewportPlatformSyncRequest::Icon));
    }
    if let Some(app_id) = options.app_id {
        window.set_app_id(&app_id);
        applied.push(DockViewportPlatformSyncAction::AppId { app_id });
    }
    if let Some(tabbing_identifier) = options.tabbing_identifier {
        unsupported_requests.push(unsupported(
            DockViewportPlatformSyncRequest::TabbingIdentifier {
                requested: tabbing_identifier,
            },
        ));
    }

    if let Some(decorations) = options.window_decorations {
        window.request_decorations(decorations);
        applied.push(DockViewportPlatformSyncAction::WindowDecorations { decorations });
    }

    if options.window_background != default_window_background() {
        if !flag_capabilities.alpha_windows
            && matches!(
                options.window_background,
                WindowBackgroundAppearance::Transparent | WindowBackgroundAppearance::Blurred
            )
        {
            unsupported_requests.push(unsupported(
                DockViewportPlatformSyncRequest::ViewportFlagAlpha { requested: 1.0 },
            ));
        } else {
            window.set_background_appearance(options.window_background);
            applied.push(DockViewportPlatformSyncAction::BackgroundAppearance {
                appearance: options.window_background,
            });
        }
    }

    match options.titlebar {
        Some(titlebar) => {
            if let Some(title) = titlebar.title {
                window.set_window_title(title.as_ref());
                applied.push(DockViewportPlatformSyncAction::Title {
                    title: title.to_string(),
                });
            }
            if titlebar.appears_transparent {
                unsupported_requests.push(unsupported(
                    DockViewportPlatformSyncRequest::TitlebarTransparency {
                        requested: titlebar.appears_transparent,
                    },
                ));
            }
            if let Some(position) = titlebar.traffic_light_position {
                #[cfg(target_os = "macos")]
                {
                    window.set_traffic_light_position(position);
                    applied.push(DockViewportPlatformSyncAction::TrafficLightPosition { position });
                }
                #[cfg(not(target_os = "macos"))]
                {
                    unsupported_requests.push(unsupported(
                        DockViewportPlatformSyncRequest::TrafficLightPosition {
                            requested: position,
                        },
                    ));
                }
            }
        }
        None => {
            unsupported_requests.push(unsupported(
                DockViewportPlatformSyncRequest::TitlebarPresence { requested: false },
            ));
        }
    }

    if let Some(window_bounds) = options.window_bounds {
        let requested_bounds = window_bounds.get_bounds();
        let current_bounds = window.bounds();
        if current_bounds.size != requested_bounds.size {
            if platform_requests.resize_requested {
                skipped_requests.push(skipped_for_platform_request(
                    DockViewportPlatformSyncRequest::WindowSize {
                        requested: requested_bounds.size,
                    },
                ));
            } else {
                window.resize(requested_bounds.size);
                applied.push(DockViewportPlatformSyncAction::Resize {
                    size: requested_bounds.size,
                });
            }
        }
        if current_bounds.origin != requested_bounds.origin {
            unsupported_requests.push(unsupported(DockViewportPlatformSyncRequest::WindowOrigin {
                requested: requested_bounds.origin,
            }));
        }

        match window_bounds {
            WindowBounds::Windowed(_) => {
                if window.is_fullscreen() {
                    window.toggle_fullscreen();
                    applied.push(DockViewportPlatformSyncAction::Fullscreen { enabled: false });
                }
            }
            WindowBounds::Fullscreen(_) => {
                if !window.is_fullscreen() {
                    window.toggle_fullscreen();
                    applied.push(DockViewportPlatformSyncAction::Fullscreen { enabled: true });
                }
            }
            WindowBounds::Maximized(_) => {
                unsupported_requests.push(unsupported(
                    DockViewportPlatformSyncRequest::WindowState {
                        requested: DockViewportPlatformWindowState::Maximized,
                    },
                ));
            }
        }
    }

    DockViewportPlatformSyncRecord {
        window_id,
        applied,
        skipped_requests,
        unsupported_requests,
    }
}
