use std::{
    cell::{Cell, Ref, RefCell, RefMut},
    ffi::c_void,
    ptr::NonNull,
    rc::{Rc, Weak},
    sync::Arc,
    time::Instant,
};

use anyhow::Context as _;
use calloop::ping::Ping;
use futures::channel::oneshot::Receiver;
use open_gpui_collections::HashMap;

use raw_window_handle as rwh;
use wayland_backend::client::ObjectId;
use wayland_client::WEnum;
use wayland_client::{
    Proxy,
    protocol::{wl_callback, wl_output, wl_seat, wl_surface},
};
use wayland_protocols::ext::background_effect::v1::client::ext_background_effect_surface_v1;
use wayland_protocols::wp::viewporter::client::wp_viewport;
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1;
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self};
use wayland_protocols::xdg::shell::client::{xdg_popup, xdg_positioner, xdg_surface};
use wayland_protocols::{
    wp::fractional_scale::v1::client::wp_fractional_scale_v1,
    xdg::dialog::v1::client::xdg_dialog_v1::XdgDialogV1,
};
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1;

use crate::linux::wayland::{
    display::WaylandDisplay,
    popup::{PopupConfigure, PopupRepositionState},
    serial::SerialKind,
};
use crate::linux::{
    Globals, Output, PendingActivation, WaylandClientStatePtr,
    interaction_gate::{WindowInteractionGate, accesskit_adapter},
};
use open_gpui::{
    AnyWindowHandle, Bounds, Capslock, Corners, CursorStyle, Decorations, DevicePixels,
    ExternalDragPayload, FrameRequestSource, GpuSpecs, Modifiers, Pixels, PlatformAtlas,
    PlatformDisplay, PlatformExternalDragSession, PlatformFrameSignal, PlatformInput,
    PlatformInputCallback, PlatformInputCallbackSlot, PlatformInputHandler,
    PlatformInputHandlerSlot, PlatformNativeWindowRetirementOutcome,
    PlatformPresentationShutdownOutcome, PlatformShouldCloseCallbackSlot, PlatformWindow,
    PlatformWindowActiveStatusObservation, PlatformWindowCommand, PlatformWindowCommandDispatcher,
    PlatformWindowCommandOutcome, PlatformWindowDispatch, PlatformWindowInteractionQuiescence,
    PlatformWindowMutationObservation, PlatformWindowMutationUnobservedTerminal,
    PlatformWindowPresentOutcome, Point, PreparedPlatformPresentationShutdown, PromptButton,
    PromptLevel, RequestFrameOptions, ResizeEdge, Scene, Size, Tiling, WindowActivationPolicy,
    WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowControls,
    WindowCreationFacts, WindowDecorations, WindowKind, WindowMutationDomain,
    WindowMutationRequest, WindowParams, WindowPlacementState, WindowPlatformFacts,
    WindowPresentationShutdownTicket, WindowVisibility,
    layer_shell::{Anchor, LayerShellNotSupportedError},
    popup::PopupOptions,
    px, size,
};
use open_gpui_wgpu::{
    CompositorGpuHint, WgpuRenderer, WgpuSurfaceConfig, WgpuSurfaceShutdownProgress, wgpu,
};

#[path = "frame_state.rs"]
mod frame_state;

use frame_state::{FrameLoop, PresentationState};

fn ensure_popup_parent_mapped(
    initial_map_committed: bool,
    acknowledged_first_configure: bool,
    presentation: PresentationState,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        initial_map_committed && acknowledged_first_configure && presentation.has_presented(),
        "popup parent has not completed native mapping"
    );
    Ok(())
}

#[derive(Default)]
pub(crate) struct Callbacks {
    request_frame: Option<Rc<dyn Fn(RequestFrameOptions)>>,
    input: PlatformInputCallbackSlot,
    active_status_change: Option<Rc<dyn Fn(PlatformWindowActiveStatusObservation)>>,
    visibility_change: Option<Box<dyn FnMut(WindowVisibility)>>,
    hover_status_change: Option<Rc<dyn Fn(bool)>>,
    resize: Option<Rc<dyn Fn(Size<Pixels>, f32)>>,
    moved: Option<Rc<dyn Fn()>>,
    display_changed: Option<Box<dyn FnMut()>>,
    window_state_change: Option<Rc<dyn Fn()>>,
    window_mutation: Option<Rc<dyn Fn(PlatformWindowMutationObservation)>>,
    should_close: PlatformShouldCloseCallbackSlot,
    close_required: Option<Box<dyn FnOnce()>>,
    close: Option<Box<dyn FnOnce()>>,
    appearance_changed: Option<Rc<dyn Fn()>>,
    button_layout_changed: Option<Rc<dyn Fn()>>,
}

#[derive(Debug, Clone, Copy)]
struct RawWindow {
    window: *mut c_void,
    display: *mut c_void,
}

// Safety: The raw pointers in RawWindow point to Wayland surface/display
// which are valid for the window's lifetime. These are used only for
// passing to wgpu which needs Send+Sync for surface creation.
unsafe impl Send for RawWindow {}
unsafe impl Sync for RawWindow {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaylandInitialToplevelState {
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum WaylandCreationRole {
    Xdg {
        initial_state: WaylandInitialToplevelState,
        restore_bounds: Bounds<Pixels>,
    },
    LayerShell,
    Popup,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WaylandWindowCreationProjection {
    bounds: Bounds<Pixels>,
    role: WaylandCreationRole,
    alpha_surface: bool,
    focus_on_appearing: bool,
    activation_policy: WindowActivationPolicy,
    topmost: bool,
    taskbar_visible: bool,
}

impl WaylandWindowCreationProjection {
    fn new(window_bounds: WindowBounds, kind: &WindowKind) -> Self {
        let requested_bounds = window_bounds.get_bounds();
        let bounds = Bounds {
            origin: Point::default(),
            size: requested_bounds.size,
        };
        let (role, focus_on_appearing, activation_policy, topmost, taskbar_visible) = match kind {
            WindowKind::AnchoredPopup(_) => (
                WaylandCreationRole::Popup,
                false,
                WindowActivationPolicy {
                    accepts_activation: false,
                    focus_on_click: false,
                },
                false,
                false,
            ),
            WindowKind::LayerShell(options) => {
                let (focus_on_appearing, focus_on_click) = match options.keyboard_interactivity {
                    open_gpui::layer_shell::KeyboardInteractivity::None => (false, false),
                    open_gpui::layer_shell::KeyboardInteractivity::Exclusive => (true, true),
                    open_gpui::layer_shell::KeyboardInteractivity::OnDemand => (false, true),
                };
                let topmost = matches!(
                    options.layer,
                    open_gpui::layer_shell::Layer::Top | open_gpui::layer_shell::Layer::Overlay
                );
                (
                    WaylandCreationRole::LayerShell,
                    focus_on_appearing,
                    WindowActivationPolicy {
                        accepts_activation: false,
                        focus_on_click,
                    },
                    topmost,
                    false,
                )
            }
            _ => {
                let initial_state = match window_bounds {
                    WindowBounds::Windowed(_) => WaylandInitialToplevelState::Windowed,
                    WindowBounds::Maximized(_) => WaylandInitialToplevelState::Maximized,
                    WindowBounds::Fullscreen(_) => WaylandInitialToplevelState::Fullscreen,
                };
                (
                    WaylandCreationRole::Xdg {
                        initial_state,
                        restore_bounds: bounds,
                    },
                    // XDG does not expose a non-activating first-map policy. Record the platform
                    // default instead of claiming that an unsupported false request was applied.
                    true,
                    WindowActivationPolicy::default(),
                    false,
                    matches!(kind, WindowKind::Normal | WindowKind::PopUp),
                )
            }
        };

        Self {
            bounds,
            role,
            alpha_surface: true,
            focus_on_appearing,
            activation_policy,
            topmost,
            taskbar_visible,
        }
    }

    fn restore_bounds(self) -> Option<Bounds<Pixels>> {
        match self.role {
            WaylandCreationRole::Xdg { restore_bounds, .. } => Some(restore_bounds),
            WaylandCreationRole::LayerShell | WaylandCreationRole::Popup => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WaylandWindowBackgroundProjection {
    observed_appearance: WindowBackgroundAppearance,
    renderer_transparent: bool,
    compositor_opaque_region: bool,
    blur_enabled: bool,
}

impl WaylandWindowBackgroundProjection {
    fn new(
        requested: WindowBackgroundAppearance,
        decorations: WindowDecorations,
        background_effects_supported: bool,
        kde_blur_supported: bool,
    ) -> Self {
        let observed_appearance = match requested {
            WindowBackgroundAppearance::Opaque | WindowBackgroundAppearance::Transparent => {
                requested
            }
            WindowBackgroundAppearance::Blurred
                if background_effects_supported || kde_blur_supported =>
            {
                WindowBackgroundAppearance::Blurred
            }
            WindowBackgroundAppearance::Blurred
            | WindowBackgroundAppearance::MicaBackdrop
            | WindowBackgroundAppearance::MicaAltBackdrop => {
                WindowBackgroundAppearance::Transparent
            }
        };
        let compositor_opaque_region = observed_appearance == WindowBackgroundAppearance::Opaque
            && decorations == WindowDecorations::Server;
        Self {
            observed_appearance,
            renderer_transparent: !compositor_opaque_region,
            compositor_opaque_region,
            blur_enabled: observed_appearance == WindowBackgroundAppearance::Blurred,
        }
    }
}

impl rwh::HasWindowHandle for RawWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let window = NonNull::new(self.window).unwrap();
        let handle = rwh::WaylandWindowHandle::new(window);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(handle.into()) })
    }
}
impl rwh::HasDisplayHandle for RawWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let display = NonNull::new(self.display).unwrap();
        let handle = rwh::WaylandDisplayHandle::new(display);
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(handle.into()) })
    }
}

#[derive(Debug)]
struct InProgressConfigure {
    size: Option<Size<Pixels>>,
    fullscreen: bool,
    maximized: bool,
    resizing: bool,
    visibility: WindowVisibility,
    tiling: Tiling,
    popup_configure: Option<PopupConfigure>,
}

pub struct WaylandWindowState {
    _connection_lease: Rc<()>,
    surface_state: WaylandSurfaceState,
    pub surface: wl_surface::WlSurface,
    app_id: Option<String>,
    appearance: WindowAppearance,
    kde_blur: Option<org_kde_kwin_blur::OrgKdeKwinBlur>,
    background_effects: Option<ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1>,
    viewport: Option<wp_viewport::WpViewport>,
    outputs: HashMap<ObjectId, Output>,
    display: Option<(ObjectId, Output)>,
    globals: Globals,
    renderer: WgpuRenderer,
    presentation_shutdown: Option<WindowPresentationShutdownTicket>,
    bounds: Bounds<Pixels>,
    scale: f32,
    input_handler: PlatformInputHandlerSlot,
    decorations: WindowDecorations,
    requested_background_appearance: WindowBackgroundAppearance,
    background_appearance: WindowBackgroundAppearance,
    fullscreen: bool,
    maximized: bool,
    /// Only xdg-shell v6 toplevels report suspension.
    /// Older compositors and layer shell stay visible.
    visibility: WindowVisibility,
    tiling: Tiling,
    window_bounds: Bounds<Pixels>,
    client: WaylandClientStatePtr,
    handle: AnyWindowHandle,
    active: bool,
    interaction_gate: WindowInteractionGate,
    hovered: bool,
    cursor_style: CursorStyle,
    redraw_requested: bool,
    presentation: PresentationState,
    pending_frame_callback: Option<wl_callback::WlCallback>,
    in_progress_configure: Option<InProgressConfigure>,
    resize_throttle: bool,
    in_progress_window_controls: Option<WindowControls>,
    window_controls: WindowControls,
    client_inset: Option<Pixels>,
    client_corner_radii: Corners<Pixels>,
    accesskit_adapter: Option<accesskit_unix::Adapter>,
    creation: WaylandWindowCreationProjection,
    initially_shown: bool,
    initial_map_committed: bool,
    initial_presentation_completed: bool,
    transient_for: Option<AnyWindowHandle>,
    parent: Option<Weak<RefCell<WaylandWindowState>>>,
    children: Vec<WaylandChild>,
    input_region: Option<Vec<Bounds<Pixels>>>,
    acknowledged_first_configure: bool,
}

struct WaylandChild {
    window: WaylandWindowWeak,
    blocking: bool,
}

pub enum WaylandSurfaceState {
    Xdg(WaylandXdgSurfaceState),
    LayerShell(WaylandLayerSurfaceState),
    Popup(WaylandPopupSurfaceState),
}

impl WaylandSurfaceState {
    fn new(
        surface: &wl_surface::WlSurface,
        globals: &Globals,
        params: &WindowParams,
        creation: &WaylandWindowCreationProjection,
        parent: Option<&WaylandWindowStatePtr>,
        popup_grab: Option<(u32, wl_seat::WlSeat)>,
        target_output: Option<wl_output::WlOutput>,
    ) -> anyhow::Result<Self> {
        // For layer_shell windows, create a layer surface instead of an xdg surface
        if let WindowKind::LayerShell(options) = &params.kind {
            let Some(layer_shell) = globals.layer_shell.as_ref() else {
                return Err(LayerShellNotSupportedError.into());
            };

            let layer_surface = layer_shell.get_layer_surface(
                &surface,
                target_output.as_ref(),
                super::layer_shell::wayland_layer(options.layer),
                options.namespace.clone(),
                &globals.qh,
                surface.id(),
            );

            let width = f32::from(creation.bounds.size.width);
            let height = f32::from(creation.bounds.size.height);
            layer_surface.set_size(width as u32, height as u32);

            layer_surface.set_anchor(super::layer_shell::wayland_anchor(options.anchor));
            layer_surface.set_keyboard_interactivity(
                super::layer_shell::wayland_keyboard_interactivity(options.keyboard_interactivity),
            );

            if let Some(margin) = options.margin {
                layer_surface.set_margin(
                    f32::from(margin.0) as i32,
                    f32::from(margin.1) as i32,
                    f32::from(margin.2) as i32,
                    f32::from(margin.3) as i32,
                )
            }

            if let Some(exclusive_zone) = options.exclusive_zone {
                layer_surface.set_exclusive_zone(f32::from(exclusive_zone) as i32);
            }

            if let Some(exclusive_edge) = options.exclusive_edge {
                Self::apply_exclusive_edge(&layer_surface, options.anchor, exclusive_edge);
            }

            return Ok(WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState {
                layer_surface,
                anchor: options.anchor,
            }));
        }

        if let WindowKind::AnchoredPopup(options) = &params.kind {
            let parent = parent.context("popup parent window not found")?;
            let parent_geometry = parent.window_geometry();
            let positioner =
                build_popup_positioner(globals, options, creation.bounds.size, parent_geometry);
            let xdg_surface = globals
                .wm_base
                .get_xdg_surface(surface, &globals.qh, surface.id());

            // A layer-shell parent takes a null xdg parent and is attached via the layer
            // surface. Every other surface kind has an xdg_surface to parent to directly.
            let xdg_popup = if let Some(parent_layer_surface) = parent.layer_surface() {
                let xdg_popup = xdg_surface.get_popup(None, &positioner, &globals.qh, surface.id());
                parent_layer_surface.get_popup(&xdg_popup);
                xdg_popup
            } else {
                xdg_surface.get_popup(
                    parent.xdg_surface().as_ref(),
                    &positioner,
                    &globals.qh,
                    surface.id(),
                )
            };
            positioner.destroy();

            let grabbed = popup_grab.is_some();
            if let Some((serial, seat)) = popup_grab {
                xdg_popup.grab(&seat, serial);
            }

            return Ok(Self::Popup(WaylandPopupSurfaceState {
                xdg_surface,
                xdg_popup,
                options: options.clone(),
                reposition: PopupRepositionState::default(),
                grabbed,
            }));
        }

        // All other WindowKinds result in a regular xdg surface
        let xdg_surface = globals
            .wm_base
            .get_xdg_surface(&surface, &globals.qh, surface.id());

        let toplevel = xdg_surface.get_toplevel(&globals.qh, surface.id());
        if let Some(parent) = parent {
            toplevel.set_parent(parent.toplevel().as_ref());
        }

        let dialog = if params.kind == WindowKind::Dialog {
            let dialog = globals.dialog.as_ref().map(|dialog| {
                let xdg_dialog = dialog.get_xdg_dialog(&toplevel, &globals.qh, ());
                xdg_dialog.set_modal();
                xdg_dialog
            });

            dialog
        } else {
            None
        };

        if let Some(size) = params.window_min_size {
            toplevel.set_min_size(f32::from(size.width) as i32, f32::from(size.height) as i32);
        }
        let WaylandCreationRole::Xdg { initial_state, .. } = creation.role else {
            unreachable!("non-layer-shell windows must use the XDG creation role")
        };
        match initial_state {
            WaylandInitialToplevelState::Windowed => {}
            WaylandInitialToplevelState::Maximized => toplevel.set_maximized(),
            WaylandInitialToplevelState::Fullscreen => toplevel.set_fullscreen(None),
        }

        // Attempt to set up window decorations based on the requested configuration
        let decoration = globals
            .decoration_manager
            .as_ref()
            .map(|decoration_manager| {
                decoration_manager.get_toplevel_decoration(&toplevel, &globals.qh, surface.id())
            });

        Ok(WaylandSurfaceState::Xdg(WaylandXdgSurfaceState {
            xdg_surface,
            toplevel,
            decoration,
            dialog,
        }))
    }
}

pub struct WaylandXdgSurfaceState {
    xdg_surface: xdg_surface::XdgSurface,
    toplevel: xdg_toplevel::XdgToplevel,
    decoration: Option<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1>,
    dialog: Option<XdgDialogV1>,
}

pub struct WaylandLayerSurfaceState {
    layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    anchor: Anchor,
}

pub struct WaylandPopupSurfaceState {
    xdg_surface: xdg_surface::XdgSurface,
    xdg_popup: xdg_popup::XdgPopup,
    options: PopupOptions,
    reposition: PopupRepositionState,
    grabbed: bool,
}

fn build_popup_positioner(
    globals: &Globals,
    options: &PopupOptions,
    size: Size<Pixels>,
    parent_geometry: Bounds<Pixels>,
) -> xdg_positioner::XdgPositioner {
    let positioner = globals.wm_base.create_positioner(&globals.qh, ());
    // A zero or negative size is a protocol error.
    positioner.set_size(
        f32::from(size.width).max(1.0) as i32,
        f32::from(size.height).max(1.0) as i32,
    );

    // The protocol wants the anchor rect relative to the parent's window geometry, while
    // `options.anchor_rect` is in gpui window coordinates (surface-local). A rect extending
    // outside the geometry or with a zero size is a protocol error, so translate, then clamp
    // to at least one pixel inside the geometry, pulling the origin inward at the edges.
    let anchor_rect = Bounds {
        origin: options.anchor_rect.origin - parent_geometry.origin,
        size: options.anchor_rect.size,
    };
    let one = Point::new(px(1.0), px(1.0));
    let geometry_bottom_right: Point<Pixels> = parent_geometry.size.into();
    let top_left = anchor_rect
        .origin
        .min(&(geometry_bottom_right - one))
        .max(&Point::default());
    let bottom_right = anchor_rect
        .bottom_right()
        .min(&geometry_bottom_right)
        .max(&(top_left + one));
    let anchor_rect = Bounds::from_corners(top_left, bottom_right);
    positioner.set_anchor_rect(
        f32::from(anchor_rect.origin.x) as i32,
        f32::from(anchor_rect.origin.y) as i32,
        f32::from(anchor_rect.size.width) as i32,
        f32::from(anchor_rect.size.height) as i32,
    );

    positioner.set_anchor(super::popup::wayland_anchor(options.anchor));
    positioner.set_gravity(super::popup::wayland_gravity(options.gravity));
    positioner.set_constraint_adjustment(super::popup::wayland_constraint_adjustment(
        options.constraint_adjustment,
    ));
    positioner.set_offset(
        f32::from(options.offset.x) as i32,
        f32::from(options.offset.y) as i32,
    );
    positioner
}

impl WaylandSurfaceState {
    fn ack_configure(&self, serial: u32) {
        match self {
            WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { xdg_surface, .. })
            | WaylandSurfaceState::Popup(WaylandPopupSurfaceState { xdg_surface, .. }) => {
                xdg_surface.ack_configure(serial);
            }
            WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState { layer_surface, .. }) => {
                layer_surface.ack_configure(serial);
            }
        }
    }

    fn decoration(&self) -> Option<&zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1> {
        if let WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { decoration, .. }) = self {
            decoration.as_ref()
        } else {
            None
        }
    }

    fn toplevel(&self) -> Option<&xdg_toplevel::XdgToplevel> {
        if let WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { toplevel, .. }) = self {
            Some(toplevel)
        } else {
            None
        }
    }

    fn set_geometry(&self, x: i32, y: i32, width: i32, height: i32) {
        match self {
            WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { xdg_surface, .. })
            | WaylandSurfaceState::Popup(WaylandPopupSurfaceState { xdg_surface, .. }) => {
                xdg_surface.set_window_geometry(x, y, width, height);
            }
            WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState { layer_surface, .. }) => {
                // cannot set window position of a layer surface
                layer_surface.set_size(width as u32, height as u32);
            }
        }
    }

    fn xdg_surface(&self) -> Option<&xdg_surface::XdgSurface> {
        match self {
            Self::Xdg(WaylandXdgSurfaceState { xdg_surface, .. })
            | Self::Popup(WaylandPopupSurfaceState { xdg_surface, .. }) => Some(xdg_surface),
            Self::LayerShell(_) => None,
        }
    }

    fn layer_surface(&self) -> Option<&zwlr_layer_surface_v1::ZwlrLayerSurfaceV1> {
        match self {
            Self::LayerShell(WaylandLayerSurfaceState { layer_surface, .. }) => Some(layer_surface),
            _ => None,
        }
    }

    fn reposition_popup(
        &mut self,
        globals: &Globals,
        generation: u64,
        size: Size<Pixels>,
        parent_geometry: Bounds<Pixels>,
    ) -> bool {
        let Self::Popup(popup) = self else {
            return false;
        };
        if popup.xdg_popup.version() < xdg_popup::REQ_REPOSITION_SINCE {
            return false;
        }
        let Some(token) = popup.reposition.request(generation) else {
            return false;
        };
        let positioner = build_popup_positioner(globals, &popup.options, size, parent_geometry);
        popup.xdg_popup.reposition(&positioner, token);
        positioner.destroy();
        true
    }

    fn set_exclusive_zone(&self, zone: i32) -> bool {
        if let Self::LayerShell(WaylandLayerSurfaceState { layer_surface, .. }) = self {
            layer_surface.set_exclusive_zone(zone);
            true
        } else {
            false
        }
    }

    /// An exclusive edge must be a single edge that the surface is anchored to,
    /// otherwise the compositor raises a fatal `invalid_exclusive_edge` protocol
    /// error. An invalid edge is logged and ignored. Returns whether it applied.
    fn apply_exclusive_edge(
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        anchor: Anchor,
        edge: Anchor,
    ) -> bool {
        if edge.bits().count_ones() == 1 && anchor.contains(edge) {
            if layer_surface.version() < zwlr_layer_surface_v1::REQ_SET_EXCLUSIVE_EDGE_SINCE {
                return false;
            }
            layer_surface.set_exclusive_edge(super::layer_shell::wayland_anchor(edge));
            true
        } else {
            log::warn!(
                "ignoring exclusive edge {edge:?}: must be a single edge of the surface anchor {anchor:?}"
            );
            false
        }
    }

    fn set_exclusive_edge(&self, edge: Anchor) -> bool {
        if let Self::LayerShell(WaylandLayerSurfaceState {
            layer_surface,
            anchor,
        }) = self
        {
            Self::apply_exclusive_edge(layer_surface, *anchor, edge)
        } else {
            false
        }
    }

    fn destroy(&mut self) {
        match self {
            WaylandSurfaceState::Xdg(WaylandXdgSurfaceState {
                xdg_surface,
                toplevel,
                decoration: _decoration,
                dialog,
            }) => {
                // drop the dialog before toplevel so compositor can explicitly unapply it's effects
                if let Some(dialog) = dialog {
                    dialog.destroy();
                }

                // The role object (toplevel) must always be destroyed before the xdg_surface.
                // See https://wayland.app/protocols/xdg-shell#xdg_surface:request:destroy
                toplevel.destroy();
                xdg_surface.destroy();
            }
            WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState { layer_surface, .. }) => {
                layer_surface.destroy();
            }
            WaylandSurfaceState::Popup(WaylandPopupSurfaceState {
                xdg_surface,
                xdg_popup,
                ..
            }) => {
                // Role object before its xdg_surface, as with the toplevel above.
                xdg_popup.destroy();
                xdg_surface.destroy();
            }
        }
    }
}

#[derive(Clone)]
pub struct WaylandWindowStatePtr {
    state: Rc<RefCell<WaylandWindowState>>,
    callbacks: Rc<RefCell<Callbacks>>,
    frame_loop: Rc<Cell<FrameLoop>>,
    frame_ping: Ping,
    scheduled_frame_at: Rc<Cell<Option<Instant>>>,
    retry_deadline: Rc<Cell<Option<Instant>>>,
    owner: Weak<()>,
}

struct WaylandWindowWeak {
    state: Weak<RefCell<WaylandWindowState>>,
    callbacks: Weak<RefCell<Callbacks>>,
    frame_loop: Weak<Cell<FrameLoop>>,
    frame_ping: Ping,
    scheduled_frame_at: Weak<Cell<Option<Instant>>>,
    retry_deadline: Weak<Cell<Option<Instant>>>,
    owner: Weak<()>,
}

impl WaylandWindowWeak {
    fn upgrade(&self) -> Option<WaylandWindowStatePtr> {
        Some(WaylandWindowStatePtr {
            state: self.state.upgrade()?,
            callbacks: self.callbacks.upgrade()?,
            frame_loop: self.frame_loop.upgrade()?,
            frame_ping: self.frame_ping.clone(),
            scheduled_frame_at: self.scheduled_frame_at.upgrade()?,
            retry_deadline: self.retry_deadline.upgrade()?,
            owner: self.owner.clone(),
        })
    }
}

struct WaylandWindowCommandTarget {
    owner: Weak<()>,
    state: Weak<RefCell<WaylandWindowState>>,
    frame_loop: Weak<Cell<FrameLoop>>,
    scheduled_frame_at: Weak<Cell<Option<Instant>>>,
    retry_deadline: Weak<Cell<Option<Instant>>>,
}

impl WaylandWindowCommandTarget {
    fn new(window: &WaylandWindow) -> Self {
        Self {
            owner: Rc::downgrade(&window.1),
            state: Rc::downgrade(&window.0.state),
            frame_loop: Rc::downgrade(&window.0.frame_loop),
            scheduled_frame_at: Rc::downgrade(&window.0.scheduled_frame_at),
            retry_deadline: Rc::downgrade(&window.0.retry_deadline),
        }
    }

    fn dispatch(&self, command: PlatformWindowCommand) -> PlatformWindowCommandOutcome {
        let Some(_owner) = self.owner.upgrade() else {
            return PlatformWindowCommandOutcome::WindowClosed;
        };
        let Some(state_ptr) = self.state.upgrade() else {
            return PlatformWindowCommandOutcome::WindowClosed;
        };
        let state = state_ptr.borrow();

        if state.interaction_gate.is_quiesced() && command.requires_interaction_authority() {
            return PlatformWindowCommandOutcome::Rejected;
        }

        match command {
            PlatformWindowCommand::CompleteNativeFrame => {
                drop(state);
                let (Some(frame_loop), Some(scheduled_frame_at), Some(retry_deadline)) = (
                    self.frame_loop.upgrade(),
                    self.scheduled_frame_at.upgrade(),
                    self.retry_deadline.upgrade(),
                ) else {
                    return PlatformWindowCommandOutcome::WindowClosed;
                };
                WaylandWindowStatePtr::complete_frame_state(
                    &state_ptr,
                    &frame_loop,
                    &scheduled_frame_at,
                    &retry_deadline,
                );
                PlatformWindowCommandOutcome::Accepted
            }
            PlatformWindowCommand::CompleteInitialPresentation { activate } => {
                if state.initial_presentation_completed {
                    return PlatformWindowCommandOutcome::Accepted;
                }
                let activate = state.initial_map_committed
                    && activate
                    && state.creation.activation_policy.accepts_activation;
                drop(state);
                state_ptr.borrow_mut().initial_presentation_completed = true;
                if activate {
                    let _ = activate_wayland_window(&state_ptr.borrow());
                }
                PlatformWindowCommandOutcome::Accepted
            }
            PlatformWindowCommand::RevealDeferredInitialPresentation { .. } => {
                PlatformWindowCommandOutcome::Rejected
            }
            PlatformWindowCommand::RestoreNativeWindowState(_)
            | PlatformWindowCommand::ToggleSimpleFullscreen => {
                PlatformWindowCommandOutcome::Unsupported
            }
            PlatformWindowCommand::ShowSoftKeyboard
            | PlatformWindowCommand::HideSoftKeyboard
            | PlatformWindowCommand::TextInputStateChanged(_) => {
                PlatformWindowCommandOutcome::Accepted
            }
            PlatformWindowCommand::Activate { .. } if state.globals.activation.is_none() => {
                PlatformWindowCommandOutcome::Unsupported
            }
            PlatformWindowCommand::Activate { .. }
                if state.creation.activation_policy.accepts_activation =>
            {
                activate_wayland_window(&state)
            }
            PlatformWindowCommand::Activate { .. } => PlatformWindowCommandOutcome::Rejected,
            PlatformWindowCommand::ShowWindowMenu(position) => {
                wayland_command_outcome(show_wayland_window_menu(&state, position))
            }
            PlatformWindowCommand::StartWindowMove => {
                wayland_command_outcome(start_wayland_window_move(&state))
            }
            PlatformWindowCommand::StartWindowResize(edge) => {
                wayland_command_outcome(start_wayland_window_resize(&state, edge))
            }
        }
    }
}

fn wayland_command_outcome(accepted: bool) -> PlatformWindowCommandOutcome {
    if accepted {
        PlatformWindowCommandOutcome::Accepted
    } else {
        PlatformWindowCommandOutcome::Rejected
    }
}

fn activate_wayland_window(state: &WaylandWindowState) -> PlatformWindowCommandOutcome {
    // Try to request an activation token. Even though the activation is likely going to be
    // rejected, KWin and Mutter can use the app_id to indicate that attention was requested.
    let Some(activation) = &state.globals.activation else {
        return PlatformWindowCommandOutcome::Unsupported;
    };
    let token = activation.get_activation_token(
        &state.globals.qh,
        PendingActivation::Window {
            surface: state.surface.id(),
            handle: state.handle,
        },
    );
    if let Some(app_id) = state.app_id.clone() {
        token.set_app_id(app_id);
    }
    if let Some(context) = state.client.activation_context() {
        token.set_serial(context.serial.as_raw(), &state.globals.seat);
        token.set_surface(&context.requesting_surface);
    } else {
        token.set_surface(&state.surface);
    }
    token.commit();
    PlatformWindowCommandOutcome::Accepted
}

fn show_wayland_window_menu(state: &WaylandWindowState, position: Point<Pixels>) -> bool {
    let serial = state.client.get_serial(SerialKind::MousePress);
    let Some(toplevel) = state.surface_state.toplevel() else {
        return false;
    };
    toplevel.show_window_menu(
        &state.globals.seat,
        serial,
        f32::from(position.x) as i32,
        f32::from(position.y) as i32,
    );
    true
}

fn start_wayland_window_move(state: &WaylandWindowState) -> bool {
    let serial = state.client.get_serial(SerialKind::MousePress);
    let Some(toplevel) = state.surface_state.toplevel() else {
        return false;
    };
    toplevel._move(&state.globals.seat, serial);
    true
}

fn start_wayland_window_resize(state: &WaylandWindowState, edge: ResizeEdge) -> bool {
    let Some(toplevel) = state.surface_state.toplevel() else {
        return false;
    };
    toplevel.resize(
        &state.globals.seat,
        state.client.get_serial(SerialKind::MousePress),
        edge.to_xdg(),
    );
    true
}

impl WaylandWindowState {
    fn new(
        handle: AnyWindowHandle,
        surface: wl_surface::WlSurface,
        mut surface_state: WaylandSurfaceState,
        appearance: WindowAppearance,
        viewport: Option<wp_viewport::WpViewport>,
        client: WaylandClientStatePtr,
        globals: Globals,
        gpu_context: open_gpui_wgpu::GpuContext,
        compositor_gpu: Option<CompositorGpuHint>,
        options: WindowParams,
        creation: WaylandWindowCreationProjection,
        transient_for: Option<AnyWindowHandle>,
        parent: Option<Weak<RefCell<WaylandWindowState>>>,
        connection_lease: Rc<()>,
    ) -> anyhow::Result<Self> {
        let initially_shown = options.show;
        let renderer = {
            let raw_window = RawWindow {
                window: surface.id().as_ptr().cast::<c_void>(),
                display: surface
                    .backend()
                    .upgrade()
                    .unwrap()
                    .display_ptr()
                    .cast::<c_void>(),
            };
            let config = WgpuSurfaceConfig {
                size: Size {
                    width: DevicePixels(f32::from(creation.bounds.size.width) as i32),
                    height: DevicePixels(f32::from(creation.bounds.size.height) as i32),
                },
                transparent: creation.alpha_surface,
                // Prefer Mailbox to avoid blocking. Falls back to FIFO if Mailbox is unsupported.
                preferred_present_mode: Some(wgpu::PresentMode::Mailbox),
            };
            match WgpuRenderer::new(gpu_context, &raw_window, config, compositor_gpu) {
                Ok(renderer) => renderer,
                Err(error) => {
                    if let Some(decoration) = surface_state.decoration() {
                        decoration.destroy();
                    }
                    surface_state.destroy();
                    if let Some(viewport) = viewport {
                        viewport.destroy();
                    }
                    surface.destroy();
                    return Err(error);
                }
            }
        };

        if let WaylandSurfaceState::Xdg(ref xdg_state) = surface_state {
            if let Some(title) = options.titlebar.and_then(|titlebar| titlebar.title) {
                xdg_state.toplevel.set_title(title.to_string());
            }
            // Set max window size based on the GPU's maximum texture dimension.
            // This prevents the window from being resized larger than what the GPU can render.
            let max_texture_size = renderer.max_texture_size() as i32;
            xdg_state
                .toplevel
                .set_max_size(max_texture_size, max_texture_size);
        }

        Ok(Self {
            _connection_lease: connection_lease,
            surface_state,
            surface,
            app_id: None,
            kde_blur: None,
            background_effects: None,
            viewport,
            globals,
            outputs: HashMap::default(),
            display: None,
            renderer,
            presentation_shutdown: None,
            bounds: creation.bounds,
            scale: 1.0,
            input_handler: PlatformInputHandlerSlot::default(),
            decorations: WindowDecorations::Client,
            background_appearance: WindowBackgroundAppearance::Opaque,
            requested_background_appearance: WindowBackgroundAppearance::Opaque,
            fullscreen: false,
            maximized: false,
            visibility: WindowVisibility::Visible,
            tiling: Tiling::default(),
            window_bounds: creation.restore_bounds().unwrap_or(creation.bounds),
            in_progress_configure: None,
            resize_throttle: false,
            client,
            appearance,
            handle,
            active: false,
            interaction_gate: WindowInteractionGate::default(),
            hovered: false,
            cursor_style: CursorStyle::Arrow,
            redraw_requested: false,
            presentation: PresentationState::Unpresented,
            pending_frame_callback: None,
            in_progress_window_controls: None,
            window_controls: WindowControls::default(),
            client_inset: None,
            client_corner_radii: Corners::default(),
            accesskit_adapter: None,
            creation,
            initially_shown,
            initial_map_committed: false,
            initial_presentation_completed: false,
            transient_for,
            parent,
            children: Vec::new(),
            input_region: None,
            acknowledged_first_configure: false,
        })
    }

    fn can_mutate_surface(&self) -> bool {
        self.surface.is_alive()
            && !self.interaction_gate.is_quiesced()
            && !self.presentation_shutdown_blocks_surface()
    }

    fn commit_surface_properties(&self) {
        if self.surface.is_alive()
            && self.initial_map_committed
            && !self.presentation_shutdown_blocks_surface()
        {
            self.surface.commit();
        }
    }

    fn apply_input_region(&self) {
        if !self.surface.is_alive() || self.presentation_shutdown_blocks_surface() {
            return;
        }
        let region = effective_input_region(&self.interaction_gate, self.input_region.as_deref());
        match region {
            None => self.surface.set_input_region(None),
            Some(rects) => {
                let wl_region = self.globals.compositor.create_region(&self.globals.qh, ());
                for rect in rects {
                    let rect = rect.map(|pixels| f32::from(pixels) as i32);
                    wl_region.add(
                        rect.origin.x,
                        rect.origin.y,
                        rect.size.width,
                        rect.size.height,
                    );
                }
                self.surface.set_input_region(Some(&wl_region));
                wl_region.destroy();
            }
        }
        self.commit_surface_properties();
    }

    fn quiesce_input(&mut self) {
        self.interaction_gate.quiesce();
        if let WaylandSurfaceState::Popup(popup) = &mut self.surface_state {
            popup.reposition.invalidate();
        }
        if let WaylandSurfaceState::LayerShell(layer) = &self.surface_state
            && self.surface.is_alive()
            && !self.presentation_shutdown_blocks_surface()
        {
            layer
                .layer_surface
                .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
        }
        self.apply_input_region();
    }

    fn accepts_pointer_input(&self) -> bool {
        effective_input_region(&self.interaction_gate, self.input_region.as_deref())
            .is_none_or(|region| !region.is_empty())
    }

    fn window_geometry(&self) -> Bounds<Pixels> {
        inset_by_tiling(
            self.bounds.map_origin(|_| px(0.0)),
            self.inset(),
            self.tiling,
        )
    }

    fn platform_facts(&self) -> WindowPlatformFacts {
        let window_bounds = if self.fullscreen {
            WindowBounds::Fullscreen(self.window_bounds)
        } else if self.maximized {
            WindowBounds::Maximized(self.window_bounds)
        } else {
            WindowBounds::Windowed(self.bounds)
        };
        let inner_window_bounds = if self.fullscreen {
            WindowBounds::Fullscreen(self.window_bounds)
        } else if self.maximized {
            WindowBounds::Maximized(self.window_bounds)
        } else {
            WindowBounds::Windowed(self.bounds.inset(self.inset()))
        };
        let display_id = self
            .current_output()
            .map(|(id, _)| open_gpui::DisplayId::from(id.protocol_id() as u64));

        WindowPlatformFacts {
            bounds: self.bounds,
            coordinate_space: open_gpui::WindowCoordinateSpace::WindowLocal,
            physical_geometry: None,
            window_bounds,
            inner_window_bounds,
            content_size: self.bounds.size,
            scale_factor: self.scale,
            display_id,
            is_minimized: false,
            is_maximized: self.maximized,
            is_fullscreen: self.fullscreen,
            accepts_pointer_input: self.accepts_pointer_input(),
            accepts_activation: !self.interaction_gate.is_quiesced()
                && self.creation.activation_policy.accepts_activation,
            focus_on_click: !self.interaction_gate.is_quiesced()
                && self.creation.activation_policy.focus_on_click,
            background_appearance: self.background_appearance,
            topmost: self.creation.topmost,
            taskbar_visible: self.creation.taskbar_visible,
            is_active: !self.interaction_gate.is_quiesced() && self.active,
        }
    }

    fn has_native_children(&self) -> bool {
        self.children.iter().any(|child| {
            child
                .window
                .state
                .upgrade()
                .is_some_and(|state| state.borrow().surface.is_alive())
        })
    }

    fn bind_presentation_shutdown(&mut self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        if let Some(current) = self.presentation_shutdown.as_ref() {
            return current.same_authority(shutdown);
        }

        self.presentation_shutdown = Some(shutdown.clone());
        true
    }

    fn presentation_shutdown_blocks_surface(&self) -> bool {
        self.presentation_shutdown.as_ref().is_some_and(|shutdown| {
            self.renderer.is_draining_for(shutdown) || self.renderer.is_quiesced_for(shutdown)
        }) || self.renderer.presentation_shutdown_active()
    }

    fn clear_presentation_bookkeeping(&mut self) {
        self.redraw_requested = false;
        self.pending_frame_callback = None;
        self.resize_throttle = false;
    }

    pub fn is_transparent(&self) -> bool {
        WaylandWindowBackgroundProjection::new(
            self.requested_background_appearance,
            self.decorations,
            self.globals.background_effects_manager.is_some()
                && self.globals.background_effects_blur.get(),
            self.globals.kde_blur_manager.is_some(),
        )
        .renderer_transparent
    }

    fn update_subpixel_layout(&mut self) {
        use wayland_client::protocol::wl_output::Subpixel;
        let is_bgr = self
            .display
            .as_ref()
            .and_then(|(_, output)| output.subpixel)
            .is_some_and(|s| s == Subpixel::HorizontalBgr);
        self.renderer.set_subpixel_layout(is_bgr);
    }

    pub fn primary_output_scale(&mut self) -> i32 {
        let mut scale = 1;
        let mut current_output = self
            .display
            .take()
            .filter(|(id, _)| self.outputs.is_empty() || self.outputs.contains_key(id));
        for (id, output) in self.outputs.iter() {
            if let Some((_, output_data)) = &current_output {
                if output.scale > output_data.scale {
                    current_output = Some((id.clone(), output.clone()));
                }
            } else {
                current_output = Some((id.clone(), output.clone()));
            }
            scale = scale.max(output.scale);
        }
        self.display = current_output;
        scale
    }

    fn current_output(&self) -> Option<(ObjectId, Output)> {
        self.display.clone().or_else(|| self.client.sole_output())
    }

    pub fn inset(&self) -> Pixels {
        match self.decorations {
            WindowDecorations::Server => px(0.0),
            WindowDecorations::Client => self.client_inset.unwrap_or(px(0.0)),
        }
    }
}

fn effective_input_region<'a>(
    gate: &WindowInteractionGate,
    requested: Option<&'a [Bounds<Pixels>]>,
) -> Option<&'a [Bounds<Pixels>]> {
    if gate.is_quiesced() {
        Some(&[])
    } else {
        requested
    }
}

pub(crate) struct WaylandWindow(pub WaylandWindowStatePtr, Rc<()>);
pub enum ImeInput {
    InsertText(String),
    SetMarkedText(String),
    UnmarkText,
    DeleteText,
}

impl Drop for WaylandWindow {
    fn drop(&mut self) {
        self.0.stop_frame_loop();
        self.0.terminate_callback_slots();
        self.0.close_children();

        let can_release_surface = self
            .0
            .state
            .borrow()
            .renderer
            .surface_owner_release_is_safe();
        if !can_release_surface || self.0.state.borrow().has_native_children() {
            log::error!(
                "Wayland window dropped before presentation shutdown or child retirement completed; retaining backend owner"
            );
            // Keep the state alive rather than releasing a surface with in-flight GPU work. The
            // normal creation/retirement path retains the same owner in NativeWindowRetirement,
            // so this is only an emergency fail-closed fallback.
            std::mem::forget(self.0.clone());
            return;
        }

        let mut state = self.0.state.borrow_mut();
        let surface_id = state.surface.id();
        let client = state.client.clone();

        state.renderer.destroy();

        // Destroy blur first, this has no dependencies.
        if let Some(blur) = state.kde_blur.take() {
            blur.release();
        }

        // Destroy background effects, this has no dependencies.
        if let Some(background_effects) = state.background_effects.take() {
            background_effects.destroy();
        }

        // Decorations must be destroyed before the xdg state.
        // See https://wayland.app/protocols/xdg-decoration-unstable-v1#zxdg_toplevel_decoration_v1
        if let Some(decoration) = &state.surface_state.decoration() {
            decoration.destroy();
        }

        // Surface state might contain xdg_toplevel/xdg_surface which can be destroyed now that
        // decorations are gone. layer_surface has no dependencies.
        state.surface_state.destroy();

        // Viewport must be destroyed before the wl_surface.
        // See https://wayland.app/protocols/viewporter#wp_viewport
        if let Some(viewport) = &state.viewport {
            viewport.destroy();
        }

        // The wl_surface itself should always be destroyed last.
        state.surface.destroy();

        let state_ptr = self.0.clone();
        state
            .globals
            .executor
            .spawn(async move {
                state_ptr.notify_closed();
                client.drop_window(&surface_id)
            })
            .detach();
        drop(state);
    }
}

impl WaylandWindow {
    fn borrow(&self) -> Ref<'_, WaylandWindowState> {
        self.0.state.borrow()
    }

    fn borrow_mut(&self) -> RefMut<'_, WaylandWindowState> {
        self.0.state.borrow_mut()
    }

    pub fn new(
        handle: AnyWindowHandle,
        globals: Globals,
        gpu_context: open_gpui_wgpu::GpuContext,
        compositor_gpu: Option<CompositorGpuHint>,
        client: WaylandClientStatePtr,
        params: WindowParams,
        appearance: WindowAppearance,
        parent: Option<WaylandWindowStatePtr>,
        popup_grab: Option<(u32, wl_seat::WlSeat)>,
        target_output: Option<wl_output::WlOutput>,
        connection_lease: Rc<()>,
    ) -> anyhow::Result<(Self, ObjectId)> {
        let creation = WaylandWindowCreationProjection::new(params.window_bounds, &params.kind);
        let transient_for = if matches!(&params.kind, WindowKind::LayerShell(_)) {
            None
        } else {
            params.transient_for
        };
        let blocking = params.kind == WindowKind::Dialog;
        let surface = globals.compositor.create_surface(&globals.qh, ());
        let surface_state = match WaylandSurfaceState::new(
            &surface,
            &globals,
            &params,
            &creation,
            parent.as_ref(),
            popup_grab,
            target_output,
        ) {
            Ok(surface_state) => surface_state,
            Err(error) => {
                surface.destroy();
                return Err(error);
            }
        };

        if let Some(fractional_scale_manager) = globals.fractional_scale_manager.as_ref() {
            fractional_scale_manager.get_fractional_scale(&surface, &globals.qh, surface.id());
        }

        let viewport = globals
            .viewporter
            .as_ref()
            .map(|viewporter| viewporter.get_viewport(&surface, &globals.qh, ()));

        let frame_ping = globals.frame_ping.clone();
        let owner = Rc::new(());
        let this = Self(
            WaylandWindowStatePtr {
                state: Rc::new(RefCell::new(WaylandWindowState::new(
                    handle,
                    surface.clone(),
                    surface_state,
                    appearance,
                    viewport,
                    client,
                    globals,
                    gpu_context,
                    compositor_gpu,
                    params,
                    creation,
                    transient_for,
                    parent.as_ref().map(|parent| Rc::downgrade(&parent.state)),
                    connection_lease,
                )?)),
                callbacks: Rc::new(RefCell::new(Callbacks::default())),
                frame_loop: Rc::new(Cell::new(FrameLoop::Unconfigured)),
                frame_ping,
                scheduled_frame_at: Rc::new(Cell::new(None)),
                retry_deadline: Rc::new(Cell::new(None)),
                owner: Rc::downgrade(&owner),
            },
            owner,
        );

        if let Some(parent) = parent {
            parent.add_child(&this.0, blocking);
        }

        #[cfg(test)]
        window_effect_tests::observe_creation(&this);

        Ok((this, surface.id()))
    }
}

impl WaylandWindowStatePtr {
    fn downgrade(&self) -> WaylandWindowWeak {
        WaylandWindowWeak {
            state: Rc::downgrade(&self.state),
            callbacks: Rc::downgrade(&self.callbacks),
            frame_loop: Rc::downgrade(&self.frame_loop),
            frame_ping: self.frame_ping.clone(),
            scheduled_frame_at: Rc::downgrade(&self.scheduled_frame_at),
            retry_deadline: Rc::downgrade(&self.retry_deadline),
            owner: self.owner.clone(),
        }
    }

    pub(crate) fn is_live(&self) -> bool {
        self.owner.strong_count() > 0 && self.state.borrow().can_mutate_surface()
    }

    pub(crate) fn ensure_popup_parent_mapped(&self) -> anyhow::Result<()> {
        let state = self.state.borrow();
        ensure_popup_parent_mapped(
            state.initial_map_committed,
            state.acknowledged_first_configure,
            state.presentation,
        )
    }

    pub(crate) fn interaction_is_quiesced(&self) -> bool {
        self.state.borrow().interaction_gate.is_quiesced()
    }

    fn terminate_callback_slots(&self) {
        self.state.borrow_mut().quiesce_input();
        self.callbacks.borrow_mut().display_changed = None;
        self.callbacks.borrow_mut().visibility_change = None;
        let (input_callback, should_close) = {
            let callbacks = self.callbacks.borrow();
            (callbacks.input.clone(), callbacks.should_close.clone())
        };
        let input_handler = self.state.borrow().input_handler.clone();
        input_callback.terminate();
        input_handler.terminate();
        should_close.terminate();
    }

    fn should_close(&self) -> bool {
        let should_close = self.callbacks.borrow().should_close.clone();
        should_close.invoke()
    }

    pub fn handle(&self) -> AnyWindowHandle {
        self.state.borrow().handle
    }

    pub fn surface(&self) -> wl_surface::WlSurface {
        self.state.borrow().surface.clone()
    }

    pub fn toplevel(&self) -> Option<xdg_toplevel::XdgToplevel> {
        self.state.borrow().surface_state.toplevel().cloned()
    }

    pub fn xdg_surface(&self) -> Option<xdg_surface::XdgSurface> {
        self.state.borrow().surface_state.xdg_surface().cloned()
    }

    pub fn layer_surface(&self) -> Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1> {
        self.state.borrow().surface_state.layer_surface().cloned()
    }

    pub fn window_geometry(&self) -> Bounds<Pixels> {
        self.state.borrow().window_geometry()
    }

    fn add_child(&self, child: &Self, blocking: bool) {
        let mut state = self.state.borrow_mut();
        state
            .children
            .retain(|child| child.window.state.strong_count() > 0);
        state.children.push(WaylandChild {
            window: child.downgrade(),
            blocking,
        });
    }

    fn children(&self) -> Vec<Self> {
        self.state
            .borrow()
            .children
            .iter()
            .rev()
            .filter_map(|child| child.window.upgrade())
            .collect()
    }

    fn close_children(&self) {
        for child in self.children() {
            child.require_close();
        }
    }

    fn quiesce_interaction(&self) {
        let mut state = self.state.borrow_mut();
        state.interaction_gate.quiesce();
        state.quiesce_input();
        state.active = false;
        state.hovered = false;
        state.creation.activation_policy = WindowActivationPolicy {
            accepts_activation: false,
            focus_on_click: false,
        };
        if let Some(adapter) = state.accesskit_adapter.as_mut() {
            adapter.update_window_focus_state(false);
        }
        let client = state.client.clone();
        let handle = state.handle;
        drop(state);
        client.quiesce_window_interaction(handle);
        self.close_children();
    }

    pub(crate) fn is_blocked(&self) -> bool {
        self.state.borrow().children.iter().any(|child| {
            child.blocking
                && child
                    .window
                    .state
                    .upgrade()
                    .is_some_and(|state| state.borrow().surface.is_alive())
        })
    }

    pub(crate) fn accepts_popup_grab(&self) -> bool {
        let state = self.state.borrow();
        if let WaylandSurfaceState::Popup(popup) = &state.surface_state
            && !popup.grabbed
        {
            return false;
        }
        !state.children.iter().any(|child| {
            child.window.upgrade().is_some_and(|child| {
                let state = child.state.borrow();
                state.surface.is_alive()
                    && matches!(&state.surface_state, WaylandSurfaceState::Popup(popup) if popup.grabbed)
            })
        })
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }

    pub fn cursor_style(&self) -> CursorStyle {
        self.state.borrow().cursor_style
    }

    pub fn set_cursor_style(&self, style: CursorStyle) {
        self.state.borrow_mut().cursor_style = style;
    }

    pub fn frame(&self, signal_at: Option<Instant>, signal_source: FrameRequestSource) {
        let Some(_owner) = self.owner.upgrade() else {
            return;
        };
        if self.frame_loop.get() == FrameLoop::Stopped {
            return;
        }
        let mut state = self.state.borrow_mut();
        if state.presentation_shutdown_blocks_surface() {
            self.stop_frame_loop();
            state.clear_presentation_bookkeeping();
            return;
        }

        self.frame_loop.set(FrameLoop::Ticking);
        state.resize_throttle = false;
        // GPUI may throttle this tick without calling draw, so leave the request
        // latched until a draw actually reaches the renderer.
        let force_render = state.redraw_requested;
        let require_presentation = state.presentation.requires_presentation();
        drop(state);

        let dispatched = self.dispatch_request_frame(RequestFrameOptions {
            force_render,
            require_presentation,
            signal_at,
            signal_source,
        });
        if !dispatched && self.frame_loop.get() != FrameLoop::Stopped {
            self.frame_loop.set(FrameLoop::Parked);
        }
    }

    fn complete_frame(&self) {
        Self::complete_frame_state(
            &self.state,
            &self.frame_loop,
            &self.scheduled_frame_at,
            &self.retry_deadline,
        );
    }

    fn complete_frame_state(
        window: &RefCell<WaylandWindowState>,
        frame_loop: &Cell<FrameLoop>,
        scheduled_frame_at: &Cell<Option<Instant>>,
        retry_deadline: &Cell<Option<Instant>>,
    ) {
        if frame_loop.get() == FrameLoop::Stopped {
            return;
        }
        let mut state = window.borrow_mut();
        if state.presentation_shutdown_blocks_surface() {
            frame_loop.set(FrameLoop::Stopped);
            scheduled_frame_at.take();
            retry_deadline.take();
            state.clear_presentation_bookkeeping();
            return;
        }

        let current = frame_loop.get();
        let next = current.complete(state.presentation, state.redraw_requested);
        if next == current {
            return;
        }
        frame_loop.set(next);
        match next {
            FrameLoop::AwaitingCallback => {
                scheduled_frame_at.take();
                retry_deadline.take();
                // Before the first present, or when throttling skipped draw, a
                // callback may never arrive. Otherwise let the compositor pace
                // retries so an occluded window does not keep polling.
                if state.pending_frame_callback.is_none() {
                    let callback = state.surface.frame(&state.globals.qh, state.surface.id());
                    state.pending_frame_callback = Some(callback);
                }
                state.surface.commit();
            }
            FrameLoop::RetryScheduled => {
                let surface_id = state.surface.id();
                let client = state.client.clone();
                drop(state);
                let deadline = client.schedule_frame_retry(&surface_id);
                retry_deadline.set(deadline);
                if deadline.is_none() {
                    frame_loop.set(FrameLoop::Parked);
                }
            }
            _ => {}
        }
    }

    pub fn frame_callback_fired(&self, signal_at: Option<Instant>) {
        // Another wl_surface commit may have carried this callback while a retry
        // timer owned the render-loop wakeup.
        self.state.borrow_mut().pending_frame_callback = None;
        if self.frame_loop.get() == FrameLoop::AwaitingCallback {
            self.frame(signal_at, FrameRequestSource::NativeCallback);
        }
    }

    pub fn scheduled_frame_fired(&self) {
        if self.frame_loop.get() == FrameLoop::Scheduled {
            self.frame(
                self.scheduled_frame_at.take(),
                FrameRequestSource::LocalSchedule,
            );
        }
    }

    pub fn retry_timer_fired(&self, deadline: Instant) {
        if self
            .frame_loop
            .get()
            .accepts_retry(self.retry_deadline.get(), deadline)
        {
            self.retry_deadline.take();
            self.frame(
                PlatformFrameSignal::capture(|| deadline),
                FrameRequestSource::LocalSchedule,
            );
        }
    }

    fn schedule_frame(&self) {
        let Some(_owner) = self.owner.upgrade() else {
            return;
        };
        Self::wake_frame_loop(&self.frame_loop, &self.scheduled_frame_at, &self.frame_ping);
    }

    fn wake_frame_loop(
        frame_loop: &Cell<FrameLoop>,
        scheduled_frame_at: &Cell<Option<Instant>>,
        frame_ping: &Ping,
    ) {
        let next = frame_loop.get().schedule();
        if next == frame_loop.get() {
            return;
        }
        frame_loop.set(next);
        if next == FrameLoop::Scheduled {
            scheduled_frame_at.set(PlatformFrameSignal::capture(Instant::now));
            frame_ping.ping();
        }
    }

    fn stop_frame_loop(&self) {
        self.frame_loop.set(FrameLoop::Stopped);
        self.scheduled_frame_at.take();
        self.retry_deadline.take();
    }

    fn request_redraw(&self) {
        if self.frame_loop.get() == FrameLoop::Stopped {
            return;
        }
        self.state.borrow_mut().redraw_requested = true;
        self.schedule_frame();
    }

    fn dispatch_request_frame(&self, options: RequestFrameOptions) -> bool {
        if self.frame_loop.get() == FrameLoop::Stopped
            || self.state.borrow().presentation_shutdown_blocks_surface()
        {
            return false;
        }
        let callback = self.callbacks.borrow().request_frame.clone();
        if let Some(callback) = callback {
            callback(options);
            self.update_ime_enabled();
            true
        } else {
            false
        }
    }

    fn update_ime_enabled(&self) {
        let (client, handle, input_handler, interaction_gate) = {
            let state = self.state.borrow();
            if !state.active {
                return;
            }
            (
                state.client.clone(),
                state.handle,
                state.input_handler.clone(),
                state.interaction_gate.clone(),
            )
        };
        if interaction_gate.is_quiesced() {
            client.quiesce_window_interaction(handle);
            return;
        }
        let ime_enabled = input_handler
            .with_handler(|input_handler| input_handler.query_accepts_text_input())
            .unwrap_or(false);
        if interaction_gate.is_quiesced() {
            client.quiesce_window_interaction(handle);
            return;
        }
        if Some(ime_enabled) == client.ime_enabled() {
            return;
        }

        if ime_enabled {
            client.enable_ime();
        } else {
            client.disable_ime();
        }
    }

    fn emit_window_state_change(&self) {
        let callback = self.callbacks.borrow().window_state_change.clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    pub(super) fn background_effects_changed(&self) {
        let state = self.state.borrow_mut();
        if !state.can_mutate_surface() {
            return;
        }
        let previous = state.background_appearance;
        update_window(state);
        let changed = self.state.borrow().background_appearance != previous;
        self.request_redraw();
        if changed {
            self.emit_window_state_change();
        }
    }

    pub fn handle_xdg_surface_event(&self, event: xdg_surface::Event) {
        let signal_at = PlatformFrameSignal::capture(Instant::now);
        if let xdg_surface::Event::Configure { serial } = event {
            if !self.is_live() {
                return;
            }
            if matches!(
                &self.state.borrow().surface_state,
                WaylandSurfaceState::Popup(_)
            ) {
                self.handle_popup_surface_configure(serial, signal_at);
                return;
            }
            {
                let mut state = self.state.borrow_mut();
                if let Some(window_controls) = state.in_progress_window_controls.take() {
                    state.window_controls = window_controls;

                    drop(state);
                    let appearance_changed = self.callbacks.borrow().appearance_changed.clone();
                    if let Some(appearance_changed) = appearance_changed {
                        appearance_changed();
                    }
                }
            }
            {
                let mut state = self.state.borrow_mut();

                if let Some(mut configure) = state.in_progress_configure.take() {
                    let state_changed = state.fullscreen != configure.fullscreen
                        || state.maximized != configure.maximized;
                    let got_unmaximized = state.maximized && !configure.maximized;
                    state.fullscreen = configure.fullscreen;
                    state.maximized = configure.maximized;
                    state.tiling = configure.tiling;
                    let visibility_changed = state.visibility != configure.visibility;
                    state.visibility = configure.visibility;
                    // Limit interactive resizes to once per vblank
                    if configure.resizing && state.resize_throttle {
                        state.surface_state.ack_configure(serial);
                        update_window(state);
                        if state_changed {
                            self.emit_window_state_change();
                        }
                        if visibility_changed {
                            self.report_visibility(configure.visibility);
                        }
                        return;
                    } else if configure.resizing {
                        state.resize_throttle = true;
                    }
                    if !configure.fullscreen && !configure.maximized {
                        configure.size = if got_unmaximized {
                            Some(state.window_bounds.size)
                        } else {
                            compute_outer_size(state.inset(), configure.size, state.tiling)
                        };
                        if let Some(size) = configure.size {
                            state.window_bounds = Bounds {
                                origin: Point::default(),
                                size,
                            };
                        }
                    }
                    drop(state);
                    if state_changed {
                        self.emit_window_state_change();
                    }
                    if visibility_changed {
                        self.report_visibility(configure.visibility);
                    }
                    if let Some(size) = configure.size {
                        self.resize(size);
                    }
                }
            }
            if !self.is_live() {
                return;
            }
            let mut state = self.state.borrow_mut();
            state.surface_state.ack_configure(serial);
            state.acknowledged_first_configure = true;

            let window_geometry = inset_by_tiling(
                state.bounds.map_origin(|_| px(0.0)),
                state.inset(),
                state.tiling,
            )
            .map(|v| f32::from(v) as i32)
            .map_size(|v| if v <= 0 { 1 } else { v });

            state.surface_state.set_geometry(
                window_geometry.origin.x,
                window_geometry.origin.y,
                window_geometry.size.width,
                window_geometry.size.height,
            );

            let initial_configure = self.frame_loop.get() == FrameLoop::Unconfigured;
            update_window(state);
            if initial_configure {
                self.frame(signal_at, FrameRequestSource::NativeCallback);
            } else {
                self.request_redraw();
            }
        }
    }

    fn handle_popup_surface_configure(&self, serial: u32, signal_at: Option<Instant>) {
        let (popup_configure, resized, initial) = {
            let mut state = self.state.borrow_mut();
            let Some(configure) = state.in_progress_configure.take() else {
                return;
            };
            let Some(popup_configure) = configure.popup_configure else {
                return;
            };
            let inset = state.inset();
            let size = compute_outer_size(inset, configure.size, configure.tiling);
            let WaylandWindowState {
                surface_state,
                bounds,
                window_bounds,
                renderer,
                viewport,
                scale,
                acknowledged_first_configure,
                ..
            } = &mut *state;
            let WaylandSurfaceState::Popup(popup) = surface_state else {
                return;
            };
            let initial = !*acknowledged_first_configure;
            let Some(resized) = popup
                .reposition
                .apply_configure(popup_configure, initial, || {
                    if let Some(size) = size {
                        *window_bounds = Bounds {
                            origin: Point::default(),
                            size,
                        };
                    }
                    let resized = size.filter(|size| *size != bounds.size);
                    if let Some(size) = resized {
                        bounds.size = size;
                        renderer.update_drawable_size(bounds.to_device_pixels(*scale).size);
                        if let Some(viewport) = viewport {
                            viewport.set_destination(
                                f32::from(size.width) as i32,
                                f32::from(size.height) as i32,
                            );
                        }
                    }
                    resized.map(|size| (size, *scale))
                })
            else {
                return;
            };
            (popup_configure, resized, initial)
        };
        if let Some((size, scale)) = resized {
            let callback = self.callbacks.borrow().resize.clone();
            if let Some(callback) = callback {
                callback(size, scale);
            }
        }
        if !self.is_live() {
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            let geometry = inset_by_tiling(
                state.bounds.map_origin(|_| px(0.0)),
                state.inset(),
                state.tiling,
            )
            .map(|value| f32::from(value) as i32)
            .map_size(|value| if value <= 0 { 1 } else { value });
            let WaylandWindowState {
                surface_state,
                acknowledged_first_configure,
                ..
            } = &mut *state;
            let WaylandSurfaceState::Popup(popup) = surface_state else {
                return;
            };
            let acknowledged = popup.reposition.apply_configure(
                popup_configure,
                !*acknowledged_first_configure,
                || {
                    popup.xdg_surface.ack_configure(serial);
                    *acknowledged_first_configure = true;
                    popup.xdg_surface.set_window_geometry(
                        geometry.origin.x,
                        geometry.origin.y,
                        geometry.size.width,
                        geometry.size.height,
                    );
                },
            );
            if acknowledged.is_none() {
                return;
            }
            update_window(state);
        }
        self.finish_popup_resize(popup_configure);
        if !self.is_live() {
            return;
        }
        if initial {
            self.frame(signal_at, FrameRequestSource::NativeCallback);
        } else {
            self.request_redraw();
        }
    }

    pub fn handle_toplevel_decoration_event(&self, event: zxdg_toplevel_decoration_v1::Event) {
        if !self.is_live() {
            return;
        }
        if let zxdg_toplevel_decoration_v1::Event::Configure { mode } = event {
            match mode {
                WEnum::Value(zxdg_toplevel_decoration_v1::Mode::ServerSide) => {
                    self.state.borrow_mut().decorations = WindowDecorations::Server;
                    let callback = self.callbacks.borrow().appearance_changed.clone();
                    if let Some(fun) = callback {
                        fun();
                    }
                }
                WEnum::Value(zxdg_toplevel_decoration_v1::Mode::ClientSide) => {
                    self.state.borrow_mut().decorations = WindowDecorations::Client;
                    // Update background to be transparent
                    let callback = self.callbacks.borrow().appearance_changed.clone();
                    if let Some(fun) = callback {
                        fun();
                    }
                }
                WEnum::Value(_) => {
                    log::warn!("Unknown decoration mode");
                    return;
                }
                WEnum::Unknown(v) => {
                    log::warn!("Unknown decoration mode: {}", v);
                    return;
                }
            }
            update_window(self.state.borrow_mut());
            self.request_redraw();
        }
    }

    pub fn handle_fractional_scale_event(&self, event: wp_fractional_scale_v1::Event) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            self.rescale(scale as f32 / 120.0);
            self.request_redraw();
        }
    }

    pub fn handle_toplevel_event(&self, event: xdg_toplevel::Event) -> bool {
        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                let size = if width == 0 || height == 0 {
                    None
                } else {
                    Some(size(px(width as f32), px(height as f32)))
                };

                let states = extract_states::<xdg_toplevel::State>(&states);

                let mut tiling = Tiling::default();
                let mut fullscreen = false;
                let mut maximized = false;
                let mut resizing = false;
                let mut visibility = WindowVisibility::Visible;

                for state in states {
                    match state {
                        xdg_toplevel::State::Maximized => {
                            maximized = true;
                        }
                        xdg_toplevel::State::Fullscreen => {
                            fullscreen = true;
                        }
                        xdg_toplevel::State::Resizing => resizing = true,
                        xdg_toplevel::State::Suspended => {
                            visibility = WindowVisibility::Hidden;
                        }
                        xdg_toplevel::State::TiledTop => {
                            tiling.top = true;
                        }
                        xdg_toplevel::State::TiledLeft => {
                            tiling.left = true;
                        }
                        xdg_toplevel::State::TiledRight => {
                            tiling.right = true;
                        }
                        xdg_toplevel::State::TiledBottom => {
                            tiling.bottom = true;
                        }
                        _ => {
                            // noop
                        }
                    }
                }

                if fullscreen || maximized {
                    tiling = Tiling::tiled();
                }

                let mut state = self.state.borrow_mut();
                state.in_progress_configure = Some(InProgressConfigure {
                    size,
                    fullscreen,
                    maximized,
                    resizing,
                    visibility,
                    tiling,
                    popup_configure: None,
                });

                false
            }
            xdg_toplevel::Event::Close => self.should_close(),
            xdg_toplevel::Event::WmCapabilities { capabilities } => {
                let mut window_controls = WindowControls {
                    maximize: false,
                    minimize: false,
                    fullscreen: false,
                    window_menu: false,
                };

                let states = extract_states::<xdg_toplevel::WmCapabilities>(&capabilities);

                for state in states {
                    match state {
                        xdg_toplevel::WmCapabilities::Maximize => {
                            window_controls.maximize = true;
                        }
                        xdg_toplevel::WmCapabilities::Minimize => {
                            window_controls.minimize = true;
                        }
                        xdg_toplevel::WmCapabilities::Fullscreen => {
                            window_controls.fullscreen = true;
                        }
                        xdg_toplevel::WmCapabilities::WindowMenu => {
                            window_controls.window_menu = true;
                        }
                        _ => {}
                    }
                }

                let mut state = self.state.borrow_mut();
                state.in_progress_window_controls = Some(window_controls);
                false
            }
            _ => false,
        }
    }

    pub fn handle_layersurface_event(&self, event: zwlr_layer_surface_v1::Event) -> bool {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                width,
                height,
                serial,
            } => {
                let size = if width == 0 || height == 0 {
                    None
                } else {
                    Some(size(px(width as f32), px(height as f32)))
                };

                let mut state = self.state.borrow_mut();
                state.in_progress_configure = Some(InProgressConfigure {
                    size,
                    fullscreen: false,
                    maximized: false,
                    resizing: false,
                    visibility: WindowVisibility::Visible,
                    tiling: Tiling::default(),
                    popup_configure: None,
                });
                drop(state);

                // just do the same thing we'd do as an xdg_surface
                self.handle_xdg_surface_event(xdg_surface::Event::Configure { serial });

                false
            }
            zwlr_layer_surface_v1::Event::Closed => {
                // unlike xdg, we don't have a choice here: the surface is closing.
                true
            }
            _ => false,
        }
    }

    pub fn handle_popup_event(&self, event: xdg_popup::Event) -> bool {
        match event {
            xdg_popup::Event::PopupDone => {
                self.owner.strong_count() > 0 && self.state.borrow().surface.is_alive()
            }
            _ if !self.is_live() => false,
            // Only the size is needed, the position is the compositor's. The following
            // xdg_surface.configure applies the change.
            xdg_popup::Event::Configure { width, height, .. } => {
                let size = if width <= 0 || height <= 0 {
                    None
                } else {
                    Some(size(px(width as f32), px(height as f32)))
                };
                let mut state = self.state.borrow_mut();
                let initial = !state.acknowledged_first_configure;
                let WaylandSurfaceState::Popup(popup) = &mut state.surface_state else {
                    return false;
                };
                let popup_configure = popup.reposition.configure(initial);
                state.in_progress_configure = Some(InProgressConfigure {
                    size,
                    fullscreen: false,
                    maximized: false,
                    resizing: false,
                    visibility: WindowVisibility::Visible,
                    tiling: Tiling::default(),
                    popup_configure: Some(popup_configure),
                });
                false
            }
            xdg_popup::Event::Repositioned { token } => {
                if let WaylandSurfaceState::Popup(popup) =
                    &mut self.state.borrow_mut().surface_state
                {
                    popup.reposition.repositioned(token);
                }
                false
            }
            _ => false,
        }
    }

    fn finish_popup_resize(&self, configure: PopupConfigure) {
        if !self.is_live() {
            return;
        }
        let observation = {
            let mut state = self.state.borrow_mut();
            if !state.can_mutate_surface() {
                return;
            }
            let WaylandSurfaceState::Popup(popup) = &mut state.surface_state else {
                return;
            };
            let Some(generation) = popup.reposition.finish(configure) else {
                return;
            };
            PlatformWindowMutationObservation::observed(
                WindowMutationDomain::Placement,
                generation,
                state.platform_facts(),
            )
        };
        let callback = self.callbacks.borrow().window_mutation.clone();
        if let Some(callback) = callback {
            callback(observation);
        }
    }

    #[allow(clippy::mutable_key_type)]
    pub fn handle_surface_event(
        &self,
        event: wl_surface::Event,
        outputs: HashMap<ObjectId, Output>,
    ) {
        let mut state = self.state.borrow_mut();

        match event {
            wl_surface::Event::Enter { output } => {
                let id = output.id();

                let Some(output) = outputs.get(&id) else {
                    return;
                };

                state.outputs.insert(id, output.clone());

                let previous_display = state.display.as_ref().map(|(id, _)| id.clone());
                let scale = state.primary_output_scale();
                let display_changed =
                    state.display.as_ref().map(|(id, _)| id) != previous_display.as_ref();
                state.update_subpixel_layout();

                // We use `PreferredBufferScale` instead to set the scale if it's available
                if state.surface.version() < wl_surface::EVT_PREFERRED_BUFFER_SCALE_SINCE {
                    state.surface.set_buffer_scale(scale);
                    drop(state);
                    self.rescale(scale as f32);
                } else {
                    drop(state);
                }
                if display_changed {
                    self.report_display_changed();
                }
                self.request_redraw();
            }
            wl_surface::Event::Leave { output } => {
                state.outputs.remove(&output.id());

                let previous_display = state.display.as_ref().map(|(id, _)| id.clone());
                let scale = state.primary_output_scale();
                let display_changed =
                    state.display.as_ref().map(|(id, _)| id) != previous_display.as_ref();
                state.update_subpixel_layout();

                // We use `PreferredBufferScale` instead to set the scale if it's available
                if state.surface.version() < wl_surface::EVT_PREFERRED_BUFFER_SCALE_SINCE {
                    state.surface.set_buffer_scale(scale);
                    drop(state);
                    self.rescale(scale as f32);
                } else {
                    drop(state);
                }
                if display_changed {
                    self.report_display_changed();
                }
                self.request_redraw();
            }
            wl_surface::Event::PreferredBufferScale { factor } => {
                // We use `WpFractionalScale` instead to set the scale if it's available
                if state.globals.fractional_scale_manager.is_none() {
                    state.surface.set_buffer_scale(factor);
                    drop(state);
                    self.rescale(factor as f32);
                    self.request_redraw();
                }
            }
            _ => {}
        }
    }

    pub fn handle_ime(&self, ime: ImeInput) {
        let state = self.state.borrow();
        if state.interaction_gate.is_quiesced() || self.is_blocked() {
            return;
        }
        let input_handler = state.input_handler.clone();
        drop(state);
        let _ = input_handler.with_handler(|input_handler| match ime {
            ImeInput::InsertText(text) => input_handler.replace_text_in_range(None, &text),
            ImeInput::SetMarkedText(text) => {
                input_handler.replace_and_mark_text_in_range(None, &text, None)
            }
            ImeInput::UnmarkText => input_handler.unmark_text(),
            ImeInput::DeleteText => {
                if let Some(marked) = input_handler.marked_text_range() {
                    input_handler.replace_text_in_range(Some(marked), "");
                }
            }
        });
    }

    pub fn get_ime_area(&self) -> Option<Bounds<Pixels>> {
        let state = self.state.borrow();
        if state.interaction_gate.is_quiesced() || self.is_blocked() {
            return None;
        }
        let input_handler = state.input_handler.clone();
        drop(state);
        input_handler
            .with_handler(|input_handler| input_handler.ime_candidate_bounds())
            .flatten()
    }

    pub fn set_size_and_scale(&self, size: Option<Size<Pixels>>, scale: Option<f32>) {
        if !self.is_live() {
            return;
        }
        let (size, scale) = {
            let mut state = self.state.borrow_mut();
            if size.is_none_or(|size| size == state.bounds.size)
                && scale.is_none_or(|scale| scale == state.scale)
            {
                return;
            }
            if let Some(size) = size {
                state.bounds.size = size;
            }
            if let Some(scale) = scale {
                state.scale = scale;
            }
            let device_bounds = state.bounds.to_device_pixels(state.scale);
            state.renderer.update_drawable_size(device_bounds.size);
            (state.bounds.size, state.scale)
        };

        let callback = self.callbacks.borrow().resize.clone();
        if let Some(fun) = callback {
            fun(size, scale);
        }

        if !self.is_live() {
            return;
        }
        let state = self.state.borrow_mut();
        if let Some(viewport) = &state.viewport {
            viewport.set_destination(
                f32::from(state.bounds.size.width) as i32,
                f32::from(state.bounds.size.height) as i32,
            );
        }
        update_window(state);
    }

    pub fn resize(&self, size: Size<Pixels>) {
        self.set_size_and_scale(Some(size), None);
    }

    pub fn rescale(&self, scale: f32) {
        self.set_size_and_scale(None, Some(scale));
    }

    pub(crate) fn require_close(&self) {
        let callback = self.callbacks.borrow_mut().close_required.take();
        self.stop_frame_loop();
        self.quiesce_interaction();
        self.terminate_callback_slots();
        if let Some(callback) = callback {
            callback();
        }
    }

    fn notify_closed(&self) {
        self.terminate_callback_slots();
        let callback = self.callbacks.borrow_mut().close.take();
        if let Some(fun) = callback {
            fun()
        }
    }

    pub fn handle_input(&self, input: PlatformInput) {
        if self.interaction_is_quiesced() || self.is_blocked() {
            return;
        }
        let input_callback = self.callbacks.borrow().input.clone();
        if !input_callback.dispatch(input.clone()).propagate {
            return;
        }
        if self.interaction_is_quiesced() || self.is_blocked() {
            return;
        }
        if let PlatformInput::KeyDown(event) = input
            && event.keystroke.modifiers.is_subset_of(&Modifiers::shift())
            && let Some(key_char) = &event.keystroke.key_char
        {
            let input_handler = self.state.borrow().input_handler.clone();
            let _ = input_handler
                .with_handler(|input_handler| input_handler.replace_text_in_range(None, key_char));
        }
    }

    pub fn set_focused(&self, observation: PlatformWindowActiveStatusObservation) {
        let focus = observation.active();
        let interaction_gate = self.state.borrow().interaction_gate.clone();
        if !interaction_gate.admits_status(focus) {
            let mut state = self.state.borrow_mut();
            state.active = false;
            if let Some(adapter) = state.accesskit_adapter.as_mut() {
                adapter.update_window_focus_state(false);
            }
            return;
        }

        self.state.borrow_mut().active = focus;
        let callback = self.callbacks.borrow().active_status_change.clone();
        if let Some(fun) = callback {
            fun(observation);
        }

        let effective_focus = focus && !interaction_gate.is_quiesced();
        let mut state = self.state.borrow_mut();
        state.active = effective_focus;
        if let Some(adapter) = state.accesskit_adapter.as_mut() {
            adapter.update_window_focus_state(effective_focus);
        }
    }

    pub fn set_hovered(&self, focus: bool) {
        let interaction_gate = self.state.borrow().interaction_gate.clone();
        if !interaction_gate.admits_status(focus) {
            self.state.borrow_mut().hovered = false;
            return;
        }

        self.state.borrow_mut().hovered = focus;
        let callback = self.callbacks.borrow().hover_status_change.clone();
        if let Some(fun) = callback {
            fun(focus);
        }
        if focus && interaction_gate.is_quiesced() {
            self.state.borrow_mut().hovered = false;
        }
    }

    fn report_visibility(&self, visibility: WindowVisibility) {
        if self.interaction_is_quiesced() {
            return;
        }
        let callback = self.callbacks.borrow_mut().visibility_change.take();
        if let Some(mut callback) = callback {
            callback(visibility);
            if !self.interaction_is_quiesced() {
                self.callbacks
                    .borrow_mut()
                    .visibility_change
                    .get_or_insert(callback);
            }
        }
    }

    pub fn set_appearance(&mut self, appearance: WindowAppearance) {
        self.state.borrow_mut().appearance = appearance;

        let callback = self.callbacks.borrow().appearance_changed.clone();
        if let Some(fun) = callback {
            fun();
        }
    }

    pub fn set_button_layout(&self) {
        let callback = self.callbacks.borrow().button_layout_changed.clone();
        if let Some(fun) = callback {
            fun();
        }
    }

    pub fn primary_output_scale(&self) -> i32 {
        self.state.borrow_mut().primary_output_scale()
    }

    pub fn report_display_changed(&self) {
        if self.interaction_is_quiesced() {
            return;
        }
        let callback = self.callbacks.borrow_mut().display_changed.take();
        if let Some(mut callback) = callback {
            callback();
            if !self.interaction_is_quiesced() {
                self.callbacks
                    .borrow_mut()
                    .display_changed
                    .get_or_insert(callback);
            }
        }
    }

    pub fn handle_output_changed(&self, id: &ObjectId, output: &Output) -> bool {
        let mut state = self.state.borrow_mut();
        if state.interaction_gate.is_quiesced() {
            return false;
        }
        if let Some(entered) = state.outputs.get_mut(id) {
            *entered = output.clone();
        }
        let previous_display = state.display.as_ref().map(|(id, _)| id.clone());
        if let Some((display_id, display)) = &mut state.display
            && display_id == id
        {
            *display = output.clone();
        }
        state.primary_output_scale();
        state.update_subpixel_layout();
        state.display.is_none()
            || state.display.as_ref().map(|(id, _)| id) != previous_display.as_ref()
    }

    pub fn handle_output_removed(&self, id: &ObjectId) -> bool {
        let mut state = self.state.borrow_mut();
        if state.interaction_gate.is_quiesced() {
            return false;
        }
        state.outputs.remove(id);
        let previous_display = state.display.as_ref().map(|(id, _)| id.clone());
        if previous_display.as_ref() == Some(id) {
            state.display = None;
        }
        state.primary_output_scale();
        state.update_subpixel_layout();
        state.display.is_none()
            || state.display.as_ref().map(|(id, _)| id) != previous_display.as_ref()
    }
}

fn extract_states<'a, S: TryFrom<u32> + 'a>(states: &'a [u8]) -> impl Iterator<Item = S> + 'a
where
    <S as TryFrom<u32>>::Error: 'a,
{
    states
        .chunks_exact(4)
        .flat_map(TryInto::<[u8; 4]>::try_into)
        .map(u32::from_ne_bytes)
        .flat_map(S::try_from)
}

impl rwh::HasWindowHandle for WaylandWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let surface = self.0.surface().id().as_ptr() as *mut libc::c_void;
        let c_ptr = NonNull::new(surface).ok_or(rwh::HandleError::Unavailable)?;
        let handle = rwh::WaylandWindowHandle::new(c_ptr);
        let raw_handle = rwh::RawWindowHandle::Wayland(handle);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(raw_handle) })
    }
}

impl rwh::HasDisplayHandle for WaylandWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let display = self
            .0
            .surface()
            .backend()
            .upgrade()
            .ok_or(rwh::HandleError::Unavailable)?
            .display_ptr() as *mut libc::c_void;

        let c_ptr = NonNull::new(display).ok_or(rwh::HandleError::Unavailable)?;
        let handle = rwh::WaylandDisplayHandle::new(c_ptr);
        let raw_handle = rwh::RawDisplayHandle::Wayland(handle);
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(raw_handle) })
    }
}

impl PlatformWindow for WaylandWindow {
    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        let target = WaylandWindowCommandTarget::new(self);
        PlatformWindowCommandDispatcher::new(move |command| target.dispatch(command))
    }

    fn interaction_quiescence(&self) -> PlatformWindowInteractionQuiescence {
        let interaction_gate = self.0.state.borrow().interaction_gate.clone();
        let window = self.0.downgrade();
        PlatformWindowInteractionQuiescence::new(move || {
            interaction_gate.quiesce();
            if let Some(window) = window.upgrade() {
                window.quiesce_interaction();
            }
        })
    }

    fn prepare_presentation_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        let state = Rc::clone(&self.0.state);
        let frame_loop = Rc::clone(&self.0.frame_loop);
        let scheduled_frame_at = Rc::clone(&self.0.scheduled_frame_at);
        let retry_deadline = Rc::clone(&self.0.retry_deadline);
        PreparedPlatformPresentationShutdown::new(shutdown, move |shutdown| {
            let Ok(mut state) = state.try_borrow_mut() else {
                return PlatformPresentationShutdownOutcome::Rejected;
            };
            if shutdown.snapshot().window_id() != state.handle.window_id() {
                return PlatformPresentationShutdownOutcome::Rejected;
            }

            let outcome = match state.renderer.quiesce_surface(shutdown) {
                WgpuSurfaceShutdownProgress::Quiesced
                    if state.bind_presentation_shutdown(shutdown) =>
                {
                    PlatformPresentationShutdownOutcome::Quiesced
                }
                WgpuSurfaceShutdownProgress::EnteredDraining
                | WgpuSurfaceShutdownProgress::Draining
                    if state.bind_presentation_shutdown(shutdown) =>
                {
                    PlatformPresentationShutdownOutcome::Rejected
                }
                WgpuSurfaceShutdownProgress::EnteredDraining
                | WgpuSurfaceShutdownProgress::Draining
                | WgpuSurfaceShutdownProgress::Quiesced
                | WgpuSurfaceShutdownProgress::Rejected => {
                    PlatformPresentationShutdownOutcome::Rejected
                }
            };
            if state.presentation_shutdown_blocks_surface() {
                frame_loop.set(FrameLoop::Stopped);
                scheduled_frame_at.take();
                retry_deadline.take();
                state.clear_presentation_bookkeeping();
            }
            outcome
        })
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.borrow().bounds
    }

    fn retire_native_window(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformNativeWindowRetirementOutcome {
        let state = self.borrow();
        if shutdown.snapshot().window_id() != state.handle.window_id()
            || !shutdown.snapshot().quiesced()
            || !state.renderer.is_quiesced_for(shutdown)
            || state.has_native_children()
        {
            PlatformNativeWindowRetirementOutcome::Rejected
        } else {
            PlatformNativeWindowRetirementOutcome::Accepted
        }
    }

    fn map_window(&mut self) -> anyhow::Result<()> {
        let mut state = self.borrow_mut();
        if !state.initially_shown || state.initial_map_committed {
            return Ok(());
        }
        if !state.can_mutate_surface() {
            self.0.stop_frame_loop();
            state.clear_presentation_bookkeeping();
            return Ok(());
        }

        state.surface.commit();
        state.initial_map_committed = true;
        Ok(())
    }

    fn creation_facts(&self) -> WindowCreationFacts {
        let state = self.borrow();
        WindowCreationFacts {
            show: state.initially_shown,
            focus_on_appearing: state.creation.focus_on_appearing,
            transient_for: state.transient_for,
        }
    }

    fn is_visible(&self) -> bool {
        let state = self.borrow();
        state.initially_shown && state.presentation.has_presented()
    }

    fn platform_facts(&self) -> WindowPlatformFacts {
        self.borrow().platform_facts()
    }

    fn accepts_pointer_input(&self) -> bool {
        self.borrow().accepts_pointer_input()
    }

    fn prepare_window_mutation(&self, domain: WindowMutationDomain, generation: u64) {
        if domain == WindowMutationDomain::Placement
            && let WaylandSurfaceState::Popup(popup) = &mut self.borrow_mut().surface_state
        {
            popup.reposition.prepare(generation);
        }
    }

    fn invalidate_window_mutation(&self, domain: WindowMutationDomain) {
        if domain == WindowMutationDomain::Placement
            && let WaylandSurfaceState::Popup(popup) = &mut self.borrow_mut().surface_state
        {
            popup.reposition.invalidate();
        }
    }

    fn finish_window_mutation_without_observation(
        &self,
        domain: WindowMutationDomain,
        generation: u64,
        _terminal: PlatformWindowMutationUnobservedTerminal,
    ) {
        if domain == WindowMutationDomain::Placement
            && let WaylandSurfaceState::Popup(popup) = &mut self.borrow_mut().surface_state
        {
            popup.reposition.finish_unobserved(generation);
        }
    }

    fn request_window_mutation(
        &mut self,
        generation: u64,
        request: WindowMutationRequest,
    ) -> PlatformWindowDispatch {
        let mut state = self.borrow_mut();
        if !state.can_mutate_surface() {
            return PlatformWindowDispatch::Rejected;
        }
        let WindowMutationRequest::Placement(placement) = request else {
            return PlatformWindowDispatch::Unsupported;
        };
        if !matches!(state.surface_state, WaylandSurfaceState::Popup(_))
            || placement.position.is_some()
            || placement.restore_bounds.is_some()
            || placement
                .state
                .is_some_and(|state| state != WindowPlacementState::Windowed)
        {
            return PlatformWindowDispatch::Unsupported;
        }
        let Some(size) = placement.size else {
            return PlatformWindowDispatch::Unchanged;
        };
        if !f32::from(size.width).is_finite()
            || !f32::from(size.height).is_finite()
            || size.width <= px(0.0)
            || size.height <= px(0.0)
        {
            return PlatformWindowDispatch::Rejected;
        }
        if size == state.bounds.size {
            return PlatformWindowDispatch::Unchanged;
        }
        if !state.acknowledged_first_configure {
            return PlatformWindowDispatch::Rejected;
        }
        let Some(parent) = state.parent.as_ref().and_then(Weak::upgrade) else {
            return PlatformWindowDispatch::WindowClosed;
        };
        let parent = parent.borrow();
        if !parent.can_mutate_surface() {
            return PlatformWindowDispatch::WindowClosed;
        }
        let parent_geometry = parent.window_geometry();
        drop(parent);
        let globals = state.globals.clone();
        if state
            .surface_state
            .reposition_popup(&globals, generation, size, parent_geometry)
        {
            PlatformWindowDispatch::Queued
        } else {
            PlatformWindowDispatch::Unsupported
        }
    }

    fn is_maximized(&self) -> bool {
        self.borrow().maximized
    }

    fn window_bounds(&self) -> WindowBounds {
        let state = self.borrow();
        if state.fullscreen {
            WindowBounds::Fullscreen(state.window_bounds)
        } else if state.maximized {
            WindowBounds::Maximized(state.window_bounds)
        } else {
            drop(state);
            WindowBounds::Windowed(self.bounds())
        }
    }

    fn inner_window_bounds(&self) -> WindowBounds {
        let state = self.borrow();
        if state.fullscreen {
            WindowBounds::Fullscreen(state.window_bounds)
        } else if state.maximized {
            WindowBounds::Maximized(state.window_bounds)
        } else {
            let inset = state.inset();
            drop(state);
            WindowBounds::Windowed(self.bounds().inset(inset))
        }
    }

    fn content_size(&self) -> Size<Pixels> {
        self.borrow().bounds.size
    }

    fn scale_factor(&self) -> f32 {
        self.borrow().scale
    }

    fn appearance(&self) -> WindowAppearance {
        self.borrow().appearance
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        let state = self.borrow();
        let (id, display) = state.current_output()?;
        Some(Rc::new(WaylandDisplay {
            id,
            name: display.name,
            bounds: display.bounds.to_pixels(display.scale as f32),
            refresh_interval: display.refresh_interval,
        }))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.borrow()
            .client
            .get_client()
            .borrow()
            .mouse_location
            .unwrap_or_default()
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        self.0.set_cursor_style(style);
        let client = self.borrow().client.clone();
        client.set_cursor_style_for_window(&self.0, style);
    }

    fn modifiers(&self) -> Modifiers {
        self.borrow().client.get_client().borrow().modifiers
    }

    fn capslock(&self) -> Capslock {
        self.borrow().client.get_client().borrow().capslock
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        let input_handler_slot = self.borrow().input_handler.clone();
        input_handler_slot.set(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        let input_handler_slot = self.borrow().input_handler.clone();
        input_handler_slot.take()
    }

    fn take_input_handler_for_frame(&mut self) -> Option<PlatformInputHandler> {
        let slot = self.borrow().input_handler.clone();
        slot.take_for_frame()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<Receiver<usize>> {
        None
    }

    fn request_attention(&self) {}

    fn is_active(&self) -> bool {
        let state = self.borrow();
        !state.interaction_gate.is_quiesced() && state.active
    }

    fn visibility(&self) -> WindowVisibility {
        self.borrow().visibility
    }

    fn is_hovered(&self) -> bool {
        let state = self.borrow();
        !state.interaction_gate.is_quiesced() && state.hovered
    }

    fn set_title(&mut self, title: &str) {
        if let Some(toplevel) = self.borrow().surface_state.toplevel() {
            toplevel.set_title(title.to_string());
        }
    }

    fn set_app_id(&mut self, app_id: &str) {
        let mut state = self.borrow_mut();
        if let Some(toplevel) = state.surface_state.toplevel() {
            toplevel.set_app_id(app_id.to_owned());
        }
        state.app_id = Some(app_id.to_owned());
    }

    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        let mut state = self.borrow_mut();
        if !state.can_mutate_surface() {
            return;
        }
        state.requested_background_appearance = background_appearance;
        let projection = WaylandWindowBackgroundProjection::new(
            background_appearance,
            state.decorations,
            state.globals.background_effects_manager.is_some()
                && state.globals.background_effects_blur.get(),
            state.globals.kde_blur_manager.is_some(),
        );
        if state.background_appearance == projection.observed_appearance {
            return;
        }
        state.background_appearance = projection.observed_appearance;
        update_window(state);
        self.0.request_redraw();
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.borrow().background_appearance
    }

    fn observes_background_appearance(&self) -> bool {
        true
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        let client = self.borrow().client.get_client();
        let state = client.borrow();
        state
            .gpu_context
            .borrow()
            .as_ref()
            .is_some_and(|ctx| ctx.supports_dual_source_blending())
    }

    fn is_fullscreen(&self) -> bool {
        self.borrow().fullscreen
    }

    fn schedule_frame(&self) {
        self.0.schedule_frame();
    }

    fn native_frame_completed_command(&self) -> Option<PlatformWindowCommand> {
        Some(PlatformWindowCommand::CompleteNativeFrame)
    }

    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        let owner = Rc::downgrade(&self.1);
        let frame_loop = Rc::clone(&self.0.frame_loop);
        let scheduled_frame_at = Rc::clone(&self.0.scheduled_frame_at);
        let frame_ping = self.0.frame_ping.clone();
        Some(Rc::new(move || {
            let Some(_owner) = owner.upgrade() else {
                return;
            };
            WaylandWindowStatePtr::wake_frame_loop(&frame_loop, &scheduled_frame_at, &frame_ping);
        }))
    }

    fn request_frame(&self, options: RequestFrameOptions) {
        let executor = self.borrow().globals.executor.clone();
        let owner = Rc::downgrade(&self.1);
        let state = Rc::downgrade(&self.0.state);
        let callbacks = Rc::downgrade(&self.0.callbacks);
        let frame_loop = Rc::clone(&self.0.frame_loop);
        let frame_ping = self.0.frame_ping.clone();
        let scheduled_frame_at = Rc::clone(&self.0.scheduled_frame_at);
        let retry_deadline = Rc::clone(&self.0.retry_deadline);
        executor
            .spawn(async move {
                let Some(_owner) = owner.upgrade() else {
                    return;
                };
                let (Some(state), Some(callbacks)) = (state.upgrade(), callbacks.upgrade()) else {
                    return;
                };
                WaylandWindowStatePtr {
                    state,
                    callbacks,
                    frame_loop,
                    frame_ping,
                    scheduled_frame_at,
                    retry_deadline,
                    owner,
                }
                .dispatch_request_frame(options);
            })
            .detach();
    }

    fn on_request_frame(&self, callback: Box<dyn Fn(RequestFrameOptions)>) {
        self.0.callbacks.borrow_mut().request_frame = Some(Rc::from(callback));
    }

    fn on_input(&self, callback: PlatformInputCallback) {
        let input = self.0.callbacks.borrow().input.clone();
        input.set(callback);
    }

    fn on_active_status_change(
        &self,
        callback: Box<dyn Fn(PlatformWindowActiveStatusObservation)>,
    ) {
        self.0.callbacks.borrow_mut().active_status_change = Some(Rc::from(callback));
    }

    fn on_hover_status_change(&self, callback: Box<dyn Fn(bool)>) {
        self.0.callbacks.borrow_mut().hover_status_change = Some(Rc::from(callback));
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        if !self.0.interaction_is_quiesced() {
            self.0.callbacks.borrow_mut().visibility_change = Some(callback);
        }
    }

    fn on_resize(&self, callback: Box<dyn Fn(Size<Pixels>, f32)>) {
        self.0.callbacks.borrow_mut().resize = Some(Rc::from(callback));
    }

    fn on_moved(&self, callback: Box<dyn Fn()>) {
        self.0.callbacks.borrow_mut().moved = Some(Rc::from(callback));
    }

    fn on_display_changed(&self, callback: Box<dyn FnMut()>) {
        if !self.0.interaction_is_quiesced() {
            self.0.callbacks.borrow_mut().display_changed = Some(callback);
        }
    }

    fn on_window_state_change(&self, callback: Box<dyn Fn()>) {
        self.0.callbacks.borrow_mut().window_state_change = Some(Rc::from(callback));
    }

    fn on_window_mutation_observation(
        &self,
        callback: Box<dyn Fn(PlatformWindowMutationObservation)>,
    ) {
        self.0.callbacks.borrow_mut().window_mutation = Some(Rc::from(callback));
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        let should_close = self.0.callbacks.borrow().should_close.clone();
        should_close.set(callback);
    }

    fn on_close_required(&self, callback: Box<dyn FnOnce()>) {
        self.0.callbacks.borrow_mut().close_required = Some(callback);
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.0.callbacks.borrow_mut().close = Some(callback);
    }

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
    }

    fn on_appearance_changed(&self, callback: Box<dyn Fn()>) {
        self.0.callbacks.borrow_mut().appearance_changed = Some(Rc::from(callback));
    }

    fn on_button_layout_changed(&self, callback: Box<dyn Fn()>) {
        self.0.callbacks.borrow_mut().button_layout_changed = Some(Rc::from(callback));
    }

    fn draw(&self, scene: &Scene) -> PlatformWindowPresentOutcome {
        let mut state = self.borrow_mut();
        let frame_loop = self.0.frame_loop.get();
        if state.presentation_shutdown_blocks_surface() {
            self.0.stop_frame_loop();
            state.clear_presentation_bookkeeping();
            return PlatformWindowPresentOutcome::Deferred;
        }
        if !state.initial_map_committed
            || matches!(frame_loop, FrameLoop::Unconfigured | FrameLoop::Stopped)
        {
            return PlatformWindowPresentOutcome::Deferred;
        }
        let complete_after_draw = !matches!(
            frame_loop,
            FrameLoop::Ticking | FrameLoop::RescheduleRequested
        );

        if state.renderer.device_lost() {
            let raw_window = RawWindow {
                window: state.surface.id().as_ptr().cast::<std::ffi::c_void>(),
                display: state
                    .surface
                    .backend()
                    .upgrade()
                    .unwrap()
                    .display_ptr()
                    .cast::<std::ffi::c_void>(),
            };
            let outcome = match state.renderer.recover(&raw_window) {
                Ok(()) => PlatformWindowPresentOutcome::RepaintRequired,
                Err(err) => {
                    log::warn!("GPU recovery failed, will retry on next frame: {err}");
                    PlatformWindowPresentOutcome::Deferred
                }
            };
            state.redraw_requested = true;
            drop(state);
            if complete_after_draw {
                self.0.complete_frame();
            }
            return outcome;
        }

        // Surface state changed during this GPUI tick is included in this presentation.
        state.redraw_requested = false;
        if state.pending_frame_callback.is_none() {
            let callback = state.surface.frame(&state.globals.qh, state.surface.id());
            state.pending_frame_callback = Some(callback);
        }
        let outcome = state.renderer.draw(scene);
        if outcome == PlatformWindowPresentOutcome::Submitted {
            state.presentation = PresentationState::Presented;
            self.0.frame_loop.set(FrameLoop::AwaitingCallback);
            self.0.scheduled_frame_at.take();
            self.0.retry_deadline.take();
        } else {
            state.presentation = state.presentation.failed();
            self.0
                .frame_loop
                .set(frame_loop.presentation_failed(state.presentation));
            state.redraw_requested = outcome == PlatformWindowPresentOutcome::RepaintRequired;
        }
        drop(state);
        if complete_after_draw {
            self.0.complete_frame();
        }

        outcome
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        let state = self.borrow();
        state.renderer.sprite_atlas().clone()
    }

    fn set_exclusive_zone(&self, zone: Pixels) {
        let state = self.borrow();
        if state.can_mutate_surface()
            && state
                .surface_state
                .set_exclusive_zone(f32::from(zone) as i32)
        {
            state.commit_surface_properties();
        }
    }

    fn set_exclusive_edge(&self, edge: Anchor) {
        let state = self.borrow();
        if state.can_mutate_surface() && state.surface_state.set_exclusive_edge(edge) {
            state.commit_surface_properties();
        }
    }

    fn set_input_region(&self, region: Option<&[Bounds<Pixels>]>) {
        let mut state = self.borrow_mut();
        if !state.can_mutate_surface() {
            return;
        }
        state.input_region = region.map(|region| region.to_vec());
        state.apply_input_region();
    }

    fn window_decorations(&self) -> Decorations {
        let state = self.borrow();
        match state.decorations {
            WindowDecorations::Server => Decorations::Server,
            WindowDecorations::Client => Decorations::Client {
                tiling: state.tiling,
            },
        }
    }

    fn request_decorations(&self, decorations: WindowDecorations) {
        let mut state = self.borrow_mut();
        if !state.can_mutate_surface() {
            return;
        }
        match state.surface_state.decoration().as_ref() {
            Some(decoration) => {
                decoration.set_mode(decorations.to_xdg());
                state.decorations = decorations;
                update_window(state);
            }
            None => {
                if matches!(decorations, WindowDecorations::Server) {
                    log::info!(
                        "Server-side decorations requested, but the Wayland server does not support them. Falling back to client-side decorations."
                    );
                }
                state.decorations = WindowDecorations::Client;
                update_window(state);
            }
        }
        self.0.request_redraw();
    }

    fn window_controls(&self) -> WindowControls {
        self.borrow().window_controls
    }

    fn set_client_inset(&self, inset: Pixels) {
        let mut state = self.borrow_mut();
        if !state.can_mutate_surface() {
            return;
        }
        if Some(inset) != state.client_inset {
            state.client_inset = Some(inset);
            update_window(state);
            self.0.request_redraw();
        }
    }

    fn set_client_corner_radii(&self, radii: Corners<Pixels>) {
        let mut state = self.borrow_mut();
        if !state.can_mutate_surface() {
            return;
        }
        if radii != state.client_corner_radii {
            state.client_corner_radii = radii;
            update_window(state);
            self.0.request_redraw();
        }
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        let state = self.borrow();
        if state.interaction_gate.is_quiesced() {
            return;
        }
        state.client.update_ime_position(&self.0, bounds);
    }

    fn can_start_external_drag(&self) -> bool {
        let state = self.borrow();
        !state.interaction_gate.is_quiesced() && state.client.can_start_external_drag()
    }

    fn start_external_drag(
        &self,
        payload: &ExternalDragPayload,
        session: PlatformExternalDragSession,
    ) -> bool {
        let (client, surface) = {
            let state = self.borrow();
            if state.interaction_gate.is_quiesced() {
                return false;
            }
            (state.client.clone(), state.surface.clone())
        };
        client.start_external_drag(&surface, payload, session)
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.borrow().renderer.gpu_specs().into()
    }

    fn play_system_bell(&self) {
        let state = self.borrow();
        let surface = if state.surface_state.toplevel().is_some() {
            Some(&state.surface)
        } else {
            None
        };
        if let Some(bell) = state.globals.system_bell.as_ref() {
            bell.ring(surface);
        }
    }

    fn a11y_init(&self, callbacks: open_gpui::A11yCallbacks) {
        let interaction_gate = self.borrow().interaction_gate.clone();
        let adapter = accesskit_adapter(callbacks, interaction_gate);
        self.borrow_mut().accesskit_adapter = Some(adapter);
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        let mut state = self.borrow_mut();
        if let Some(adapter) = state.accesskit_adapter.as_mut() {
            adapter.update_if_active(|| tree_update);
        }
    }

    fn a11y_update_window_bounds(&self) {
        // Wayland doesn't expose window position, so this is a no-op
    }
}

fn add_rounded_rect(
    area: Bounds<i32>,
    radii: Corners<Pixels>,
    mut add_rectangle: impl FnMut(i32, i32, i32, i32),
) {
    if area.size.width <= 0 || area.size.height <= 0 {
        return;
    }

    let clamp_radius = |radius: Pixels| {
        radius
            .to_i32()
            .max(0)
            .min(area.size.width / 2)
            .min(area.size.height / 2)
    };
    let top_left = clamp_radius(radii.top_left);
    let top_right = clamp_radius(radii.top_right);
    let bottom_left = clamp_radius(radii.bottom_left);
    let bottom_right = clamp_radius(radii.bottom_right);

    let corner_inset = |radius: i32, row: i32| {
        if row >= radius {
            return 0;
        }
        let y = f64::from(radius) - (f64::from(row) + 0.5);
        let x = (f64::from(radius).powi(2) - y * y).sqrt();
        (f64::from(radius) - x - 0.5).ceil() as i32
    };

    for (left_radius, right_radius, bottom) in [
        (top_left, top_right, false),
        (bottom_left, bottom_right, true),
    ] {
        for row in 0..left_radius.max(right_radius) {
            let left_inset = corner_inset(left_radius, row);
            let right_inset = corner_inset(right_radius, row);
            let width = area.size.width - left_inset - right_inset;
            if width <= 0 {
                continue;
            }
            let y = if bottom {
                area.origin.y + area.size.height - row - 1
            } else {
                area.origin.y + row
            };
            add_rectangle(area.origin.x + left_inset, y, width, 1);
        }
    }

    let top_height = top_left.max(top_right);
    let bottom_height = bottom_left.max(bottom_right);
    let height = area.size.height - top_height - bottom_height;
    if height > 0 {
        add_rectangle(
            area.origin.x,
            area.origin.y + top_height,
            area.size.width,
            height,
        );
    }
}

#[cfg(test)]
#[path = "window_effect_tests.rs"]
mod window_effect_tests;

fn update_window(mut state: RefMut<WaylandWindowState>) {
    if !state.can_mutate_surface() {
        state.clear_presentation_bookkeeping();
        return;
    }

    let projection = WaylandWindowBackgroundProjection::new(
        state.requested_background_appearance,
        state.decorations,
        state.globals.background_effects_manager.is_some()
            && state.globals.background_effects_blur.get(),
        state.globals.kde_blur_manager.is_some(),
    );
    state.background_appearance = projection.observed_appearance;

    state
        .renderer
        .update_transparency(projection.renderer_transparent);
    let area = inset_by_tiling(
        state.bounds.map_origin(|_| px(0.0)),
        state.inset(),
        state.tiling,
    )
    .map(|value| value.to_i32())
    .map_size(|value| value.max(0));

    let region = state
        .globals
        .compositor
        .create_region(&state.globals.qh, ());

    // Client decorations include transparent shadows and antialiased corners,
    // so don't promise the compositor an opaque region.
    if projection.compositor_opaque_region {
        region.add(
            area.origin.x,
            area.origin.y,
            area.size.width,
            area.size.height,
        );

        // Promise the compositor that this region of the window surface
        // contains no transparent pixels. This allows the compositor to skip
        // updating whatever is behind the surface for better performance.
        state.surface.set_opaque_region(Some(&region));
    } else {
        state.surface.set_opaque_region(None);
    }

    // Prefer the ext-background-effect protocol, as it is DE agnostic and
    // replaces the deprecated kde-blur protocol, which is no longer supported
    // since KDE Plasma 6.7.
    // https://invent.kde.org/plasma/kwin/-/commit/7f9e0aa1cd2c2ab4fea1097800893b6f780089eb
    let background_effects_manager = state
        .globals
        .background_effects_manager
        .clone()
        .filter(|_| state.globals.background_effects_blur.get());
    if background_effects_manager.is_none() {
        if let Some(effect) = state.background_effects.take() {
            effect.destroy();
        }
    } else if let Some(blur) = state.kde_blur.take() {
        if let Some(manager) = &state.globals.kde_blur_manager {
            manager.unset(&state.surface);
        }
        blur.release();
    }
    if let Some(ref background_effects_manager) = background_effects_manager {
        if projection.blur_enabled {
            if state.background_effects.is_none() {
                log::debug!("Using ext-background-effect protocol");
                let background_effects = background_effects_manager.get_background_effect(
                    &state.surface,
                    &state.globals.qh,
                    (),
                );
                state.background_effects = Some(background_effects);
            }

            add_rounded_rect(area, state.client_corner_radii, |x, y, width, height| {
                region.add(x, y, width, height);
            });

            state
                .background_effects
                .as_ref()
                .unwrap()
                .set_blur_region(Some(&region));
        } else if let Some(background_effects) = state.background_effects.take() {
            background_effects.destroy();
        }
    } else if let Some(ref blur_manager) = state.globals.kde_blur_manager {
        if projection.blur_enabled {
            if state.kde_blur.is_none() {
                log::debug!("Using kde-blur protocol");
                let blur = blur_manager.create(&state.surface, &state.globals.qh, ());
                state.kde_blur = Some(blur);
            }
            state.kde_blur.as_ref().unwrap().commit();
        } else {
            blur_manager.unset(&state.surface);
            if let Some(b) = state.kde_blur.take() {
                b.release()
            }
        }
    }

    region.destroy();
}

pub(crate) trait WindowDecorationsExt {
    fn to_xdg(self) -> zxdg_toplevel_decoration_v1::Mode;
}

impl WindowDecorationsExt for WindowDecorations {
    fn to_xdg(self) -> zxdg_toplevel_decoration_v1::Mode {
        match self {
            WindowDecorations::Client => zxdg_toplevel_decoration_v1::Mode::ClientSide,
            WindowDecorations::Server => zxdg_toplevel_decoration_v1::Mode::ServerSide,
        }
    }
}

pub(crate) trait ResizeEdgeWaylandExt {
    fn to_xdg(self) -> xdg_toplevel::ResizeEdge;
}

impl ResizeEdgeWaylandExt for ResizeEdge {
    fn to_xdg(self) -> xdg_toplevel::ResizeEdge {
        match self {
            ResizeEdge::Top => xdg_toplevel::ResizeEdge::Top,
            ResizeEdge::TopRight => xdg_toplevel::ResizeEdge::TopRight,
            ResizeEdge::Right => xdg_toplevel::ResizeEdge::Right,
            ResizeEdge::BottomRight => xdg_toplevel::ResizeEdge::BottomRight,
            ResizeEdge::Bottom => xdg_toplevel::ResizeEdge::Bottom,
            ResizeEdge::BottomLeft => xdg_toplevel::ResizeEdge::BottomLeft,
            ResizeEdge::Left => xdg_toplevel::ResizeEdge::Left,
            ResizeEdge::TopLeft => xdg_toplevel::ResizeEdge::TopLeft,
        }
    }
}

/// The configuration event is in terms of the window geometry, which we are constantly
/// updating to account for the client decorations. But that's not the area we want to render
/// to, due to our intrusize CSD. So, here we calculate the 'actual' size, by adding back in the insets
fn compute_outer_size(
    inset: Pixels,
    new_size: Option<Size<Pixels>>,
    tiling: Tiling,
) -> Option<Size<Pixels>> {
    new_size.map(|mut new_size| {
        if !tiling.top {
            new_size.height += inset;
        }
        if !tiling.bottom {
            new_size.height += inset;
        }
        if !tiling.left {
            new_size.width += inset;
        }
        if !tiling.right {
            new_size.width += inset;
        }

        new_size
    })
}

fn inset_by_tiling(mut bounds: Bounds<Pixels>, inset: Pixels, tiling: Tiling) -> Bounds<Pixels> {
    if !tiling.top {
        bounds.origin.y += inset;
        bounds.size.height -= inset;
    }
    if !tiling.bottom {
        bounds.size.height -= inset;
    }
    if !tiling.left {
        bounds.origin.x += inset;
        bounds.size.width -= inset;
    }
    if !tiling.right {
        bounds.size.width -= inset;
    }

    bounds
}

#[cfg(test)]
mod creation_projection_tests {
    use super::*;
    use open_gpui::{layer_shell::LayerShellOptions, point};

    fn restore_bounds() -> Bounds<Pixels> {
        Bounds::new(point(px(24.0), px(36.0)), size(px(900.0), px(640.0)))
    }

    #[test]
    fn popup_parent_refuses_hidden_and_configured_surfaces_without_a_presented_buffer() {
        for (initial_map_committed, acknowledged_first_configure, presentation) in [
            (false, false, PresentationState::Unpresented),
            (false, true, PresentationState::Unpresented),
            (false, true, PresentationState::Presented),
            (true, false, PresentationState::Unpresented),
            (true, true, PresentationState::Unpresented),
            (true, true, PresentationState::RetryBeforeFirstPresent),
        ] {
            let error = ensure_popup_parent_mapped(
                initial_map_committed,
                acknowledged_first_configure,
                presentation,
            )
            .unwrap_err();
            assert_eq!(
                error.to_string(),
                "popup parent has not completed native mapping"
            );
        }
    }

    #[test]
    fn popup_parent_accepts_successful_native_mapping_including_a_later_render_retry() {
        for presentation in [
            PresentationState::Presented,
            PresentationState::RetryAfterPresent,
        ] {
            assert!(ensure_popup_parent_mapped(true, true, presentation).is_ok());
        }
    }

    #[test]
    fn popup_projection_keeps_surface_size_without_toplevel_creation_state() {
        let requested = restore_bounds();
        let kind = WindowKind::AnchoredPopup(super::super::popup::test_options());
        let projection =
            WaylandWindowCreationProjection::new(WindowBounds::Windowed(requested), &kind);
        assert_eq!(projection.role, WaylandCreationRole::Popup);
        assert_eq!(projection.bounds.origin, Point::default());
        assert_eq!(projection.bounds.size, requested.size);
        assert_eq!(projection.restore_bounds(), None);
        assert!(!projection.focus_on_appearing);
        assert!(!projection.activation_policy.accepts_activation);
        assert!(!projection.taskbar_visible);
    }

    #[test]
    fn input_regions_preserve_default_empty_and_rectangles_until_quiescence() {
        let gate = WindowInteractionGate::default();
        let rectangles = [restore_bounds()];
        assert_eq!(effective_input_region(&gate, None), None);
        assert_eq!(effective_input_region(&gate, Some(&[])), Some(&[][..]));
        assert_eq!(
            effective_input_region(&gate, Some(&rectangles)),
            Some(&rectangles[..]),
        );
        gate.quiesce();
        for requested in [None, Some(&[][..]), Some(&rectangles[..])] {
            assert_eq!(effective_input_region(&gate, requested), Some(&[][..]));
        }
    }

    fn creation_projection(
        window_bounds: WindowBounds,
        kind: &WindowKind,
    ) -> WaylandWindowCreationProjection {
        WaylandWindowCreationProjection::new(window_bounds, kind)
    }

    #[test]
    fn xdg_creation_projection_preserves_size_state_restore_and_alpha_surface() {
        let restore_bounds = restore_bounds();
        let local_restore_bounds = Bounds::new(Point::default(), restore_bounds.size);
        let cases = [
            (
                WindowBounds::Windowed(restore_bounds),
                WaylandInitialToplevelState::Windowed,
            ),
            (
                WindowBounds::Maximized(restore_bounds),
                WaylandInitialToplevelState::Maximized,
            ),
            (
                WindowBounds::Fullscreen(restore_bounds),
                WaylandInitialToplevelState::Fullscreen,
            ),
        ];

        for (window_bounds, expected_state) in cases {
            let projection = creation_projection(window_bounds, &WindowKind::Normal);
            assert_eq!(projection.bounds, local_restore_bounds);
            assert_eq!(projection.restore_bounds(), Some(local_restore_bounds));
            assert_eq!(
                projection.role,
                WaylandCreationRole::Xdg {
                    initial_state: expected_state,
                    restore_bounds: local_restore_bounds,
                }
            );
            assert!(projection.alpha_surface);
            assert!(projection.focus_on_appearing);
            assert_eq!(
                projection.activation_policy,
                WindowActivationPolicy::default()
            );
            assert!(!projection.topmost);
            assert!(projection.taskbar_visible);
        }

        let dialog =
            creation_projection(WindowBounds::Maximized(restore_bounds), &WindowKind::Dialog);
        assert!(!dialog.topmost);
        assert!(!dialog.taskbar_visible);
    }

    #[test]
    fn layer_shell_projection_uses_size_and_alpha_without_xdg_state_or_restore() {
        let restore_bounds = restore_bounds();
        let projection = creation_projection(
            WindowBounds::Fullscreen(restore_bounds),
            &WindowKind::LayerShell(LayerShellOptions::default()),
        );

        assert_eq!(
            projection.bounds,
            Bounds::new(Point::default(), restore_bounds.size)
        );
        assert_eq!(projection.role, WaylandCreationRole::LayerShell);
        assert_eq!(projection.restore_bounds(), None);
        assert!(projection.alpha_surface);
        assert!(!projection.focus_on_appearing);
        assert!(projection.activation_policy.focus_on_click);
        assert!(!projection.activation_policy.accepts_activation);
        assert!(projection.topmost);
        assert!(!projection.taskbar_visible);

        let mut background_options = LayerShellOptions::default();
        background_options.layer = open_gpui::layer_shell::Layer::Background;
        background_options.keyboard_interactivity =
            open_gpui::layer_shell::KeyboardInteractivity::None;
        let background = creation_projection(
            WindowBounds::Windowed(restore_bounds),
            &WindowKind::LayerShell(background_options),
        );
        assert!(!background.focus_on_appearing);
        assert!(!background.activation_policy.focus_on_click);
        assert!(!background.topmost);
        assert!(!background.taskbar_visible);

        let mut exclusive_options = LayerShellOptions::default();
        exclusive_options.keyboard_interactivity =
            open_gpui::layer_shell::KeyboardInteractivity::Exclusive;
        let exclusive = creation_projection(
            WindowBounds::Windowed(restore_bounds),
            &WindowKind::LayerShell(exclusive_options),
        );
        assert!(exclusive.focus_on_appearing);
        assert!(exclusive.activation_policy.focus_on_click);
    }

    #[test]
    fn background_projection_matches_renderer_region_and_blur_creation_inputs() {
        let opaque = WaylandWindowBackgroundProjection::new(
            WindowBackgroundAppearance::Opaque,
            WindowDecorations::Server,
            true,
            false,
        );
        assert_eq!(
            opaque.observed_appearance,
            WindowBackgroundAppearance::Opaque
        );
        assert!(!opaque.renderer_transparent);
        assert!(opaque.compositor_opaque_region);
        assert!(!opaque.blur_enabled);

        let client_decorated = WaylandWindowBackgroundProjection::new(
            WindowBackgroundAppearance::Opaque,
            WindowDecorations::Client,
            true,
            false,
        );
        assert!(client_decorated.renderer_transparent);
        assert!(!client_decorated.compositor_opaque_region);

        let transparent = WaylandWindowBackgroundProjection::new(
            WindowBackgroundAppearance::Transparent,
            WindowDecorations::Server,
            true,
            false,
        );
        assert!(transparent.renderer_transparent);
        assert!(!transparent.compositor_opaque_region);
        assert!(!transparent.blur_enabled);

        let blurred = WaylandWindowBackgroundProjection::new(
            WindowBackgroundAppearance::Blurred,
            WindowDecorations::Server,
            true,
            false,
        );
        assert_eq!(
            blurred.observed_appearance,
            WindowBackgroundAppearance::Blurred
        );
        assert!(blurred.renderer_transparent);
        assert!(!blurred.compositor_opaque_region);
        assert!(blurred.blur_enabled);

        let blur_adjusted = WaylandWindowBackgroundProjection::new(
            WindowBackgroundAppearance::Blurred,
            WindowDecorations::Server,
            false,
            false,
        );
        assert_eq!(
            blur_adjusted.observed_appearance,
            WindowBackgroundAppearance::Transparent
        );
        assert!(blur_adjusted.renderer_transparent);
        assert!(!blur_adjusted.blur_enabled);

        for requested in [
            WindowBackgroundAppearance::MicaBackdrop,
            WindowBackgroundAppearance::MicaAltBackdrop,
        ] {
            let adjusted = WaylandWindowBackgroundProjection::new(
                requested,
                WindowDecorations::Server,
                true,
                false,
            );
            assert_eq!(
                adjusted.observed_appearance,
                WindowBackgroundAppearance::Transparent
            );
            assert!(adjusted.renderer_transparent);
            assert!(!adjusted.blur_enabled);
        }
    }
}
