mod native_close_smoke;
mod renderer;
mod smoke;

pub(super) use native_close_smoke::start as start_native_close_smoke;

use super::input::HostInput;
use super::*;
use open_gpui::{
    AsyncApp, Entity, HostedSurface, HostedSurfaceOptions, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PlatformInput, RequestFrameOptions, WindowHandle,
};
use renderer::Renderer;

pub(super) struct View {
    image: ImageTexture,
    count: Rc<Cell<u32>>,
    rendered_count: Rc<Cell<u32>>,
    input: Entity<HostInput>,
    button_shift: Pixels,
}

impl Render for View {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered_count.set(self.count.get());
        let image = self.image.clone();
        div()
            .relative()
            .size_full()
            .bg(rgb(0x162334))
            .text_color(rgb(0xf2f4f8))
            .font_family("Segoe UI")
            .text_size(px(16.))
            .child(
                div()
                    .absolute()
                    .left(px(24.))
                    .top(px(24.))
                    .child("Logical GPUI root: actual GPU output"),
            )
            .child(
                div()
                    .id("logical-change-image")
                    .absolute()
                    .left(px(24.) + self.button_shift)
                    .top(px(100.))
                    .w(px(200.))
                    .h(px(40.))
                    .px_4()
                    .py_2()
                    .bg(rgb(0x276650))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count.set(this.count.get().wrapping_add(1));
                        cx.notify();
                    }))
                    .child("Change GPU image"),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        window
                            .paint_image_texture(bounds, Corners::all(px(0.)), &image)
                            .expect("logical imported image uses its own renderer atlas");
                    },
                )
                .absolute()
                .left(px(24.))
                .top(px(164.))
                .w(px(128.))
                .h(px(128.)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(24.))
                    .top(px(320.))
                    .w(px(580.))
                    .child(self.input.clone()),
            )
    }
}

pub(super) struct LogicalRoot {
    surface: HostedSurface,
    window: WindowHandle<View>,
    input: Entity<HostInput>,
    renderer: Rc<Renderer>,
    smoke: Option<smoke::Smoke>,
    native_creations: (u64, u64),
    thread_windows: Vec<isize>,
    wake_turns: Cell<u64>,
    empty_drives: Cell<u64>,
}

impl LogicalRoot {
    pub fn open(
        cx: &mut App,
        factory: Rc<Factory>,
        smoke: bool,
        wake: impl Fn() + 'static,
    ) -> Result<Self> {
        let thread_windows = thread_ui_windows()?;
        let native_creations = (
            factory.renderers_created.get(),
            factory.gpu.surfaces_created.load(Ordering::Relaxed),
        );
        let count = Rc::new(Cell::new(0));
        let wake: Rc<dyn Fn()> = Rc::new(wake);
        let renderer = Renderer::new(factory.gpu.clone(), count.clone(), smoke, wake.clone())?;
        let surface = HostedSurface::new(
            HostedSurfaceOptions {
                size: size(px(640.), px(420.)),
                scale_factor: 1.,
                active: true,
                visible: true,
                ..Default::default()
            },
            renderer.clone(),
            move || wake(),
        )?;
        let mut input = None;
        let window = cx.open_surface(&surface, |window, cx| {
            let receiver = cx.new(|cx| HostInput::new(count.clone(), cx));
            receiver.read(cx).focus.clone().focus(window, cx);
            input = Some(receiver.clone());
            cx.new(|_| View {
                image: renderer.image.clone(),
                count,
                rendered_count: renderer.rendered_count.clone(),
                input: receiver,
                button_shift: px(0.),
            })
        })?;
        ensure!(
            native_creations
                == (
                    factory.renderers_created.get(),
                    factory.gpu.surfaces_created.load(Ordering::Relaxed),
                ),
            "opening a logical root created a native renderer or WGPU surface"
        );
        ensure!(
            thread_ui_windows()? == thread_windows,
            "opening a logical root added a thread HWND"
        );
        Ok(Self {
            surface,
            window,
            input: input.context("logical root did not create its text receiver")?,
            renderer,
            smoke: smoke.then(smoke::Smoke::new),
            native_creations,
            thread_windows,
            wake_turns: Cell::new(0),
            empty_drives: Cell::new(0),
        })
    }

    pub fn process_pending(&self) -> Result<()> {
        if self.surface.is_closed() && self.renderer.delivery.borrow().quiesced {
            return Ok(());
        }
        let automatic = self.smoke.is_none();
        self.renderer.process_pending(automatic)?;
        if !self.surface.is_closed()
            && self.renderer.delivery.borrow().pending.is_none()
            && self.surface.needs_frame()
        {
            self.surface.drive_frame()?;
            if self.renderer.delivery.borrow().pending.is_none() {
                self.empty_drives.set(self.empty_drives.get() + 1);
            }
        }
        if automatic {
            self.renderer.process_pending(true)?;
        }
        Ok(())
    }

    fn request_frame(&self) -> Result<()> {
        self.surface.request_frame(RequestFrameOptions {
            force_render: true,
            require_presentation: true,
            ..Default::default()
        })
    }

    fn set_geometry(&self, size: Size<Pixels>, scale: f32) -> Result<()> {
        self.surface.set_geometry(size, scale)?;
        let mut delivery = self.renderer.delivery.borrow_mut();
        delivery.basis.size = size;
        delivery.basis.scale = scale;
        delivery.basis.revision += 1;
        Ok(())
    }

    fn set_visible(&self, visible: bool) -> Result<()> {
        self.surface.set_visible(visible)?;
        let mut delivery = self.renderer.delivery.borrow_mut();
        delivery.basis.visible = visible;
        delivery.basis.revision += 1;
        Ok(())
    }

    fn click(&self, position: open_gpui::Point<Pixels>) -> Result<()> {
        self.surface
            .dispatch_input(PlatformInput::MouseMove(MouseMoveEvent {
                position,
                ..Default::default()
            }))?;
        self.surface
            .dispatch_input(PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position,
                click_count: 1,
                ..Default::default()
            }))?;
        self.surface
            .dispatch_input(PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position,
                click_count: 1,
                ..Default::default()
            }))?;
        Ok(())
    }

    pub fn allows_native_smoke(&self) -> bool {
        self.smoke
            .as_ref()
            .is_none_or(smoke::Smoke::logical_complete)
    }

    pub fn smoke_complete(&self) -> bool {
        self.smoke.as_ref().is_none_or(smoke::Smoke::complete)
    }

    pub fn verify_shutdown(&self, require_smoke: bool) -> Result<()> {
        let delivery = self.renderer.delivery.borrow();
        ensure!(
            delivery.failure.is_none(),
            "logical renderer failed during close: {:?}",
            delivery.failure
        );
        let ticket = delivery
            .ticket
            .as_ref()
            .context("logical renderer did not receive its exact shutdown ticket")?;
        let snapshot = ticket.snapshot();
        ensure!(
            snapshot.window_id() == self.window.window_id()
                && snapshot.quiesced()
                && snapshot.detached()
                && snapshot.binding_terminal()
                && !snapshot.native_terminal()
                && !snapshot.protocol_violation(),
            "logical shutdown did not detach without native destruction: {snapshot:?}"
        );
        ensure!(
            delivery.quiesced
                && delivery.pending.is_none()
                && delivery.in_flight.is_empty()
                && delivery.completed.load(Ordering::Acquire) >= delivery.prefix,
            "logical close lost original GPU-prefix quiescence"
        );
        if delivery.pin.is_some() {
            let entry = delivery
                .entry
                .context("logical close lost entry recording custody")?;
            ensure!(
                Some(entry) == delivery.after_wait
                    && entry.0 == delivery.prefix
                    && entry.1 > 0
                    && entry.2,
                "logical submitted recording/output were released before actual GPU-prefix completion"
            );
        }
        ensure!(
            !require_smoke || self.smoke_complete(),
            "native shutdown overtook logical-root smoke acceptance"
        );
        Ok(())
    }

    pub fn advance_owned(
        &mut self,
        cx: &AsyncApp,
        factory: &Factory,
        native: &WindowHandle<Root>,
        input: &Entity<HostInput>,
    ) -> Result<()> {
        self.process_pending()?;
        self.advance(cx, factory, native, input, true)
    }

    pub fn advance_external(
        &mut self,
        app: &open_gpui::ApplicationHandle,
        factory: &Factory,
        native: &WindowHandle<Root>,
        input: &Entity<HostInput>,
    ) -> Result<()> {
        self.advance(&app.to_async(), factory, native, input, false)
    }

    fn advance(
        &mut self,
        cx: &AsyncApp,
        factory: &Factory,
        native: &WindowHandle<Root>,
        input: &Entity<HostInput>,
        owned: bool,
    ) -> Result<()> {
        if let Some(mut smoke) = self.smoke.take() {
            let result = smoke.advance(self, cx, factory, native, input, owned);
            self.smoke = Some(smoke);
            result?;
        }
        Ok(())
    }
}

fn thread_ui_windows() -> Result<Vec<isize>> {
    use windows::Win32::{
        Foundation::{HWND, LPARAM},
        System::Threading::GetCurrentThreadId,
        UI::WindowsAndMessaging::{EnumThreadWindows, GetClassNameW},
    };

    unsafe extern "system" fn count(hwnd: HWND, data: LPARAM) -> windows::core::BOOL {
        let mut class = [0u16; 128];
        // SAFETY: The HWND comes from synchronous enumeration and class is writable local storage.
        let length = unsafe { GetClassNameW(hwnd, &mut class) };
        let class = String::from_utf16_lossy(&class[..length as usize]);
        if class != "IME" && class != "MSCTFIME UI" {
            // SAFETY: EnumThreadWindows calls synchronously with the live local collection below.
            unsafe { &mut *(data.0 as *mut Vec<isize>) }.push(hwnd.0 as isize);
        }
        windows::core::BOOL(1)
    }

    let mut windows = Vec::new();
    // SAFETY: Enumeration only updates its own collection and does not dispatch App callbacks.
    unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(count),
            LPARAM(&mut windows as *mut Vec<isize> as isize),
        )
        .ok()?;
    }
    windows.sort_unstable();
    Ok(windows)
}

pub(super) fn install_owned(
    cx: &mut App,
    factory: Rc<Factory>,
    smoke: bool,
) -> Result<Rc<RefCell<Option<LogicalRoot>>>> {
    let state: Rc<RefCell<Option<LogicalRoot>>> = Rc::new(RefCell::new(None));
    let queued = Rc::new(Cell::new(false));
    let executor = cx.foreground_executor().clone();
    let weak = Rc::downgrade(&state);
    let root = LogicalRoot::open(cx, factory, smoke, move || {
        if queued.replace(true) {
            return;
        }
        let weak = weak.clone();
        let queued = queued.clone();
        executor
            .spawn(async move {
                queued.set(false);
                if let Some(state) = weak.upgrade() {
                    if let Some(root) = state.borrow().as_ref() {
                        root.wake_turns.set(root.wake_turns.get() + 1);
                        if let Err(error) = root.process_pending() {
                            eprintln!("Driving the logical root failed: {error:#}");
                            std::process::exit(1);
                        }
                    }
                }
            })
            .detach();
    })?;
    *state.borrow_mut() = Some(root);
    Ok(state)
}
