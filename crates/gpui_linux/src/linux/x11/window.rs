use anyhow::{Context as _, anyhow};
use x11rb::connection::RequestConnection;

use crate::linux::{
    X11ClientStatePtr,
    interaction_gate::{WindowInteractionGate, accesskit_adapter},
};
use open_gpui::{
    AnyWindowHandle, Bounds, CursorStyle, Decorations, DevicePixels, DisplayId, ForegroundExecutor,
    GpuSpecs, Modifiers, NativeInputHandlerOutcome, Pixels, PlatformAtlas, PlatformDisplay,
    PlatformInput, PlatformInputCallback, PlatformInputCallbackSlot, PlatformInputHandler,
    PlatformInputHandlerSlot, PlatformNativeWindowRetirementOutcome,
    PlatformPresentationShutdownOutcome, PlatformShouldCloseCallbackSlot, PlatformWindow,
    PlatformWindowActiveStatusObservation, PlatformWindowCommand, PlatformWindowCommandDispatcher,
    PlatformWindowCommandOutcome, PlatformWindowInteractionQuiescence,
    PlatformWindowPresentOutcome, Point, PreparedPlatformPresentationShutdown, PromptButton,
    PromptLevel, RequestFrameOptions, ResizeEdge, ScaledPixels, Scene, Size, Task, Tiling,
    WindowActivationPolicy, WindowAppearance, WindowBackgroundAppearance, WindowBounds,
    WindowControlArea, WindowCreationFacts, WindowDecorations, WindowKind, WindowParams,
    WindowPresentationShutdownTicket, WindowVisibility, px,
};
use open_gpui_wgpu::{
    CompositorGpuHint, WgpuRenderer, WgpuSurfaceConfig, WgpuSurfaceShutdownProgress,
};

use open_gpui_util::{ResultExt, maybe};
use raw_window_handle as rwh;
use x11rb::{
    connection::Connection,
    cookie::{Cookie, VoidCookie},
    errors::{ConnectionError, ReplyError},
    properties::WmSizeHints,
    protocol::{
        ErrorKind, sync,
        xinput::{self, ConnectionExt as _},
        xproto::{self, ClientMessageEvent, ConnectionExt, TranslateCoordinatesReply},
    },
    wrapper::ConnectionExt as _,
    xcb_ffi::XCBConnection,
};

use std::{
    cell::RefCell,
    ffi::c_void,
    fmt::Display,
    num::NonZeroU32,
    ptr::{NonNull, null_mut},
    rc::{Rc, Weak},
    sync::Arc,
    time::Duration,
};

use super::{
    X11Display, XINPUT_ALL_DEVICE_GROUPS, XINPUT_ALL_DEVICES, point_from_x11_window_coords,
    window_observation::{
        X11PropertyAttempt, X11PropertyDomain, X11PropertyPayload, X11PropertyPayloadError,
        X11PropertyRetry, X11RestoreSettlement, X11SyncAcknowledgementObservation,
        X11SyncAcknowledgementSubmission, X11SyncCounterValue, X11SyncRetry,
        X11WindowObservationCoordinator, X11WindowStateAtoms, X11WindowStateFacts,
        parse_x11_edge_constraints_property, parse_x11_window_state_property,
        reconcile_x11_configure_geometry,
    },
};

x11rb::atom_manager! {
    pub XcbAtoms: AtomsCookie {
        XA_ATOM,
        XdndAware,
        XdndStatus,
        XdndEnter,
        XdndLeave,
        XdndPosition,
        XdndSelection,
        XdndDrop,
        XdndFinished,
        XdndTypeList,
        XdndActionCopy,
        TextUriList: b"text/uri-list",
        UTF8_STRING,
        TEXT,
        STRING,
        TEXT_PLAIN_UTF8: b"text/plain;charset=utf-8",
        TEXT_PLAIN: b"text/plain",
        XDND_DATA,
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        WM_CHANGE_STATE,
        WM_TRANSIENT_FOR,
        _NET_WM_PID,
        _NET_WM_NAME,
        _NET_WM_ICON,
        _NET_WM_STATE,
        _NET_WM_STATE_MAXIMIZED_VERT,
        _NET_WM_STATE_MAXIMIZED_HORZ,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_HIDDEN,
        _NET_ACTIVE_WINDOW,
        _NET_WM_USER_TIME,
        _NET_WM_SYNC_REQUEST,
        _NET_WM_SYNC_REQUEST_COUNTER,
        _NET_WM_BYPASS_COMPOSITOR,
        _NET_WM_MOVERESIZE,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_NOTIFICATION,
        _NET_WM_WINDOW_TYPE_DIALOG,
        _NET_WM_STATE_MODAL,
        _NET_WM_SYNC,
        _NET_SUPPORTED,
        _MOTIF_WM_HINTS,
        _GTK_SHOW_WINDOW_MENU,
        _GTK_FRAME_EXTENTS,
        _GTK_EDGE_CONSTRAINTS,
        _NET_CLIENT_LIST_STACKING,
    }
}

#[link(name = "xcb")]
unsafe extern "C" {
    fn xcb_poll_for_reply64(
        connection: *mut c_void,
        sequence: u64,
        reply: *mut *mut c_void,
        error: *mut *mut c_void,
    ) -> libc::c_int;
    fn xcb_discard_reply64(connection: *mut c_void, sequence: u64);
}

fn query_render_extent(
    xcb: &Rc<XCBConnection>,
    x_window: xproto::Window,
) -> anyhow::Result<Size<DevicePixels>> {
    let reply = get_reply(|| "X11 GetGeometry failed.", xcb.get_geometry(x_window))?;
    Ok(Size {
        width: DevicePixels(reply.width as i32),
        height: DevicePixels(reply.height as i32),
    })
}

fn resize_edge_to_moveresize(edge: ResizeEdge) -> u32 {
    match edge {
        ResizeEdge::TopLeft => 0,
        ResizeEdge::Top => 1,
        ResizeEdge::TopRight => 2,
        ResizeEdge::Right => 3,
        ResizeEdge::BottomRight => 4,
        ResizeEdge::Bottom => 5,
        ResizeEdge::BottomLeft => 6,
        ResizeEdge::Left => 7,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EdgeConstraints {
    top_tiled: bool,
    #[allow(dead_code)]
    top_resizable: bool,

    right_tiled: bool,
    #[allow(dead_code)]
    right_resizable: bool,

    bottom_tiled: bool,
    #[allow(dead_code)]
    bottom_resizable: bool,

    left_tiled: bool,
    #[allow(dead_code)]
    left_resizable: bool,
}

impl EdgeConstraints {
    fn from_atom(atom: u32) -> Self {
        EdgeConstraints {
            top_tiled: (atom & (1 << 0)) != 0,
            top_resizable: (atom & (1 << 1)) != 0,
            right_tiled: (atom & (1 << 2)) != 0,
            right_resizable: (atom & (1 << 3)) != 0,
            bottom_tiled: (atom & (1 << 4)) != 0,
            bottom_resizable: (atom & (1 << 5)) != 0,
            left_tiled: (atom & (1 << 6)) != 0,
            left_resizable: (atom & (1 << 7)) != 0,
        }
    }

    fn to_tiling(&self) -> Tiling {
        Tiling {
            top: self.top_tiled,
            right: self.right_tiled,
            bottom: self.bottom_tiled,
            left: self.left_tiled,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Visual {
    id: xproto::Visualid,
    colormap: u32,
    depth: u8,
}

struct VisualSet {
    inherit: Visual,
    opaque: Option<Visual>,
    transparent: Option<Visual>,
    root: u32,
    black_pixel: u32,
}

fn find_visuals(screens: &[xproto::Screen], screen_index: usize) -> Option<VisualSet> {
    let screen = screens.get(screen_index)?;
    let mut set = VisualSet {
        inherit: Visual {
            id: screen.root_visual,
            colormap: screen.default_colormap,
            depth: screen.root_depth,
        },
        opaque: None,
        transparent: None,
        root: screen.root,
        black_pixel: screen.black_pixel,
    };

    for depth_info in screen.allowed_depths.iter() {
        for visual_type in depth_info.visuals.iter() {
            let visual = Visual {
                id: visual_type.visual_id,
                colormap: 0,
                depth: depth_info.depth,
            };
            log::debug!(
                "Visual id: {}, class: {:?}, depth: {}, bits_per_value: {}, masks: 0x{:x} 0x{:x} 0x{:x}",
                visual_type.visual_id,
                visual_type.class,
                depth_info.depth,
                visual_type.bits_per_rgb_value,
                visual_type.red_mask,
                visual_type.green_mask,
                visual_type.blue_mask,
            );

            if (
                visual_type.red_mask,
                visual_type.green_mask,
                visual_type.blue_mask,
            ) != (0xFF0000, 0xFF00, 0xFF)
            {
                continue;
            }
            let color_mask = visual_type.red_mask | visual_type.green_mask | visual_type.blue_mask;
            let alpha_mask = color_mask as usize ^ ((1usize << depth_info.depth) - 1);

            if alpha_mask == 0 {
                if set.opaque.is_none() {
                    set.opaque = Some(visual);
                }
            } else {
                if set.transparent.is_none() {
                    set.transparent = Some(visual);
                }
            }
        }
    }

    Some(set)
}

pub(crate) fn x11_supports_alpha_creation(screens: &[xproto::Screen], screen_index: usize) -> bool {
    find_visuals(screens, screen_index).is_some_and(|visuals| visuals.transparent.is_some())
}

pub(crate) fn resolve_x11_screen_index(
    display_id: Option<DisplayId>,
    default_screen_index: usize,
    screen_count: usize,
) -> Option<usize> {
    display_id
        .and_then(|display_id| usize::try_from(u64::from(display_id)).ok())
        .filter(|screen_index| *screen_index < screen_count)
        .or_else(|| (default_screen_index < screen_count).then_some(default_screen_index))
}

#[derive(Debug, Clone, Copy)]
struct RawWindow {
    connection: *mut c_void,
    screen_id: usize,
    window_id: u32,
    visual_id: u32,
}

// Safety: The raw pointers in RawWindow point to X11 connection
// which is valid for the window's lifetime. These are used only for
// passing to wgpu which needs Send+Sync for surface creation.
unsafe impl Send for RawWindow {}
unsafe impl Sync for RawWindow {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum X11InitialWindowState {
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct X11WindowCreationProjection {
    device_bounds: Bounds<DevicePixels>,
    restore_bounds: Bounds<Pixels>,
    initial_state: X11InitialWindowState,
    alpha_capable: bool,
    focus_on_appearing: bool,
    activation_policy: WindowActivationPolicy,
    topmost: bool,
    taskbar_visible: bool,
}

impl X11WindowCreationProjection {
    fn new(
        window_bounds: WindowBounds,
        kind: &WindowKind,
        scale_factor: f32,
        alpha_capable: bool,
        focus_on_appearing: bool,
        activation_policy: WindowActivationPolicy,
    ) -> Self {
        let mut device_bounds = window_bounds.get_bounds().to_device_pixels(scale_factor);
        let mut restore_bounds = window_bounds.get_bounds();
        if device_bounds.size.width.0 == 0 || device_bounds.size.height.0 == 0 {
            device_bounds.size.width = 800.into();
            device_bounds.size.height = 600.into();
            restore_bounds = device_bounds.to_pixels(scale_factor);
        }

        let initial_state = if x11_supports_toplevel_creation_state(kind) {
            match window_bounds {
                WindowBounds::Windowed(_) => X11InitialWindowState::Windowed,
                WindowBounds::Maximized(_) => X11InitialWindowState::Maximized,
                WindowBounds::Fullscreen(_) => X11InitialWindowState::Fullscreen,
            }
        } else {
            X11InitialWindowState::Windowed
        };

        let popup = matches!(kind, WindowKind::PopUp);
        Self {
            device_bounds,
            restore_bounds,
            initial_state,
            alpha_capable,
            focus_on_appearing,
            activation_policy: WindowActivationPolicy {
                accepts_activation: activation_policy.accepts_activation,
                // Override-redirect popups do not receive window-manager click focus. Managed
                // toplevels use the window manager's global click-focus policy.
                focus_on_click: !popup,
            },
            topmost: false,
            taskbar_visible: !matches!(kind, WindowKind::PopUp),
        }
    }

    fn create_x(self) -> i16 {
        (self.device_bounds.origin.x.0 + 2) as i16
    }

    fn create_y(self) -> i16 {
        self.device_bounds.origin.y.0 as i16
    }
}

#[derive(Clone, Copy)]
pub(crate) struct X11TransientOwner {
    pub(crate) handle: AnyWindowHandle,
    pub(crate) x_window: xproto::Window,
}

pub(crate) fn x11_supports_transient_owner(kind: &WindowKind) -> bool {
    matches!(
        kind,
        WindowKind::Normal | WindowKind::Floating | WindowKind::Dialog
    )
}

pub(crate) fn x11_supports_initial_appearance_policy(_kind: &WindowKind) -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct X11WindowBackgroundProjection {
    observed_appearance: WindowBackgroundAppearance,
    renderer_transparent: bool,
}

impl X11WindowBackgroundProjection {
    fn new(requested: WindowBackgroundAppearance, alpha_capable: bool) -> Self {
        let observed_appearance = if alpha_capable {
            match requested {
                WindowBackgroundAppearance::Opaque | WindowBackgroundAppearance::Transparent => {
                    requested
                }
                WindowBackgroundAppearance::Blurred
                | WindowBackgroundAppearance::MicaBackdrop
                | WindowBackgroundAppearance::MicaAltBackdrop => {
                    WindowBackgroundAppearance::Transparent
                }
            }
        } else {
            WindowBackgroundAppearance::Opaque
        };
        Self {
            observed_appearance,
            renderer_transparent: observed_appearance != WindowBackgroundAppearance::Opaque,
        }
    }
}

pub(crate) fn x11_supports_toplevel_creation_state(kind: &WindowKind) -> bool {
    matches!(kind, WindowKind::Normal | WindowKind::Floating)
}

#[derive(Default)]
pub struct Callbacks {
    request_frame: Option<Rc<dyn Fn(RequestFrameOptions)>>,
    input: PlatformInputCallbackSlot,
    active_status_change: Option<Rc<dyn Fn(PlatformWindowActiveStatusObservation)>>,
    visibility_change: Option<Box<dyn FnMut(WindowVisibility)>>,
    hovered_status_change: Option<Rc<dyn Fn(bool)>>,
    resize: Option<Rc<dyn Fn(Size<Pixels>, f32)>>,
    moved: Option<Rc<dyn Fn()>>,
    window_state_change: Option<Rc<dyn Fn()>>,
    should_close: PlatformShouldCloseCallbackSlot,
    close: Option<Box<dyn FnOnce()>>,
    appearance_changed: Option<Rc<dyn Fn()>>,
    button_layout_changed: Option<Rc<dyn Fn()>>,
}

pub struct X11WindowState {
    pub destroyed: bool,
    client: X11ClientStatePtr,
    executor: ForegroundExecutor,
    atoms: XcbAtoms,
    x_root_window: xproto::Window,
    x_screen_index: usize,
    visual_id: u32,
    sync_counter: Option<sync::Counter>,
    sync_acknowledgement_task: Option<Task<()>>,
    observation: X11WindowObservationCoordinator,
    bounds: Bounds<Pixels>,
    restore_bounds: Bounds<Pixels>,
    scale_factor: f32,
    renderer: WgpuRenderer,
    display: Rc<dyn PlatformDisplay>,
    input_handler: PlatformInputHandlerSlot,
    appearance: WindowAppearance,
    background_appearance: WindowBackgroundAppearance,
    alpha_capable: bool,
    focus_on_appearing: bool,
    activation_policy: WindowActivationPolicy,
    transient_for: Option<AnyWindowHandle>,
    topmost: bool,
    taskbar_visible: bool,
    initially_shown: bool,
    initial_presentation_completed: bool,
    maximized_vertical: bool,
    maximized_horizontal: bool,
    hidden: bool,
    mapped: bool,
    interaction_gate: WindowInteractionGate,
    active: bool,
    /// The client's mapped state combined with its last `VisibilityNotify` observation.
    visibility: WindowVisibility,
    hovered: bool,
    fullscreen: bool,
    client_side_decorations_supported: bool,
    decorations: WindowDecorations,
    edge_constraints: Option<EdgeConstraints>,
    pub handle: AnyWindowHandle,
    last_insets: [u32; 4],
    accesskit_adapter: Option<accesskit_unix::Adapter>,
}

impl X11WindowState {
    fn is_transparent(&self) -> bool {
        self.background_appearance != WindowBackgroundAppearance::Opaque
    }
}

#[derive(Clone)]
pub(crate) struct X11WindowStatePtr {
    pub state: Rc<RefCell<X11WindowState>>,
    pub(crate) callbacks: Rc<RefCell<Callbacks>>,
    xcb: Rc<XCBConnection>,
    pub(crate) x_window: xproto::Window,
}

enum X11PropertyReadError {
    Retryable(anyhow::Error),
    Malformed(X11PropertyPayloadError),
}

type X11PropertyRead<T> = Result<T, X11PropertyReadError>;

enum X11PropertyObservation {
    WindowState(X11WindowStateFacts),
    EdgeConstraints(Option<EdgeConstraints>),
}

#[derive(Clone)]
struct X11ObservationRetryTarget {
    state: Weak<RefCell<X11WindowState>>,
    client: X11ClientStatePtr,
    x_window: xproto::Window,
}

struct X11WindowCommandTarget {
    owner: Weak<()>,
    state: Weak<RefCell<X11WindowState>>,
    xcb: Rc<XCBConnection>,
    x_window: xproto::Window,
}

impl X11WindowCommandTarget {
    fn new(window: &X11Window) -> Self {
        Self {
            owner: Rc::downgrade(&window.1),
            state: Rc::downgrade(&window.0.state),
            xcb: window.0.xcb.clone(),
            x_window: window.0.x_window,
        }
    }

    fn dispatch(&self, command: PlatformWindowCommand) -> PlatformWindowCommandOutcome {
        let Some(_owner) = self.owner.upgrade() else {
            return PlatformWindowCommandOutcome::WindowClosed;
        };
        let Some(state) = self.state.upgrade() else {
            return PlatformWindowCommandOutcome::WindowClosed;
        };
        let mut state = state.borrow_mut();
        if state.destroyed {
            return PlatformWindowCommandOutcome::WindowClosed;
        }
        if state.interaction_gate.is_quiesced() && command.requires_interaction_authority() {
            return PlatformWindowCommandOutcome::Rejected;
        }

        match command {
            PlatformWindowCommand::CompleteInitialPresentation { activate } => {
                if state.initial_presentation_completed {
                    return PlatformWindowCommandOutcome::Accepted;
                }
                if state.initially_shown && activate && state.activation_policy.accepts_activation {
                    activate_x11_window(&state, &self.xcb, self.x_window).log_err();
                }
                state.initial_presentation_completed = true;
                PlatformWindowCommandOutcome::Accepted
            }
            PlatformWindowCommand::RevealDeferredInitialPresentation { .. } => {
                PlatformWindowCommandOutcome::Rejected
            }
            PlatformWindowCommand::Activate { .. }
                if state.activation_policy.accepts_activation =>
            {
                x11_command_outcome(activate_x11_window(&state, &self.xcb, self.x_window))
            }
            PlatformWindowCommand::Activate { .. } => PlatformWindowCommandOutcome::Rejected,
            PlatformWindowCommand::ShowWindowMenu(position) => x11_command_outcome(
                show_x11_window_menu(&state, &self.xcb, self.x_window, position),
            ),
            PlatformWindowCommand::StartWindowMove => {
                const MOVERESIZE_MOVE: u32 = 8;
                x11_command_outcome(send_x11_moveresize(
                    &state,
                    &self.xcb,
                    self.x_window,
                    MOVERESIZE_MOVE,
                ))
            }
            PlatformWindowCommand::StartWindowResize(edge) => {
                x11_command_outcome(send_x11_moveresize(
                    &state,
                    &self.xcb,
                    self.x_window,
                    resize_edge_to_moveresize(edge),
                ))
            }
        }
    }
}

fn x11_command_outcome(result: anyhow::Result<()>) -> PlatformWindowCommandOutcome {
    if result.log_err().is_some() {
        PlatformWindowCommandOutcome::Accepted
    } else {
        PlatformWindowCommandOutcome::Rejected
    }
}

fn activate_x11_window(
    state: &X11WindowState,
    xcb: &XCBConnection,
    x_window: xproto::Window,
) -> anyhow::Result<()> {
    let data = [1, xproto::Time::CURRENT_TIME.into(), 0, 0, 0];
    let message =
        xproto::ClientMessageEvent::new(32, x_window, state.atoms._NET_ACTIVE_WINDOW, data);
    check_reply(
        || "X11 SendEvent to activate window failed.",
        xcb.send_event(
            false,
            state.x_root_window,
            xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
            message,
        ),
    )?;
    check_reply(
        || "X11 SetInputFocus failed.",
        xcb.set_input_focus(
            xproto::InputFocus::POINTER_ROOT,
            x_window,
            xproto::Time::CURRENT_TIME,
        ),
    )?;
    xcb_flush(xcb);
    Ok(())
}

fn get_x11_root_position(
    state: &X11WindowState,
    xcb: &XCBConnection,
    x_window: xproto::Window,
    position: Point<Pixels>,
) -> anyhow::Result<TranslateCoordinatesReply> {
    get_reply(
        || "X11 TranslateCoordinates failed.",
        xcb.translate_coordinates(
            x_window,
            state.x_root_window,
            (f32::from(position.x) * state.scale_factor) as i16,
            (f32::from(position.y) * state.scale_factor) as i16,
        ),
    )
}

fn show_x11_window_menu(
    state: &X11WindowState,
    xcb: &XCBConnection,
    x_window: xproto::Window,
    position: Point<Pixels>,
) -> anyhow::Result<()> {
    check_reply(
        || "X11 UngrabPointer failed.",
        xcb.ungrab_pointer(x11rb::CURRENT_TIME),
    )?;

    let coords = get_x11_root_position(state, xcb, x_window, position)?;
    let message = ClientMessageEvent::new(
        32,
        x_window,
        state.atoms._GTK_SHOW_WINDOW_MENU,
        [
            XINPUT_ALL_DEVICE_GROUPS as u32,
            coords.dst_x as u32,
            coords.dst_y as u32,
            0,
            0,
        ],
    );
    check_reply(
        || "X11 SendEvent to show window menu failed.",
        xcb.send_event(
            false,
            state.x_root_window,
            xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
            message,
        ),
    )?;
    xcb_flush(xcb);
    Ok(())
}

fn send_x11_moveresize(
    state: &X11WindowState,
    xcb: &XCBConnection,
    x_window: xproto::Window,
    flag: u32,
) -> anyhow::Result<()> {
    check_reply(
        || "X11 UngrabPointer before move/resize of window failed.",
        xcb.ungrab_pointer(x11rb::CURRENT_TIME),
    )?;

    let pointer = get_reply(
        || "X11 QueryPointer before move/resize of window failed.",
        xcb.query_pointer(x_window),
    )?;
    let message = ClientMessageEvent::new(
        32,
        x_window,
        state.atoms._NET_WM_MOVERESIZE,
        [
            pointer.root_x as u32,
            pointer.root_y as u32,
            flag,
            0, // Left mouse button
            0,
        ],
    );
    check_reply(
        || "X11 SendEvent to move/resize window failed.",
        xcb.send_event(
            false,
            state.x_root_window,
            xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
            message,
        ),
    )?;

    xcb_flush(xcb);
    Ok(())
}

impl rwh::HasWindowHandle for RawWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let Some(non_zero) = NonZeroU32::new(self.window_id) else {
            log::error!("RawWindow.window_id zero when getting window handle.");
            return Err(rwh::HandleError::Unavailable);
        };
        let mut handle = rwh::XcbWindowHandle::new(non_zero);
        handle.visual_id = NonZeroU32::new(self.visual_id);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(handle.into()) })
    }
}
impl rwh::HasDisplayHandle for RawWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let Some(non_zero) = NonNull::new(self.connection) else {
            log::error!("Null RawWindow.connection when getting display handle.");
            return Err(rwh::HandleError::Unavailable);
        };
        let handle = rwh::XcbDisplayHandle::new(Some(non_zero), self.screen_id as i32);
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(handle.into()) })
    }
}

impl rwh::HasWindowHandle for X11Window {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let Some(non_zero) = NonZeroU32::new(self.0.x_window) else {
            return Err(rwh::HandleError::Unavailable);
        };
        let handle = rwh::XcbWindowHandle::new(non_zero);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(handle.into()) })
    }
}

impl rwh::HasDisplayHandle for X11Window {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let connection =
            as_raw_xcb_connection::AsRawXcbConnection::as_raw_xcb_connection(&*self.0.xcb)
                as *mut _;
        let Some(non_zero) = NonNull::new(connection) else {
            return Err(rwh::HandleError::Unavailable);
        };
        let screen_id = {
            let state = self.0.state.borrow();
            u64::from(state.display.id()) as i32
        };
        let handle = rwh::XcbDisplayHandle::new(Some(non_zero), screen_id);
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(handle.into()) })
    }
}

pub(crate) fn xcb_flush(xcb: &XCBConnection) {
    xcb.flush()
        .map_err(handle_connection_error)
        .context("X11 flush failed")
        .log_err();
}

enum X11SyncTransportFailure {
    Retryable(anyhow::Error),
    Permanent {
        error: anyhow::Error,
        counter_invalid: bool,
    },
}

enum X11SyncTerminalObservation {
    Pending(X11SyncAcknowledgementSubmission),
    Succeeded,
    Failed {
        submission: X11SyncAcknowledgementSubmission,
        failure: X11SyncTransportFailure,
    },
}

enum X11SyncSequenceObservation {
    Pending,
    Succeeded,
    Failed {
        failure: X11SyncTransportFailure,
        terminal: bool,
    },
}

fn classify_sync_connection_failure(
    context: &'static str,
    error: ConnectionError,
) -> X11SyncTransportFailure {
    let retryable = matches!(
        &error,
        ConnectionError::IoError(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::TimedOut
            )
    );
    let error = anyhow!(error).context(context);
    if retryable {
        X11SyncTransportFailure::Retryable(error)
    } else {
        X11SyncTransportFailure::Permanent {
            error,
            counter_invalid: false,
        }
    }
}

fn classify_sync_protocol_failure(
    context: &'static str,
    error: x11rb::x11_utils::X11Error,
) -> X11SyncTransportFailure {
    let counter_invalid = error.error_kind == ErrorKind::SyncCounter;
    X11SyncTransportFailure::Permanent {
        error: anyhow!(ReplyError::X11Error(error)).context(context),
        counter_invalid,
    }
}

fn submit_sync_acknowledgement(
    xcb: &XCBConnection,
    counter: sync::Counter,
    value: X11SyncCounterValue,
) -> Result<(u64, u64), X11SyncTransportFailure> {
    let request = sync::set_counter(
        xcb,
        counter,
        sync::Int64 {
            lo: value.lo,
            hi: value.hi,
        },
    )
    .map_err(|error| {
        classify_sync_connection_failure("X11 sync SetCounter submission failed", error)
    })?;
    let set_counter_sequence = request.sequence_number();

    // A reply-bearing request behind SetCounter lets libxcb prove the void request terminal
    // without xcb_request_check's synchronous server round-trip on the draw path.
    let fence = match xcb.get_input_focus() {
        Ok(fence) => fence,
        Err(error) => {
            request.ignore_error();
            return Err(classify_sync_connection_failure(
                "X11 sync acknowledgement fence submission failed",
                error,
            ));
        }
    };
    let fence_sequence = fence.sequence_number();
    if let Err(error) = xcb.flush() {
        request.ignore_error();
        fence.discard_reply_and_errors();
        return Err(classify_sync_connection_failure(
            "X11 sync acknowledgement flush failed",
            error,
        ));
    }

    // The non-blocking observer below now owns consumption of both request sequences.
    std::mem::forget(request);
    std::mem::forget(fence);
    Ok((set_counter_sequence, fence_sequence))
}

fn poll_sync_sequence(xcb: &XCBConnection, sequence: u64) -> X11SyncSequenceObservation {
    let mut reply = null_mut();
    let mut error = null_mut();
    // SAFETY: the connection pointer is borrowed from `xcb` for this call. libxcb initializes the
    // two out-pointers and never blocks in xcb_poll_for_reply64. Any returned allocation is freed
    // below with libc::free, matching libxcb's ownership contract.
    let found = unsafe {
        xcb_poll_for_reply64(
            xcb.get_raw_xcb_connection(),
            sequence,
            &mut reply,
            &mut error,
        )
    };

    if found == 0 {
        if let Some(error) = xcb.has_error() {
            return X11SyncSequenceObservation::Failed {
                failure: classify_sync_connection_failure(
                    "X11 sync acknowledgement observation failed",
                    error,
                ),
                terminal: false,
            };
        }
        return X11SyncSequenceObservation::Pending;
    }

    if !reply.is_null() {
        // Reply-bearing fences return an allocation; checked void requests normally do not.
        // SAFETY: non-null reply allocations returned by libxcb are released with libc::free.
        unsafe { libc::free(reply) };
    }
    if error.is_null() {
        return X11SyncSequenceObservation::Succeeded;
    }

    // SAFETY: X11 error packets are exactly 32 bytes and remain valid until freed below.
    let parsed = xcb.parse_error(unsafe { std::slice::from_raw_parts(error.cast::<u8>(), 32) });
    // SAFETY: non-null error allocations returned by libxcb are released with libc::free.
    unsafe { libc::free(error) };
    match parsed {
        Ok(error) => X11SyncSequenceObservation::Failed {
            failure: classify_sync_protocol_failure(
                "X11 sync acknowledgement request failed",
                error,
            ),
            terminal: true,
        },
        Err(error) => X11SyncSequenceObservation::Failed {
            failure: X11SyncTransportFailure::Permanent {
                error: anyhow!(error).context("X11 sync acknowledgement error parsing failed"),
                counter_invalid: false,
            },
            terminal: true,
        },
    }
}

fn observe_sync_acknowledgement_with(
    submission: X11SyncAcknowledgementSubmission,
    mut poll: impl FnMut(u64) -> X11SyncSequenceObservation,
) -> X11SyncTerminalObservation {
    let mut progress = submission;
    if !progress.fence_terminal {
        match poll(progress.fence_sequence) {
            X11SyncSequenceObservation::Pending => {
                return X11SyncTerminalObservation::Pending(progress);
            }
            X11SyncSequenceObservation::Succeeded => progress.fence_terminal = true,
            X11SyncSequenceObservation::Failed { failure, terminal } => {
                progress.fence_terminal = terminal;
                return X11SyncTerminalObservation::Failed {
                    submission: progress,
                    failure,
                };
            }
        }
    }

    match poll(progress.set_counter_sequence) {
        X11SyncSequenceObservation::Pending => X11SyncTerminalObservation::Pending(progress),
        X11SyncSequenceObservation::Succeeded => X11SyncTerminalObservation::Succeeded,
        X11SyncSequenceObservation::Failed { failure, terminal } => {
            progress.set_counter_terminal = terminal;
            X11SyncTerminalObservation::Failed {
                submission: progress,
                failure,
            }
        }
    }
}

fn observe_sync_acknowledgement(
    xcb: &XCBConnection,
    submission: X11SyncAcknowledgementSubmission,
) -> X11SyncTerminalObservation {
    observe_sync_acknowledgement_with(submission, |sequence| poll_sync_sequence(xcb, sequence))
}

fn sync_sequences_to_discard(submission: X11SyncAcknowledgementSubmission) -> [Option<u64>; 2] {
    [
        (!submission.set_counter_terminal).then_some(submission.set_counter_sequence),
        (!submission.fence_terminal).then_some(submission.fence_sequence),
    ]
}

fn discard_sync_acknowledgement(xcb: &XCBConnection, submission: X11SyncAcknowledgementSubmission) {
    for sequence in sync_sequences_to_discard(submission).into_iter().flatten() {
        // SAFETY: each pending sequence belongs to this connection and is discarded at most once
        // when observation fails or window closure abandons the in-flight obligation.
        unsafe { xcb_discard_reply64(xcb.get_raw_xcb_connection(), sequence) };
    }
}

pub(crate) fn check_reply<E, F, C>(
    failure_context: F,
    result: Result<VoidCookie<'_, C>, ConnectionError>,
) -> anyhow::Result<()>
where
    E: Display + Send + Sync + 'static,
    F: FnOnce() -> E,
    C: RequestConnection,
{
    result
        .map_err(handle_connection_error)
        .and_then(|response| response.check().map_err(|reply_error| anyhow!(reply_error)))
        .with_context(failure_context)
}

pub(crate) fn get_reply<E, F, C, O>(
    failure_context: F,
    result: Result<Cookie<'_, C, O>, ConnectionError>,
) -> anyhow::Result<O>
where
    E: Display + Send + Sync + 'static,
    F: FnOnce() -> E,
    C: RequestConnection,
    O: x11rb::x11_utils::TryParse,
{
    result
        .map_err(handle_connection_error)
        .and_then(|response| response.reply().map_err(|reply_error| anyhow!(reply_error)))
        .with_context(failure_context)
}

/// Convert X11 connection errors to `anyhow::Error` and panic for unrecoverable errors.
pub(crate) fn handle_connection_error(err: ConnectionError) -> anyhow::Error {
    match err {
        ConnectionError::UnknownError => anyhow!("X11 connection: Unknown error"),
        ConnectionError::UnsupportedExtension => anyhow!("X11 connection: Unsupported extension"),
        ConnectionError::MaximumRequestLengthExceeded => {
            anyhow!("X11 connection: Maximum request length exceeded")
        }
        ConnectionError::FdPassingFailed => {
            panic!("X11 connection: File descriptor passing failed")
        }
        ConnectionError::ParseError(parse_error) => {
            anyhow!(parse_error).context("Parse error in X11 response")
        }
        ConnectionError::InsufficientMemory => panic!("X11 connection: Insufficient memory"),
        ConnectionError::IoError(err) => anyhow!(err).context("X11 connection: IOError"),
        _ => anyhow!(err),
    }
}

impl X11WindowState {
    pub fn new(
        handle: AnyWindowHandle,
        client: X11ClientStatePtr,
        executor: ForegroundExecutor,
        gpu_context: open_gpui_wgpu::GpuContext,
        compositor_gpu: Option<CompositorGpuHint>,
        params: WindowParams,
        xcb: &Rc<XCBConnection>,
        client_side_decorations_supported: bool,
        x_main_screen_index: usize,
        x_window: xproto::Window,
        atoms: &XcbAtoms,
        scale_factor: f32,
        appearance: WindowAppearance,
        transient_owner: Option<X11TransientOwner>,
        supports_xinput_gestures: bool,
        is_bgr: bool,
    ) -> anyhow::Result<Self> {
        let x_screen_index = resolve_x11_screen_index(
            params.display_id,
            x_main_screen_index,
            xcb.setup().roots.len(),
        )
        .context("X11 has no available screen for the requested or default display")?;

        let visual_set = find_visuals(&xcb.setup().roots, x_screen_index)
            .context("X11 target screen disappeared")?;
        let alpha_capable = visual_set.transparent.is_some();

        let visual = match visual_set.transparent {
            Some(visual) => visual,
            None => {
                log::warn!("Unable to find a transparent visual",);
                visual_set.inherit
            }
        };
        log::info!("Using {:?}", visual);
        let creation = X11WindowCreationProjection::new(
            params.window_bounds,
            &params.kind,
            scale_factor,
            alpha_capable,
            params.focus_on_appearing,
            params.activation_policy,
        );
        let initially_shown = params.show;

        let colormap = if visual.colormap != 0 {
            visual.colormap
        } else {
            let id = xcb.generate_id()?;
            log::info!("Creating colormap {}", id);
            check_reply(
                || format!("X11 CreateColormap failed. id: {}", id),
                xcb.create_colormap(xproto::ColormapAlloc::NONE, id, visual_set.root, visual.id),
            )?;
            id
        };

        let win_aux = xproto::CreateWindowAux::new()
            // https://stackoverflow.com/questions/43218127/x11-xlib-xcb-creating-a-window-requires-border-pixel-if-specifying-colormap-wh
            .border_pixel(visual_set.black_pixel)
            .colormap(colormap)
            .override_redirect((params.kind == WindowKind::PopUp) as u32)
            .event_mask(
                xproto::EventMask::EXPOSURE
                    | xproto::EventMask::STRUCTURE_NOTIFY
                    | xproto::EventMask::FOCUS_CHANGE
                    | xproto::EventMask::KEY_PRESS
                    | xproto::EventMask::KEY_RELEASE
                    | xproto::EventMask::PROPERTY_CHANGE
                    | xproto::EventMask::VISIBILITY_CHANGE,
            );

        let requested_bounds = params
            .window_bounds
            .get_bounds()
            .to_device_pixels(scale_factor);
        if requested_bounds.size.width.0 == 0 || requested_bounds.size.height.0 == 0 {
            log::warn!(
                "Window bounds contain a zero value. height={}, width={}. Falling back to defaults.",
                requested_bounds.size.height.0,
                requested_bounds.size.width.0
            );
        }
        let mut bounds = creation.device_bounds;

        check_reply(
            || {
                format!(
                    "X11 CreateWindow failed. depth: {}, x_window: {}, visual_set.root: {}, bounds.origin.x.0: {}, bounds.origin.y.0: {}, bounds.size.width.0: {}, bounds.size.height.0: {}",
                    visual.depth,
                    x_window,
                    visual_set.root,
                    creation.create_x(),
                    creation.create_y(),
                    bounds.size.width.0,
                    bounds.size.height.0
                )
            },
            xcb.create_window(
                visual.depth,
                x_window,
                visual_set.root,
                creation.create_x(),
                creation.create_y(),
                bounds.size.width.0 as u16,
                bounds.size.height.0 as u16,
                0,
                xproto::WindowClass::INPUT_OUTPUT,
                visual.id,
                &win_aux,
            ),
        )?;

        let mut created_sync_counter = None;
        // Collect errors during setup, so that every created native resource can be retired on
        // failure.
        let setup_result = maybe!({
            let pid = std::process::id();
            check_reply(
                || "X11 ChangeProperty for _NET_WM_PID failed.",
                xcb.change_property32(
                    xproto::PropMode::REPLACE,
                    x_window,
                    atoms._NET_WM_PID,
                    xproto::AtomEnum::CARDINAL,
                    &[pid],
                ),
            )?;
            if !creation.focus_on_appearing {
                check_reply(
                    || "X11 ChangeProperty32 setting _NET_WM_USER_TIME failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_USER_TIME,
                        xproto::AtomEnum::CARDINAL,
                        &[0],
                    ),
                )?;
            }

            let reply = get_reply(|| "X11 GetGeometry failed.", xcb.get_geometry(x_window))?;
            if reply.x == 0 && reply.y == 0 {
                bounds.origin.x.0 += 2;
                // Work around a bug where our rendered content appears
                // outside the window bounds when opened at the default position
                // (14px, 49px on X + Gnome + Ubuntu 22).
                let x = bounds.origin.x.0;
                let y = bounds.origin.y.0;
                check_reply(
                    || format!("X11 ConfigureWindow failed. x: {}, y: {}", x, y),
                    xcb.configure_window(x_window, &xproto::ConfigureWindowAux::new().x(x).y(y)),
                )?;
            }
            if let Some(titlebar) = params.titlebar
                && let Some(title) = titlebar.title
            {
                check_reply(
                    || "X11 ChangeProperty8 on WM_NAME failed.",
                    xcb.change_property8(
                        xproto::PropMode::REPLACE,
                        x_window,
                        xproto::AtomEnum::WM_NAME,
                        xproto::AtomEnum::STRING,
                        title.as_bytes(),
                    ),
                )?;
                check_reply(
                    || "X11 ChangeProperty8 on _NET_WM_NAME failed.",
                    xcb.change_property8(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_NAME,
                        atoms.UTF8_STRING,
                        title.as_bytes(),
                    ),
                )?;
            }

            let initial_window_state = match creation.initial_state {
                X11InitialWindowState::Windowed => &[][..],
                X11InitialWindowState::Maximized => &[
                    atoms._NET_WM_STATE_MAXIMIZED_VERT,
                    atoms._NET_WM_STATE_MAXIMIZED_HORZ,
                ],
                X11InitialWindowState::Fullscreen => &[atoms._NET_WM_STATE_FULLSCREEN],
            };
            if !initial_window_state.is_empty() {
                check_reply(
                    || "X11 ChangeProperty32 setting initial _NET_WM_STATE failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_STATE,
                        atoms.XA_ATOM,
                        initial_window_state,
                    ),
                )?;
            }

            if params.kind == WindowKind::PopUp {
                check_reply(
                    || "X11 ChangeProperty32 setting window type for pop-up failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_WINDOW_TYPE,
                        xproto::AtomEnum::ATOM,
                        &[atoms._NET_WM_WINDOW_TYPE_NOTIFICATION],
                    ),
                )?;
            }

            if let Some(transient_owner) = transient_owner {
                // WM_TRANSIENT_FOR is a top-level ownership relationship. It does not make the
                // owner responsible for the transient window's lifetime.
                check_reply(
                    || "X11 ChangeProperty32 setting WM_TRANSIENT_FOR failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms.WM_TRANSIENT_FOR,
                        xproto::AtomEnum::WINDOW,
                        &[transient_owner.x_window],
                    ),
                )?;
            }

            if params.kind == WindowKind::Dialog {
                // _NET_WM_WINDOW_TYPE_DIALOG indicates that this is a dialog (floating) window
                // https://specifications.freedesktop.org/wm-spec/1.4/ar01s05.html
                check_reply(
                    || "X11 ChangeProperty32 setting window type for dialog window failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_WINDOW_TYPE,
                        xproto::AtomEnum::ATOM,
                        &[atoms._NET_WM_WINDOW_TYPE_DIALOG],
                    ),
                )?;

                // We set the modal state for dialog windows, so that the window manager
                // can handle it appropriately (e.g., prevent interaction with the parent window
                // while the dialog is open).
                check_reply(
                    || "X11 ChangeProperty32 setting modal state for dialog window failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_STATE,
                        xproto::AtomEnum::ATOM,
                        &[atoms._NET_WM_STATE_MODAL],
                    ),
                )?;
            }

            check_reply(
                || "X11 ChangeProperty32 setting protocols failed.",
                xcb.change_property32(
                    xproto::PropMode::REPLACE,
                    x_window,
                    atoms.WM_PROTOCOLS,
                    xproto::AtomEnum::ATOM,
                    &[atoms.WM_DELETE_WINDOW, atoms._NET_WM_SYNC_REQUEST],
                ),
            )?;

            get_reply(
                || "X11 sync protocol initialize failed.",
                sync::initialize(xcb, 3, 1),
            )?;
            let sync_request_counter = xcb.generate_id()?;
            check_reply(
                || "X11 sync CreateCounter failed.",
                sync::create_counter(xcb, sync_request_counter, sync::Int64 { lo: 0, hi: 0 }),
            )?;
            created_sync_counter = Some(sync_request_counter);

            check_reply(
                || "X11 ChangeProperty32 setting sync request counter failed.",
                xcb.change_property32(
                    xproto::PropMode::REPLACE,
                    x_window,
                    atoms._NET_WM_SYNC_REQUEST_COUNTER,
                    xproto::AtomEnum::CARDINAL,
                    &[sync_request_counter],
                ),
            )?;

            let mut xi_event_mask = xinput::XIEventMask::MOTION
                | xinput::XIEventMask::BUTTON_PRESS
                | xinput::XIEventMask::BUTTON_RELEASE
                | xinput::XIEventMask::ENTER
                | xinput::XIEventMask::LEAVE;
            if supports_xinput_gestures {
                // x11rb 0.13 doesn't define XIEventMask constants for gesture
                // events, so we construct them from the event opcodes (each
                // XInput event type N maps to mask bit N).
                xi_event_mask |=
                    xinput::XIEventMask::from(1u32 << xinput::GESTURE_PINCH_BEGIN_EVENT)
                        | xinput::XIEventMask::from(1u32 << xinput::GESTURE_PINCH_UPDATE_EVENT)
                        | xinput::XIEventMask::from(1u32 << xinput::GESTURE_PINCH_END_EVENT);
            }
            check_reply(
                || "X11 XiSelectEvents failed.",
                xcb.xinput_xi_select_events(
                    x_window,
                    &[xinput::EventMask {
                        deviceid: XINPUT_ALL_DEVICE_GROUPS,
                        mask: vec![xi_event_mask],
                    }],
                ),
            )?;

            check_reply(
                || "X11 XiSelectEvents for device changes failed.",
                xcb.xinput_xi_select_events(
                    x_window,
                    &[xinput::EventMask {
                        deviceid: XINPUT_ALL_DEVICES,
                        mask: vec![
                            xinput::XIEventMask::HIERARCHY | xinput::XIEventMask::DEVICE_CHANGED,
                        ],
                    }],
                ),
            )?;

            xcb_flush(xcb);

            let mut renderer = {
                let raw_window = RawWindow {
                    connection: as_raw_xcb_connection::AsRawXcbConnection::as_raw_xcb_connection(
                        xcb,
                    ) as *mut _,
                    screen_id: x_screen_index,
                    window_id: x_window,
                    visual_id: visual.id,
                };
                let config = WgpuSurfaceConfig {
                    // Note: this has to be done after the GPU init, or otherwise
                    // the sizes are immediately invalidated.
                    size: query_render_extent(xcb, x_window)?,
                    // We set it to transparent by default, even if we have client-side
                    // decorations, since those seem to work on X11 even without `true` here.
                    // If the window appearance changes, then the renderer will get updated
                    // too
                    transparent: false,
                    preferred_present_mode: None,
                };
                WgpuRenderer::new(gpu_context, &raw_window, config, compositor_gpu)?
            };

            renderer.set_subpixel_layout(is_bgr);

            // Set max window size hints based on the GPU's maximum texture dimension.
            // This prevents the window from being resized larger than what the GPU can render.
            let max_texture_size = renderer.max_texture_size();
            let mut size_hints = WmSizeHints::new();
            if let Some(size) = params.window_min_size {
                size_hints.min_size =
                    Some((f32::from(size.width) as i32, f32::from(size.height) as i32));
            }
            size_hints.max_size = Some((max_texture_size as i32, max_texture_size as i32));
            check_reply(
                || {
                    format!(
                        "X11 change of WM_SIZE_HINTS failed. max_size: {:?}",
                        max_texture_size
                    )
                },
                size_hints.set_normal_hints(xcb, x_window),
            )?;

            if let Some(image) = params.icon {
                // https://specifications.freedesktop.org/wm-spec/1.4/ar01s05.html#id-1.6.13
                let property_size = 2 + (image.width() * image.height()) as usize;
                let mut property_data: Vec<u32> = Vec::with_capacity(property_size);
                property_data.push(image.width());
                property_data.push(image.height());
                property_data.extend(image.pixels().map(|px| {
                    let [r, g, b, a]: [u8; 4] = px.0;
                    u32::from_le_bytes([b, g, r, a])
                }));

                check_reply(
                    || "X11 ChangeProperty32 for _NET_ICON_NAME failed.",
                    xcb.change_property32(
                        xproto::PropMode::REPLACE,
                        x_window,
                        atoms._NET_WM_ICON,
                        xproto::AtomEnum::CARDINAL,
                        &property_data,
                    ),
                )?;
            }

            let display = Rc::new(X11Display::new(xcb, scale_factor, x_screen_index)?);

            Ok(Self {
                client,
                executor,
                display,
                x_root_window: visual_set.root,
                x_screen_index,
                visual_id: visual.id,
                bounds: bounds.to_pixels(scale_factor),
                restore_bounds: creation.restore_bounds,
                scale_factor,
                renderer,
                atoms: *atoms,
                input_handler: PlatformInputHandlerSlot::default(),
                active: false,
                visibility: WindowVisibility::Hidden,
                hovered: false,
                fullscreen: false,
                maximized_vertical: false,
                maximized_horizontal: false,
                hidden: false,
                mapped: false,
                interaction_gate: WindowInteractionGate::default(),
                appearance,
                handle,
                background_appearance: WindowBackgroundAppearance::Opaque,
                alpha_capable: creation.alpha_capable,
                focus_on_appearing: creation.focus_on_appearing,
                activation_policy: creation.activation_policy,
                transient_for: transient_owner.map(|owner| owner.handle),
                topmost: creation.topmost,
                taskbar_visible: creation.taskbar_visible,
                initially_shown,
                initial_presentation_completed: false,
                destroyed: false,
                client_side_decorations_supported,
                decorations: WindowDecorations::Server,
                last_insets: [0, 0, 0, 0],
                edge_constraints: None,
                accesskit_adapter: None,
                sync_counter: Some(sync_request_counter),
                sync_acknowledgement_task: None,
                observation: X11WindowObservationCoordinator::default(),
            })
        });

        if setup_result.is_err() {
            if let Some(sync_request_counter) = created_sync_counter {
                check_reply(
                    || "X11 DestroyCounter failed while cleaning up window setup.",
                    sync::destroy_counter(xcb, sync_request_counter),
                )
                .log_err();
            }
            check_reply(
                || "X11 DestroyWindow failed while cleaning it up after setup failure.",
                xcb.destroy_window(x_window),
            )
            .log_err();
            xcb_flush(xcb);
        }

        setup_result
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds.size
    }
}

pub(crate) struct X11Window(pub X11WindowStatePtr, Rc<()>);

impl Drop for X11Window {
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
                "X11 window dropped before exact presentation shutdown drained submitted GPU work; retaining backend owner"
            );
            std::mem::forget(self.0.clone());
            return;
        }

        let this_ptr = self.0.clone();
        let client_ptr = self.0.state.borrow().client.clone();
        client_ptr.drop_window_if_current(&this_ptr);

        let mut state = self.0.state.borrow_mut();
        let native_destroy_needed = !state.destroyed;
        let sync_counter = state.sync_counter;
        state.renderer.destroy();

        if native_destroy_needed {
            if let Err(error) = check_reply(
                || "X11 DestroyWindow failed during a safe presentation rollback.",
                self.0.xcb.destroy_window(self.0.x_window),
            ) {
                log::error!(
                    "X11 window rollback could not retire its native window; retaining backend owner: {error}"
                );
                drop(state);
                std::mem::forget(self.0.clone());
                return;
            }
            state.destroyed = true;
        }
        if let Some(counter_id) = sync_counter {
            if let Err(error) = check_reply(
                || "X11 DestroyCounter failed during a safe presentation rollback.",
                sync::destroy_counter(&self.0.xcb, counter_id),
            ) {
                log::error!(
                    "X11 window rollback could not retire its sync counter; retaining backend owner: {error}"
                );
                drop(state);
                std::mem::forget(self.0.clone());
                return;
            }
            state.sync_counter = None;
        }
        xcb_flush(&self.0.xcb);

        let executor = state.executor.clone();
        drop(state);
        executor
            .spawn(async move {
                this_ptr.close();
            })
            .detach();
    }
}

enum WmHintPropertyState {
    // Remove = 0,
    // Add = 1,
    Toggle = 2,
}

impl X11Window {
    pub fn new(
        handle: AnyWindowHandle,
        client: X11ClientStatePtr,
        executor: ForegroundExecutor,
        gpu_context: open_gpui_wgpu::GpuContext,
        compositor_gpu: Option<CompositorGpuHint>,
        params: WindowParams,
        xcb: &Rc<XCBConnection>,
        client_side_decorations_supported: bool,
        x_main_screen_index: usize,
        x_window: xproto::Window,
        atoms: &XcbAtoms,
        scale_factor: f32,
        appearance: WindowAppearance,
        transient_owner: Option<X11TransientOwner>,
        supports_xinput_gestures: bool,
        is_bgr: bool,
    ) -> anyhow::Result<Self> {
        let ptr = X11WindowStatePtr {
            state: Rc::new(RefCell::new(X11WindowState::new(
                handle,
                client,
                executor,
                gpu_context,
                compositor_gpu,
                params,
                xcb,
                client_side_decorations_supported,
                x_main_screen_index,
                x_window,
                atoms,
                scale_factor,
                appearance,
                transient_owner,
                supports_xinput_gestures,
                is_bgr,
            )?)),
            callbacks: Rc::new(RefCell::new(Callbacks::default())),
            xcb: xcb.clone(),
            x_window,
        };

        let window = Self(ptr, Rc::new(()));
        window.0.initialize_window_state_observation()?;
        Ok(window)
    }

    fn set_wm_hints<C: Display + Send + Sync + 'static, F: FnOnce() -> C>(
        &self,
        failure_context: F,
        wm_hint_property_state: WmHintPropertyState,
        prop1: u32,
        prop2: u32,
    ) -> anyhow::Result<()> {
        let state = self.0.state.borrow();
        let message = ClientMessageEvent::new(
            32,
            self.0.x_window,
            state.atoms._NET_WM_STATE,
            [wm_hint_property_state as u32, prop1, prop2, 1, 0],
        );
        check_reply(
            failure_context,
            self.0.xcb.send_event(
                false,
                state.x_root_window,
                xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                message,
            ),
        )?;
        xcb_flush(&self.0.xcb);
        Ok(())
    }
}

impl X11WindowStatePtr {
    pub(crate) fn interaction_is_quiesced(&self) -> bool {
        self.state.borrow().interaction_gate.is_quiesced()
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }

    fn terminate_callback_slots(&self) {
        self.state.borrow().interaction_gate.quiesce();
        let (input_callback, should_close) = {
            let mut callbacks = self.callbacks.borrow_mut();
            callbacks.visibility_change = None;
            (callbacks.input.clone(), callbacks.should_close.clone())
        };
        let (input_handler, sync_task, abandoned_sync_submission) = {
            let mut state = self.state.borrow_mut();
            let abandoned_sync_submission = state.observation.close();
            (
                state.input_handler.clone(),
                state.sync_acknowledgement_task.take(),
                abandoned_sync_submission,
            )
        };
        drop(sync_task);
        if let Some(submission) = abandoned_sync_submission {
            discard_sync_acknowledgement(&self.xcb, submission);
        }
        input_callback.terminate();
        input_handler.terminate();
        should_close.terminate();
    }

    pub fn should_close(&self) -> bool {
        let should_close = self.callbacks.borrow().should_close.clone();
        should_close.invoke()
    }

    fn initialize_window_state_observation(&self) -> anyhow::Result<()> {
        let attempt = self
            .state
            .borrow_mut()
            .observation
            .invalidate_property(X11PropertyDomain::WindowState)
            .context("X11 window closed while initializing its window-state observation")?;
        match self.sample_window_state_property() {
            Ok(facts) => {
                self.commit_property_observation(
                    attempt,
                    X11PropertyObservation::WindowState(facts),
                );
                Ok(())
            }
            Err(X11PropertyReadError::Retryable(error)) => Err(error),
            Err(X11PropertyReadError::Malformed(error)) => Err(anyhow!(error)),
        }
    }

    pub fn property_notify(&self, event: xproto::PropertyNotifyEvent) {
        let (domain, attempt) = {
            let mut state = self.state.borrow_mut();
            let domain = if event.atom == state.atoms._NET_WM_STATE {
                X11PropertyDomain::WindowState
            } else if event.atom == state.atoms._GTK_EDGE_CONSTRAINTS {
                X11PropertyDomain::EdgeConstraints
            } else {
                return;
            };
            let Some(attempt) = state.observation.invalidate_property(domain) else {
                return;
            };
            (domain, attempt)
        };

        if event.state == xproto::Property::DELETE {
            let observation = match domain {
                X11PropertyDomain::WindowState => {
                    X11PropertyObservation::WindowState(X11WindowStateFacts::default())
                }
                X11PropertyDomain::EdgeConstraints => X11PropertyObservation::EdgeConstraints(None),
            };
            self.commit_property_observation(attempt, observation);
        } else {
            self.observe_property_attempt(attempt);
        }
    }

    fn observe_property_attempt(&self, attempt: X11PropertyAttempt) {
        let observation = match attempt.domain {
            X11PropertyDomain::WindowState => self
                .sample_window_state_property()
                .map(X11PropertyObservation::WindowState),
            X11PropertyDomain::EdgeConstraints => self
                .sample_edge_constraints_property()
                .map(X11PropertyObservation::EdgeConstraints),
        };

        match observation {
            Ok(observation) => self.commit_property_observation(attempt, observation),
            Err(X11PropertyReadError::Retryable(error)) => {
                log::warn!(
                    "X11 {:?} observation will retry while retaining the previous coherent facts: {error:#}",
                    attempt.domain
                );
                let retry = self
                    .state
                    .borrow_mut()
                    .observation
                    .property_retryable_failure(attempt);
                if let Some(retry) = retry {
                    self.schedule_property_retry(retry);
                }
            }
            Err(X11PropertyReadError::Malformed(error)) => {
                log::error!(
                    "X11 {:?} observation is malformed; retaining the previous coherent facts until a newer PropertyNotify: {error}",
                    attempt.domain
                );
                self.state
                    .borrow_mut()
                    .observation
                    .retain_malformed_property(attempt);
            }
        }
    }

    fn sample_window_state_property(&self) -> X11PropertyRead<X11WindowStateFacts> {
        let atoms = self.state.borrow().atoms;
        let reply = match get_reply(
            || "X11 GetProperty for _NET_WM_STATE failed.",
            self.xcb.get_property(
                false,
                self.x_window,
                atoms._NET_WM_STATE,
                xproto::AtomEnum::ATOM,
                0,
                u32::MAX,
            ),
        ) {
            Ok(reply) => reply,
            Err(error) => return Err(X11PropertyReadError::Retryable(error)),
        };
        let payload = X11PropertyPayload {
            type_atom: reply.type_,
            format: reply.format,
            value_len: reply.value_len,
            bytes_after: reply.bytes_after,
            value: &reply.value,
        };
        let state_atoms = X11WindowStateAtoms {
            fullscreen: atoms._NET_WM_STATE_FULLSCREEN,
            maximized_vertical: atoms._NET_WM_STATE_MAXIMIZED_VERT,
            maximized_horizontal: atoms._NET_WM_STATE_MAXIMIZED_HORZ,
            hidden: atoms._NET_WM_STATE_HIDDEN,
        };
        match parse_x11_window_state_property(payload, xproto::AtomEnum::ATOM.into(), state_atoms) {
            Ok(facts) => Ok(facts),
            Err(error) => Err(X11PropertyReadError::Malformed(error)),
        }
    }

    fn sample_edge_constraints_property(&self) -> X11PropertyRead<Option<EdgeConstraints>> {
        let atoms = self.state.borrow().atoms;
        let reply = match get_reply(
            || "X11 GetProperty for _GTK_EDGE_CONSTRAINTS failed.",
            self.xcb.get_property(
                false,
                self.x_window,
                atoms._GTK_EDGE_CONSTRAINTS,
                xproto::AtomEnum::CARDINAL,
                0,
                4,
            ),
        ) {
            Ok(reply) => reply,
            Err(error) => return Err(X11PropertyReadError::Retryable(error)),
        };
        let payload = X11PropertyPayload {
            type_atom: reply.type_,
            format: reply.format,
            value_len: reply.value_len,
            bytes_after: reply.bytes_after,
            value: &reply.value,
        };
        match parse_x11_edge_constraints_property(payload, xproto::AtomEnum::CARDINAL.into()) {
            Ok(constraints) => Ok(constraints.map(EdgeConstraints::from_atom)),
            Err(error) => Err(X11PropertyReadError::Malformed(error)),
        }
    }

    fn commit_property_observation(
        &self,
        attempt: X11PropertyAttempt,
        observation: X11PropertyObservation,
    ) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if !state.observation.commit_property(attempt) {
                return;
            }

            let mut changed = match observation {
                X11PropertyObservation::WindowState(facts) => {
                    let previous = (
                        state.fullscreen,
                        state.maximized_vertical,
                        state.maximized_horizontal,
                        state.hidden,
                    );
                    state.fullscreen = facts.fullscreen;
                    state.maximized_vertical = facts.maximized_vertical;
                    state.maximized_horizontal = facts.maximized_horizontal;
                    state.hidden = facts.hidden;
                    previous
                        != (
                            state.fullscreen,
                            state.maximized_vertical,
                            state.maximized_horizontal,
                            state.hidden,
                        )
                }
                X11PropertyObservation::EdgeConstraints(constraints) => {
                    let changed = state.edge_constraints != constraints;
                    state.edge_constraints = constraints;
                    changed
                }
            };

            if attempt.domain == X11PropertyDomain::WindowState {
                let windowed = !state.fullscreen
                    && !state.maximized_vertical
                    && !state.maximized_horizontal
                    && !state.hidden;
                if let Some(bounds) = state.observation.settle_restore_candidate(windowed) {
                    state.restore_bounds = bounds;
                }
            }
            changed
        };

        if changed {
            self.emit_window_state_change();
        }
    }

    fn schedule_property_retry(&self, retry: X11PropertyRetry) {
        let target = self.observation_retry_target();
        self.schedule_observation_retry(retry.delay, move || target.run_property_retry(retry));
    }

    fn observation_retry_target(&self) -> X11ObservationRetryTarget {
        X11ObservationRetryTarget {
            state: Rc::downgrade(&self.state),
            client: self.state.borrow().client.clone(),
            x_window: self.x_window,
        }
    }

    fn schedule_restore_settlement(&self, settlement: X11RestoreSettlement) {
        let target = self.observation_retry_target();
        let executor = self.state.borrow().executor.clone();
        executor
            .spawn(async move {
                target.run_restore_settlement(settlement);
            })
            .detach();
    }

    pub(crate) fn record_sync_request(&self, value: sync::Int64) {
        self.state
            .borrow_mut()
            .observation
            .enqueue_sync_request(X11SyncCounterValue {
                lo: value.lo,
                hi: value.hi,
            });
    }

    fn drain_sync_acknowledgements(&self) {
        let (counter_id, acknowledgement) = {
            let state = self.state.borrow();
            (
                state.sync_counter,
                state.observation.ready_sync_acknowledgement(),
            )
        };
        let (Some(counter_id), Some(acknowledgement)) = (counter_id, acknowledgement) else {
            return;
        };

        match submit_sync_acknowledgement(&self.xcb, counter_id, acknowledgement) {
            Ok((set_counter_sequence, fence_sequence)) => {
                let observation = self
                    .state
                    .borrow_mut()
                    .observation
                    .sync_acknowledgement_submitted(
                        acknowledgement,
                        set_counter_sequence,
                        fence_sequence,
                    );
                if let Some(observation) = observation {
                    self.schedule_sync_acknowledgement_observation(observation);
                } else {
                    discard_sync_acknowledgement(
                        &self.xcb,
                        X11SyncAcknowledgementSubmission::pending(
                            acknowledgement,
                            set_counter_sequence,
                            fence_sequence,
                        ),
                    );
                }
            }
            Err(X11SyncTransportFailure::Retryable(error)) => {
                log::warn!(
                    "X11 sync acknowledgement submission will retry while retaining its protocol obligation: {error:#}"
                );
                let retry = self
                    .state
                    .borrow_mut()
                    .observation
                    .sync_acknowledgement_retryable_submission_failure(acknowledgement);
                if let Some(retry) = retry {
                    self.schedule_sync_retry(retry);
                }
            }
            Err(X11SyncTransportFailure::Permanent {
                error,
                counter_invalid,
            }) => {
                log::error!(
                    "X11 sync acknowledgement submission failed permanently; abandoning the sync lane: {error:#}"
                );
                let mut state = self.state.borrow_mut();
                if state
                    .observation
                    .sync_acknowledgement_submission_permanently_failed(acknowledgement)
                    && counter_invalid
                {
                    state.sync_counter = None;
                }
            }
        }
    }

    fn schedule_sync_retry(&self, retry: X11SyncRetry) {
        let target = self.observation_retry_target();
        self.schedule_sync_wake(retry.delay, move || target.run_sync_retry(retry));
    }

    fn schedule_sync_acknowledgement_observation(
        &self,
        observation: X11SyncAcknowledgementObservation,
    ) {
        let target = self.observation_retry_target();
        self.schedule_sync_wake(observation.delay, move || {
            target.run_sync_acknowledgement_observation(observation)
        });
    }

    fn schedule_sync_wake(&self, delay: Duration, wake: impl FnOnce() + 'static) {
        let (foreground, timer) = {
            let state = self.state.borrow();
            if state.destroyed {
                return;
            }
            let Some(client) = state.client.get_client() else {
                return;
            };
            let foreground = state.executor.clone();
            let timer = client.0.borrow().common.background_executor.timer(delay);
            (foreground, timer)
        };
        let task = foreground.spawn(async move {
            timer.await;
            wake();
        });
        let previous = self
            .state
            .borrow_mut()
            .sync_acknowledgement_task
            .replace(task);
        drop(previous);
    }

    fn schedule_observation_retry(&self, delay: Duration, retry: impl FnOnce() + 'static) {
        let (foreground, timer) = {
            let state = self.state.borrow();
            if state.destroyed {
                return;
            }
            let Some(client) = state.client.get_client() else {
                return;
            };
            let foreground = state.executor.clone();
            let timer = client.0.borrow().common.background_executor.timer(delay);
            (foreground, timer)
        };
        foreground
            .spawn(async move {
                timer.await;
                retry();
            })
            .detach();
    }

    pub(crate) fn set_visibility(&self, visibility: WindowVisibility, mapped: Option<bool>) {
        let (mapped_changed, visibility_changed) = {
            let mut state = self.state.borrow_mut();
            let mapped_changed = if let Some(mapped) = mapped {
                let hidden = !mapped;
                let changed = state.hidden != hidden || state.mapped != mapped;
                state.hidden = hidden;
                state.mapped = mapped;
                changed
            } else {
                false
            };
            let visibility_changed =
                std::mem::replace(&mut state.visibility, visibility) != visibility;
            (mapped_changed, visibility_changed)
        };
        if mapped_changed {
            self.emit_window_state_change();
        }
        if !visibility_changed || self.interaction_is_quiesced() {
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

    fn emit_window_state_change(&self) {
        let callback = self.callbacks.borrow().window_state_change.clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    pub fn close(&self) {
        self.terminate_callback_slots();
        let callback = self.callbacks.borrow_mut().close.take();
        if let Some(fun) = callback {
            fun()
        }
    }

    pub fn refresh(&self, request_frame_options: RequestFrameOptions) {
        let callback = self.callbacks.borrow().request_frame.clone();
        if let Some(fun) = callback {
            fun(request_frame_options);
        }
    }

    pub fn handle_input(&self, input: PlatformInput) {
        if self.interaction_is_quiesced() {
            return;
        }
        let input_callback = self.callbacks.borrow().input.clone();
        if !input_callback.dispatch(input.clone()).propagate {
            return;
        }
        if self.interaction_is_quiesced() {
            return;
        }
        if let PlatformInput::KeyDown(event) = input {
            // only allow shift modifier when inserting text
            if event.keystroke.modifiers.is_subset_of(&Modifiers::shift())
                && let Some(key_char) = &event.keystroke.key_char
            {
                let input_handler = self.state.borrow().input_handler.clone();
                let _ = input_handler.with_handler(|input_handler| {
                    input_handler.replace_text_in_range(None, key_char)
                });
            }
        }
    }

    pub fn handle_ime_commit(&self, text: String) {
        if self.interaction_is_quiesced() {
            return;
        }
        let input_handler = self.state.borrow().input_handler.clone();
        let _ = input_handler
            .with_handler(|input_handler| input_handler.replace_text_in_range(None, &text));
    }

    pub fn handle_ime_preedit(&self, text: String) {
        if self.interaction_is_quiesced() {
            return;
        }
        let input_handler = self.state.borrow().input_handler.clone();
        let _ = input_handler.with_handler(|input_handler| {
            input_handler.replace_and_mark_text_in_range(None, &text, None)
        });
    }

    pub fn handle_ime_unmark(&self) {
        if self.interaction_is_quiesced() {
            return;
        }
        let input_handler = self.state.borrow().input_handler.clone();
        let _ = input_handler.with_handler(|input_handler| input_handler.unmark_text());
    }

    pub fn handle_ime_delete(&self) {
        if self.interaction_is_quiesced() {
            return;
        }
        let input_handler = self.state.borrow().input_handler.clone();
        let _ =
            input_handler.with_handler(|input_handler| match input_handler.marked_text_range() {
                NativeInputHandlerOutcome::Delivered(Some(marked)) => {
                    input_handler.replace_text_in_range(Some(marked), "")
                }
                NativeInputHandlerOutcome::Delivered(None) => {
                    NativeInputHandlerOutcome::Delivered(())
                }
                NativeInputHandlerOutcome::StaleWindow => NativeInputHandlerOutcome::StaleWindow,
                NativeInputHandlerOutcome::Quitting => NativeInputHandlerOutcome::Quitting,
            });
    }

    pub fn get_ime_area(&self) -> Option<Bounds<ScaledPixels>> {
        let (scale_factor, input_handler) = {
            let state = self.state.borrow();
            if state.interaction_gate.is_quiesced() {
                return None;
            }
            (state.scale_factor, state.input_handler.clone())
        };
        let bounds = input_handler
            .with_handler(|input_handler| {
                input_handler
                    .selected_text_range(true)
                    .and_then(|selection| match selection {
                        Some(selection) => input_handler.bounds_for_range(selection.range),
                        None => NativeInputHandlerOutcome::Delivered(None),
                    })
            })
            .and_then(NativeInputHandlerOutcome::into_delivered)
            .flatten();
        bounds.map(|b| b.scale(scale_factor))
    }

    pub fn configure_notify(&self, bounds: Bounds<DevicePixels>) {
        let (is_resize, content_size, scale_factor, restore_settlement) = {
            let mut state = self.state.borrow_mut();
            let geometry =
                reconcile_x11_configure_geometry(state.bounds, bounds, state.scale_factor);
            state.bounds = geometry.bounds;
            state.renderer.update_drawable_size(geometry.device_size);
            let restore_settlement = state.observation.stage_restore_candidate(geometry.bounds);
            state.observation.configure_committed();
            (
                geometry.resized,
                state.content_size(),
                state.scale_factor,
                restore_settlement,
            )
        };

        if let Some(restore_settlement) = restore_settlement {
            self.schedule_restore_settlement(restore_settlement);
        }

        let (resize_callback, moved_callback) = {
            let callbacks = self.callbacks.borrow();
            (callbacks.resize.clone(), callbacks.moved.clone())
        };
        if let Some(callback) = resize_callback {
            callback(content_size, scale_factor);
        }

        if !is_resize {
            if let Some(callback) = moved_callback {
                callback();
            }
        }
    }

    pub fn set_active(&self, observation: PlatformWindowActiveStatusObservation) {
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
        let callback = self.callbacks.borrow().hovered_status_change.clone();
        if let Some(fun) = callback {
            fun(focus);
        }
        if focus && interaction_gate.is_quiesced() {
            self.state.borrow_mut().hovered = false;
        }
    }

    pub fn set_appearance(&mut self, appearance: WindowAppearance) {
        let mut state = self.state.borrow_mut();
        state.appearance = appearance;
        let is_transparent = state.is_transparent();
        state.renderer.update_transparency(is_transparent);
        state.appearance = appearance;
        drop(state);
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
}

impl X11ObservationRetryTarget {
    fn live_window(&self) -> Option<X11WindowStatePtr> {
        let state = self.state.upgrade()?;
        let client = self.client.get_client()?;
        let current = client.get_window(self.x_window)?;
        if !Rc::ptr_eq(&current.state, &state) {
            return None;
        }
        Some(current)
    }

    fn run_property_retry(&self, retry: X11PropertyRetry) {
        let Some(window) = self.live_window() else {
            return;
        };
        let Some(attempt) = window
            .state
            .borrow_mut()
            .observation
            .claim_property_retry(retry)
        else {
            return;
        };
        window.observe_property_attempt(attempt);
    }

    fn run_restore_settlement(&self, settlement: X11RestoreSettlement) {
        let Some(window) = self.live_window() else {
            return;
        };
        let attempt = {
            let mut state = window.state.borrow_mut();
            if !state.observation.claim_restore_settlement(settlement) {
                return;
            }
            let Some(attempt) = state
                .observation
                .invalidate_property(X11PropertyDomain::WindowState)
            else {
                return;
            };
            attempt
        };
        window.observe_property_attempt(attempt);
    }

    fn run_sync_acknowledgement_observation(&self, observation: X11SyncAcknowledgementObservation) {
        let Some(window) = self.live_window() else {
            return;
        };
        let Some(submission) = window
            .state
            .borrow_mut()
            .observation
            .claim_sync_acknowledgement_observation(observation)
        else {
            return;
        };

        match observe_sync_acknowledgement(&window.xcb, submission) {
            X11SyncTerminalObservation::Pending(pending_submission) => {
                let observation = window
                    .state
                    .borrow_mut()
                    .observation
                    .sync_acknowledgement_observation_pending(submission, pending_submission);
                if let Some(observation) = observation {
                    window.schedule_sync_acknowledgement_observation(observation);
                }
            }
            X11SyncTerminalObservation::Succeeded => {
                if window
                    .state
                    .borrow_mut()
                    .observation
                    .sync_acknowledgement_succeeded(submission)
                {
                    window.drain_sync_acknowledgements();
                }
            }
            X11SyncTerminalObservation::Failed {
                submission: transport_submission,
                failure: X11SyncTransportFailure::Retryable(error),
            } => {
                discard_sync_acknowledgement(&window.xcb, transport_submission);
                log::warn!(
                    "X11 sync acknowledgement observation will retry while retaining its protocol obligation: {error:#}"
                );
                let retry = window
                    .state
                    .borrow_mut()
                    .observation
                    .sync_acknowledgement_retryable_observation_failure(submission);
                if let Some(retry) = retry {
                    window.schedule_sync_retry(retry);
                }
            }
            X11SyncTerminalObservation::Failed {
                submission: transport_submission,
                failure:
                    X11SyncTransportFailure::Permanent {
                        error,
                        counter_invalid,
                    },
            } => {
                discard_sync_acknowledgement(&window.xcb, transport_submission);
                log::error!(
                    "X11 sync acknowledgement failed permanently; abandoning the sync lane: {error:#}"
                );
                let mut state = window.state.borrow_mut();
                if state
                    .observation
                    .sync_acknowledgement_permanently_failed(submission)
                    && counter_invalid
                {
                    state.sync_counter = None;
                }
            }
        }
    }

    fn run_sync_retry(&self, retry: X11SyncRetry) {
        let Some(window) = self.live_window() else {
            return;
        };
        if !window
            .state
            .borrow_mut()
            .observation
            .claim_sync_retry(retry)
        {
            return;
        }
        window.drain_sync_acknowledgements();
    }
}

impl PlatformWindow for X11Window {
    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        let target = X11WindowCommandTarget::new(self);
        PlatformWindowCommandDispatcher::new(move |command| target.dispatch(command))
    }

    fn interaction_quiescence(&self) -> PlatformWindowInteractionQuiescence {
        let state = Rc::downgrade(&self.0.state);
        let interaction_gate = self.0.state.borrow().interaction_gate.clone();
        let client = self.0.state.borrow().client.clone();
        let x_window = self.0.x_window;
        PlatformWindowInteractionQuiescence::new(move || {
            interaction_gate.quiesce();
            if let Some(state) = state.upgrade() {
                let mut state = state.borrow_mut();
                state.active = false;
                state.hovered = false;
                state.activation_policy = WindowActivationPolicy {
                    accepts_activation: false,
                    focus_on_click: false,
                };
                if let Some(adapter) = state.accesskit_adapter.as_mut() {
                    adapter.update_window_focus_state(false);
                }
            }
            client.quiesce_window_interaction(x_window);
        })
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
                WgpuSurfaceShutdownProgress::Quiesced => {
                    PlatformPresentationShutdownOutcome::Quiesced
                }
                WgpuSurfaceShutdownProgress::EnteredDraining
                | WgpuSurfaceShutdownProgress::Draining
                | WgpuSurfaceShutdownProgress::Rejected => {
                    PlatformPresentationShutdownOutcome::Rejected
                }
            }
        })
    }

    fn retire_native_window(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformNativeWindowRetirementOutcome {
        if !shutdown.snapshot().quiesced() {
            return PlatformNativeWindowRetirementOutcome::Rejected;
        }
        let Ok(mut state) = self.0.state.try_borrow_mut() else {
            return PlatformNativeWindowRetirementOutcome::Rejected;
        };
        if shutdown.snapshot().window_id() != state.handle.window_id()
            || !state.renderer.is_quiesced_for(shutdown)
        {
            return PlatformNativeWindowRetirementOutcome::Rejected;
        }
        if state.destroyed && state.sync_counter.is_none() {
            return PlatformNativeWindowRetirementOutcome::NativeWindowTerminal;
        }

        if !state.destroyed {
            if let Err(error) = check_reply(
                || "X11 DestroyWindow failed while retiring a quiesced native window.",
                self.0.xcb.destroy_window(self.0.x_window),
            ) {
                log::warn!("X11 native-window retirement will retry: {error:#}");
                return PlatformNativeWindowRetirementOutcome::Rejected;
            }
            state.destroyed = true;
        }
        let abandoned_sync_submission = state.observation.close();
        let sync_task = state.sync_acknowledgement_task.take();
        drop(sync_task);
        if let Some(submission) = abandoned_sync_submission {
            discard_sync_acknowledgement(&self.0.xcb, submission);
        }
        if let Some(counter_id) = state.sync_counter {
            if let Err(error) = check_reply(
                || "X11 DestroyCounter failed while retiring a quiesced native window.",
                sync::destroy_counter(&self.0.xcb, counter_id),
            ) {
                log::warn!("X11 sync-counter retirement will retry: {error:#}");
                xcb_flush(&self.0.xcb);
                return PlatformNativeWindowRetirementOutcome::Rejected;
            }
            state.sync_counter = None;
        }
        xcb_flush(&self.0.xcb);
        PlatformNativeWindowRetirementOutcome::Accepted
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.0.state.borrow().bounds
    }

    fn is_maximized(&self) -> bool {
        let state = self.0.state.borrow();

        // A maximized window that gets minimized will still retain its maximized state.
        state.maximized_vertical && state.maximized_horizontal
    }

    fn is_minimized(&self) -> bool {
        self.0.state.borrow().hidden
    }

    fn creation_facts(&self) -> WindowCreationFacts {
        let state = self.0.state.borrow();
        WindowCreationFacts {
            show: state.initially_shown,
            focus_on_appearing: state.focus_on_appearing,
            transient_for: state.transient_for,
        }
    }

    fn is_visible(&self) -> bool {
        self.0.state.borrow().mapped
    }

    fn platform_facts(&self) -> open_gpui::WindowPlatformFacts {
        let window_bounds = self.window_bounds();
        let (activation_policy, interaction_quiesced, topmost, taskbar_visible) = {
            let state = self.0.state.borrow();
            (
                state.activation_policy,
                state.interaction_gate.is_quiesced(),
                state.topmost,
                state.taskbar_visible,
            )
        };
        open_gpui::WindowPlatformFacts {
            bounds: self.bounds(),
            coordinate_space: open_gpui::WindowCoordinateSpace::WindowLocal,
            physical_geometry: None,
            window_bounds,
            inner_window_bounds: self.inner_window_bounds(),
            content_size: self.content_size(),
            scale_factor: self.scale_factor(),
            display_id: self.display().map(|display| display.id()),
            is_minimized: self.is_minimized(),
            is_maximized: self.is_maximized(),
            is_fullscreen: self.is_fullscreen(),
            accepts_pointer_input: self.accepts_pointer_input(),
            accepts_activation: !interaction_quiesced && activation_policy.accepts_activation,
            focus_on_click: !interaction_quiesced && activation_policy.focus_on_click,
            background_appearance: self.background_appearance(),
            topmost,
            taskbar_visible,
            is_active: self.is_active(),
        }
    }

    fn window_bounds(&self) -> WindowBounds {
        let state = self.0.state.borrow();
        if state.fullscreen {
            WindowBounds::Fullscreen(state.restore_bounds)
        } else if state.maximized_vertical && state.maximized_horizontal {
            WindowBounds::Maximized(state.restore_bounds)
        } else {
            WindowBounds::Windowed(state.bounds)
        }
    }

    fn inner_window_bounds(&self) -> WindowBounds {
        let state = self.0.state.borrow();
        if state.fullscreen {
            WindowBounds::Fullscreen(state.restore_bounds)
        } else if state.maximized_vertical && state.maximized_horizontal {
            WindowBounds::Maximized(state.restore_bounds)
        } else {
            let mut bounds = state.bounds;
            let [left, right, top, bottom] = state.last_insets;

            let [left, right, top, bottom] = [
                px((left as f32) / state.scale_factor),
                px((right as f32) / state.scale_factor),
                px((top as f32) / state.scale_factor),
                px((bottom as f32) / state.scale_factor),
            ];

            bounds.origin.x += left;
            bounds.origin.y += top;
            bounds.size.width -= left + right;
            bounds.size.height -= top + bottom;

            WindowBounds::Windowed(bounds)
        }
    }

    fn content_size(&self) -> Size<Pixels> {
        // After the wgpu migration, X11WindowState::content_size() returns logical pixels
        // (bounds.size is already divided by scale_factor in set_bounds), so no further
        // division is needed here. This matches the Wayland implementation.
        self.0.state.borrow().content_size()
    }

    fn scale_factor(&self) -> f32 {
        self.0.state.borrow().scale_factor
    }

    fn appearance(&self) -> WindowAppearance {
        self.0.state.borrow().appearance
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.0.state.borrow().display.clone())
    }

    fn mouse_position(&self) -> Point<Pixels> {
        let scale_factor = self.0.state.borrow().scale_factor;
        get_reply(
            || "X11 QueryPointer failed.",
            self.0.xcb.query_pointer(self.0.x_window),
        )
        .log_err()
        .map_or(Point::new(Pixels::ZERO, Pixels::ZERO), |reply| {
            point_from_x11_window_coords(reply.win_x, reply.win_y, scale_factor)
        })
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        let client = self.0.state.borrow().client.clone();
        client.set_cursor_style_for_window(self.0.x_window, style);
    }

    fn modifiers(&self) -> Modifiers {
        self.0
            .state
            .borrow()
            .client
            .0
            .upgrade()
            .map(|ref_cell| ref_cell.borrow().modifiers)
            .unwrap_or_default()
    }

    fn capslock(&self) -> open_gpui::Capslock {
        self.0
            .state
            .borrow()
            .client
            .0
            .upgrade()
            .map(|ref_cell| ref_cell.borrow().capslock)
            .unwrap_or_default()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        let input_handler_slot = self.0.state.borrow().input_handler.clone();
        input_handler_slot.set(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        let input_handler_slot = self.0.state.borrow().input_handler.clone();
        input_handler_slot.take()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        None
    }

    fn is_active(&self) -> bool {
        let state = self.0.state.borrow();
        state.active && !state.interaction_gate.is_quiesced()
    }

    fn visibility(&self) -> WindowVisibility {
        self.0.state.borrow().visibility
    }

    fn is_hovered(&self) -> bool {
        let state = self.0.state.borrow();
        state.hovered && !state.interaction_gate.is_quiesced()
    }

    fn set_title(&mut self, title: &str) {
        check_reply(
            || "X11 ChangeProperty8 on WM_NAME failed.",
            self.0.xcb.change_property8(
                xproto::PropMode::REPLACE,
                self.0.x_window,
                xproto::AtomEnum::WM_NAME,
                xproto::AtomEnum::STRING,
                title.as_bytes(),
            ),
        )
        .log_err();

        check_reply(
            || "X11 ChangeProperty8 on _NET_WM_NAME failed.",
            self.0.xcb.change_property8(
                xproto::PropMode::REPLACE,
                self.0.x_window,
                self.0.state.borrow().atoms._NET_WM_NAME,
                self.0.state.borrow().atoms.UTF8_STRING,
                title.as_bytes(),
            ),
        )
        .log_err();
        xcb_flush(&self.0.xcb);
    }

    fn set_app_id(&mut self, app_id: &str) {
        let mut data = Vec::with_capacity(app_id.len() * 2 + 1);
        data.extend(app_id.bytes()); // instance https://unix.stackexchange.com/a/494170
        data.push(b'\0');
        data.extend(app_id.bytes()); // class

        check_reply(
            || "X11 ChangeProperty8 for WM_CLASS failed.",
            self.0.xcb.change_property8(
                xproto::PropMode::REPLACE,
                self.0.x_window,
                xproto::AtomEnum::WM_CLASS,
                xproto::AtomEnum::STRING,
                &data,
            ),
        )
        .log_err();
    }

    fn map_window(&mut self) -> anyhow::Result<()> {
        if !self.0.state.borrow().initially_shown {
            return Ok(());
        }
        check_reply(
            || "X11 MapWindow failed.",
            self.0.xcb.map_window(self.0.x_window),
        )?;
        Ok(())
    }

    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        let mut state = self.0.state.borrow_mut();
        let projection =
            X11WindowBackgroundProjection::new(background_appearance, state.alpha_capable);
        state.background_appearance = projection.observed_appearance;
        state
            .renderer
            .update_transparency(projection.renderer_transparent);
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.0.state.borrow().background_appearance
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        self.0
            .state
            .borrow()
            .client
            .0
            .upgrade()
            .map(|ref_cell| {
                let state = ref_cell.borrow();
                state
                    .gpu_context
                    .borrow()
                    .as_ref()
                    .is_some_and(|ctx| ctx.supports_dual_source_blending())
            })
            .unwrap_or_default()
    }

    fn is_fullscreen(&self) -> bool {
        self.0.state.borrow().fullscreen
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
        self.0.callbacks.borrow_mut().hovered_status_change = Some(Rc::from(callback));
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

    fn on_display_changed(&self, _callback: Box<dyn FnMut()>) {}

    fn on_window_state_change(&self, callback: Box<dyn Fn()>) {
        self.0.callbacks.borrow_mut().window_state_change = Some(Rc::from(callback));
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

    fn on_appearance_changed(&self, callback: Box<dyn Fn()>) {
        self.0.callbacks.borrow_mut().appearance_changed = Some(Rc::from(callback));
    }

    fn on_button_layout_changed(&self, callback: Box<dyn Fn()>) {
        self.0.callbacks.borrow_mut().button_layout_changed = Some(Rc::from(callback));
    }

    fn draw(&self, scene: &Scene) -> PlatformWindowPresentOutcome {
        let outcome = {
            let mut inner = self.0.state.borrow_mut();

            if inner.renderer.device_lost() {
                let raw_window = RawWindow {
                    connection: as_raw_xcb_connection::AsRawXcbConnection::as_raw_xcb_connection(
                        &*self.0.xcb,
                    ) as *mut _,
                    screen_id: inner.x_screen_index,
                    window_id: self.0.x_window,
                    visual_id: inner.visual_id,
                };
                match inner.renderer.recover(&raw_window) {
                    Ok(()) => PlatformWindowPresentOutcome::RepaintRequired,
                    Err(err) => {
                        log::warn!("GPU recovery failed, will retry on next frame: {err}");
                        PlatformWindowPresentOutcome::Deferred
                    }
                }
            } else {
                inner.renderer.draw(scene)
            }
        };

        if outcome == PlatformWindowPresentOutcome::Submitted {
            self.0
                .state
                .borrow_mut()
                .observation
                .presentation_submitted();
            self.0.drain_sync_acknowledgements();
        }

        outcome
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        let inner = self.0.state.borrow();
        inner.renderer.sprite_atlas().clone()
    }

    fn window_decorations(&self) -> open_gpui::Decorations {
        let state = self.0.state.borrow();

        // Client window decorations require compositor support
        if !state.client_side_decorations_supported {
            return Decorations::Server;
        }

        match state.decorations {
            WindowDecorations::Server => Decorations::Server,
            WindowDecorations::Client => {
                let tiling = if state.fullscreen {
                    Tiling::tiled()
                } else if let Some(edge_constraints) = &state.edge_constraints {
                    edge_constraints.to_tiling()
                } else {
                    // https://source.chromium.org/chromium/chromium/src/+/main:ui/ozone/platform/x11/x11_window.cc;l=2519;drc=1f14cc876cc5bf899d13284a12c451498219bb2d
                    Tiling {
                        top: state.maximized_vertical,
                        bottom: state.maximized_vertical,
                        left: state.maximized_horizontal,
                        right: state.maximized_horizontal,
                    }
                };
                Decorations::Client { tiling }
            }
        }
    }

    fn set_client_inset(&self, inset: Pixels) {
        let mut state = self.0.state.borrow_mut();

        let dp = (f32::from(inset) * state.scale_factor) as u32;

        let insets = if state.fullscreen {
            [0, 0, 0, 0]
        } else if let Some(edge_constraints) = &state.edge_constraints {
            let left = if edge_constraints.left_tiled { 0 } else { dp };
            let top = if edge_constraints.top_tiled { 0 } else { dp };
            let right = if edge_constraints.right_tiled { 0 } else { dp };
            let bottom = if edge_constraints.bottom_tiled { 0 } else { dp };

            [left, right, top, bottom]
        } else {
            let (left, right) = if state.maximized_horizontal {
                (0, 0)
            } else {
                (dp, dp)
            };
            let (top, bottom) = if state.maximized_vertical {
                (0, 0)
            } else {
                (dp, dp)
            };
            [left, right, top, bottom]
        };

        if state.last_insets != insets {
            state.last_insets = insets;

            check_reply(
                || "X11 ChangeProperty for _GTK_FRAME_EXTENTS failed.",
                self.0.xcb.change_property(
                    xproto::PropMode::REPLACE,
                    self.0.x_window,
                    state.atoms._GTK_FRAME_EXTENTS,
                    xproto::AtomEnum::CARDINAL,
                    size_of::<u32>() as u8 * 8,
                    4,
                    bytemuck::cast_slice::<u32, u8>(&insets),
                ),
            )
            .log_err();
        }
    }

    fn request_decorations(&self, mut decorations: open_gpui::WindowDecorations) {
        let mut state = self.0.state.borrow_mut();

        if matches!(decorations, open_gpui::WindowDecorations::Client)
            && !state.client_side_decorations_supported
        {
            log::info!(
                "x11: no compositor present, falling back to server-side window decorations"
            );
            decorations = open_gpui::WindowDecorations::Server;
        }

        // https://github.com/rust-windowing/winit/blob/master/src/platform_impl/linux/x11/util/hint.rs#L53-L87
        let hints_data: [u32; 5] = match decorations {
            WindowDecorations::Server => [1 << 1, 0, 1, 0, 0],
            WindowDecorations::Client => [1 << 1, 0, 0, 0, 0],
        };

        let success = check_reply(
            || "X11 ChangeProperty for _MOTIF_WM_HINTS failed.",
            self.0.xcb.change_property(
                xproto::PropMode::REPLACE,
                self.0.x_window,
                state.atoms._MOTIF_WM_HINTS,
                state.atoms._MOTIF_WM_HINTS,
                size_of::<u32>() as u8 * 8,
                5,
                bytemuck::cast_slice::<u32, u8>(&hints_data),
            ),
        )
        .log_err();

        let Some(()) = success else {
            return;
        };

        match decorations {
            WindowDecorations::Server => {
                state.decorations = WindowDecorations::Server;
                let is_transparent = state.is_transparent();
                state.renderer.update_transparency(is_transparent);
            }
            WindowDecorations::Client => {
                state.decorations = WindowDecorations::Client;
                let is_transparent = state.is_transparent();
                state.renderer.update_transparency(is_transparent);
            }
        }

        drop(state);
        let appearance_changed = self.0.callbacks.borrow().appearance_changed.clone();
        if let Some(appearance_changed) = appearance_changed {
            appearance_changed();
        }
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        let state = self.0.state.borrow();
        if state.interaction_gate.is_quiesced() {
            return;
        }
        let client = state.client.clone();
        drop(state);
        client.update_ime_position(bounds);
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.0.state.borrow().renderer.gpu_specs().into()
    }

    fn play_system_bell(&self) {
        // Volume 0% means don't increase or decrease from system volume
        let _ = self.0.xcb.bell(0);
    }

    fn a11y_init(&self, callbacks: open_gpui::A11yCallbacks) {
        let interaction_gate = self.0.state.borrow().interaction_gate.clone();
        let adapter = accesskit_adapter(callbacks, interaction_gate);
        self.0.state.borrow_mut().accesskit_adapter = Some(adapter);
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        let mut state = self.0.state.borrow_mut();
        if let Some(adapter) = state.accesskit_adapter.as_mut() {
            adapter.update_if_active(|| tree_update);
        }
    }

    fn a11y_update_window_bounds(&self) {
        let mut state = self.0.state.borrow_mut();
        let scale = state.scale_factor;
        let bounds = state.bounds;
        let [left, right, top, bottom] = state.last_insets;

        let x = f32::from(bounds.origin.x);
        let y = f32::from(bounds.origin.y);
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);

        let outer = accesskit::Rect {
            x0: (x * scale) as f64,
            y0: (y * scale) as f64,
            x1: ((x + width) * scale) as f64,
            y1: ((y + height) * scale) as f64,
        };

        let inner = accesskit::Rect {
            x0: (x * scale) as f64 + left as f64,
            y0: (y * scale) as f64 + top as f64,
            x1: ((x + width) * scale) as f64 - right as f64,
            y1: ((y + height) * scale) as f64 - bottom as f64,
        };

        if let Some(adapter) = state.accesskit_adapter.as_mut() {
            adapter.set_root_window_bounds(outer, inner);
        }
    }
}

#[cfg(test)]
mod creation_projection_tests {
    use super::*;
    use open_gpui::{point, size};

    fn sync_submission() -> X11SyncAcknowledgementSubmission {
        X11SyncAcknowledgementSubmission::pending(X11SyncCounterValue { lo: 7, hi: 0 }, 41, 42)
    }

    fn bad_counter_error() -> x11rb::x11_utils::X11Error {
        x11rb::x11_utils::X11Error {
            error_kind: x11rb::protocol::ErrorKind::SyncCounter,
            error_code: 0,
            sequence: 1,
            bad_value: 17,
            minor_opcode: sync::SET_COUNTER_REQUEST.into(),
            major_opcode: 0,
            extension_name: Some("SYNC".into()),
            request_name: Some("SetCounter"),
        }
    }

    fn restore_bounds() -> Bounds<Pixels> {
        Bounds::new(point(px(10.0), px(20.0)), size(px(640.0), px(480.0)))
    }

    #[test]
    fn screen_resolution_uses_valid_targets_and_falls_back_without_panicking() {
        assert_eq!(
            resolve_x11_screen_index(Some(DisplayId::from(1)), 0, 2),
            Some(1)
        );
        assert_eq!(
            resolve_x11_screen_index(Some(DisplayId::from(9)), 0, 2),
            Some(0)
        );
        assert_eq!(
            resolve_x11_screen_index(Some(DisplayId::from(9)), 3, 2),
            None
        );
    }

    #[test]
    fn creation_projection_drives_position_size_state_restore_and_alpha_visual() {
        let restore_bounds = restore_bounds();
        let cases = [
            (
                WindowBounds::Windowed(restore_bounds),
                X11InitialWindowState::Windowed,
            ),
            (
                WindowBounds::Maximized(restore_bounds),
                X11InitialWindowState::Maximized,
            ),
            (
                WindowBounds::Fullscreen(restore_bounds),
                X11InitialWindowState::Fullscreen,
            ),
        ];

        for (window_bounds, expected_state) in cases {
            let projection = X11WindowCreationProjection::new(
                window_bounds,
                &WindowKind::Normal,
                2.0,
                true,
                true,
                WindowActivationPolicy::default(),
            );
            assert_eq!(projection.device_bounds.origin.x.0, 20);
            assert_eq!(projection.device_bounds.origin.y.0, 40);
            assert_eq!(projection.device_bounds.size.width.0, 1280);
            assert_eq!(projection.device_bounds.size.height.0, 960);
            assert_eq!(projection.create_x(), 22);
            assert_eq!(projection.create_y(), 40);
            assert_eq!(projection.restore_bounds, restore_bounds);
            assert_eq!(projection.initial_state, expected_state);
            assert!(projection.alpha_capable);
            assert!(projection.focus_on_appearing);
            assert_eq!(
                projection.activation_policy,
                WindowActivationPolicy::default()
            );
            assert!(!projection.topmost);
            assert!(projection.taskbar_visible);
        }
    }

    #[test]
    fn popup_projection_does_not_emit_unmanaged_toplevel_state() {
        let projection = X11WindowCreationProjection::new(
            WindowBounds::Fullscreen(restore_bounds()),
            &WindowKind::PopUp,
            1.0,
            true,
            true,
            WindowActivationPolicy::default(),
        );

        assert_eq!(projection.initial_state, X11InitialWindowState::Windowed);
        assert_eq!(projection.restore_bounds, restore_bounds());
        assert!(projection.focus_on_appearing);
        assert_eq!(
            projection.activation_policy,
            WindowActivationPolicy {
                accepts_activation: true,
                focus_on_click: false,
            }
        );
        assert!(!projection.topmost);
        assert!(!projection.taskbar_visible);

        let dialog = X11WindowCreationProjection::new(
            WindowBounds::Maximized(restore_bounds()),
            &WindowKind::Dialog,
            1.0,
            true,
            true,
            WindowActivationPolicy::default(),
        );
        assert_eq!(dialog.initial_state, X11InitialWindowState::Windowed);
    }

    #[test]
    fn creation_projection_does_not_claim_x11_click_focus_control() {
        let projection = X11WindowCreationProjection::new(
            WindowBounds::Windowed(restore_bounds()),
            &WindowKind::Normal,
            1.0,
            true,
            false,
            WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: false,
            },
        );

        assert!(!projection.focus_on_appearing);
        assert_eq!(
            projection.activation_policy,
            WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: true,
            }
        );
    }

    #[test]
    fn zero_sized_creation_projection_uses_the_native_fallback_as_restore_bounds() {
        let projection = X11WindowCreationProjection::new(
            WindowBounds::Maximized(Bounds::new(point(px(4.0), px(8.0)), size(px(0.0), px(0.0)))),
            &WindowKind::Normal,
            2.0,
            true,
            true,
            WindowActivationPolicy::default(),
        );

        assert_eq!(projection.device_bounds.size.width.0, 800);
        assert_eq!(projection.device_bounds.size.height.0, 600);
        assert_eq!(projection.restore_bounds.size, size(px(400.0), px(300.0)));
    }

    #[test]
    fn background_projection_reports_actual_alpha_visual_result() {
        let transparent =
            X11WindowBackgroundProjection::new(WindowBackgroundAppearance::Transparent, true);
        assert_eq!(
            transparent.observed_appearance,
            WindowBackgroundAppearance::Transparent
        );
        assert!(transparent.renderer_transparent);

        let blurred = X11WindowBackgroundProjection::new(WindowBackgroundAppearance::Blurred, true);
        assert_eq!(
            blurred.observed_appearance,
            WindowBackgroundAppearance::Transparent
        );
        assert!(blurred.renderer_transparent);

        for requested in [
            WindowBackgroundAppearance::MicaBackdrop,
            WindowBackgroundAppearance::MicaAltBackdrop,
        ] {
            let adjusted = X11WindowBackgroundProjection::new(requested, true);
            assert_eq!(
                adjusted.observed_appearance,
                WindowBackgroundAppearance::Transparent
            );
            assert!(adjusted.renderer_transparent);
        }

        let adjusted =
            X11WindowBackgroundProjection::new(WindowBackgroundAppearance::Transparent, false);
        assert_eq!(
            adjusted.observed_appearance,
            WindowBackgroundAppearance::Opaque
        );
        assert!(!adjusted.renderer_transparent);
    }

    #[test]
    fn sync_transport_retries_only_connection_unavailability() {
        let unavailable = classify_sync_connection_failure(
            "X11 sync SetCounter submission failed.",
            ConnectionError::IoError(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "transport unavailable",
            )),
        );
        assert!(matches!(unavailable, X11SyncTransportFailure::Retryable(_)));

        let unsupported = classify_sync_connection_failure(
            "X11 sync SetCounter submission failed.",
            ConnectionError::UnsupportedExtension,
        );
        assert!(matches!(
            unsupported,
            X11SyncTransportFailure::Permanent {
                counter_invalid: false,
                ..
            }
        ));

        let exhausted = classify_sync_connection_failure(
            "X11 sync SetCounter submission failed.",
            ConnectionError::InsufficientMemory,
        );
        assert!(matches!(
            exhausted,
            X11SyncTransportFailure::Permanent {
                counter_invalid: false,
                ..
            }
        ));

        let disconnected = classify_sync_connection_failure(
            "X11 sync SetCounter submission failed.",
            ConnectionError::IoError(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "connection closed",
            )),
        );
        assert!(matches!(
            disconnected,
            X11SyncTransportFailure::Permanent {
                counter_invalid: false,
                ..
            }
        ));
    }

    #[test]
    fn bad_counter_is_a_permanent_resource_failure() {
        let failure =
            classify_sync_protocol_failure("X11 sync SetCounter failed", bad_counter_error());

        assert!(matches!(
            failure,
            X11SyncTransportFailure::Permanent {
                counter_invalid: true,
                ..
            }
        ));
    }

    #[test]
    fn sync_transport_keeps_set_counter_unobserved_while_the_fence_is_pending() {
        let submission = sync_submission();
        let mut polled = Vec::new();

        let observation = observe_sync_acknowledgement_with(submission, |sequence| {
            polled.push(sequence);
            X11SyncSequenceObservation::Pending
        });

        let X11SyncTerminalObservation::Pending(progress) = observation else {
            panic!("pending fence must retain the acknowledgement attempt");
        };
        assert_eq!(polled, [submission.fence_sequence]);
        assert!(!progress.fence_terminal);
        assert!(!progress.set_counter_terminal);
    }

    #[test]
    fn sync_transport_observes_the_fence_before_accepting_set_counter_success() {
        let submission = sync_submission();
        let mut polled = Vec::new();

        let observation = observe_sync_acknowledgement_with(submission, |sequence| {
            polled.push(sequence);
            X11SyncSequenceObservation::Succeeded
        });

        assert!(matches!(observation, X11SyncTerminalObservation::Succeeded));
        assert_eq!(
            polled,
            [submission.fence_sequence, submission.set_counter_sequence]
        );
    }

    #[test]
    fn sync_transport_resumes_at_set_counter_after_the_fence_is_terminal() {
        let submission = sync_submission();
        let mut first_poll = Vec::new();
        let first_observation = observe_sync_acknowledgement_with(submission, |sequence| {
            first_poll.push(sequence);
            if sequence == submission.fence_sequence {
                X11SyncSequenceObservation::Succeeded
            } else {
                X11SyncSequenceObservation::Pending
            }
        });
        let X11SyncTerminalObservation::Pending(progress) = first_observation else {
            panic!("SetCounter must remain pending after the fence becomes terminal");
        };
        assert!(progress.fence_terminal);
        assert_eq!(
            first_poll,
            [submission.fence_sequence, submission.set_counter_sequence]
        );

        let mut second_poll = Vec::new();
        let second_observation = observe_sync_acknowledgement_with(progress, |sequence| {
            second_poll.push(sequence);
            X11SyncSequenceObservation::Succeeded
        });

        assert!(matches!(
            second_observation,
            X11SyncTerminalObservation::Succeeded
        ));
        assert_eq!(second_poll, [submission.set_counter_sequence]);
    }

    #[test]
    fn sync_transport_reports_bad_counter_after_the_fence_reply() {
        let submission = sync_submission();
        let mut polled = Vec::new();
        let mut bad_counter = Some(classify_sync_protocol_failure(
            "X11 sync SetCounter failed",
            bad_counter_error(),
        ));

        let observation = observe_sync_acknowledgement_with(submission, |sequence| {
            polled.push(sequence);
            if sequence == submission.fence_sequence {
                X11SyncSequenceObservation::Succeeded
            } else {
                X11SyncSequenceObservation::Failed {
                    failure: bad_counter.take().unwrap(),
                    terminal: true,
                }
            }
        });

        let X11SyncTerminalObservation::Failed {
            submission: progress,
            failure:
                X11SyncTransportFailure::Permanent {
                    counter_invalid, ..
                },
        } = observation
        else {
            panic!("BadCounter must terminate the acknowledgement attempt");
        };
        assert!(counter_invalid);
        assert!(progress.fence_terminal);
        assert!(progress.set_counter_terminal);
        assert_eq!(
            polled,
            [submission.fence_sequence, submission.set_counter_sequence]
        );
    }

    #[test]
    fn sync_transport_connection_failure_preserves_both_sequences_for_discard() {
        let submission = sync_submission();

        let observation =
            observe_sync_acknowledgement_with(submission, |_| X11SyncSequenceObservation::Failed {
                failure: X11SyncTransportFailure::Retryable(anyhow!("transport unavailable")),
                terminal: false,
            });

        let X11SyncTerminalObservation::Failed {
            submission: progress,
            failure: X11SyncTransportFailure::Retryable(_),
        } = observation
        else {
            panic!("connection failure must remain retryable");
        };
        assert_eq!(
            sync_sequences_to_discard(progress),
            [
                Some(submission.set_counter_sequence),
                Some(submission.fence_sequence),
            ]
        );
    }

    #[test]
    fn closing_an_unobserved_sync_attempt_discards_both_sequences() {
        let submission = sync_submission();

        assert_eq!(
            sync_sequences_to_discard(submission),
            [
                Some(submission.set_counter_sequence),
                Some(submission.fence_sequence),
            ]
        );
    }
}
