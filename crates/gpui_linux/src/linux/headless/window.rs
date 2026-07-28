//! Headless windows for the Linux platform client.
//!
//! A headless window has no compositor surface and no GPU. Layout, text
//! shaping, and entity plumbing run normally, `draw` discards the scene, and
//! the sprite atlas hands out tiles without uploading pixels.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use open_gpui_collections::HashMap;
use parking_lot::Mutex;
use uuid::Uuid;

use open_gpui::{
    AtlasAccess, AtlasAccessDiagnostic, AtlasAccessOutcome, AtlasKey, AtlasRemoveDiagnostic,
    AtlasRemoveOutcome, AtlasTextureId, AtlasTile, Bounds, Capslock, CursorStyle, DevicePixels,
    DisplayId, GpuSpecs, Modifiers, Pixels, PlatformAtlas, PlatformDisplay, PlatformInputCallback,
    PlatformInputHandler, PlatformInputHandlerSlot, PlatformWindow, PlatformWindowCommand,
    PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome, PlatformWindowPresentOutcome,
    Point, PromptButton, PromptLevel, RequestFrameOptions, Scene, Size, TileId, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowCreationFacts, WindowParams,
    WindowPlatformFacts, px,
};

#[derive(Debug)]
pub(crate) struct HeadlessDisplay {
    bounds: Bounds<Pixels>,
}

impl HeadlessDisplay {
    pub(crate) fn new() -> Self {
        Self {
            bounds: Bounds::from_corners(Point::default(), Point::new(px(1920.), px(1080.))),
        }
    }
}

impl PlatformDisplay for HeadlessDisplay {
    fn id(&self) -> DisplayId {
        DisplayId::new(0)
    }

    fn uuid(&self) -> anyhow::Result<Uuid> {
        Ok(Uuid::nil())
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }
}

struct HeadlessWindowState {
    bounds: Bounds<Pixels>,
    window_bounds: WindowBounds,
    display: Rc<dyn PlatformDisplay>,
    input_callback: PlatformInputCallbackSlot,
    input_handler: PlatformInputHandlerSlot,
    title: Option<String>,
    cursor_style: CursorStyle,
    accepts_pointer_input: bool,
    creation_facts: WindowCreationFacts,
    atlas: Arc<HeadlessAtlas>,
    close_callback: Option<Box<dyn FnOnce()>>,
}

pub(crate) struct HeadlessWindow(Rc<RefCell<HeadlessWindowState>>);

impl Drop for HeadlessWindow {
    fn drop(&mut self) {
        let (input_callback, input_handler, close_callback) = {
            let mut state = self.0.borrow_mut();
            (
                state.input_callback.clone(),
                state.input_handler.clone(),
                state.close_callback.take(),
            )
        };
        input_callback.terminate();
        input_handler.terminate();
        if let Some(callback) = close_callback {
            callback();
        }
    }
}

impl raw_window_handle::HasWindowHandle for HeadlessWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::NotSupported)
    }
}

impl raw_window_handle::HasDisplayHandle for HeadlessWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::NotSupported)
    }
}

impl HeadlessWindow {
    pub(crate) fn new(params: WindowParams, display: Rc<dyn PlatformDisplay>) -> Self {
        let window_bounds = params.window_bounds;
        Self(Rc::new(RefCell::new(HeadlessWindowState {
            bounds: window_bounds.get_bounds(),
            window_bounds,
            display,
            input_callback: PlatformInputCallbackSlot::default(),
            input_handler: PlatformInputHandlerSlot::default(),
            title: None,
            cursor_style: CursorStyle::Arrow,
            accepts_pointer_input: params.accepts_pointer_input,
            creation_facts: WindowCreationFacts {
                show: params.show,
                focus_on_appearing: params.focus_on_appearing,
                transient_for: None,
            },
            atlas: Arc::new(HeadlessAtlas::default()),
            close_callback: None,
        })))
    }
}

impl PlatformWindow for HeadlessWindow {
    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        PlatformWindowCommandDispatcher::new(|command| match command {
            PlatformWindowCommand::CompleteInitialPresentation { .. } => {
                PlatformWindowCommandOutcome::Accepted
            }
            PlatformWindowCommand::Activate
            | PlatformWindowCommand::ShowWindowMenu(_)
            | PlatformWindowCommand::StartWindowMove
            | PlatformWindowCommand::StartWindowResize(_) => PlatformWindowCommandOutcome::Rejected,
        })
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.0.borrow().bounds
    }

    fn is_maximized(&self) -> bool {
        matches!(self.0.borrow().window_bounds, WindowBounds::Maximized(_))
    }

    fn window_bounds(&self) -> WindowBounds {
        self.0.borrow().window_bounds
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }

    fn scale_factor(&self) -> f32 {
        1.0
    }

    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Dark
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.0.borrow().display.clone())
    }

    fn mouse_position(&self) -> Point<Pixels> {
        Point::default()
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        self.0.borrow_mut().cursor_style = style;
    }

    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }

    fn capslock(&self) -> Capslock {
        Capslock::default()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        let input_handler_slot = self.0.borrow().input_handler.clone();
        input_handler_slot.set(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        let input_handler_slot = self.0.borrow().input_handler.clone();
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
        false
    }

    fn is_hovered(&self) -> bool {
        false
    }

    fn accepts_pointer_input(&self) -> bool {
        self.0.borrow().accepts_pointer_input
    }

    fn creation_facts(&self) -> WindowCreationFacts {
        self.0.borrow().creation_facts.clone()
    }

    fn is_visible(&self) -> bool {
        false
    }

    fn platform_facts(&self) -> WindowPlatformFacts {
        WindowPlatformFacts {
            bounds: self.bounds(),
            coordinate_space: open_gpui::WindowCoordinateSpace::WindowLocal,
            window_bounds: self.window_bounds(),
            inner_window_bounds: self.inner_window_bounds(),
            content_size: self.content_size(),
            scale_factor: self.scale_factor(),
            display_id: self.display().map(|display| display.id()),
            is_minimized: false,
            is_maximized: self.is_maximized(),
            is_fullscreen: self.is_fullscreen(),
            accepts_pointer_input: self.accepts_pointer_input(),
            accepts_activation: false,
            focus_on_click: false,
            background_appearance: self.background_appearance(),
            topmost: false,
            taskbar_visible: false,
            is_active: false,
        }
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_title(&mut self, title: &str) {
        self.0.borrow_mut().title = Some(title.to_owned());
    }

    fn get_title(&self) -> String {
        self.0.borrow().title.clone().unwrap_or_default()
    }

    fn set_background_appearance(&self, _background: WindowBackgroundAppearance) {}

    fn is_fullscreen(&self) -> bool {
        matches!(self.0.borrow().window_bounds, WindowBounds::Fullscreen(_))
    }

    fn on_request_frame(&self, _callback: Box<dyn FnMut(RequestFrameOptions)>) {}

    fn on_input(&self, callback: PlatformInputCallback) {
        let input = self.0.borrow().input_callback.clone();
        input.set(callback);
    }

    fn on_active_status_change(&self, _callback: Box<dyn FnMut(bool)>) {}

    fn on_hover_status_change(&self, _callback: Box<dyn FnMut(bool)>) {}

    fn on_resize(&self, _callback: Box<dyn FnMut(Size<Pixels>, f32)>) {}

    fn on_moved(&self, _callback: Box<dyn FnMut()>) {}

    fn on_should_close(&self, _callback: Box<dyn FnMut() -> bool>) {}

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.0.borrow_mut().close_callback = Some(callback);
    }

    fn on_appearance_changed(&self, _callback: Box<dyn FnMut()>) {}

    fn draw(&self, _scene: &Scene) -> PlatformWindowPresentOutcome {
        PlatformWindowPresentOutcome::Submitted
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.0.borrow().atlas.clone()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {}

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }
}

#[derive(Default)]
struct HeadlessAtlas(Mutex<HeadlessAtlasState>);

#[derive(Default)]
struct HeadlessAtlasState {
    next_id: u32,
    tiles: HashMap<AtlasKey, AtlasTile>,
}

impl PlatformAtlas for HeadlessAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<AtlasTile>> {
        {
            let state = self.0.lock();
            if let Some(&tile) = state.tiles.get(key) {
                return Ok(Some(tile));
            }
        }

        let Some((size, _)) = build()? else {
            return Ok(None);
        };

        let mut state = self.0.lock();
        state.next_id += 1;
        let texture_id = state.next_id;
        state.next_id += 1;
        let tile_id = state.next_id;
        let tile = AtlasTile {
            texture_id: AtlasTextureId {
                index: texture_id,
                kind: key.texture_kind(),
            },
            tile_id: TileId(tile_id),
            padding: 0,
            bounds: Bounds {
                origin: Point::default(),
                size,
            },
        };
        state.tiles.insert(key.clone(), tile);
        Ok(Some(tile))
    }

    fn get_or_insert_with_diagnostics<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<AtlasAccess> {
        {
            let state = self.0.lock();
            if let Some(&tile) = state.tiles.get(key) {
                return Ok(AtlasAccess {
                    tile: Some(tile),
                    diagnostic: AtlasAccessDiagnostic::new(
                        key,
                        AtlasAccessOutcome::Hit,
                        Some(tile),
                        Some(tile.bounds.size),
                    ),
                });
            }
        }

        let Some((size, _)) = build()? else {
            return Ok(AtlasAccess {
                tile: None,
                diagnostic: AtlasAccessDiagnostic::new(
                    key,
                    AtlasAccessOutcome::Unavailable,
                    None,
                    None,
                ),
            });
        };

        let mut state = self.0.lock();
        state.next_id += 1;
        let texture_id = state.next_id;
        state.next_id += 1;
        let tile_id = state.next_id;
        let tile = AtlasTile {
            texture_id: AtlasTextureId {
                index: texture_id,
                kind: key.texture_kind(),
            },
            tile_id: TileId(tile_id),
            padding: 0,
            bounds: Bounds {
                origin: Point::default(),
                size,
            },
        };
        state.tiles.insert(key.clone(), tile);
        Ok(AtlasAccess {
            tile: Some(tile),
            diagnostic: AtlasAccessDiagnostic::new(
                key,
                AtlasAccessOutcome::Inserted,
                Some(tile),
                Some(size),
            ),
        })
    }

    fn remove(&self, key: &AtlasKey) {
        self.0.lock().tiles.remove(key);
    }

    fn remove_with_diagnostics(&self, key: &AtlasKey) -> AtlasRemoveDiagnostic {
        let removed = self.0.lock().tiles.remove(key);
        AtlasRemoveDiagnostic::new(
            key,
            if removed.is_some() {
                AtlasRemoveOutcome::RemoveHit
            } else {
                AtlasRemoveOutcome::RemoveNoop
            },
            removed.map(|tile| tile.texture_id),
        )
    }
}
