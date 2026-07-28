use anyhow::{Context as _, anyhow};
use x11rb::connection::RequestConnection;

use crate::linux::{X11ClientStatePtr, should_close_callback::ShouldCloseCallbackSlot};
use open_gpui::{
    AnyWindowHandle, Bounds, CursorStyle, Decorations, DevicePixels, DisplayId, ForegroundExecutor,
    GpuSpecs, Modifiers, NativeInputHandlerOutcome, Pixels, PlatformAtlas, PlatformDisplay,
    PlatformInput, PlatformInputCallback, PlatformInputCallbackSlot, PlatformInputHandler,
    PlatformInputHandlerSlot, PlatformWindow, PlatformWindowCommand,
    PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome, Point, PromptButton,
    PromptLevel, RequestFrameOptions, ResizeEdge, ScaledPixels, Scene, Size, Tiling,
    WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowControlArea,
    WindowDecorations, WindowKind, WindowParams, px,
};
use open_gpui_wgpu::{CompositorGpuHint, WgpuRenderer, WgpuSurfaceConfig};

use open_gpui_collections::FxHashSet;
use open_gpui_util::{ResultExt, maybe};
use raw_window_handle as rwh;
use x11rb::{
    connection::Connection,
    cookie::{Cookie, VoidCookie},
    errors::ConnectionError,
    properties::WmSizeHints,
    protocol::{
        sync,
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
    ptr::NonNull,
    rc::{Rc, Weak},
    sync::Arc,
};

use super::{
    X11Display, XINPUT_ALL_DEVICE_GROUPS, XINPUT_ALL_DEVICES, point_from_x11_window_coords,
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
        _NET_WM_STATE_FOCUSED,
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

#[derive(Debug)]
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
    focus_on_click: bool,
    topmost: bool,
    taskbar_visible: bool,
}

impl X11WindowCreationProjection {
    fn new(
        window_bounds: WindowBounds,
        kind: &WindowKind,
        scale_factor: f32,
        alpha_capable: bool,
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

        Self {
            device_bounds,
            restore_bounds,
            initial_state,
            alpha_capable,
            focus_on_appearing: !matches!(kind, WindowKind::PopUp),
            focus_on_click: !matches!(kind, WindowKind::PopUp),
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
    request_frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    input: PlatformInputCallbackSlot,
    active_status_change: Option<Box<dyn FnMut(bool)>>,
    hovered_status_change: Option<Box<dyn FnMut(bool)>>,
    resize: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    moved: Option<Box<dyn FnMut()>>,
    window_state_change: Option<Box<dyn FnMut()>>,
    should_close: ShouldCloseCallbackSlot,
    close: Option<Box<dyn FnOnce()>>,
    appearance_changed: Option<Box<dyn FnMut()>>,
    button_layout_changed: Option<Box<dyn FnMut()>>,
}

pub struct X11WindowState {
    pub destroyed: bool,
    parent: Option<X11WindowStatePtr>,
    children: FxHashSet<xproto::Window>,
    client: X11ClientStatePtr,
    executor: ForegroundExecutor,
    atoms: XcbAtoms,
    x_root_window: xproto::Window,
    x_screen_index: usize,
    visual_id: u32,
    pub(crate) counter_id: sync::Counter,
    pub(crate) last_sync_counter: Option<sync::Int64>,
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
    focus_on_click: bool,
    topmost: bool,
    taskbar_visible: bool,
    initially_shown: bool,
    initial_presentation_completed: bool,
    maximized_vertical: bool,
    maximized_horizontal: bool,
    hidden: bool,
    active: bool,
    hovered: bool,
    pub(crate) force_render_after_recovery: bool,
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
            return PlatformWindowCommandOutcome::Rejected;
        };
        let Some(state) = self.state.upgrade() else {
            return PlatformWindowCommandOutcome::Rejected;
        };
        let mut state = state.borrow_mut();
        if state.destroyed {
            return PlatformWindowCommandOutcome::Rejected;
        }

        match command {
            PlatformWindowCommand::CompleteInitialPresentation { activate } => {
                if state.initial_presentation_completed {
                    return PlatformWindowCommandOutcome::Accepted;
                }
                if state.initially_shown && activate {
                    activate_x11_window(&state, &self.xcb, self.x_window).log_err();
                }
                state.initial_presentation_completed = true;
                PlatformWindowCommandOutcome::Accepted
            }
            PlatformWindowCommand::Activate => {
                x11_command_outcome(activate_x11_window(&state, &self.xcb, self.x_window))
            }
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
        parent_window: Option<X11WindowStatePtr>,
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

        // Collect errors during setup, so that window can be destroyed on failure.
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
            if !params.focus {
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

            if params.kind == WindowKind::Floating || params.kind == WindowKind::Dialog {
                if let Some(parent_window) = parent_window.as_ref().map(|w| w.x_window) {
                    // WM_TRANSIENT_FOR hint indicating the main application window. For floating windows, we set
                    // a parent window (WM_TRANSIENT_FOR) such that the window manager knows where to
                    // place the floating window in relation to the main window.
                    // https://specifications.freedesktop.org/wm-spec/1.4/ar01s05.html
                    check_reply(
                        || "X11 ChangeProperty32 setting WM_TRANSIENT_FOR for floating window failed.",
                        xcb.change_property32(
                            xproto::PropMode::REPLACE,
                            x_window,
                            atoms.WM_TRANSIENT_FOR,
                            xproto::AtomEnum::WINDOW,
                            &[parent_window],
                        ),
                    )?;
                }
            }

            let parent = if params.kind == WindowKind::Dialog
                && let Some(parent) = parent_window
            {
                Some(parent)
            } else {
                None
            };

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
                parent,
                children: FxHashSet::default(),
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
                hovered: false,
                force_render_after_recovery: false,
                fullscreen: false,
                maximized_vertical: false,
                maximized_horizontal: false,
                hidden: false,
                appearance,
                handle,
                background_appearance: WindowBackgroundAppearance::Opaque,
                alpha_capable: creation.alpha_capable,
                focus_on_appearing: creation.focus_on_appearing,
                focus_on_click: creation.focus_on_click,
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
                counter_id: sync_request_counter,
                last_sync_counter: None,
            })
        });

        if setup_result.is_err() {
            check_reply(
                || "X11 DestroyWindow failed while cleaning it up after setup failure.",
                xcb.destroy_window(x_window),
            )?;
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
        let mut state = self.0.state.borrow_mut();

        if let Some(parent) = state.parent.as_ref() {
            parent.state.borrow_mut().children.remove(&self.0.x_window);
        }

        state.renderer.destroy();

        let destroy_x_window = maybe!({
            check_reply(
                || "X11 DestroyWindow failure.",
                self.0.xcb.destroy_window(self.0.x_window),
            )?;
            xcb_flush(&self.0.xcb);

            anyhow::Ok(())
        })
        .log_err();

        if destroy_x_window.is_some() {
            state.destroyed = true;

            let this_ptr = self.0.clone();
            let client_ptr = state.client.clone();
            state
                .executor
                .spawn(async move {
                    this_ptr.close();
                    client_ptr.drop_window(this_ptr.x_window);
                })
                .detach();
        }

        drop(state);
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
        parent_window: Option<X11WindowStatePtr>,
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
                parent_window,
                supports_xinput_gestures,
                is_bgr,
            )?)),
            callbacks: Rc::new(RefCell::new(Callbacks::default())),
            xcb: xcb.clone(),
            x_window,
        };

        let state = ptr.state.borrow_mut();
        let _ = ptr.set_wm_properties(state)?;
        if let Some(parent) = ptr.state.borrow().parent.as_ref() {
            parent.add_child(x_window);
        }

        Ok(Self(ptr, Rc::new(())))
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

    pub fn should_close(&self) -> bool {
        let should_close = self.callbacks.borrow().should_close.clone();
        should_close.invoke()
    }

    pub fn property_notify(&self, event: xproto::PropertyNotifyEvent) -> anyhow::Result<()> {
        let state = self.state.borrow_mut();
        let state_changed = if event.atom == state.atoms._NET_WM_STATE {
            self.set_wm_properties(state)?
        } else if event.atom == state.atoms._GTK_EDGE_CONSTRAINTS {
            self.set_edge_constraints(state)?;
            false
        } else {
            false
        };
        if state_changed {
            self.emit_window_state_change();
        }
        Ok(())
    }

    pub(crate) fn set_mapped(&self, mapped: bool) {
        let changed = {
            let mut state = self.state.borrow_mut();
            let hidden = !mapped;
            if state.hidden == hidden {
                false
            } else {
                state.hidden = hidden;
                true
            }
        };
        if changed {
            self.emit_window_state_change();
        }
    }

    fn emit_window_state_change(&self) {
        let callback = self.callbacks.borrow_mut().window_state_change.take();
        if let Some(mut callback) = callback {
            callback();
            self.callbacks.borrow_mut().window_state_change = Some(callback);
        }
    }

    fn set_edge_constraints(
        &self,
        mut state: std::cell::RefMut<X11WindowState>,
    ) -> anyhow::Result<()> {
        let reply = get_reply(
            || "X11 GetProperty for _GTK_EDGE_CONSTRAINTS failed.",
            self.xcb.get_property(
                false,
                self.x_window,
                state.atoms._GTK_EDGE_CONSTRAINTS,
                xproto::AtomEnum::CARDINAL,
                0,
                4,
            ),
        )?;

        if reply.value_len != 0 {
            if let Ok(bytes) = reply.value[0..4].try_into() {
                let atom = u32::from_ne_bytes(bytes);
                let edge_constraints = EdgeConstraints::from_atom(atom);
                state.edge_constraints.replace(edge_constraints);
            } else {
                log::error!("Failed to parse GTK_EDGE_CONSTRAINTS");
            }
        }

        Ok(())
    }

    fn set_wm_properties(
        &self,
        mut state: std::cell::RefMut<X11WindowState>,
    ) -> anyhow::Result<bool> {
        let reply = get_reply(
            || "X11 GetProperty for _NET_WM_STATE failed.",
            self.xcb.get_property(
                false,
                self.x_window,
                state.atoms._NET_WM_STATE,
                xproto::AtomEnum::ATOM,
                0,
                u32::MAX,
            ),
        )?;

        let atoms = reply
            .value
            .chunks_exact(4)
            .map(|chunk| u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));

        let previous = (
            state.active,
            state.fullscreen,
            state.maximized_vertical,
            state.maximized_horizontal,
            state.hidden,
        );
        state.active = false;
        state.fullscreen = false;
        state.maximized_vertical = false;
        state.maximized_horizontal = false;
        state.hidden = false;

        for atom in atoms {
            if atom == state.atoms._NET_WM_STATE_FOCUSED {
                state.active = true;
            } else if atom == state.atoms._NET_WM_STATE_FULLSCREEN {
                state.fullscreen = true;
            } else if atom == state.atoms._NET_WM_STATE_MAXIMIZED_VERT {
                state.maximized_vertical = true;
            } else if atom == state.atoms._NET_WM_STATE_MAXIMIZED_HORZ {
                state.maximized_horizontal = true;
            } else if atom == state.atoms._NET_WM_STATE_HIDDEN {
                state.hidden = true;
            }
        }

        Ok(previous
            != (
                state.active,
                state.fullscreen,
                state.maximized_vertical,
                state.maximized_horizontal,
                state.hidden,
            ))
    }

    pub fn add_child(&self, child: xproto::Window) {
        let mut state = self.state.borrow_mut();
        state.children.insert(child);
    }

    pub fn is_blocked(&self) -> bool {
        let state = self.state.borrow();
        !state.children.is_empty()
    }

    pub fn close(&self) {
        self.terminate_callback_slots();
        let state = self.state.borrow();
        let client = state.client.clone();
        #[allow(clippy::mutable_key_type)]
        let children = state.children.clone();
        drop(state);

        if let Some(client) = client.get_client() {
            for child in children {
                if let Some(child_window) = client.get_window(child) {
                    child_window.close();
                }
            }
        }

        let callback = self.callbacks.borrow_mut().close.take();
        if let Some(fun) = callback {
            fun()
        }
    }

    pub fn refresh(&self, request_frame_options: RequestFrameOptions) {
        let callback = self.callbacks.borrow_mut().request_frame.take();
        if let Some(mut fun) = callback {
            fun(request_frame_options);
            self.callbacks.borrow_mut().request_frame = Some(fun);
        }
    }

    pub fn handle_input(&self, input: PlatformInput) {
        if self.is_blocked() {
            return;
        }
        let input_callback = self.callbacks.borrow().input.clone();
        if input_callback
            .dispatch(input.clone())
            .is_some_and(|result| !result.propagate)
        {
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
        if self.is_blocked() {
            return;
        }
        let input_handler = self.state.borrow().input_handler.clone();
        let _ = input_handler
            .with_handler(|input_handler| input_handler.replace_text_in_range(None, &text));
    }

    pub fn handle_ime_preedit(&self, text: String) {
        if self.is_blocked() {
            return;
        }
        let input_handler = self.state.borrow().input_handler.clone();
        let _ = input_handler.with_handler(|input_handler| {
            input_handler.replace_and_mark_text_in_range(None, &text, None)
        });
    }

    pub fn handle_ime_unmark(&self) {
        if self.is_blocked() {
            return;
        }
        let input_handler = self.state.borrow().input_handler.clone();
        let _ = input_handler.with_handler(|input_handler| input_handler.unmark_text());
    }

    pub fn handle_ime_delete(&self) {
        if self.is_blocked() {
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

    pub fn set_bounds(&self, bounds: Bounds<i32>) -> anyhow::Result<()> {
        let (is_resize, content_size, scale_factor) = {
            let mut state = self.state.borrow_mut();
            let bounds = bounds.map(|f| px(f as f32 / state.scale_factor));

            let is_resize = bounds.size.width != state.bounds.size.width
                || bounds.size.height != state.bounds.size.height;

            // If it's a resize event (only width/height changed), we ignore `bounds.origin`
            // because it contains wrong values.
            if is_resize {
                state.bounds.size = bounds.size;
            } else {
                state.bounds = bounds;
            }
            if !state.fullscreen
                && !state.maximized_vertical
                && !state.maximized_horizontal
                && !state.hidden
            {
                state.restore_bounds = state.bounds;
            }

            let gpu_size = query_render_extent(&self.xcb, self.x_window)?;
            state.renderer.update_drawable_size(gpu_size);
            let result = (is_resize, state.content_size(), state.scale_factor);
            if let Some(value) = state.last_sync_counter.take() {
                check_reply(
                    || "X11 sync SetCounter failed.",
                    sync::set_counter(&self.xcb, state.counter_id, value),
                )?;
            }
            result
        };

        let mut callbacks = self.callbacks.borrow_mut();
        if let Some(ref mut fun) = callbacks.resize {
            fun(content_size, scale_factor)
        }

        if !is_resize && let Some(ref mut fun) = callbacks.moved {
            fun();
        }

        Ok(())
    }

    pub fn set_active(&self, focus: bool) {
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
        let callback = self.callbacks.borrow_mut().hovered_status_change.take();
        if let Some(mut fun) = callback {
            fun(focus);
            self.callbacks.borrow_mut().hovered_status_change = Some(fun);
        }
    }

    pub fn set_appearance(&mut self, appearance: WindowAppearance) {
        let mut state = self.state.borrow_mut();
        state.appearance = appearance;
        let is_transparent = state.is_transparent();
        state.renderer.update_transparency(is_transparent);
        state.appearance = appearance;
        drop(state);
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
}

impl PlatformWindow for X11Window {
    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        let target = X11WindowCommandTarget::new(self);
        PlatformWindowCommandDispatcher::new(move |command| target.dispatch(command))
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

    fn platform_facts(&self) -> open_gpui::WindowPlatformFacts {
        let window_bounds = self.window_bounds();
        open_gpui::WindowPlatformFacts {
            bounds: self.bounds(),
            coordinate_space: open_gpui::WindowCoordinateSpace::GlobalScreen,
            window_bounds,
            inner_window_bounds: self.inner_window_bounds(),
            content_size: self.content_size(),
            scale_factor: self.scale_factor(),
            display_id: self.display().map(|display| display.id()),
            is_minimized: self.is_minimized(),
            is_maximized: self.is_maximized(),
            is_fullscreen: self.is_fullscreen(),
            accepts_pointer_input: self.accepts_pointer_input(),
            focus_on_appearing: self.0.state.borrow().focus_on_appearing,
            focus_on_click: self.0.state.borrow().focus_on_click,
            background_appearance: self.background_appearance(),
            topmost: self.0.state.borrow().topmost,
            taskbar_visible: self.0.state.borrow().taskbar_visible,
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
        self.0.state.borrow().active
    }

    fn is_hovered(&self) -> bool {
        self.0.state.borrow().hovered
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
        self.0.callbacks.borrow_mut().hovered_status_change = Some(callback);
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

    fn draw(&self, scene: &Scene) {
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
                Ok(()) => {}
                Err(err) => {
                    log::warn!("GPU recovery failed, will retry on next frame: {err}");
                }
            }

            inner.force_render_after_recovery = true;
            return;
        }

        inner.renderer.draw(scene);

        if inner.renderer.needs_redraw() {
            inner.force_render_after_recovery = true;
        }
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
        let mut callbacks = self.0.callbacks.borrow_mut();
        if let Some(appearance_changed) = callbacks.appearance_changed.as_mut() {
            appearance_changed();
        }
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        let state = self.0.state.borrow();
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
        let activation_handler = TrivialActivationHandler {
            callback: callbacks.activation,
        };
        let action_handler = TrivialActionHandler(callbacks.action);
        let deactivation_handler = TrivialDeactivationHandler {
            callback: callbacks.deactivation,
        };

        let adapter =
            accesskit_unix::Adapter::new(activation_handler, action_handler, deactivation_handler);

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

#[cfg(test)]
mod creation_projection_tests {
    use super::*;
    use open_gpui::{point, size};

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
            let projection =
                X11WindowCreationProjection::new(window_bounds, &WindowKind::Normal, 2.0, true);
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
            assert!(projection.focus_on_click);
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
        );

        assert_eq!(projection.initial_state, X11InitialWindowState::Windowed);
        assert_eq!(projection.restore_bounds, restore_bounds());
        assert!(!projection.focus_on_appearing);
        assert!(!projection.focus_on_click);
        assert!(!projection.topmost);
        assert!(!projection.taskbar_visible);

        let dialog = X11WindowCreationProjection::new(
            WindowBounds::Maximized(restore_bounds()),
            &WindowKind::Dialog,
            1.0,
            true,
        );
        assert_eq!(dialog.initial_state, X11InitialWindowState::Windowed);
    }

    #[test]
    fn zero_sized_creation_projection_uses_the_native_fallback_as_restore_bounds() {
        let projection = X11WindowCreationProjection::new(
            WindowBounds::Maximized(Bounds::new(point(px(4.0), px(8.0)), size(px(0.0), px(0.0)))),
            &WindowKind::Normal,
            2.0,
            true,
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
}

#[cfg(test)]
mod should_close_callback_tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn replacement_installed_during_x11_should_close_survives_old_callback_return() {
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
    fn close_during_x11_should_close_permanently_retires_checked_out_callback() {
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
