use std::{
    cell::{Ref, RefCell, RefMut},
    ffi::c_void,
    ptr::NonNull,
    rc::{Rc, Weak},
    sync::Arc,
};

use futures::channel::oneshot::Receiver;
use open_gpui_collections::HashMap;

use raw_window_handle as rwh;
use wayland_backend::client::ObjectId;
use wayland_client::WEnum;
use wayland_client::{
    Proxy,
    protocol::{wl_output, wl_surface},
};
use wayland_protocols::wp::viewporter::client::wp_viewport;
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1;
use wayland_protocols::xdg::shell::client::xdg_surface;
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self};
use wayland_protocols::{
    wp::fractional_scale::v1::client::wp_fractional_scale_v1,
    xdg::dialog::v1::client::xdg_dialog_v1::XdgDialogV1,
};
use wayland_protocols_plasma::blur::client::org_kde_kwin_blur;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1;

use crate::linux::wayland::{display::WaylandDisplay, serial::SerialKind};
use crate::linux::{
    Globals, Output, WaylandClientStatePtr, should_close_callback::ShouldCloseCallbackSlot,
};
use open_gpui::{
    AnyWindowHandle, Bounds, Capslock, CursorStyle, Decorations, DevicePixels, GpuSpecs, Modifiers,
    NativeInputHandlerOutcome, Pixels, PlatformAtlas, PlatformDisplay, PlatformInput,
    PlatformInputCallback, PlatformInputCallbackSlot, PlatformInputHandler,
    PlatformInputHandlerSlot, PlatformPresentationShutdownOutcome, PlatformWindow,
    PlatformWindowCommand, PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome,
    PlatformWindowPresentOutcome, Point, PreparedPlatformPresentationShutdown, PromptButton,
    PromptLevel, RequestFrameOptions, ResizeEdge, Scene, Size, Tiling, WindowActivationPolicy,
    WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowControls,
    WindowCreationFacts, WindowDecorations, WindowKind, WindowParams,
    WindowPresentationShutdownTicket, layer_shell::LayerShellNotSupportedError, px, size,
};
use open_gpui_wgpu::{
    CompositorGpuHint, WgpuRenderer, WgpuSurfaceConfig, WgpuSurfaceShutdownProgress, wgpu,
};

#[derive(Default)]
pub(crate) struct Callbacks {
    request_frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    input: PlatformInputCallbackSlot,
    active_status_change: Option<Box<dyn FnMut(bool)>>,
    hover_status_change: Option<Box<dyn FnMut(bool)>>,
    resize: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    moved: Option<Box<dyn FnMut()>>,
    window_state_change: Option<Box<dyn FnMut()>>,
    should_close: ShouldCloseCallbackSlot,
    close: Option<Box<dyn FnOnce()>>,
    appearance_changed: Option<Box<dyn FnMut()>>,
    button_layout_changed: Option<Box<dyn FnMut()>>,
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
            WaylandCreationRole::LayerShell => None,
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
        blur_supported: bool,
    ) -> Self {
        let observed_appearance = match requested {
            WindowBackgroundAppearance::Opaque | WindowBackgroundAppearance::Transparent => {
                requested
            }
            WindowBackgroundAppearance::Blurred if blur_supported => {
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
    tiling: Tiling,
}

pub struct WaylandWindowState {
    surface_state: WaylandSurfaceState,
    acknowledged_first_configure: bool,
    pub surface: wl_surface::WlSurface,
    app_id: Option<String>,
    appearance: WindowAppearance,
    blur: Option<org_kde_kwin_blur::OrgKdeKwinBlur>,
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
    background_appearance: WindowBackgroundAppearance,
    fullscreen: bool,
    maximized: bool,
    tiling: Tiling,
    window_bounds: Bounds<Pixels>,
    client: WaylandClientStatePtr,
    handle: AnyWindowHandle,
    active: bool,
    hovered: bool,
    cursor_style: CursorStyle,
    renderer_presented: bool,
    has_presented_frame: bool,
    in_progress_configure: Option<InProgressConfigure>,
    resize_throttle: bool,
    in_progress_window_controls: Option<WindowControls>,
    window_controls: WindowControls,
    client_inset: Option<Pixels>,
    accesskit_adapter: Option<accesskit_unix::Adapter>,
    creation: WaylandWindowCreationProjection,
    initially_shown: bool,
    initial_map_committed: bool,
    initial_presentation_completed: bool,
    transient_for: Option<AnyWindowHandle>,
}

#[derive(Clone)]
pub(crate) struct WaylandTransientOwner {
    pub(crate) handle: AnyWindowHandle,
    pub(crate) toplevel: xdg_toplevel::XdgToplevel,
}

pub enum WaylandSurfaceState {
    Xdg(WaylandXdgSurfaceState),
    LayerShell(WaylandLayerSurfaceState),
}

impl WaylandSurfaceState {
    fn new(
        surface: &wl_surface::WlSurface,
        globals: &Globals,
        params: &WindowParams,
        creation: &WaylandWindowCreationProjection,
        transient_owner: Option<&WaylandTransientOwner>,
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
                layer_surface
                    .set_exclusive_edge(super::layer_shell::wayland_anchor(exclusive_edge));
            }

            return Ok(WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState {
                layer_surface,
            }));
        }

        // All other WindowKinds result in a regular xdg surface
        let xdg_surface = globals
            .wm_base
            .get_xdg_surface(&surface, &globals.qh, surface.id());

        let toplevel = xdg_surface.get_toplevel(&globals.qh, surface.id());
        if let Some(transient_owner) = transient_owner {
            toplevel.set_parent(Some(&transient_owner.toplevel));
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
}

impl WaylandSurfaceState {
    fn ack_configure(&self, serial: u32) {
        match self {
            WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { xdg_surface, .. }) => {
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
            WaylandSurfaceState::Xdg(WaylandXdgSurfaceState { xdg_surface, .. }) => {
                xdg_surface.set_window_geometry(x, y, width, height);
            }
            WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState { layer_surface, .. }) => {
                // cannot set window position of a layer surface
                layer_surface.set_size(width as u32, height as u32);
            }
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
            WaylandSurfaceState::LayerShell(WaylandLayerSurfaceState { layer_surface }) => {
                layer_surface.destroy();
            }
        }
    }
}

#[derive(Clone)]
pub struct WaylandWindowStatePtr {
    state: Rc<RefCell<WaylandWindowState>>,
    callbacks: Rc<RefCell<Callbacks>>,
}

struct WaylandWindowCommandTarget {
    owner: Weak<()>,
    state: Weak<RefCell<WaylandWindowState>>,
}

impl WaylandWindowCommandTarget {
    fn new(window: &WaylandWindow) -> Self {
        Self {
            owner: Rc::downgrade(&window.1),
            state: Rc::downgrade(&window.0.state),
        }
    }

    fn dispatch(&self, command: PlatformWindowCommand) -> PlatformWindowCommandOutcome {
        let Some(_owner) = self.owner.upgrade() else {
            return PlatformWindowCommandOutcome::Rejected;
        };
        let Some(state) = self.state.upgrade() else {
            return PlatformWindowCommandOutcome::Rejected;
        };
        let mut state = state.borrow_mut();

        match command {
            PlatformWindowCommand::CompleteInitialPresentation { activate } => {
                if state.initial_presentation_completed {
                    return PlatformWindowCommandOutcome::Accepted;
                }
                state.initial_presentation_completed = true;
                if state.initial_map_committed
                    && activate
                    && state.creation.activation_policy.accepts_activation
                {
                    let _ = activate_wayland_window(&state);
                }
                PlatformWindowCommandOutcome::Accepted
            }
            PlatformWindowCommand::RevealDeferredInitialPresentation { .. } => {
                PlatformWindowCommandOutcome::Rejected
            }
            PlatformWindowCommand::Activate
                if state.creation.activation_policy.accepts_activation =>
            {
                wayland_command_outcome(activate_wayland_window(&state))
            }
            PlatformWindowCommand::Activate => PlatformWindowCommandOutcome::Rejected,
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

fn activate_wayland_window(state: &WaylandWindowState) -> bool {
    // Try to request an activation token. Even though the activation is likely going to be
    // rejected, KWin and Mutter can use the app_id to indicate that attention was requested.
    if let Some(activation) = &state.globals.activation {
        let token = activation.get_activation_token(&state.globals.qh, ());
        state
            .client
            .set_pending_window_activation(token.id(), state.surface.id(), state.handle);
        let serial = state.client.get_serial(SerialKind::MousePress);
        if let Some(app_id) = state.app_id.clone() {
            token.set_app_id(app_id);
        }
        token.set_serial(serial, &state.globals.seat);
        token.set_surface(&state.surface);
        token.commit();
        true
    } else {
        false
    }
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
    pub(crate) fn new(
        handle: AnyWindowHandle,
        surface: wl_surface::WlSurface,
        surface_state: WaylandSurfaceState,
        appearance: WindowAppearance,
        viewport: Option<wp_viewport::WpViewport>,
        client: WaylandClientStatePtr,
        globals: Globals,
        gpu_context: open_gpui_wgpu::GpuContext,
        compositor_gpu: Option<CompositorGpuHint>,
        options: WindowParams,
        creation: WaylandWindowCreationProjection,
        transient_for: Option<AnyWindowHandle>,
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
            WgpuRenderer::new(gpu_context, &raw_window, config, compositor_gpu)?
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
            surface_state,
            acknowledged_first_configure: false,
            surface,
            app_id: None,
            blur: None,
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
            fullscreen: false,
            maximized: false,
            tiling: Tiling::default(),
            window_bounds: creation.restore_bounds().unwrap_or(creation.bounds),
            in_progress_configure: None,
            resize_throttle: false,
            client,
            appearance,
            handle,
            active: false,
            hovered: false,
            cursor_style: CursorStyle::Arrow,
            renderer_presented: false,
            has_presented_frame: false,
            in_progress_window_controls: None,
            window_controls: WindowControls::default(),
            client_inset: None,
            accesskit_adapter: None,
            creation,
            initially_shown,
            initial_map_committed: false,
            initial_presentation_completed: false,
            transient_for,
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
        self.renderer_presented = false;
        self.resize_throttle = false;
    }

    pub fn is_transparent(&self) -> bool {
        WaylandWindowBackgroundProjection::new(
            self.background_appearance,
            self.decorations,
            self.globals.blur_manager.is_some(),
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
        let mut current_output = self.display.take();
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

    pub fn inset(&self) -> Pixels {
        match self.decorations {
            WindowDecorations::Server => px(0.0),
            WindowDecorations::Client => self.client_inset.unwrap_or(px(0.0)),
        }
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
        self.0.terminate_callback_slots();

        let can_release_surface = self
            .0
            .state
            .borrow()
            .renderer
            .surface_owner_release_is_safe();
        if !can_release_surface {
            log::error!(
                "Wayland window dropped before exact presentation shutdown drained submitted GPU work; retaining backend owner"
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
        if let Some(blur) = &state.blur {
            blur.release();
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
                state_ptr.close();
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
        transient_owner: Option<WaylandTransientOwner>,
        target_output: Option<wl_output::WlOutput>,
    ) -> anyhow::Result<(Self, ObjectId)> {
        let creation = WaylandWindowCreationProjection::new(params.window_bounds, &params.kind);
        let transient_for = transient_owner.as_ref().map(|owner| owner.handle);
        let surface = globals.compositor.create_surface(&globals.qh, ());
        let surface_state = WaylandSurfaceState::new(
            &surface,
            &globals,
            &params,
            &creation,
            transient_owner.as_ref(),
            target_output,
        )?;

        if let Some(fractional_scale_manager) = globals.fractional_scale_manager.as_ref() {
            fractional_scale_manager.get_fractional_scale(&surface, &globals.qh, surface.id());
        }

        let viewport = globals
            .viewporter
            .as_ref()
            .map(|viewporter| viewporter.get_viewport(&surface, &globals.qh, ()));

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
                )?)),
                callbacks: Rc::new(RefCell::new(Callbacks::default())),
            },
            Rc::new(()),
        );

        Ok((this, surface.id()))
    }
}

impl WaylandWindowStatePtr {
    fn terminate_callback_slots(&self) {
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

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }

    pub fn cursor_style(&self) -> CursorStyle {
        self.state.borrow().cursor_style
    }

    pub fn set_cursor_style(&self, style: CursorStyle) {
        self.state.borrow_mut().cursor_style = style;
    }

    pub fn frame(&self) {
        let mut state = self.state.borrow_mut();
        if state.presentation_shutdown_blocks_surface() {
            state.clear_presentation_bookkeeping();
            return;
        }

        state.surface.frame(&state.globals.qh, state.surface.id());
        state.resize_throttle = false;
        drop(state);

        let callback = self.callbacks.borrow_mut().request_frame.take();
        if let Some(mut callback) = callback {
            // Native callbacks may re-enter GPUI, so keep callback-table borrows outside the call.
            callback(RequestFrameOptions {
                force_render: false,
                ..Default::default()
            });
            let mut callbacks = self.callbacks.borrow_mut();
            if callbacks.request_frame.is_none() {
                callbacks.request_frame = Some(callback);
            }
            drop(callbacks);
            self.update_ime_enabled();
        }
    }

    fn update_ime_enabled(&self) {
        let (client, input_handler) = {
            let state = self.state.borrow();
            (state.client.clone(), state.input_handler.clone())
        };
        let ime_enabled = input_handler
            .with_handler(|input_handler| input_handler.query_accepts_text_input())
            .and_then(NativeInputHandlerOutcome::into_delivered)
            .unwrap_or(false);
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
        let callback = self.callbacks.borrow_mut().window_state_change.take();
        if let Some(mut callback) = callback {
            callback();
            self.callbacks.borrow_mut().window_state_change = Some(callback);
        }
    }

    pub fn handle_xdg_surface_event(&self, event: xdg_surface::Event) {
        if let xdg_surface::Event::Configure { serial } = event {
            {
                let mut state = self.state.borrow_mut();
                if let Some(window_controls) = state.in_progress_window_controls.take() {
                    state.window_controls = window_controls;

                    drop(state);
                    let mut callbacks = self.callbacks.borrow_mut();
                    if let Some(appearance_changed) = callbacks.appearance_changed.as_mut() {
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
                    // Limit interactive resizes to once per vblank
                    if configure.resizing && state.resize_throttle {
                        state.surface_state.ack_configure(serial);
                        drop(state);
                        if state_changed {
                            self.emit_window_state_change();
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
                    if let Some(size) = configure.size {
                        self.resize(size);
                    }
                }
            }
            let mut state = self.state.borrow_mut();
            state.surface_state.ack_configure(serial);

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

            let request_frame_callback = !state.acknowledged_first_configure;
            if request_frame_callback {
                state.acknowledged_first_configure = true;
                drop(state);
                self.frame();
            }
        }
    }

    pub fn handle_toplevel_decoration_event(&self, event: zxdg_toplevel_decoration_v1::Event) {
        if let zxdg_toplevel_decoration_v1::Event::Configure { mode } = event {
            match mode {
                WEnum::Value(zxdg_toplevel_decoration_v1::Mode::ServerSide) => {
                    self.state.borrow_mut().decorations = WindowDecorations::Server;
                    let callback = self.callbacks.borrow_mut().appearance_changed.take();
                    if let Some(mut fun) = callback {
                        fun();
                        self.callbacks.borrow_mut().appearance_changed = Some(fun);
                    }
                }
                WEnum::Value(zxdg_toplevel_decoration_v1::Mode::ClientSide) => {
                    self.state.borrow_mut().decorations = WindowDecorations::Client;
                    // Update background to be transparent
                    let callback = self.callbacks.borrow_mut().appearance_changed.take();
                    if let Some(mut fun) = callback {
                        fun();
                        self.callbacks.borrow_mut().appearance_changed = Some(fun);
                    }
                }
                WEnum::Value(_) => {
                    log::warn!("Unknown decoration mode");
                }
                WEnum::Unknown(v) => {
                    log::warn!("Unknown decoration mode: {}", v);
                }
            }
        }
    }

    pub fn handle_fractional_scale_event(&self, event: wp_fractional_scale_v1::Event) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            self.rescale(scale as f32 / 120.0);
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

                for state in states {
                    match state {
                        xdg_toplevel::State::Maximized => {
                            maximized = true;
                        }
                        xdg_toplevel::State::Fullscreen => {
                            fullscreen = true;
                        }
                        xdg_toplevel::State::Resizing => resizing = true,
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
                    tiling,
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
                    tiling: Tiling::default(),
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

                let scale = state.primary_output_scale();
                state.update_subpixel_layout();

                // We use `PreferredBufferScale` instead to set the scale if it's available
                if state.surface.version() < wl_surface::EVT_PREFERRED_BUFFER_SCALE_SINCE {
                    state.surface.set_buffer_scale(scale);
                    drop(state);
                    self.rescale(scale as f32);
                }
            }
            wl_surface::Event::Leave { output } => {
                state.outputs.remove(&output.id());

                let scale = state.primary_output_scale();
                state.update_subpixel_layout();

                // We use `PreferredBufferScale` instead to set the scale if it's available
                if state.surface.version() < wl_surface::EVT_PREFERRED_BUFFER_SCALE_SINCE {
                    state.surface.set_buffer_scale(scale);
                    drop(state);
                    self.rescale(scale as f32);
                }
            }
            wl_surface::Event::PreferredBufferScale { factor } => {
                // We use `WpFractionalScale` instead to set the scale if it's available
                if state.globals.fractional_scale_manager.is_none() {
                    state.surface.set_buffer_scale(factor);
                    drop(state);
                    self.rescale(factor as f32);
                }
            }
            _ => {}
        }
    }

    pub fn handle_ime(&self, ime: ImeInput) {
        let input_handler = self.state.borrow().input_handler.clone();
        let _ = input_handler.with_handler(|input_handler| match ime {
            ImeInput::InsertText(text) => input_handler.replace_text_in_range(None, &text),
            ImeInput::SetMarkedText(text) => {
                input_handler.replace_and_mark_text_in_range(None, &text, None)
            }
            ImeInput::UnmarkText => input_handler.unmark_text(),
            ImeInput::DeleteText => match input_handler.marked_text_range() {
                NativeInputHandlerOutcome::Delivered(Some(marked)) => {
                    input_handler.replace_text_in_range(Some(marked), "")
                }
                NativeInputHandlerOutcome::Delivered(None) => {
                    NativeInputHandlerOutcome::Delivered(())
                }
                NativeInputHandlerOutcome::StaleWindow => NativeInputHandlerOutcome::StaleWindow,
                NativeInputHandlerOutcome::Quitting => NativeInputHandlerOutcome::Quitting,
            },
        });
    }

    pub fn get_ime_area(&self) -> Option<Bounds<Pixels>> {
        let input_handler = self.state.borrow().input_handler.clone();
        input_handler
            .with_handler(|input_handler| input_handler.ime_candidate_bounds())
            .and_then(NativeInputHandlerOutcome::into_delivered)
            .flatten()
    }

    pub fn set_size_and_scale(&self, size: Option<Size<Pixels>>, scale: Option<f32>) {
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

        let callback = self.callbacks.borrow_mut().resize.take();
        if let Some(mut fun) = callback {
            fun(size, scale);
            self.callbacks.borrow_mut().resize = Some(fun);
        }

        {
            let state = self.state.borrow();
            if let Some(viewport) = &state.viewport {
                viewport
                    .set_destination(f32::from(size.width) as i32, f32::from(size.height) as i32);
            }
        }
    }

    pub fn resize(&self, size: Size<Pixels>) {
        self.set_size_and_scale(Some(size), None);
    }

    pub fn rescale(&self, scale: f32) {
        self.set_size_and_scale(None, Some(scale));
    }

    pub fn close(&self) {
        self.terminate_callback_slots();
        let callback = self.callbacks.borrow_mut().close.take();
        if let Some(fun) = callback {
            fun()
        }
    }

    pub fn handle_input(&self, input: PlatformInput) {
        let input_callback = self.callbacks.borrow().input.clone();
        if input_callback
            .dispatch(input.clone())
            .is_some_and(|result| !result.propagate)
        {
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

    pub fn set_focused(&self, focus: bool) {
        self.state.borrow_mut().active = focus;
        let callback = self.callbacks.borrow_mut().active_status_change.take();
        if let Some(mut fun) = callback {
            fun(focus);
            self.callbacks.borrow_mut().active_status_change = Some(fun);
        }
        if let Some(adapter) = self.state.borrow_mut().accesskit_adapter.as_mut() {
            adapter.update_window_focus_state(focus);
        }
    }

    pub fn set_hovered(&self, focus: bool) {
        self.state.borrow_mut().hovered = focus;
        let callback = self.callbacks.borrow_mut().hover_status_change.take();
        if let Some(mut fun) = callback {
            fun(focus);
            self.callbacks.borrow_mut().hover_status_change = Some(fun);
        }
    }

    pub fn set_appearance(&mut self, appearance: WindowAppearance) {
        self.state.borrow_mut().appearance = appearance;

        let callback = self.callbacks.borrow_mut().appearance_changed.take();
        if let Some(mut fun) = callback {
            fun();
            self.callbacks.borrow_mut().appearance_changed = Some(fun);
        }
    }

    pub fn set_button_layout(&self) {
        let callback = self.callbacks.borrow_mut().button_layout_changed.take();
        if let Some(mut fun) = callback {
            fun();
            self.callbacks.borrow_mut().button_layout_changed = Some(fun);
        }
    }

    pub fn primary_output_scale(&self) -> i32 {
        self.state.borrow_mut().primary_output_scale()
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

    fn prepare_presentation_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        let state = Rc::clone(&self.0.state);
        PreparedPlatformPresentationShutdown::new(shutdown, move |shutdown| {
            let Ok(mut state) = state.try_borrow_mut() else {
                return PlatformPresentationShutdownOutcome::Rejected;
            };
            if shutdown.snapshot().window_id() != state.handle.window_id() {
                return PlatformPresentationShutdownOutcome::Rejected;
            }

            match state.renderer.quiesce_surface(shutdown) {
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
            }
        })
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.borrow().bounds
    }

    fn map_window(&mut self) -> anyhow::Result<()> {
        let mut state = self.borrow_mut();
        if !state.initially_shown || state.initial_map_committed {
            return Ok(());
        }
        if state.presentation_shutdown_blocks_surface() {
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
        state.initially_shown && state.has_presented_frame
    }

    fn platform_facts(&self) -> open_gpui::WindowPlatformFacts {
        let state = self.borrow();
        let window_bounds = if state.fullscreen {
            WindowBounds::Fullscreen(state.window_bounds)
        } else if state.maximized {
            WindowBounds::Maximized(state.window_bounds)
        } else {
            WindowBounds::Windowed(state.bounds)
        };
        let inner_window_bounds = if state.fullscreen {
            WindowBounds::Fullscreen(state.window_bounds)
        } else if state.maximized {
            WindowBounds::Maximized(state.window_bounds)
        } else {
            WindowBounds::Windowed(state.bounds.inset(state.inset()))
        };
        let display_id = state
            .display
            .as_ref()
            .map(|(id, _)| open_gpui::DisplayId::from(id.protocol_id() as u64));

        open_gpui::WindowPlatformFacts {
            bounds: state.bounds,
            coordinate_space: open_gpui::WindowCoordinateSpace::WindowLocal,
            window_bounds,
            inner_window_bounds,
            content_size: state.bounds.size,
            scale_factor: state.scale,
            display_id,
            is_minimized: false,
            is_maximized: state.maximized,
            is_fullscreen: state.fullscreen,
            accepts_pointer_input: true,
            accepts_activation: state.creation.activation_policy.accepts_activation,
            focus_on_click: state.creation.activation_policy.focus_on_click,
            background_appearance: state.background_appearance,
            topmost: state.creation.topmost,
            taskbar_visible: state.creation.taskbar_visible,
            is_active: state.active,
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
        state.display.as_ref().map(|(id, display)| {
            Rc::new(WaylandDisplay {
                id: id.clone(),
                name: display.name.clone(),
                bounds: display.bounds.to_pixels(state.scale),
            }) as Rc<dyn PlatformDisplay>
        })
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

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<Receiver<usize>> {
        None
    }

    fn is_active(&self) -> bool {
        self.borrow().active
    }

    fn is_hovered(&self) -> bool {
        self.borrow().hovered
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
        let projection = WaylandWindowBackgroundProjection::new(
            background_appearance,
            state.decorations,
            state.globals.blur_manager.is_some(),
        );
        state.background_appearance = projection.observed_appearance;
        update_window(state);
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.borrow().background_appearance
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

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.0.callbacks.borrow_mut().request_frame = Some(callback);
    }

    fn on_input(&self, callback: PlatformInputCallback) {
        let input = self.0.callbacks.borrow().input.clone();
        input.set(callback);
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.callbacks.borrow_mut().active_status_change = Some(callback);
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.callbacks.borrow_mut().hover_status_change = Some(callback);
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.callbacks.borrow_mut().resize = Some(callback);
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().moved = Some(callback);
    }

    fn on_window_state_change(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().window_state_change = Some(callback);
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        let should_close = self.0.callbacks.borrow().should_close.clone();
        should_close.set(callback);
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.0.callbacks.borrow_mut().close = Some(callback);
    }

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().appearance_changed = Some(callback);
    }

    fn on_button_layout_changed(&self, callback: Box<dyn FnMut()>) {
        self.0.callbacks.borrow_mut().button_layout_changed = Some(callback);
    }

    fn draw(&self, scene: &Scene) -> PlatformWindowPresentOutcome {
        let mut state = self.borrow_mut();
        if state.presentation_shutdown_blocks_surface() {
            state.clear_presentation_bookkeeping();
            return PlatformWindowPresentOutcome::Deferred;
        }
        if !state.initial_map_committed {
            return PlatformWindowPresentOutcome::Deferred;
        }

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
            return match state.renderer.recover(&raw_window) {
                Ok(()) => PlatformWindowPresentOutcome::RepaintRequired,
                Err(err) => {
                    log::warn!("GPU recovery failed, will retry on next frame: {err}");
                    PlatformWindowPresentOutcome::Deferred
                }
            };
        }

        let outcome = state.renderer.draw(scene);
        state.renderer_presented = outcome == PlatformWindowPresentOutcome::Submitted;
        if state.renderer_presented {
            state.has_presented_frame = true;
        }

        outcome
    }

    fn completed_frame(&self) {
        let mut state = self.borrow_mut();
        if state.presentation_shutdown_blocks_surface() {
            state.clear_presentation_bookkeeping();
            return;
        }
        if !state.initial_map_committed {
            return;
        }

        // Work around a bug in old versions of wlroots where committing without a buffer attached
        // can cause invalid synchronization that leads to graphical corruption.
        if !state.renderer_presented {
            state.surface.commit();
        }

        state.renderer_presented = false;
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        let state = self.borrow();
        state.renderer.sprite_atlas().clone()
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
    }

    fn window_controls(&self) -> WindowControls {
        self.borrow().window_controls
    }

    fn set_client_inset(&self, inset: Pixels) {
        let mut state = self.borrow_mut();
        if Some(inset) != state.client_inset {
            state.client_inset = Some(inset);
            update_window(state);
        }
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        let state = self.borrow();
        state.client.update_ime_position(bounds);
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
        let activation_handler = TrivialActivationHandler {
            callback: callbacks.activation,
        };
        let action_handler = TrivialActionHandler(callbacks.action);
        let deactivation_handler = TrivialDeactivationHandler {
            callback: callbacks.deactivation,
        };

        let adapter =
            accesskit_unix::Adapter::new(activation_handler, action_handler, deactivation_handler);

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

struct TrivialActivationHandler {
    callback: Box<dyn Fn() -> Option<accesskit::TreeUpdate> + Send + 'static>,
}

impl accesskit::ActivationHandler for TrivialActivationHandler {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        (self.callback)()
    }
}

struct TrivialActionHandler(Box<dyn Fn(accesskit::ActionRequest) + Send + 'static>);

impl accesskit::ActionHandler for TrivialActionHandler {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        (self.0)(request);
    }
}

struct TrivialDeactivationHandler {
    callback: Box<dyn Fn() + Send + 'static>,
}

impl accesskit::DeactivationHandler for TrivialDeactivationHandler {
    fn deactivate_accessibility(&mut self) {
        (self.callback)();
    }
}

fn update_window(mut state: RefMut<WaylandWindowState>) {
    if state.presentation_shutdown_blocks_surface() {
        state.clear_presentation_bookkeeping();
        return;
    }

    let projection = WaylandWindowBackgroundProjection::new(
        state.background_appearance,
        state.decorations,
        state.globals.blur_manager.is_some(),
    );

    state
        .renderer
        .update_transparency(projection.renderer_transparent);
    let opaque_area = state.window_bounds.map(|v| f32::from(v) as i32);
    opaque_area.inset(f32::from(state.inset()) as i32);

    let region = state
        .globals
        .compositor
        .create_region(&state.globals.qh, ());
    region.add(
        opaque_area.origin.x,
        opaque_area.origin.y,
        opaque_area.size.width,
        opaque_area.size.height,
    );

    // Note that rounded corners make this rectangle API hard to work with.
    // As this is common when using CSD, let's just disable this API.
    if projection.compositor_opaque_region {
        // Promise the compositor that this region of the window surface
        // contains no transparent pixels. This allows the compositor to skip
        // updating whatever is behind the surface for better performance.
        state.surface.set_opaque_region(Some(&region));
    } else {
        state.surface.set_opaque_region(None);
    }

    if let Some(ref blur_manager) = state.globals.blur_manager {
        if projection.blur_enabled {
            if state.blur.is_none() {
                let blur = blur_manager.create(&state.surface, &state.globals.qh, ());
                state.blur = Some(blur);
            }
            state.blur.as_ref().unwrap().commit();
        } else {
            // It probably doesn't hurt to clear the blur for opaque windows
            blur_manager.unset(&state.surface);
            if let Some(b) = state.blur.take() {
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
        );
        assert!(client_decorated.renderer_transparent);
        assert!(!client_decorated.compositor_opaque_region);

        let transparent = WaylandWindowBackgroundProjection::new(
            WindowBackgroundAppearance::Transparent,
            WindowDecorations::Server,
            true,
        );
        assert!(transparent.renderer_transparent);
        assert!(!transparent.compositor_opaque_region);
        assert!(!transparent.blur_enabled);

        let blurred = WaylandWindowBackgroundProjection::new(
            WindowBackgroundAppearance::Blurred,
            WindowDecorations::Server,
            true,
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
            let adjusted =
                WaylandWindowBackgroundProjection::new(requested, WindowDecorations::Server, true);
            assert_eq!(
                adjusted.observed_appearance,
                WindowBackgroundAppearance::Transparent
            );
            assert!(adjusted.renderer_transparent);
            assert!(!adjusted.blur_enabled);
        }
    }
}

#[cfg(test)]
mod should_close_callback_tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn replacement_installed_during_wayland_should_close_survives_old_callback_return() {
        let should_close = Callbacks::default().should_close;
        let slot_holder = Rc::new(RefCell::new(Some(should_close.clone())));
        let slot_holder_weak = Rc::downgrade(&slot_holder);
        let replacement_calls = Rc::new(Cell::new(0));

        should_close.set(Box::new({
            let replacement_calls = replacement_calls.clone();
            move || {
                let replacement_slot = slot_holder_weak
                    .upgrade()
                    .expect("test slot holder must remain alive")
                    .borrow()
                    .clone()
                    .expect("test slot must remain installed");
                let replacement_calls = replacement_calls.clone();
                replacement_slot.set(Box::new(move || {
                    replacement_calls.set(replacement_calls.get() + 1);
                    false
                }));

                assert!(!replacement_slot.invoke());
                true
            }
        }));

        assert!(should_close.invoke());
        assert!(!should_close.invoke());
        assert_eq!(replacement_calls.get(), 2);
    }

    #[test]
    fn close_during_wayland_should_close_permanently_retires_checked_out_callback() {
        let should_close = Callbacks::default().should_close;
        let slot_holder = Rc::new(RefCell::new(Some(should_close.clone())));
        let slot_holder_weak = Rc::downgrade(&slot_holder);
        let calls = Rc::new(Cell::new(0));

        should_close.set(Box::new({
            let calls = calls.clone();
            move || {
                calls.set(calls.get() + 1);
                slot_holder_weak
                    .upgrade()
                    .expect("test slot holder must remain alive")
                    .borrow()
                    .as_ref()
                    .expect("test slot must remain installed")
                    .terminate();
                true
            }
        }));

        assert!(should_close.invoke());
        assert!(!should_close.invoke());
        assert_eq!(calls.get(), 1);
    }
}
