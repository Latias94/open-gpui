use crate::{
    DockViewportPlatformSyncAction, DockViewportPlatformSyncRecord,
    DockViewportPlatformSyncRequest, DockViewportPlatformSyncSkipped,
    DockViewportPlatformSyncSkippedReason, DockViewportPlatformSyncUnsupported,
    DockViewportPlatformSyncUnsupportedReason, DockViewportPlatformWindowState,
    viewport_registry::DockViewportPlatformRequests,
};
use open_gpui::{Window, WindowBackgroundAppearance, WindowBounds, WindowKind};

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

fn skipped_for_platform_request(
    request: DockViewportPlatformSyncRequest,
) -> DockViewportPlatformSyncSkipped {
    DockViewportPlatformSyncSkipped {
        request,
        reason: DockViewportPlatformSyncSkippedReason::PlatformRequestInProgress,
    }
}

/// Applies the subset of `WindowOptions` that GPUI exposes as live window mutations.
pub(crate) fn sync_reused_viewport_window(
    window: &mut Window,
    options: open_gpui::WindowOptions,
    platform_requests: DockViewportPlatformRequests,
) -> DockViewportPlatformSyncRecord {
    let window_id = window.window_handle().window_id();
    let mut applied = Vec::new();
    let mut skipped_requests = Vec::new();
    let mut unsupported_requests = Vec::new();

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
    if options.accepts_pointer_input != window.accepts_pointer_input() {
        if window.set_accepts_pointer_input(options.accepts_pointer_input) {
            applied.push(DockViewportPlatformSyncAction::PointerInput {
                enabled: options.accepts_pointer_input,
            });
        } else {
            unsupported_requests.push(unsupported(DockViewportPlatformSyncRequest::PointerInput {
                requested: options.accepts_pointer_input,
            }));
        }
    }
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
        window.set_background_appearance(options.window_background);
        applied.push(DockViewportPlatformSyncAction::BackgroundAppearance {
            appearance: options.window_background,
        });
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
