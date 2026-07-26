#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    cell::{Cell, RefCell},
    num::NonZeroIsize,
    path::PathBuf,
    rc::{Rc, Weak},
    str::FromStr,
    sync::{
        Arc, Once,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use ::open_gpui_util::ResultExt;
use anyhow::{Context as _, Result};
use futures::channel::oneshot::{self, Receiver};
use raw_window_handle as rwh;
use smallvec::SmallVec;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Dwm::*,
        Graphics::Gdi::*,
        System::{
            Com::*, Diagnostics::Debug::MessageBeep, LibraryLoader::*, Ole::*, SystemServices::*,
        },
        UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::direct_manipulation::DirectManipulationHandler;
use crate::*;
use open_gpui::*;

pub(crate) struct WindowsWindow(pub Rc<WindowsWindowInner>);

impl std::ops::Deref for WindowsWindow {
    type Target = WindowsWindowInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct WindowsWindowState {
    pub origin: Cell<Point<Pixels>>,
    pub logical_size: Cell<Size<Pixels>>,
    pub min_size: Option<Size<Pixels>>,
    pub fullscreen_restore_bounds: Cell<Bounds<Pixels>>,
    pub border_offset: WindowBorderOffset,
    pub appearance: Cell<WindowAppearance>,
    pub background_appearance: Cell<WindowBackgroundAppearance>,
    pub scale_factor: Cell<f32>,
    pub restore_from_minimized: Cell<Option<Box<dyn FnMut(RequestFrameOptions)>>>,

    pub callbacks: Callbacks,
    pub input_handler: Cell<Option<PlatformInputHandler>>,
    pub ime_enabled: Cell<bool>,
    pub pending_surrogate: Cell<Option<u16>>,
    pub last_reported_modifiers: Cell<Option<Modifiers>>,
    pub last_reported_capslock: Cell<Option<Capslock>>,
    pub hovered: Cell<bool>,
    pub direct_manipulation: DirectManipulationHandler,

    pub renderer: RefCell<DirectXRenderer>,
    /// Set after a GPU device-lost recovery so the next `draw_window` call is
    /// treated as a forced render. This guarantees the next frame both
    /// re-enables drawing (via `mark_drawable`) and bypasses the GPUI view
    /// cache, which would otherwise replay stale atlas tile references from
    /// the previous frame and panic in `DirectXAtlasState::texture`.
    pub force_render_after_recovery: Cell<bool>,

    pub click_state: ClickState,
    pub current_cursor: Cell<Option<HCURSOR>>,
    /// Shared with [`WindowsPlatformState::cursor_visible`].
    pub cursor_visible: Arc<AtomicBool>,
    /// Client-area pointer session and its native capture ownership.
    pub pointer_capture: Cell<WindowsPointerCaptureState>,
    /// Prevents terminal pointer cancellation from re-entering the core input callback.
    pub input_dispatch: Cell<WindowsInputDispatchState>,
    pub pressed_caption_button: Cell<Option<WindowsCaptionButtonAction>>,
    accepts_pointer_input: Cell<bool>,
    focus_on_appearing: bool,
    focus_on_click: bool,
    taskbar_visible: bool,

    pub display: Cell<WindowsDisplay>,
    /// Flag to instruct the `VSyncProvider` thread to invalidate the directx devices
    /// as resizing them has failed, causing us to have lost at least the render target.
    pub invalidate_devices: Arc<AtomicBool>,
    placement_mutation_generation: Cell<Option<u64>>,
    pointer_input_mutation_generation: Cell<Option<u64>>,
    deferred_placement_mutation: Cell<Option<DeferredWindowPlacementMutation>>,
    #[cfg(test)]
    pub(crate) fail_next_pointer_input_frame_change: Cell<bool>,
    fullscreen: Cell<Option<StyleAndBounds>>,
    initial_placement: Cell<Option<WindowOpenStatus>>,
    hwnd: HWND,
    pub(crate) a11y: RefCell<Option<A11yState>>,
}

pub(crate) struct WindowsWindowInner {
    hwnd: HWND,
    drop_target_helper: IDropTargetHelper,
    pub(crate) state: WindowsWindowState,
    system_settings: WindowsSystemSettings,
    pub(crate) handle: AnyWindowHandle,
    pub(crate) hide_title_bar: bool,
    pub(crate) is_movable: bool,
    pub(crate) executor: ForegroundExecutor,
    pub(crate) validation_number: usize,
    pub(crate) main_receiver: PriorityQueueReceiver<RunnableVariant>,
    pub(crate) platform_window_handle: HWND,
    pub(crate) parent_hwnd: Option<HWND>,
}

impl WindowsWindowState {
    fn new(
        hwnd: HWND,
        directx_devices: &DirectXDevices,
        window_params: &CREATESTRUCTW,
        current_cursor: Option<HCURSOR>,
        cursor_visible: Arc<AtomicBool>,
        display: WindowsDisplay,
        min_size: Option<Size<Pixels>>,
        appearance: WindowAppearance,
        disable_direct_composition: bool,
        invalidate_devices: Arc<AtomicBool>,
        accepts_pointer_input: bool,
        focus_on_appearing: bool,
        focus_on_click: bool,
        taskbar_visible: bool,
    ) -> Result<Self> {
        let scale_factor = {
            let monitor_dpi = unsafe { GetDpiForWindow(hwnd) } as f32;
            monitor_dpi / USER_DEFAULT_SCREEN_DPI as f32
        };
        let origin = logical_point(window_params.x as f32, window_params.y as f32, scale_factor);
        let logical_size = {
            let physical_size = size(
                DevicePixels(window_params.cx),
                DevicePixels(window_params.cy),
            );
            physical_size.to_pixels(scale_factor)
        };
        let fullscreen_restore_bounds = Bounds {
            origin,
            size: logical_size,
        };
        let border_offset = WindowBorderOffset::default();
        let restore_from_minimized = None;
        let renderer = DirectXRenderer::new(hwnd, directx_devices, disable_direct_composition)
            .context("Creating DirectX renderer")?;
        let callbacks = Callbacks::default();
        let input_handler = None;
        let pending_surrogate = None;
        let last_reported_modifiers = None;
        let last_reported_capslock = None;
        let hovered = false;
        let click_state = ClickState::new();
        let pointer_capture = Cell::new(WindowsPointerCaptureState::default());
        let input_dispatch = Cell::new(WindowsInputDispatchState::default());
        let pressed_caption_button = None;
        let fullscreen = None;
        let initial_placement = None;
        let placement_mutation_generation = Cell::new(None);
        let pointer_input_mutation_generation = Cell::new(None);
        let deferred_placement_mutation = Cell::new(None);

        let direct_manipulation = DirectManipulationHandler::new(hwnd, scale_factor)
            .context("initializing Direct Manipulation")?;

        Ok(Self {
            origin: Cell::new(origin),
            logical_size: Cell::new(logical_size),
            fullscreen_restore_bounds: Cell::new(fullscreen_restore_bounds),
            border_offset,
            appearance: Cell::new(appearance),
            background_appearance: Cell::new(WindowBackgroundAppearance::Opaque),
            scale_factor: Cell::new(scale_factor),
            restore_from_minimized: Cell::new(restore_from_minimized),
            min_size,
            callbacks,
            input_handler: Cell::new(input_handler),
            ime_enabled: Cell::new(true),
            pending_surrogate: Cell::new(pending_surrogate),
            last_reported_modifiers: Cell::new(last_reported_modifiers),
            last_reported_capslock: Cell::new(last_reported_capslock),
            hovered: Cell::new(hovered),
            renderer: RefCell::new(renderer),
            force_render_after_recovery: Cell::new(false),
            click_state,
            current_cursor: Cell::new(current_cursor),
            cursor_visible,
            pointer_capture,
            input_dispatch,
            pressed_caption_button: Cell::new(pressed_caption_button),
            accepts_pointer_input: Cell::new(accepts_pointer_input),
            focus_on_appearing,
            focus_on_click,
            taskbar_visible,
            display: Cell::new(display),
            placement_mutation_generation,
            pointer_input_mutation_generation,
            deferred_placement_mutation,
            #[cfg(test)]
            fail_next_pointer_input_frame_change: Cell::new(false),
            fullscreen: Cell::new(fullscreen),
            initial_placement: Cell::new(initial_placement),
            hwnd,
            invalidate_devices,
            direct_manipulation,
            a11y: RefCell::new(None),
        })
    }

    #[inline]
    pub(crate) fn is_fullscreen(&self) -> bool {
        self.fullscreen.get().is_some()
    }

    pub(crate) fn accepts_pointer_input(&self) -> bool {
        self.accepts_pointer_input.get()
    }

    pub(crate) fn is_maximized(&self) -> bool {
        !self.is_fullscreen() && unsafe { IsZoomed(self.hwnd) }.as_bool()
    }

    fn bounds(&self) -> Bounds<Pixels> {
        Bounds {
            origin: self.origin.get(),
            size: self.logical_size.get(),
        }
    }

    // Calculate the bounds used for saving and whether the window is maximized.
    fn calculate_window_bounds(&self) -> (Bounds<Pixels>, bool) {
        let placement = unsafe {
            let mut placement = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            GetWindowPlacement(self.hwnd, &mut placement)
                .context("failed to get window placement")
                .log_err();
            placement
        };
        (
            calculate_client_rect(
                placement.rcNormalPosition,
                &self.border_offset,
                self.scale_factor.get(),
            ),
            placement.showCmd == SW_SHOWMAXIMIZED.0 as u32,
        )
    }

    fn window_bounds(&self) -> WindowBounds {
        let (bounds, maximized) = self.calculate_window_bounds();

        if self.is_fullscreen() {
            WindowBounds::Fullscreen(self.fullscreen_restore_bounds.get())
        } else if maximized {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        }
    }

    /// get the logical size of the app's drawable area.
    ///
    /// Currently, GPUI uses the logical size of the app to handle mouse interactions (such as
    /// whether the mouse collides with other elements of GPUI).
    fn content_size(&self) -> Size<Pixels> {
        self.logical_size.get()
    }
}

impl WindowsWindowInner {
    fn new(context: &mut WindowCreateContext, hwnd: HWND, cs: &CREATESTRUCTW) -> Result<Rc<Self>> {
        let state = WindowsWindowState::new(
            hwnd,
            &context.directx_devices,
            cs,
            context.current_cursor,
            context.cursor_visible.clone(),
            context.display,
            context.min_size,
            context.appearance,
            context.disable_direct_composition,
            context.invalidate_devices.clone(),
            context.accepts_pointer_input,
            context.focus_on_appearing,
            context.focus_on_click,
            context.taskbar_visible,
        )?;

        Ok(Rc::new(Self {
            hwnd,
            drop_target_helper: context.drop_target_helper.clone(),
            state,
            handle: context.handle,
            hide_title_bar: context.hide_title_bar,
            is_movable: context.is_movable,
            executor: context.executor.clone(),
            validation_number: context.validation_number,
            main_receiver: context.main_receiver.clone(),
            platform_window_handle: context.platform_window_handle,
            system_settings: WindowsSystemSettings::new(),
            parent_hwnd: context.parent_hwnd,
        }))
    }

    /// Applies a fullscreen transition on the window-owning thread.
    ///
    /// Initial placement uses this directly so the creation path finishes its requested state
    /// before the GPUI window has installed callbacks. Live placement schedules this method through
    /// the foreground executor and reports the resulting coherent facts through a mutation ticket.
    fn toggle_fullscreen_now(&self) -> Result<()> {
        let previous_fullscreen = self.state.fullscreen.take();
        let previous_restore_bounds = self.state.fullscreen_restore_bounds.get();
        let StyleAndBounds {
            style,
            x,
            y,
            cx,
            cy,
        } = match previous_fullscreen {
            Some(state) => state,
            None => {
                let (window_bounds, _) = self.state.calculate_window_bounds();

                let style = WINDOW_STYLE(
                    self.get_window_long_checked(GWL_STYLE, "failed to read window style")? as _,
                );
                let mut rc = RECT::default();
                unsafe { GetWindowRect(self.hwnd, &mut rc) }
                    .context("failed to get window rect")?;
                let fullscreen_restore = StyleAndBounds {
                    style,
                    x: rc.left,
                    y: rc.top,
                    cx: rc.right - rc.left,
                    cy: rc.bottom - rc.top,
                };
                self.state.fullscreen_restore_bounds.set(window_bounds);
                let style = style
                    & !(WS_THICKFRAME | WS_SYSMENU | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_CAPTION);
                let physical_bounds = self.state.display.get().physical_bounds();
                let fullscreen_bounds = StyleAndBounds {
                    style,
                    x: physical_bounds.left().0,
                    y: physical_bounds.top().0,
                    cx: physical_bounds.size.width.0,
                    cy: physical_bounds.size.height.0,
                };
                let result = self.apply_fullscreen_style_and_bounds(fullscreen_bounds);
                if result.is_ok() {
                    self.state.fullscreen.set(Some(fullscreen_restore));
                    set_non_rude_hwnd(self.hwnd, false)?;
                } else if self.native_is_fullscreen_from_native().unwrap_or(false) {
                    self.state.fullscreen.set(Some(fullscreen_restore));
                    if let Err(non_rude_error) = set_non_rude_hwnd(self.hwnd, false) {
                        return Err(result.expect_err("fullscreen application failed")).context(
                            format!(
                                "fullscreen NonRudeHWND recovery also failed: {non_rude_error:#}"
                            ),
                        );
                    }
                } else {
                    self.state
                        .fullscreen_restore_bounds
                        .set(previous_restore_bounds);
                }
                return result;
            }
        };

        let result = self.apply_fullscreen_style_and_bounds(StyleAndBounds {
            style,
            x,
            y,
            cx,
            cy,
        });
        if let Err(error) = result {
            if self.native_is_fullscreen_from_native().unwrap_or(false) {
                self.state.fullscreen.set(previous_fullscreen);
                self.state
                    .fullscreen_restore_bounds
                    .set(previous_restore_bounds);
            } else if let Err(non_rude_error) = set_non_rude_hwnd(self.hwnd, true) {
                return Err(error).context(format!(
                    "fullscreen NonRudeHWND recovery also failed: {non_rude_error:#}"
                ));
            }
            return Err(error);
        }
        set_non_rude_hwnd(self.hwnd, true)?;
        Ok(())
    }

    fn apply_fullscreen_style_and_bounds(&self, style_and_bounds: StyleAndBounds) -> Result<()> {
        let rollback = self.current_style_and_bounds()?;
        let StyleAndBounds {
            style,
            x,
            y,
            cx,
            cy,
        } = style_and_bounds;
        self.set_window_long_checked(
            GWL_STYLE,
            style.0 as isize,
            "failed to update fullscreen window style",
        )?;
        let placement_result = unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                x,
                y,
                cx,
                cy,
                SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOZORDER,
            )
        }
        .context("failed to apply fullscreen window placement");
        if let Err(error) = placement_result {
            if let Err(rollback_error) = self.restore_style_and_bounds(rollback) {
                return Err(error).context(format!(
                    "fullscreen placement rollback also failed: {rollback_error:#}"
                ));
            }
            return Err(error);
        }
        Ok(())
    }

    fn current_style_and_bounds(&self) -> Result<StyleAndBounds> {
        let style = WINDOW_STYLE(
            self.get_window_long_checked(GWL_STYLE, "failed to read window style")? as _,
        );
        let mut rect = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut rect) }.context("failed to get window rect")?;
        Ok(StyleAndBounds {
            style,
            x: rect.left,
            y: rect.top,
            cx: rect.right - rect.left,
            cy: rect.bottom - rect.top,
        })
    }

    fn restore_style_and_bounds(&self, snapshot: StyleAndBounds) -> Result<()> {
        self.set_window_long_checked(
            GWL_STYLE,
            snapshot.style.0 as isize,
            "failed to restore window style",
        )?;
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                snapshot.x,
                snapshot.y,
                snapshot.cx,
                snapshot.cy,
                SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOZORDER,
            )
        }
        .context("failed to restore window bounds")?;
        Ok(())
    }

    fn capture_window_placement_snapshot(&self) -> Result<WindowPlacementRollbackSnapshot> {
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe { GetWindowPlacement(self.hwnd, &mut placement) }
            .context("failed to capture window placement rollback state")?;
        Ok(WindowPlacementRollbackSnapshot {
            placement,
            style_and_bounds: self.current_style_and_bounds()?,
            fullscreen: self.state.fullscreen.get(),
            fullscreen_restore_bounds: self.state.fullscreen_restore_bounds.get(),
            non_rude_hwnd: non_rude_hwnd_for_fullscreen(self.state.fullscreen.get()),
            display: self.state.display.get(),
            scale_factor: self.state.scale_factor.get(),
        })
    }

    fn restore_window_placement_snapshot(
        &self,
        snapshot: WindowPlacementRollbackSnapshot,
    ) -> Result<()> {
        self.restore_style_and_bounds(snapshot.style_and_bounds)?;
        unsafe { SetWindowPlacement(self.hwnd, &snapshot.placement) }
            .context("failed to restore native window placement")?;
        self.state.fullscreen.set(snapshot.fullscreen);
        self.state
            .fullscreen_restore_bounds
            .set(snapshot.fullscreen_restore_bounds);
        set_non_rude_hwnd(self.hwnd, snapshot.non_rude_hwnd)?;
        self.state.display.set(snapshot.display);
        self.state.scale_factor.set(snapshot.scale_factor);
        Ok(())
    }

    fn window_placement_for_bounds(&self, bounds: Bounds<Pixels>) -> Result<WINDOWPLACEMENT> {
        retrieve_window_placement(
            self.hwnd,
            self.state.display.get(),
            bounds,
            self.state.scale_factor.get(),
            &self.state.border_offset,
        )
    }

    fn set_window_restore_bounds(
        &self,
        bounds: Bounds<Pixels>,
        state: WindowPlacementState,
    ) -> Result<()> {
        let mut placement = self.window_placement_for_bounds(bounds)?;
        placement.showCmd = match state {
            WindowPlacementState::Windowed | WindowPlacementState::Fullscreen => {
                SW_SHOWNORMAL.0 as u32
            }
            WindowPlacementState::Maximized => SW_SHOWMAXIMIZED.0 as u32,
            WindowPlacementState::Minimized => SW_SHOWMINIMIZED.0 as u32,
        };
        unsafe { SetWindowPlacement(self.hwnd, &placement) }
            .context("failed to set window restore placement")?;
        Ok(())
    }

    fn set_fullscreen_restore_bounds(&self, bounds: Bounds<Pixels>) -> Result<()> {
        let placement = self.window_placement_for_bounds(bounds)?;
        unsafe { SetWindowPlacement(self.hwnd, &placement) }
            .context("failed to set fullscreen restore placement")?;
        self.state.fullscreen_restore_bounds.set(bounds);
        if let Some(mut fullscreen) = self.state.fullscreen.take() {
            let rect = placement.rcNormalPosition;
            fullscreen.x = rect.left;
            fullscreen.y = rect.top;
            fullscreen.cx = rect.right - rect.left;
            fullscreen.cy = rect.bottom - rect.top;
            self.state.fullscreen.set(Some(fullscreen));
        }
        Ok(())
    }

    fn apply_windowed_placement(&self, bounds: Bounds<Pixels>) -> Result<()> {
        if self.state.is_fullscreen() {
            self.toggle_fullscreen_now()?;
        }
        self.set_window_restore_bounds(bounds, WindowPlacementState::Windowed)?;
        if unsafe {
            IsWindowVisible(self.hwnd).as_bool()
                && (IsZoomed(self.hwnd).as_bool() || IsIconic(self.hwnd).as_bool())
        } {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_RESTORE);
            }
        }
        Ok(())
    }

    fn apply_maximized_placement(&self, restore_bounds: Bounds<Pixels>) -> Result<()> {
        if self.state.is_fullscreen() {
            self.toggle_fullscreen_now()?;
        }
        self.set_window_restore_bounds(restore_bounds, WindowPlacementState::Maximized)?;
        if unsafe { IsWindowVisible(self.hwnd).as_bool() } {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_MAXIMIZE);
            }
        }
        Ok(())
    }

    fn apply_fullscreen_placement(&self, restore_bounds: Bounds<Pixels>) -> Result<()> {
        if self.state.is_fullscreen() {
            return self.set_fullscreen_restore_bounds(restore_bounds);
        }

        if self.state.is_maximized() && unsafe { IsWindowVisible(self.hwnd).as_bool() } {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_RESTORE);
            }
        }
        self.set_window_restore_bounds(restore_bounds, WindowPlacementState::Windowed)?;
        self.toggle_fullscreen_now()?;
        self.state.fullscreen_restore_bounds.set(restore_bounds);
        Ok(())
    }

    fn apply_window_placement_request(
        &self,
        request: WindowPlacementRequest,
        current_facts: &WindowPlatformFacts,
    ) -> Result<()> {
        let rollback = self.capture_window_placement_snapshot()?;
        let result = (|| {
            self.state.scale_factor.set(current_facts.scale_factor);
            if let Some(display_id) = current_facts.display_id
                && let Some(display) = WindowsDisplay::new(display_id)
            {
                self.state.display.set(display);
            }

            let window_bounds = current_facts.window_bounds;
            let current_state = if current_facts.is_minimized {
                WindowPlacementState::Minimized
            } else if current_facts.is_fullscreen {
                WindowPlacementState::Fullscreen
            } else if current_facts.is_maximized {
                WindowPlacementState::Maximized
            } else {
                WindowPlacementState::Windowed
            };

            if request.state.is_none() {
                return match current_state {
                    WindowPlacementState::Windowed => {
                        let bounds = Bounds::new(
                            request
                                .position
                                .unwrap_or(window_bounds.get_bounds().origin),
                            request.size.unwrap_or(window_bounds.get_bounds().size),
                        );
                        self.apply_windowed_placement(bounds)
                    }
                    WindowPlacementState::Maximized => self.set_window_restore_bounds(
                        request
                            .restore_bounds
                            .unwrap_or_else(|| window_bounds.get_bounds()),
                        WindowPlacementState::Maximized,
                    ),
                    WindowPlacementState::Fullscreen => self.set_fullscreen_restore_bounds(
                        request
                            .restore_bounds
                            .unwrap_or_else(|| window_bounds.get_bounds()),
                    ),
                    WindowPlacementState::Minimized => {
                        let restore_bounds = request
                            .restore_bounds
                            .unwrap_or_else(|| window_bounds.get_bounds());
                        if current_facts.is_fullscreen {
                            self.set_fullscreen_restore_bounds(restore_bounds)
                        } else {
                            self.set_window_restore_bounds(
                                restore_bounds,
                                WindowPlacementState::Minimized,
                            )
                        }
                    }
                };
            }

            match request.state.expect("state checked above") {
                WindowPlacementState::Windowed => {
                    let bounds = Bounds::new(
                        request
                            .position
                            .unwrap_or(window_bounds.get_bounds().origin),
                        request.size.unwrap_or(window_bounds.get_bounds().size),
                    );
                    self.apply_windowed_placement(bounds)
                }
                WindowPlacementState::Maximized => {
                    let restore_bounds = request
                        .restore_bounds
                        .unwrap_or_else(|| window_bounds.get_bounds());
                    self.apply_maximized_placement(restore_bounds)
                }
                WindowPlacementState::Fullscreen => {
                    let restore_bounds = request
                        .restore_bounds
                        .unwrap_or_else(|| window_bounds.get_bounds());
                    self.apply_fullscreen_placement(restore_bounds)
                }
                WindowPlacementState::Minimized => {
                    Err(anyhow::anyhow!("live minimized placement is not supported"))
                }
            }
        })();

        if let Err(error) = result {
            if let Err(rollback_error) = self.restore_window_placement_snapshot(rollback) {
                return Err(error).context(format!(
                    "window placement rollback also failed: {rollback_error:#}"
                ));
            }
            return Err(error);
        }
        Ok(())
    }

    fn set_accepts_pointer_input_now(&self, accepts_pointer_input: bool) -> Result<()> {
        let current = self.native_accepts_pointer_input()?;
        self.state.accepts_pointer_input.set(current);
        if current == accepts_pointer_input {
            return Ok(());
        }
        let original_style =
            self.get_window_long_checked(GWL_EXSTYLE, "failed to read pointer-input window style")?;
        let mut style = original_style;
        if accepts_pointer_input {
            style &= !(WS_EX_TRANSPARENT.0 as isize);
        } else {
            style |= WS_EX_TRANSPARENT.0 as isize;
        }
        self.set_window_long_checked(
            GWL_EXSTYLE,
            style,
            "failed to update pointer-input window style",
        )?;
        #[cfg(test)]
        let fail_frame_change = self
            .state
            .fail_next_pointer_input_frame_change
            .replace(false);
        #[cfg(not(test))]
        let fail_frame_change = false;
        let frame_result = if fail_frame_change {
            Err(anyhow::anyhow!(
                "injected pointer-input frame-change failure"
            ))
        } else {
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
                )
            }
            .context("failed to apply pointer-input window style")
        };
        if let Err(error) = frame_result {
            let rollback_result = self.set_window_long_checked(
                GWL_EXSTYLE,
                original_style,
                "failed to roll back pointer-input window style",
            );
            if rollback_result.is_ok() {
                let _ = unsafe {
                    SetWindowPos(
                        self.hwnd,
                        None,
                        0,
                        0,
                        0,
                        0,
                        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
                    )
                };
            }
            if let Ok(actual) = self.native_accepts_pointer_input() {
                self.state.accepts_pointer_input.set(actual);
            }
            if let Err(rollback_error) = rollback_result {
                return Err(error).context(format!(
                    "pointer-input style rollback also failed: {rollback_error:#}"
                ));
            }
            return Err(error);
        }
        let actual = self.native_accepts_pointer_input()?;
        self.state.accepts_pointer_input.set(actual);
        if actual != accepts_pointer_input {
            return Err(anyhow::anyhow!(
                "native pointer-input style did not match the requested value"
            ));
        }
        Ok(())
    }

    fn get_window_long_checked(
        &self,
        index: WINDOW_LONG_PTR_INDEX,
        error_context: &'static str,
    ) -> Result<isize> {
        unsafe {
            SetLastError(WIN32_ERROR(0));
            let value = get_window_long(self.hwnd, index);
            if value == 0 && GetLastError().0 != 0 {
                return Err(windows::core::Error::from_thread()).context(error_context);
            }
            Ok(value)
        }
    }

    fn set_window_long_checked(
        &self,
        index: WINDOW_LONG_PTR_INDEX,
        value: isize,
        error_context: &'static str,
    ) -> Result<()> {
        unsafe {
            SetLastError(WIN32_ERROR(0));
            if set_window_long(self.hwnd, index, value) == 0 && GetLastError().0 != 0 {
                return Err(windows::core::Error::from_thread()).context(error_context);
            }
        }
        Ok(())
    }

    fn native_accepts_pointer_input(&self) -> Result<bool> {
        Ok((self
            .get_window_long_checked(GWL_EXSTYLE, "failed to read pointer-input window style")?
            & WS_EX_TRANSPARENT.0 as isize)
            == 0)
    }

    fn native_is_fullscreen(
        window_rect: RECT,
        monitor: HMONITOR,
        window_style: WINDOW_STYLE,
    ) -> bool {
        if monitor.is_invalid() || !Self::has_fullscreen_window_style(window_style) {
            return false;
        }

        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let read_monitor_info = unsafe { GetMonitorInfoW(monitor, &mut monitor_info).as_bool() };
        if !read_monitor_info {
            return false;
        }
        let monitor_rect = monitor_info.rcMonitor;
        window_rect.left <= monitor_rect.left
            && window_rect.top <= monitor_rect.top
            && window_rect.right >= monitor_rect.right
            && window_rect.bottom >= monitor_rect.bottom
    }

    fn native_is_fullscreen_from_native(&self) -> Result<bool> {
        let mut window_rect = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut window_rect) }
            .context("failed to read native fullscreen bounds")?;
        let monitor = unsafe { MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONULL) };
        let window_style = WINDOW_STYLE(
            self.get_window_long_checked(GWL_STYLE, "failed to read window style")? as u32,
        );
        Ok(Self::native_is_fullscreen(
            window_rect,
            monitor,
            window_style,
        ))
    }

    fn has_fullscreen_window_style(window_style: WINDOW_STYLE) -> bool {
        !window_style.contains(WS_THICKFRAME)
            && !window_style.contains(WS_SYSMENU)
            && !window_style.contains(WS_MAXIMIZEBOX)
            && !window_style.contains(WS_MINIMIZEBOX)
            && !window_style.contains(WS_CAPTION)
    }

    fn set_window_placement(self: &Rc<Self>) -> Result<()> {
        let Some(open_status) = self.state.initial_placement.take() else {
            return Ok(());
        };
        match open_status.state {
            WindowOpenState::Maximized => unsafe {
                if open_status.activate {
                    SetWindowPlacement(self.hwnd, &open_status.placement)
                        .context("failed to set window placement")?;
                    let _ = ShowWindow(self.hwnd, SW_MAXIMIZE);
                } else {
                    let mut placement = open_status.placement;
                    placement.showCmd = SW_SHOWMAXIMIZED.0 as u32;
                    SetWindowPlacement(self.hwnd, &placement)
                        .context("failed to set maximized window placement")?;
                    let _ = ShowWindow(self.hwnd, SW_SHOWNA);
                }
            },
            WindowOpenState::Fullscreen => {
                unsafe {
                    SetWindowPlacement(self.hwnd, &open_status.placement)
                        .context("failed to set window placement")?
                };
                self.toggle_fullscreen_now()?;
                unsafe {
                    let _ = ShowWindow(
                        self.hwnd,
                        if open_status.activate {
                            SW_SHOWNORMAL
                        } else {
                            SW_SHOWNOACTIVATE
                        },
                    );
                };
            }
            WindowOpenState::Windowed => unsafe {
                if open_status.activate {
                    SetWindowPlacement(self.hwnd, &open_status.placement)
                        .context("failed to set window placement")?;
                    let _ = ShowWindow(self.hwnd, SW_SHOWNORMAL);
                } else {
                    apply_window_open_position_without_activation(self.hwnd, &open_status)?;
                    let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                }
            },
        }
        Ok(())
    }

    fn has_pending_initial_placement(&self) -> bool {
        let initial_placement = self.state.initial_placement.take();
        let is_pending = initial_placement.is_some();
        self.state.initial_placement.set(initial_placement);
        is_pending
    }

    fn merge_deferred_initial_placement(&self, request: WindowPlacementRequest) -> Result<()> {
        let Some(mut open_status) = self.state.initial_placement.take() else {
            anyhow::bail!("pending creation placement disappeared before activation");
        };
        let mut restore_bounds = calculate_client_rect(
            open_status.placement.rcNormalPosition,
            &self.state.border_offset,
            self.state.scale_factor.get(),
        );
        if let Some(bounds) = request.restore_bounds {
            restore_bounds = bounds;
        }
        if let Some(position) = request.position {
            restore_bounds.origin = position;
        }
        if let Some(size) = request.size {
            restore_bounds.size = size;
        }
        if let Some(state) = request.state {
            open_status.state = match state {
                WindowPlacementState::Windowed => WindowOpenState::Windowed,
                WindowPlacementState::Maximized => WindowOpenState::Maximized,
                WindowPlacementState::Fullscreen => WindowOpenState::Fullscreen,
                WindowPlacementState::Minimized => {
                    self.state.initial_placement.set(Some(open_status));
                    anyhow::bail!("live minimized placement is not supported");
                }
            };
        }

        open_status.placement.rcNormalPosition = calculate_window_rect(
            restore_bounds.to_device_pixels(self.state.scale_factor.get()),
            &self.state.border_offset,
        );
        open_status.placement.showCmd = match open_status.state {
            WindowOpenState::Windowed | WindowOpenState::Fullscreen => SW_SHOWNORMAL.0 as u32,
            WindowOpenState::Maximized => SW_SHOWMAXIMIZED.0 as u32,
        };
        self.state.initial_placement.set(Some(open_status));
        Ok(())
    }

    fn observed_platform_facts(&self) -> WindowPlatformFacts {
        self.observed_platform_facts_from_native()
            .unwrap_or_else(|error| {
                log::warn!("Windows platform fact readback failed: {error:#}");
                self.cached_platform_facts()
            })
    }

    #[cfg(test)]
    pub(crate) fn observed_platform_facts_for_test(&self) -> Result<WindowPlatformFacts> {
        self.observed_platform_facts_from_native()
    }

    fn observed_platform_facts_from_native(&self) -> Result<WindowPlatformFacts> {
        let dpi = unsafe { GetDpiForWindow(self.hwnd) };
        if dpi == 0 {
            anyhow::bail!("failed to read native window DPI");
        }
        let scale_factor = dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32;
        let mut window_rect = RECT::default();
        unsafe { GetWindowRect(self.hwnd, &mut window_rect) }
            .context("failed to read native window bounds")?;
        let mut client_rect = RECT::default();
        unsafe { GetClientRect(self.hwnd, &mut client_rect) }
            .context("failed to read native client bounds")?;
        let mut client_origin = POINT::default();
        unsafe { ClientToScreen(self.hwnd, &mut client_origin) }
            .ok()
            .context("failed to read native client origin")?;
        let bounds = Bounds::new(
            logical_point(client_origin.x as f32, client_origin.y as f32, scale_factor),
            size(
                DevicePixels(client_rect.right - client_rect.left),
                DevicePixels(client_rect.bottom - client_rect.top),
            )
            .to_pixels(scale_factor),
        );
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe { GetWindowPlacement(self.hwnd, &mut placement) }
            .context("failed to read native window placement")?;
        let is_minimized = unsafe { IsIconic(self.hwnd).as_bool() };
        let monitor = unsafe { MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONULL) };
        let window_style = WINDOW_STYLE(
            self.get_window_long_checked(GWL_STYLE, "failed to read window style")? as u32,
        );
        let window_ex_style = WINDOW_EX_STYLE(
            self.get_window_long_checked(GWL_EXSTYLE, "failed to read extended window style")?
                as u32,
        );
        let is_fullscreen = self.state.is_fullscreen()
            && if is_minimized {
                Self::has_fullscreen_window_style(window_style)
            } else {
                Self::native_is_fullscreen(window_rect, monitor, window_style)
            };
        let is_maximized = !is_fullscreen
            && (placement.showCmd == SW_SHOWMAXIMIZED.0 as u32
                || (is_minimized && placement.flags.contains(WPF_RESTORETOMAXIMIZED)));
        let restore_bounds = calculate_client_rect(
            placement.rcNormalPosition,
            &self.state.border_offset,
            scale_factor,
        );
        let window_bounds = if is_fullscreen {
            WindowBounds::Fullscreen(restore_bounds)
        } else if is_maximized {
            WindowBounds::Maximized(restore_bounds)
        } else {
            WindowBounds::Windowed(restore_bounds)
        };
        let display_id =
            (!monitor.is_invalid()).then(|| WindowsDisplay::display_id_for_monitor(monitor));
        let accepts_pointer_input = self.native_accepts_pointer_input()?;
        let focus_on_click = window_ex_style.0 & WS_EX_NOACTIVATE.0 == 0;
        let taskbar_visible = window_ex_style.0 & WS_EX_APPWINDOW.0 != 0
            && window_ex_style.0 & WS_EX_TOOLWINDOW.0 == 0;
        let topmost = window_ex_style.0 & WS_EX_TOPMOST.0 != 0;

        Ok(WindowPlatformFacts {
            bounds,
            coordinate_space: WindowCoordinateSpace::WindowLocal,
            window_bounds,
            inner_window_bounds: window_bounds,
            content_size: bounds.size,
            scale_factor,
            display_id,
            is_minimized,
            is_maximized,
            is_fullscreen,
            accepts_pointer_input,
            focus_on_appearing: self.state.focus_on_appearing,
            focus_on_click,
            background_appearance: self.state.background_appearance.get(),
            topmost,
            taskbar_visible,
            is_active: self.hwnd == unsafe { GetForegroundWindow() },
        })
    }

    fn cached_platform_facts(&self) -> WindowPlatformFacts {
        let window_bounds = self.state.window_bounds();
        WindowPlatformFacts {
            bounds: self.state.bounds(),
            coordinate_space: WindowCoordinateSpace::WindowLocal,
            window_bounds,
            inner_window_bounds: window_bounds,
            content_size: self.state.content_size(),
            scale_factor: self.state.scale_factor.get(),
            display_id: Some(self.state.display.get().id()),
            is_minimized: unsafe { IsIconic(self.hwnd).as_bool() },
            is_maximized: self.state.is_maximized(),
            is_fullscreen: self.state.is_fullscreen(),
            accepts_pointer_input: self.state.accepts_pointer_input(),
            focus_on_appearing: self.state.focus_on_appearing,
            focus_on_click: self.state.focus_on_click,
            background_appearance: self.state.background_appearance.get(),
            topmost: false,
            taskbar_visible: self.state.taskbar_visible,
            is_active: self.hwnd == unsafe { GetForegroundWindow() },
        }
    }

    fn prepare_window_mutation(&self, domain: WindowMutationDomain, generation: u64) {
        match domain {
            WindowMutationDomain::Placement => {
                self.state
                    .placement_mutation_generation
                    .set(Some(generation));
                self.state.deferred_placement_mutation.set(None);
            }
            WindowMutationDomain::PointerInput => {
                self.state
                    .pointer_input_mutation_generation
                    .set(Some(generation));
            }
            WindowMutationDomain::FocusOnAppearing
            | WindowMutationDomain::FocusOnClick
            | WindowMutationDomain::Alpha
            | WindowMutationDomain::Topmost
            | WindowMutationDomain::TaskbarVisibility => {}
        }
    }

    fn invalidate_window_mutation(&self, domain: WindowMutationDomain) {
        match domain {
            WindowMutationDomain::Placement => {
                self.state.placement_mutation_generation.set(None);
                self.state.deferred_placement_mutation.set(None);
            }
            WindowMutationDomain::PointerInput => {
                self.state.pointer_input_mutation_generation.set(None);
            }
            WindowMutationDomain::FocusOnAppearing
            | WindowMutationDomain::FocusOnClick
            | WindowMutationDomain::Alpha
            | WindowMutationDomain::Topmost
            | WindowMutationDomain::TaskbarVisibility => {}
        }
    }

    fn placement_mutation_is_current(&self, generation: u64) -> bool {
        self.state.placement_mutation_generation.get() == Some(generation)
    }

    fn pointer_input_mutation_is_current(&self, generation: u64) -> bool {
        self.state.pointer_input_mutation_generation.get() == Some(generation)
    }

    fn terminal_facts_after_mutation(
        &self,
        mutation: &str,
        result: Result<()>,
        before_facts: WindowPlatformFacts,
    ) -> (PlatformWindowMutationTerminal, WindowPlatformFacts) {
        match result {
            Ok(()) => match self.observed_platform_facts_from_native() {
                Ok(facts) => (PlatformWindowMutationTerminal::Observed, facts),
                Err(error) => {
                    log::warn!(
                        "Windows {mutation} completed but terminal fact readback failed: {error:#}"
                    );
                    (PlatformWindowMutationTerminal::Rejected, before_facts)
                }
            },
            Err(error) => {
                log::warn!("Windows {mutation} request failed: {error:#}");
                match self.observed_platform_facts_from_native() {
                    Ok(facts) => (PlatformWindowMutationTerminal::Rejected, facts),
                    Err(readback_error) => {
                        log::warn!(
                            "Windows {mutation} rejected and terminal fact readback failed: {readback_error:#}"
                        );
                        (PlatformWindowMutationTerminal::Rejected, before_facts)
                    }
                }
            }
        }
    }

    fn emit_window_mutation_observation(
        &self,
        domain: WindowMutationDomain,
        generation: u64,
        terminal: PlatformWindowMutationTerminal,
        facts: WindowPlatformFacts,
    ) {
        let callback = self.state.callbacks.window_mutation_observation.take();
        if let Some(mut callback) = callback {
            callback(PlatformWindowMutationObservation::terminal(
                domain, generation, terminal, facts,
            ));
            self.state
                .callbacks
                .window_mutation_observation
                .set(Some(callback));
        }
    }

    pub(crate) fn system_settings(&self) -> &WindowsSystemSettings {
        &self.system_settings
    }
}

#[derive(Default)]
pub(crate) struct Callbacks {
    pub(crate) request_frame: Cell<Option<Box<dyn FnMut(RequestFrameOptions)>>>,
    pub(crate) input: Cell<Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>>,
    pub(crate) active_status_change: Cell<Option<Box<dyn FnMut(bool)>>>,
    pub(crate) hovered_status_change: Cell<Option<Box<dyn FnMut(bool)>>>,
    pub(crate) resize: Cell<Option<Box<dyn FnMut(Size<Pixels>, f32)>>>,
    pub(crate) moved: Cell<Option<Box<dyn FnMut()>>>,
    pub(crate) window_state_change: Cell<Option<Box<dyn FnMut()>>>,
    pub(crate) window_mutation_observation:
        Cell<Option<Box<dyn FnMut(PlatformWindowMutationObservation)>>>,
    pub(crate) should_close: Cell<Option<Box<dyn FnMut() -> bool>>>,
    pub(crate) close: Cell<Option<Box<dyn FnOnce()>>>,
    pub(crate) hit_test_window_control: Cell<Option<Box<dyn FnMut() -> Option<WindowControlArea>>>>,
    pub(crate) appearance_changed: Cell<Option<Box<dyn FnMut()>>>,
}

impl Callbacks {
    pub(crate) fn set_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.input.set(Some(callback));
    }
}

struct WindowCreateContext {
    inner: Option<Result<Rc<WindowsWindowInner>>>,
    handle: AnyWindowHandle,
    hide_title_bar: bool,
    display: WindowsDisplay,
    is_movable: bool,
    min_size: Option<Size<Pixels>>,
    executor: ForegroundExecutor,
    current_cursor: Option<HCURSOR>,
    cursor_visible: Arc<AtomicBool>,
    drop_target_helper: IDropTargetHelper,
    validation_number: usize,
    main_receiver: PriorityQueueReceiver<RunnableVariant>,
    platform_window_handle: HWND,
    appearance: WindowAppearance,
    disable_direct_composition: bool,
    directx_devices: DirectXDevices,
    invalidate_devices: Arc<AtomicBool>,
    parent_hwnd: Option<HWND>,
    accepts_pointer_input: bool,
    focus_on_appearing: bool,
    focus_on_click: bool,
    taskbar_visible: bool,
}

impl WindowsWindow {
    pub(crate) fn new(
        handle: AnyWindowHandle,
        params: WindowParams,
        creation_info: WindowCreationInfo,
    ) -> Result<Self> {
        let WindowCreationInfo {
            icon,
            executor,
            current_cursor,
            cursor_visible,
            drop_target_helper,
            validation_number,
            main_receiver,
            platform_window_handle,
            disable_direct_composition,
            directx_devices,
            invalidate_devices,
        } = creation_info;
        register_window_class(icon);
        let parent_hwnd = if params.kind == WindowKind::Dialog {
            let parent_window = unsafe { GetActiveWindow() };
            if parent_window.is_invalid() {
                None
            } else {
                // Disable the parent window to make this dialog modal
                unsafe {
                    EnableWindow(parent_window, false).as_bool();
                };
                Some(parent_window)
            }
        } else {
            None
        };
        let hide_title_bar = params
            .titlebar
            .as_ref()
            .map(|titlebar| titlebar.appears_transparent)
            .unwrap_or(true);
        let window_name = HSTRING::from(
            params
                .titlebar
                .as_ref()
                .and_then(|titlebar| titlebar.title.as_ref())
                .map(|title| title.as_ref())
                .unwrap_or(""),
        );

        let (mut dwexstyle, dwstyle) = if params.kind == WindowKind::PopUp {
            (WS_EX_TOOLWINDOW, WINDOW_STYLE(0x0))
        } else {
            let mut dwstyle = WS_SYSMENU;

            if params.is_resizable {
                dwstyle |= WS_THICKFRAME | WS_MAXIMIZEBOX;
            }

            if params.is_minimizable {
                dwstyle |= WS_MINIMIZEBOX;
            }
            let dwexstyle = if params.kind == WindowKind::Dialog {
                dwstyle |= WS_POPUP | WS_CAPTION;
                WS_EX_DLGMODALFRAME
            } else {
                WS_EX_APPWINDOW
            };

            (dwexstyle, dwstyle)
        };
        if !disable_direct_composition {
            dwexstyle |= WS_EX_NOREDIRECTIONBITMAP;
        }
        if !params.accepts_pointer_input {
            dwexstyle |= WS_EX_TRANSPARENT;
        }
        if !params.focus {
            dwexstyle |= WS_EX_NOACTIVATE;
        }
        let focus_on_appearing = params.focus;
        let focus_on_click = params.focus;
        let taskbar_visible = matches!(params.kind, WindowKind::Normal | WindowKind::Floating);

        let hinstance = get_module_handle();
        let display = if let Some(display_id) = params.display_id {
            WindowsDisplay::new(display_id)
        } else {
            None
        }
        .or_else(WindowsDisplay::primary_monitor)
        .context("failed to find any monitor")?;
        let appearance = system_appearance().unwrap_or_default();
        let mut context = WindowCreateContext {
            inner: None,
            handle,
            hide_title_bar,
            display,
            is_movable: params.is_movable,
            min_size: params.window_min_size,
            executor,
            current_cursor,
            cursor_visible,
            drop_target_helper,
            validation_number,
            main_receiver,
            platform_window_handle,
            appearance,
            disable_direct_composition,
            directx_devices,
            invalidate_devices,
            parent_hwnd,
            accepts_pointer_input: params.accepts_pointer_input,
            focus_on_appearing,
            focus_on_click,
            taskbar_visible,
        };
        let creation_result = unsafe {
            CreateWindowExW(
                dwexstyle,
                WINDOW_CLASS_NAME,
                &window_name,
                dwstyle,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                parent_hwnd,
                None,
                Some(hinstance.into()),
                Some(&context as *const _ as *const _),
            )
        };

        // Failure to create a `WindowsWindowState` can cause window creation to fail,
        // so check the inner result first.
        let this = context.inner.take().transpose()?;
        let hwnd = creation_result?;
        let this = this.unwrap();

        register_drag_drop(&this)?;
        set_non_rude_hwnd(hwnd, true)?;
        configure_dwm_dark_mode(hwnd, appearance);
        this.state.border_offset.update(hwnd)?;
        let placement = retrieve_window_placement(
            hwnd,
            display,
            params.window_bounds.get_bounds(),
            this.state.scale_factor.get(),
            &this.state.border_offset,
        )?;
        let open_status = WindowOpenStatus {
            placement,
            state: WindowOpenState::from(params.window_bounds),
            activate: params.focus,
        };
        this.state.initial_placement.set(Some(open_status));
        if params.show {
            this.set_window_placement()?;
        }

        Ok(Self(this))
    }
}

impl rwh::HasWindowHandle for WindowsWindow {
    fn window_handle(&self) -> std::result::Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let raw = rwh::Win32WindowHandle::new(unsafe {
            NonZeroIsize::new_unchecked(self.0.hwnd.0 as isize)
        })
        .into();
        Ok(unsafe { rwh::WindowHandle::borrow_raw(raw) })
    }
}

impl rwh::HasDisplayHandle for WindowsWindow {
    fn display_handle(&self) -> std::result::Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        Ok(rwh::DisplayHandle::windows())
    }
}

impl Drop for WindowsWindow {
    fn drop(&mut self) {
        // clone this `Rc` to prevent early release of the pointer
        let this = self.0.clone();
        self.0
            .executor
            .spawn(async move {
                let handle = this.hwnd;
                unsafe {
                    RevokeDragDrop(handle).log_err();
                    DestroyWindow(handle).log_err();
                }
            })
            .detach();
    }
}

impl WindowsWindow {
    fn request_pointer_input_mutation(
        &mut self,
        generation: u64,
        accepts_pointer_input: bool,
    ) -> PlatformWindowDispatch {
        let current = match self.0.native_accepts_pointer_input() {
            Ok(current) => current,
            Err(error) => {
                log::warn!(
                    "Windows pointer-input request rejected because native facts could not be read: {error:#}"
                );
                return PlatformWindowDispatch::Rejected;
            }
        };
        self.state.accepts_pointer_input.set(current);
        if current == accepts_pointer_input {
            return PlatformWindowDispatch::Unchanged;
        }
        if !self.0.pointer_input_mutation_is_current(generation) {
            return PlatformWindowDispatch::Rejected;
        }
        let this = self.0.clone();
        let executor = this.executor.clone();
        executor
            .spawn(async move {
                if !this.pointer_input_mutation_is_current(generation)
                    || (unsafe { !IsWindow(Some(this.hwnd)).as_bool() })
                {
                    return;
                }
                let before_facts = match this.observed_platform_facts_from_native() {
                    Ok(facts) => facts,
                    Err(error) => {
                        log::warn!(
                            "Windows pointer-input mutation rejected before dispatch because native facts could not be read: {error:#}"
                        );
                        this.emit_window_mutation_observation(
                            WindowMutationDomain::PointerInput,
                            generation,
                            PlatformWindowMutationTerminal::Rejected,
                            this.cached_platform_facts(),
                        );
                        return;
                    }
                };
                let result = this.set_accepts_pointer_input_now(accepts_pointer_input);
                if this.pointer_input_mutation_is_current(generation)
                    && (unsafe { IsWindow(Some(this.hwnd)).as_bool() })
                {
                    let (terminal, facts) = this.terminal_facts_after_mutation(
                        "pointer-input",
                        result,
                        before_facts,
                    );
                    this.emit_window_mutation_observation(
                        WindowMutationDomain::PointerInput,
                        generation,
                        terminal,
                        facts,
                    );
                }
            })
            .detach();
        PlatformWindowDispatch::Queued
    }
}

impl PlatformWindow for WindowsWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.state.bounds()
    }

    fn is_maximized(&self) -> bool {
        self.state.is_maximized()
    }

    fn is_minimized(&self) -> bool {
        unsafe { IsIconic(self.0.hwnd).as_bool() }
    }

    fn accepts_pointer_input(&self) -> bool {
        self.state.accepts_pointer_input.get()
    }

    fn platform_facts(&self) -> WindowPlatformFacts {
        self.0.observed_platform_facts()
    }

    fn request_window_mutation(
        &mut self,
        generation: u64,
        request: WindowMutationRequest,
    ) -> PlatformWindowDispatch {
        let WindowMutationRequest::Placement(request) = request else {
            if let WindowMutationRequest::PointerInput(accepts_pointer_input) = request {
                return self.request_pointer_input_mutation(generation, accepts_pointer_input);
            }
            return PlatformWindowDispatch::Unsupported;
        };
        if request.state == Some(WindowPlacementState::Minimized) {
            return PlatformWindowDispatch::Unsupported;
        }
        if !self.0.placement_mutation_is_current(generation) {
            return PlatformWindowDispatch::Rejected;
        }
        if self.0.has_pending_initial_placement() {
            self.state
                .deferred_placement_mutation
                .set(Some(DeferredWindowPlacementMutation {
                    generation,
                    request,
                }));
            return PlatformWindowDispatch::Queued;
        }
        if unsafe { !IsWindowVisible(self.0.hwnd).as_bool() } {
            return PlatformWindowDispatch::Rejected;
        }
        let this = self.0.clone();
        let executor = this.executor.clone();
        executor
            .spawn(async move {
                if !this.placement_mutation_is_current(generation)
                    || (unsafe { !IsWindow(Some(this.hwnd)).as_bool() })
                {
                    return;
                }
                let before_facts = match this.observed_platform_facts_from_native() {
                    Ok(facts) => facts,
                    Err(error) => {
                        log::warn!(
                            "Windows live placement rejected before dispatch because native facts could not be read: {error:#}"
                        );
                        this.emit_window_mutation_observation(
                            WindowMutationDomain::Placement,
                            generation,
                            PlatformWindowMutationTerminal::Rejected,
                            this.cached_platform_facts(),
                        );
                        return;
                    }
                };
                let result = this.apply_window_placement_request(request, &before_facts);
                if this.placement_mutation_is_current(generation)
                    && (unsafe { IsWindow(Some(this.hwnd)).as_bool() })
                {
                    let (terminal, facts) = this.terminal_facts_after_mutation(
                        "live placement",
                        result,
                        before_facts,
                    );
                    this.emit_window_mutation_observation(
                        WindowMutationDomain::Placement,
                        generation,
                        terminal,
                        facts,
                    );
                }
            })
            .detach();
        PlatformWindowDispatch::Queued
    }

    fn window_bounds(&self) -> WindowBounds {
        self.state.window_bounds()
    }

    /// get the logical size of the app's drawable area.
    ///
    /// Currently, GPUI uses the logical size of the app to handle mouse interactions (such as
    /// whether the mouse collides with other elements of GPUI).
    fn content_size(&self) -> Size<Pixels> {
        self.state.content_size()
    }

    fn scale_factor(&self) -> f32 {
        self.state.scale_factor.get()
    }

    fn appearance(&self) -> WindowAppearance {
        self.state.appearance.get()
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(self.state.display.get()))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        let scale_factor = self.scale_factor();
        let point = unsafe {
            let mut point: POINT = std::mem::zeroed();
            GetCursorPos(&mut point)
                .context("unable to get cursor position")
                .log_err();
            ScreenToClient(self.0.hwnd, &mut point).ok().log_err();
            point
        };
        logical_point(point.x as f32, point.y as f32, scale_factor)
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        let hcursor = load_cursor(style);
        if self.state.current_cursor.get().map(|cursor| cursor.0) == hcursor.map(|cursor| cursor.0)
        {
            return;
        }

        self.state.current_cursor.set(hcursor);
        if self.state.hovered.get() && self.state.cursor_visible.load(Ordering::Relaxed) {
            unsafe {
                SetCursor(hcursor);
            }
        }
    }

    fn modifiers(&self) -> Modifiers {
        current_modifiers()
    }

    fn capslock(&self) -> Capslock {
        current_capslock()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.state.input_handler.set(Some(input_handler));
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.state.input_handler.take()
    }

    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<Receiver<usize>> {
        let (done_tx, done_rx) = oneshot::channel();
        let msg = msg.to_string();
        let detail_string = detail.map(|detail| detail.to_string());
        let handle = self.0.hwnd;
        let answers = answers.to_vec();
        self.0
            .executor
            .spawn(async move {
                unsafe {
                    let mut config = TASKDIALOGCONFIG::default();
                    config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as _;
                    config.hwndParent = handle;
                    let title;
                    let main_icon;
                    match level {
                        PromptLevel::Info => {
                            title = windows::core::w!("Info");
                            main_icon = TD_INFORMATION_ICON;
                        }
                        PromptLevel::Warning => {
                            title = windows::core::w!("Warning");
                            main_icon = TD_WARNING_ICON;
                        }
                        PromptLevel::Critical => {
                            title = windows::core::w!("Critical");
                            main_icon = TD_ERROR_ICON;
                        }
                    };
                    config.pszWindowTitle = title;
                    config.Anonymous1.pszMainIcon = main_icon;
                    let instruction = HSTRING::from(msg);
                    config.pszMainInstruction = PCWSTR::from_raw(instruction.as_ptr());
                    let hints_encoded;
                    if let Some(ref hints) = detail_string {
                        hints_encoded = HSTRING::from(hints);
                        config.pszContent = PCWSTR::from_raw(hints_encoded.as_ptr());
                    };
                    let mut button_id_map = Vec::with_capacity(answers.len());
                    let mut buttons = Vec::new();
                    let mut btn_encoded = Vec::new();
                    for (index, btn) in answers.iter().enumerate() {
                        let encoded = HSTRING::from(btn.label().as_ref());
                        let button_id = match btn {
                            PromptButton::Ok(_) => IDOK.0,
                            PromptButton::Cancel(_) => IDCANCEL.0,
                            // the first few low integer values are reserved for known buttons
                            // so for simplicity we just go backwards from -1
                            PromptButton::Other(_) => -(index as i32) - 1,
                        };
                        button_id_map.push(button_id);
                        buttons.push(TASKDIALOG_BUTTON {
                            nButtonID: button_id,
                            pszButtonText: PCWSTR::from_raw(encoded.as_ptr()),
                        });
                        btn_encoded.push(encoded);
                    }
                    config.cButtons = buttons.len() as _;
                    config.pButtons = buttons.as_ptr();

                    config.pfCallback = None;
                    let mut res = std::mem::zeroed();
                    let _ = TaskDialogIndirect(&config, Some(&mut res), None, None)
                        .context("unable to create task dialog")
                        .log_err();

                    if let Some(clicked) =
                        button_id_map.iter().position(|&button_id| button_id == res)
                    {
                        let _ = done_tx.send(clicked);
                    }
                }
            })
            .detach();

        Some(done_rx)
    }

    fn activate(&self) {
        let hwnd = self.0.hwnd;
        let this = self.0.clone();
        self.0
            .executor
            .spawn(async move {
                let deferred_placement = this.state.deferred_placement_mutation.take();
                let had_initial_placement = this.has_pending_initial_placement();
                let before_facts = deferred_placement.map(|_| {
                    this.observed_platform_facts_from_native()
                        .unwrap_or_else(|_| this.cached_platform_facts())
                });
                let placement_result = (|| {
                    if let Some(deferred) = deferred_placement {
                        this.merge_deferred_initial_placement(deferred.request)?;
                    }
                    this.set_window_placement()
                })();

                if let Some(deferred) = deferred_placement {
                    if this.placement_mutation_is_current(deferred.generation)
                        && (unsafe { IsWindow(Some(hwnd)).as_bool() })
                    {
                        let (terminal, facts) = this.terminal_facts_after_mutation(
                            "deferred live placement",
                            placement_result,
                            before_facts.expect("deferred placement captured initial facts"),
                        );
                        this.emit_window_mutation_observation(
                            WindowMutationDomain::Placement,
                            deferred.generation,
                            terminal,
                            facts,
                        );
                    }
                } else {
                    placement_result.log_err();
                }

                unsafe {
                    if !had_initial_placement && !IsWindowVisible(hwnd).as_bool() {
                        let command = if this.state.is_maximized() {
                            SW_MAXIMIZE
                        } else {
                            SW_SHOWNORMAL
                        };
                        let _ = ShowWindow(hwnd, command);
                    }
                    // If the window is minimized, restore it.
                    if IsIconic(hwnd).as_bool() {
                        ShowWindowAsync(hwnd, SW_RESTORE).ok().log_err();
                    }

                    SetActiveWindow(hwnd).ok();
                    SetFocus(Some(hwnd)).ok();
                }

                // premium ragebait by windows, this is needed because the window
                // must have received an input event to be able to set itself to foreground
                // so let's just simulate user input as that seems to be the most reliable way
                // some more info: https://gist.github.com/Aetopia/1581b40f00cc0cadc93a0e8ccb65dc8c
                // bonus: this bug also doesn't manifest if you have vs attached to the process
                let inputs = [
                    INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: VK_MENU,
                                dwFlags: KEYBD_EVENT_FLAGS(0),
                                ..Default::default()
                            },
                        },
                    },
                    INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: VK_MENU,
                                dwFlags: KEYEVENTF_KEYUP,
                                ..Default::default()
                            },
                        },
                    },
                ];
                unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };

                // todo(windows)
                // crate `windows 0.56` reports true as Err
                unsafe { SetForegroundWindow(hwnd).as_bool() };
            })
            .detach();
    }

    fn is_active(&self) -> bool {
        self.0.hwnd == unsafe { GetForegroundWindow() }
    }

    fn is_hovered(&self) -> bool {
        self.state.hovered.get()
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.state.background_appearance.get()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        true
    }

    fn set_title(&mut self, title: &str) {
        unsafe { SetWindowTextW(self.0.hwnd, &HSTRING::from(title)) }
            .inspect_err(|e| log::error!("Set title failed: {e}"))
            .ok();
    }

    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        self.state.background_appearance.set(background_appearance);
        let hwnd = self.0.hwnd;

        // using Dwm APIs for Mica and MicaAlt backdrops.
        // others follow the set_window_composition_attribute approach
        match background_appearance {
            WindowBackgroundAppearance::Opaque => {
                set_window_composition_attribute(hwnd, None, 0);
            }
            WindowBackgroundAppearance::Transparent => {
                set_window_composition_attribute(hwnd, None, 2);
            }
            WindowBackgroundAppearance::Blurred => {
                set_window_composition_attribute(hwnd, Some((0, 0, 0, 0)), 4);
            }
            WindowBackgroundAppearance::MicaBackdrop => {
                // DWMSBT_MAINWINDOW => MicaBase
                dwm_set_window_composition_attribute(hwnd, 2);
            }
            WindowBackgroundAppearance::MicaAltBackdrop => {
                // DWMSBT_TABBEDWINDOW => MicaAlt
                dwm_set_window_composition_attribute(hwnd, 4);
            }
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.state.is_fullscreen()
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.state.callbacks.request_frame.set(Some(callback));
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.state.callbacks.set_input(callback);
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0
            .state
            .callbacks
            .active_status_change
            .set(Some(callback));
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0
            .state
            .callbacks
            .hovered_status_change
            .set(Some(callback));
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.state.callbacks.resize.set(Some(callback));
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.state.callbacks.moved.set(Some(callback));
    }

    fn on_window_state_change(&self, callback: Box<dyn FnMut()>) {
        self.state.callbacks.window_state_change.set(Some(callback));
    }

    fn on_window_mutation_observation(
        &self,
        callback: Box<dyn FnMut(PlatformWindowMutationObservation)>,
    ) {
        self.state
            .callbacks
            .window_mutation_observation
            .set(Some(callback));
    }

    fn prepare_window_mutation(&self, domain: WindowMutationDomain, generation: u64) {
        self.0.prepare_window_mutation(domain, generation);
    }

    fn invalidate_window_mutation(&self, domain: WindowMutationDomain) {
        self.0.invalidate_window_mutation(domain);
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.state.callbacks.should_close.set(Some(callback));
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.state.callbacks.close.set(Some(callback));
    }

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.0
            .state
            .callbacks
            .hit_test_window_control
            .set(Some(callback));
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.0
            .state
            .callbacks
            .appearance_changed
            .set(Some(callback));
    }

    fn draw(&self, scene: &Scene) {
        self.state
            .renderer
            .borrow_mut()
            .draw(scene, self.state.background_appearance.get())
            .log_err();
    }

    #[cfg(feature = "test-support")]
    fn render_to_image(&self, scene: &Scene) -> Result<image::RgbaImage> {
        self.state
            .renderer
            .borrow_mut()
            .render_to_image(scene, self.state.background_appearance.get())
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.state.renderer.borrow().sprite_atlas()
    }

    fn get_raw_handle(&self) -> HWND {
        self.0.hwnd
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.state.renderer.borrow().gpu_specs().log_err()
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        let scale_factor = self.state.scale_factor.get();
        let caret_position = POINT {
            x: (bounds.origin.x.as_f32() * scale_factor) as i32,
            y: (bounds.origin.y.as_f32() * scale_factor) as i32
                + ((bounds.size.height.as_f32() * scale_factor) as i32 / 2),
        };

        self.0.update_ime_position(self.0.hwnd, caret_position);
    }

    fn play_system_bell(&self) {
        // MB_OK: The sound specified as the Windows Default Beep sound.
        let _ = unsafe { MessageBeep(MB_OK) };
    }

    fn a11y_init(&self, callbacks: open_gpui::A11yCallbacks) {
        let action_handler = A11yActionHandler(callbacks.action);
        let is_focused = unsafe { GetForegroundWindow() } == self.0.hwnd;

        let adapter = accesskit_windows::Adapter::new(
            accesskit_windows::HWND(self.0.hwnd.0),
            is_focused,
            action_handler,
        );

        let activation_handler = A11yActivationHandler {
            callback: callbacks.activation,
        };

        *self.state.a11y.borrow_mut() = Some(A11yState {
            adapter,
            activation_handler,
        });
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        let events = {
            let mut a11y = self.state.a11y.borrow_mut();
            a11y.as_mut()
                .and_then(|a11y| a11y.adapter.update_if_active(|| tree_update))
        };
        // The borrow must be dropped before raising events, because
        // `events.raise()` calls `UiaRaiseAutomationPropertyChangedEvent`
        // which may send a nested `WM_GETOBJECT` back into this window
        // procedure, re-entering `handle_wm_getobject` which also borrows
        // `self.state.a11y`.
        if let Some(events) = events {
            events.raise();
        }
    }

    fn a11y_update_window_bounds(&self) {
        // Windows UIA handles window bounds tracking automatically.
    }
}

pub(crate) struct A11yState {
    pub(crate) adapter: accesskit_windows::Adapter,
    pub(crate) activation_handler: A11yActivationHandler,
}

pub(crate) struct A11yActivationHandler {
    callback: Box<dyn Fn() -> Option<accesskit::TreeUpdate> + Send + 'static>,
}

impl accesskit::ActivationHandler for A11yActivationHandler {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        (self.callback)()
    }
}

struct A11yActionHandler(Box<dyn Fn(accesskit::ActionRequest) + Send + 'static>);

impl accesskit::ActionHandler for A11yActionHandler {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        (self.0)(request);
    }
}

#[implement(IDropTarget)]
struct WindowsDragDropHandler(pub Rc<WindowsWindowInner>);

impl WindowsDragDropHandler {
    fn handle_drag_drop(&self, input: PlatformInput) {
        let _ = self.0.dispatch_input(input);
    }
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for WindowsDragDropHandler_Impl {
    fn DragEnter(
        &self,
        pdataobj: windows::core::Ref<IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe {
            let idata_obj = pdataobj.ok()?;
            let config = FORMATETC {
                cfFormat: CF_HDROP.0,
                ptd: std::ptr::null_mut() as _,
                dwAspect: DVASPECT_CONTENT.0,
                lindex: -1,
                tymed: TYMED_HGLOBAL.0 as _,
            };
            let cursor_position = POINT { x: pt.x, y: pt.y };
            if idata_obj.QueryGetData(&config as _) == S_OK {
                *pdweffect = DROPEFFECT_COPY;
                let Some(mut idata) = idata_obj.GetData(&config as _).log_err() else {
                    return Ok(());
                };
                if idata.u.hGlobal.is_invalid() {
                    return Ok(());
                }
                let hdrop = HDROP(idata.u.hGlobal.0);
                let mut paths = SmallVec::<[PathBuf; 2]>::new();
                with_file_names(hdrop, |file_name| {
                    if let Some(path) = PathBuf::from_str(&file_name).log_err() {
                        paths.push(path);
                    }
                });
                ReleaseStgMedium(&mut idata);
                let mut cursor_position = cursor_position;
                ScreenToClient(self.0.hwnd, &mut cursor_position)
                    .ok()
                    .log_err();
                let scale_factor = self.0.state.scale_factor.get();
                let input = PlatformInput::FileDrop(FileDropEvent::Entered {
                    position: logical_point(
                        cursor_position.x as f32,
                        cursor_position.y as f32,
                        scale_factor,
                    ),
                    paths: ExternalPaths(paths),
                });
                self.handle_drag_drop(input);
            } else {
                *pdweffect = DROPEFFECT_NONE;
            }
            self.0
                .drop_target_helper
                .DragEnter(self.0.hwnd, idata_obj, &cursor_position, *pdweffect)
                .log_err();
        }
        Ok(())
    }

    fn DragOver(
        &self,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let mut cursor_position = POINT { x: pt.x, y: pt.y };
        unsafe {
            *pdweffect = DROPEFFECT_COPY;
            self.0
                .drop_target_helper
                .DragOver(&cursor_position, *pdweffect)
                .log_err();
            ScreenToClient(self.0.hwnd, &mut cursor_position)
                .ok()
                .log_err();
        }
        let scale_factor = self.0.state.scale_factor.get();
        let input = PlatformInput::FileDrop(FileDropEvent::Pending {
            position: logical_point(
                cursor_position.x as f32,
                cursor_position.y as f32,
                scale_factor,
            ),
        });
        self.handle_drag_drop(input);

        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        unsafe {
            self.0.drop_target_helper.DragLeave().log_err();
        }
        let input = PlatformInput::FileDrop(FileDropEvent::Exited);
        self.handle_drag_drop(input);

        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: windows::core::Ref<IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let idata_obj = pdataobj.ok()?;
        let mut cursor_position = POINT { x: pt.x, y: pt.y };
        unsafe {
            *pdweffect = DROPEFFECT_COPY;
            self.0
                .drop_target_helper
                .Drop(idata_obj, &cursor_position, *pdweffect)
                .log_err();
            ScreenToClient(self.0.hwnd, &mut cursor_position)
                .ok()
                .log_err();
        }
        let scale_factor = self.0.state.scale_factor.get();
        let input = PlatformInput::FileDrop(FileDropEvent::Submit {
            position: logical_point(
                cursor_position.x as f32,
                cursor_position.y as f32,
                scale_factor,
            ),
        });
        self.handle_drag_drop(input);

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClickState {
    button: Cell<MouseButton>,
    last_click: Cell<Instant>,
    last_position: Cell<Point<DevicePixels>>,
    double_click_spatial_tolerance_width: Cell<i32>,
    double_click_spatial_tolerance_height: Cell<i32>,
    double_click_interval: Cell<Duration>,
    pub(crate) current_count: Cell<usize>,
}

impl ClickState {
    pub fn new() -> Self {
        let double_click_spatial_tolerance_width = unsafe { GetSystemMetrics(SM_CXDOUBLECLK) };
        let double_click_spatial_tolerance_height = unsafe { GetSystemMetrics(SM_CYDOUBLECLK) };
        let double_click_interval = Duration::from_millis(unsafe { GetDoubleClickTime() } as u64);

        ClickState {
            button: Cell::new(MouseButton::Left),
            last_click: Cell::new(Instant::now()),
            last_position: Cell::new(Point::default()),
            double_click_spatial_tolerance_width: Cell::new(double_click_spatial_tolerance_width),
            double_click_spatial_tolerance_height: Cell::new(double_click_spatial_tolerance_height),
            double_click_interval: Cell::new(double_click_interval),
            current_count: Cell::new(0),
        }
    }

    /// update self and return the needed click count
    pub fn update(&self, button: MouseButton, new_position: Point<DevicePixels>) -> usize {
        if self.button.get() == button && self.is_double_click(new_position) {
            self.current_count.update(|it| it + 1);
        } else {
            self.current_count.set(1);
        }
        self.last_click.set(Instant::now());
        self.last_position.set(new_position);
        self.button.set(button);

        self.current_count.get()
    }

    pub fn system_update(&self, wparam: usize) {
        match wparam {
            // SPI_SETDOUBLECLKWIDTH
            29 => self
                .double_click_spatial_tolerance_width
                .set(unsafe { GetSystemMetrics(SM_CXDOUBLECLK) }),
            // SPI_SETDOUBLECLKHEIGHT
            30 => self
                .double_click_spatial_tolerance_height
                .set(unsafe { GetSystemMetrics(SM_CYDOUBLECLK) }),
            // SPI_SETDOUBLECLICKTIME
            32 => self
                .double_click_interval
                .set(Duration::from_millis(unsafe { GetDoubleClickTime() } as u64)),
            _ => {}
        }
    }

    #[inline]
    fn is_double_click(&self, new_position: Point<DevicePixels>) -> bool {
        let diff = self.last_position.get() - new_position;

        self.last_click.get().elapsed() < self.double_click_interval.get()
            && diff.x.0.abs() <= self.double_click_spatial_tolerance_width.get()
            && diff.y.0.abs() <= self.double_click_spatial_tolerance_height.get()
    }
}

#[derive(Copy, Clone)]
struct StyleAndBounds {
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
}

#[derive(Copy, Clone)]
struct WindowPlacementRollbackSnapshot {
    placement: WINDOWPLACEMENT,
    style_and_bounds: StyleAndBounds,
    fullscreen: Option<StyleAndBounds>,
    fullscreen_restore_bounds: Bounds<Pixels>,
    non_rude_hwnd: bool,
    display: WindowsDisplay,
    scale_factor: f32,
}

#[repr(C)]
struct WINDOWCOMPOSITIONATTRIBDATA {
    attrib: u32,
    pv_data: *mut std::ffi::c_void,
    cb_data: usize,
}

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

type Color = (u8, u8, u8, u8);

#[derive(Debug, Default, Clone)]
pub(crate) struct WindowBorderOffset {
    pub(crate) width_offset: Cell<i32>,
    pub(crate) height_offset: Cell<i32>,
}

impl WindowBorderOffset {
    pub(crate) fn update(&self, hwnd: HWND) -> anyhow::Result<()> {
        let window_rect = unsafe {
            let mut rect = std::mem::zeroed();
            GetWindowRect(hwnd, &mut rect)?;
            rect
        };
        let client_rect = unsafe {
            let mut rect = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect)?;
            rect
        };
        self.width_offset
            .set((window_rect.right - window_rect.left) - (client_rect.right - client_rect.left));
        self.height_offset
            .set((window_rect.bottom - window_rect.top) - (client_rect.bottom - client_rect.top));
        Ok(())
    }
}

#[derive(Clone)]
struct WindowOpenStatus {
    placement: WINDOWPLACEMENT,
    state: WindowOpenState,
    activate: bool,
}

#[derive(Clone, Copy)]
struct DeferredWindowPlacementMutation {
    generation: u64,
    request: WindowPlacementRequest,
}

#[derive(Clone, Copy)]
enum WindowOpenState {
    Maximized,
    Fullscreen,
    Windowed,
}

impl From<WindowBounds> for WindowOpenState {
    fn from(window_bounds: WindowBounds) -> Self {
        match window_bounds {
            WindowBounds::Windowed(_) => Self::Windowed,
            WindowBounds::Maximized(_) => Self::Maximized,
            WindowBounds::Fullscreen(_) => Self::Fullscreen,
        }
    }
}

const WINDOW_CLASS_NAME: PCWSTR = w!("OpenGPUI::Window");

fn register_window_class(icon_handle: HICON) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_procedure),
            hIcon: icon_handle,
            lpszClassName: PCWSTR(WINDOW_CLASS_NAME.as_ptr()),
            style: CS_HREDRAW | CS_VREDRAW,
            hInstance: get_module_handle().into(),
            hbrBackground: unsafe { CreateSolidBrush(COLORREF(0x00000000)) },
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc) };
    });
}

unsafe extern "system" fn window_procedure(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let window_params = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let window_creation_context = window_params.lpCreateParams as *mut WindowCreateContext;
        let window_creation_context = unsafe { &mut *window_creation_context };
        return match WindowsWindowInner::new(window_creation_context, hwnd, window_params) {
            Ok(window_state) => {
                let weak = Box::new(Rc::downgrade(&window_state));
                unsafe { set_window_long(hwnd, GWLP_USERDATA, Box::into_raw(weak) as isize) };
                window_creation_context.inner = Some(Ok(window_state));
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            Err(error) => {
                window_creation_context.inner = Some(Err(error));
                LRESULT(0)
            }
        };
    }

    let ptr = unsafe { get_window_long(hwnd, GWLP_USERDATA) } as *mut Weak<WindowsWindowInner>;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let inner = unsafe { &*ptr };
    let result = if let Some(inner) = inner.upgrade() {
        inner.handle_msg(hwnd, msg, wparam, lparam)
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    };

    if msg == WM_NCDESTROY {
        unsafe { set_window_long(hwnd, GWLP_USERDATA, 0) };
        unsafe { drop(Box::from_raw(ptr)) };
    }

    result
}

pub(crate) fn window_from_hwnd(hwnd: HWND) -> Option<Rc<WindowsWindowInner>> {
    if hwnd.is_invalid() {
        return None;
    }

    let ptr = unsafe { get_window_long(hwnd, GWLP_USERDATA) } as *mut Weak<WindowsWindowInner>;
    if !ptr.is_null() {
        let inner = unsafe { &*ptr };
        inner.upgrade()
    } else {
        None
    }
}

fn get_module_handle() -> HMODULE {
    unsafe {
        let mut h_module = std::mem::zeroed();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            windows::core::w!("ZedModule"),
            &mut h_module,
        )
        .expect("Unable to get module handle"); // this should never fail

        h_module
    }
}

fn register_drag_drop(window: &Rc<WindowsWindowInner>) -> Result<()> {
    let window_handle = window.hwnd;
    let handler = WindowsDragDropHandler(window.clone());
    // The lifetime of `IDropTarget` is handled by Windows, it won't release until
    // we call `RevokeDragDrop`.
    // So, it's safe to drop it here.
    let drag_drop_handler: IDropTarget = handler.into();
    unsafe {
        RegisterDragDrop(window_handle, &drag_drop_handler)
            .context("unable to register drag-drop event")?;
    }
    Ok(())
}

fn calculate_window_rect(bounds: Bounds<DevicePixels>, border_offset: &WindowBorderOffset) -> RECT {
    // NOTE:
    // The reason we're not using `AdjustWindowRectEx()` here is
    // that the size reported by this function is incorrect.
    // You can test it, and there are similar discussions online.
    // See: https://stackoverflow.com/questions/12423584/how-to-set-exact-client-size-for-overlapped-window-winapi
    //
    // So we manually calculate these values here.
    let mut rect = RECT {
        left: bounds.left().0,
        top: bounds.top().0,
        right: bounds.right().0,
        bottom: bounds.bottom().0,
    };
    let left_offset = border_offset.width_offset.get() / 2;
    let top_offset = border_offset.height_offset.get() / 2;
    let right_offset = border_offset.width_offset.get() - left_offset;
    let bottom_offset = border_offset.height_offset.get() - top_offset;
    rect.left -= left_offset;
    rect.top -= top_offset;
    rect.right += right_offset;
    rect.bottom += bottom_offset;
    rect
}

fn calculate_client_rect(
    rect: RECT,
    border_offset: &WindowBorderOffset,
    scale_factor: f32,
) -> Bounds<Pixels> {
    let left_offset = border_offset.width_offset.get() / 2;
    let top_offset = border_offset.height_offset.get() / 2;
    let right_offset = border_offset.width_offset.get() - left_offset;
    let bottom_offset = border_offset.height_offset.get() - top_offset;
    let left = rect.left + left_offset;
    let top = rect.top + top_offset;
    let right = rect.right - right_offset;
    let bottom = rect.bottom - bottom_offset;
    let physical_size = size(DevicePixels(right - left), DevicePixels(bottom - top));
    Bounds {
        origin: logical_point(left as f32, top as f32, scale_factor),
        size: physical_size.to_pixels(scale_factor),
    }
}

fn retrieve_window_placement(
    hwnd: HWND,
    display: WindowsDisplay,
    initial_bounds: Bounds<Pixels>,
    scale_factor: f32,
    border_offset: &WindowBorderOffset,
) -> Result<WINDOWPLACEMENT> {
    let mut placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    unsafe { GetWindowPlacement(hwnd, &mut placement)? };
    // the bounds may be not inside the display
    let bounds = if display.check_given_bounds(initial_bounds) {
        initial_bounds
    } else {
        display.default_bounds()
    };
    let bounds = bounds.to_device_pixels(scale_factor);
    placement.rcNormalPosition = calculate_window_rect(bounds, border_offset);
    Ok(placement)
}

unsafe fn apply_window_open_position_without_activation(
    hwnd: HWND,
    open_status: &WindowOpenStatus,
) -> Result<()> {
    let rect = open_status.placement.rcNormalPosition;
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .context("failed to set window position without activation")?;
    }
    Ok(())
}

fn dwm_set_window_composition_attribute(hwnd: HWND, backdrop_type: u32) {
    let mut version = unsafe { std::mem::zeroed() };
    let status = unsafe { windows::Wdk::System::SystemServices::RtlGetVersion(&mut version) };

    // DWMWA_SYSTEMBACKDROP_TYPE is available only on version 22621 or later
    // using SetWindowCompositionAttributeType as a fallback
    if !status.is_ok() || version.dwBuildNumber < 22621 {
        return;
    }

    unsafe {
        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop_type as *const _ as *const _,
            std::mem::size_of_val(&backdrop_type) as u32,
        );

        if !result.is_ok() {
            return;
        }
    }
}

fn set_window_composition_attribute(hwnd: HWND, color: Option<Color>, state: u32) {
    let mut version = unsafe { std::mem::zeroed() };
    let status = unsafe { windows::Wdk::System::SystemServices::RtlGetVersion(&mut version) };

    if !status.is_ok() || version.dwBuildNumber < 17763 {
        return;
    }

    unsafe {
        type SetWindowCompositionAttributeType =
            unsafe extern "system" fn(HWND, *mut WINDOWCOMPOSITIONATTRIBDATA) -> BOOL;
        let module_name = PCSTR::from_raw(c"user32.dll".as_ptr() as *const u8);
        if let Some(user32) = GetModuleHandleA(module_name)
            .context("Unable to get user32.dll handle")
            .log_err()
        {
            let func_name = PCSTR::from_raw(c"SetWindowCompositionAttribute".as_ptr() as *const u8);
            let set_window_composition_attribute: SetWindowCompositionAttributeType =
                std::mem::transmute(GetProcAddress(user32, func_name));
            let mut color = color.unwrap_or_default();
            let is_acrylic = state == 4;
            if is_acrylic && color.3 == 0 {
                color.3 = 1;
            }
            let accent = AccentPolicy {
                accent_state: state,
                accent_flags: if is_acrylic { 0 } else { 2 },
                gradient_color: (color.0 as u32)
                    | ((color.1 as u32) << 8)
                    | ((color.2 as u32) << 16)
                    | ((color.3 as u32) << 24),
                animation_id: 0,
            };
            let mut data = WINDOWCOMPOSITIONATTRIBDATA {
                attrib: 0x13,
                pv_data: &accent as *const _ as *mut _,
                cb_data: std::mem::size_of::<AccentPolicy>(),
            };
            let _ = set_window_composition_attribute(hwnd, &mut data as *mut _ as _);
        }
    }
}

// When the platform title bar is hidden, Windows may think that our application is meant to appear 'fullscreen'
// and will stop the taskbar from appearing on top of our window. Prevent this.
// https://devblogs.microsoft.com/oldnewthing/20250522-00/?p=111211
fn non_rude_hwnd_for_fullscreen(fullscreen: Option<StyleAndBounds>) -> bool {
    fullscreen.is_none()
}

fn set_non_rude_hwnd(hwnd: HWND, non_rude: bool) -> Result<()> {
    if non_rude {
        unsafe { SetPropW(hwnd, w!("NonRudeHWND"), Some(HANDLE(1 as _))) }
            .context("failed to set NonRudeHWND")?;
    } else {
        unsafe { RemovePropW(hwnd, w!("NonRudeHWND")) }.context("failed to remove NonRudeHWND")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ClickState, StyleAndBounds, WindowOpenState, non_rude_hwnd_for_fullscreen};
    use open_gpui::{DevicePixels, MouseButton, WindowBounds, point};
    use std::time::Duration;
    use windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE;

    #[test]
    fn canonical_window_bounds_select_open_state() {
        assert!(matches!(
            WindowOpenState::from(WindowBounds::Windowed(Default::default())),
            WindowOpenState::Windowed
        ));
        assert!(matches!(
            WindowOpenState::from(WindowBounds::Maximized(Default::default())),
            WindowOpenState::Maximized
        ));
        assert!(matches!(
            WindowOpenState::from(WindowBounds::Fullscreen(Default::default())),
            WindowOpenState::Fullscreen
        ));
    }

    #[test]
    fn test_double_click_interval() {
        let state = ClickState::new();
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(0), DevicePixels(0))),
            1
        );
        assert_eq!(
            state.update(MouseButton::Right, point(DevicePixels(0), DevicePixels(0))),
            1
        );
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(0), DevicePixels(0))),
            1
        );
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(0), DevicePixels(0))),
            2
        );
        state
            .last_click
            .update(|it| it - Duration::from_millis(700));
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(0), DevicePixels(0))),
            1
        );
    }

    #[test]
    fn test_double_click_spatial_tolerance() {
        let state = ClickState::new();
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(-3), DevicePixels(0))),
            1
        );
        assert_eq!(
            state.update(MouseButton::Left, point(DevicePixels(0), DevicePixels(3))),
            2
        );
        assert_eq!(
            state.update(MouseButton::Right, point(DevicePixels(3), DevicePixels(2))),
            1
        );
        assert_eq!(
            state.update(MouseButton::Right, point(DevicePixels(10), DevicePixels(0))),
            1
        );
    }

    #[test]
    fn non_rude_hwnd_is_the_inverse_of_fullscreen_state() {
        assert!(non_rude_hwnd_for_fullscreen(None));
        assert!(!non_rude_hwnd_for_fullscreen(Some(StyleAndBounds {
            style: WINDOW_STYLE(0),
            x: 0,
            y: 0,
            cx: 0,
            cy: 0,
        })));
    }
}
