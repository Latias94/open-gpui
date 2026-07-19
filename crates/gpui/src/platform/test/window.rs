use crate::{
    A11yCallbacks, AnyWindowHandle, AtlasAccess, AtlasAccessDiagnostic, AtlasAccessOutcome,
    AtlasKey, AtlasRemoveDiagnostic, AtlasRemoveOutcome, AtlasTextureId, AtlasTile, Bounds,
    CursorStyle, DevicePixels, DispatchEventResult, GpuSpecs, Pixels, Platform, PlatformAtlas,
    PlatformDisplay, PlatformHeadlessRenderer, PlatformInput, PlatformInputHandler, PlatformWindow,
    Point, PromptButton, RequestFrameOptions, Scene, Size, TestPlatform, TileId, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowParams,
};
use image::RgbaImage;
use open_gpui_collections::HashMap;
use parking_lot::Mutex;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    rc::{Rc, Weak},
    sync::{self, Arc},
};

pub(crate) struct TestWindowState {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) handle: AnyWindowHandle,
    display: Rc<dyn PlatformDisplay>,
    pub(crate) title: Option<String>,
    pub(crate) edited: bool,
    pub(crate) document_path: Option<std::path::PathBuf>,
    platform: Weak<TestPlatform>,
    // TODO: Replace with `Rc`
    sprite_atlas: Arc<dyn PlatformAtlas>,
    renderer: Option<Box<dyn PlatformHeadlessRenderer>>,
    pub(crate) should_close_handler: Option<Box<dyn FnMut() -> bool>>,
    hit_test_window_control_callback: Option<Box<dyn FnMut() -> Option<WindowControlArea>>>,
    input_callback: Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>,
    active_status_change_callback: Option<Box<dyn FnMut(bool)>>,
    hover_status_change_callback: Option<Box<dyn FnMut(bool)>>,
    request_frame_callback: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    resize_callback: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    moved_callback: Option<Box<dyn FnMut()>>,
    input_handler: Option<PlatformInputHandler>,
    ime_position_history: Vec<Bounds<Pixels>>,
    is_minimized: bool,
    is_fullscreen: bool,
    accepts_pointer_input: bool,
    pub(crate) cursor_style: CursorStyle,
    accessibility: TestAccessibilityState,
}

#[derive(Default)]
struct TestAccessibilityState {
    callbacks: Option<Rc<A11yCallbacks>>,
    active: bool,
    updates: Vec<accesskit::TreeUpdate>,
}

impl TestAccessibilityState {
    fn record_platform_delivery(&mut self, update: accesskit::TreeUpdate) {
        self.updates.push(update);
    }

    fn retain_activation_result(
        &mut self,
        callbacks: &Rc<A11yCallbacks>,
        update: Option<accesskit::TreeUpdate>,
    ) {
        let is_current_adapter = self
            .callbacks
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(current, callbacks));
        if self.active
            && is_current_adapter
            && let Some(update) = update
        {
            self.record_platform_delivery(update);
        }
    }
}

#[derive(Clone)]
pub struct TestWindow(pub(crate) Rc<Mutex<TestWindowState>>);

impl HasWindowHandle for TestWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        unimplemented!("Test Windows are not backed by a real platform window")
    }
}

impl HasDisplayHandle for TestWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        unimplemented!("Test Windows are not backed by a real platform window")
    }
}

impl TestWindow {
    pub(crate) fn new(
        handle: AnyWindowHandle,
        params: WindowParams,
        platform: Weak<TestPlatform>,
        display: Rc<dyn PlatformDisplay>,
        renderer: Option<Box<dyn PlatformHeadlessRenderer>>,
    ) -> Self {
        let sprite_atlas: Arc<dyn PlatformAtlas> = match &renderer {
            Some(r) => r.sprite_atlas(),
            None => Arc::new(TestAtlas::new()),
        };
        Self(Rc::new(Mutex::new(TestWindowState {
            bounds: params.bounds,
            display,
            platform,
            handle,
            sprite_atlas,
            renderer,
            title: Default::default(),
            edited: false,
            document_path: None,
            should_close_handler: None,
            hit_test_window_control_callback: None,
            input_callback: None,
            active_status_change_callback: None,
            hover_status_change_callback: None,
            request_frame_callback: None,
            resize_callback: None,
            moved_callback: None,
            input_handler: None,
            ime_position_history: Vec::new(),
            is_minimized: false,
            is_fullscreen: false,
            accepts_pointer_input: params.accepts_pointer_input,
            cursor_style: CursorStyle::Arrow,
            accessibility: TestAccessibilityState::default(),
        })))
    }

    pub(crate) fn activate_accessibility(&self) -> bool {
        let callbacks = {
            let mut state = self.0.lock();
            let Some(callbacks) = state.accessibility.callbacks.clone() else {
                return false;
            };
            state.accessibility.active = true;
            callbacks
        };

        let initial_update = (callbacks.activation)();
        self.0
            .lock()
            .accessibility
            .retain_activation_result(&callbacks, initial_update);
        true
    }

    #[cfg(test)]
    pub(crate) fn ime_position_history(&self) -> Vec<Bounds<Pixels>> {
        self.0.lock().ime_position_history.clone()
    }

    #[cfg(test)]
    pub(crate) fn clear_ime_position_history(&self) {
        self.0.lock().ime_position_history.clear();
    }

    pub(crate) fn deactivate_accessibility(&self) -> bool {
        let callbacks = {
            let mut state = self.0.lock();
            if !state.accessibility.active {
                return false;
            }
            let Some(callbacks) = state.accessibility.callbacks.clone() else {
                return false;
            };
            state.accessibility.active = false;
            callbacks
        };

        (callbacks.deactivation)();
        true
    }

    pub(crate) fn dispatch_accessibility_action(&self, request: accesskit::ActionRequest) -> bool {
        let callbacks = {
            let state = self.0.lock();
            if !state.accessibility.active {
                return false;
            }
            let Some(callbacks) = state.accessibility.callbacks.clone() else {
                return false;
            };
            callbacks
        };

        (callbacks.action)(request);
        true
    }

    pub(crate) fn latest_accessibility_tree_update(&self) -> Option<accesskit::TreeUpdate> {
        self.0
            .lock()
            .accessibility
            .updates
            .last()
            .cloned()
            .map(normalize_accessibility_tree_update)
    }

    pub(crate) fn accessibility_tree_update_history(&self) -> Vec<accesskit::TreeUpdate> {
        self.0
            .lock()
            .accessibility
            .updates
            .iter()
            .cloned()
            .map(normalize_accessibility_tree_update)
            .collect()
    }

    pub fn simulate_resize(&mut self, size: Size<Pixels>) {
        let scale_factor = self.scale_factor();
        let mut lock = self.0.lock();
        // Always update bounds, even if no callback is registered
        lock.bounds.size = size;
        let Some(mut callback) = lock.resize_callback.take() else {
            return;
        };
        drop(lock);
        callback(size, scale_factor);
        self.0.lock().resize_callback = Some(callback);
    }

    pub(crate) fn simulate_active_status_change(&self, active: bool) {
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.active_status_change_callback.take() else {
            return;
        };
        drop(lock);
        callback(active);
        self.0.lock().active_status_change_callback = Some(callback);
    }

    pub(crate) fn simulate_hover_status_change(&self, hovered: bool) {
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.hover_status_change_callback.take() else {
            return;
        };
        drop(lock);
        callback(hovered);
        self.0.lock().hover_status_change_callback = Some(callback);
    }

    pub fn simulate_input_result(&mut self, event: PlatformInput) -> DispatchEventResult {
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.input_callback.take() else {
            return DispatchEventResult::default();
        };
        drop(lock);
        let result = callback(event);
        self.0.lock().input_callback = Some(callback);
        result
    }

    pub fn simulate_input(&mut self, event: PlatformInput) -> bool {
        !self.simulate_input_result(event).propagate
    }

    /// Simulates the platform delivering a frame request.
    pub fn simulate_frame(&self, options: RequestFrameOptions) -> bool {
        let mut lock = self.0.lock();
        let Some(mut callback) = lock.request_frame_callback.take() else {
            return false;
        };
        drop(lock);
        callback(options);
        self.0.lock().request_frame_callback = Some(callback);
        true
    }
}

impl PlatformWindow for TestWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.0.lock().bounds
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(self.bounds())
    }

    fn is_maximized(&self) -> bool {
        false
    }

    fn is_minimized(&self) -> bool {
        self.0.lock().is_minimized
    }

    fn accepts_pointer_input(&self) -> bool {
        self.0.lock().accepts_pointer_input
    }

    fn set_accepts_pointer_input(&mut self, accepts_pointer_input: bool) -> bool {
        self.0.lock().accepts_pointer_input = accepts_pointer_input;
        true
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }

    fn resize(&mut self, size: Size<Pixels>) {
        let mut lock = self.0.lock();
        lock.bounds.size = size;
    }

    fn scale_factor(&self) -> f32 {
        2.0
    }

    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Light
    }

    fn display(&self) -> Option<std::rc::Rc<dyn crate::PlatformDisplay>> {
        Some(self.0.lock().display.clone())
    }

    fn mouse_position(&self) -> Point<Pixels> {
        Point::default()
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        let platform = {
            let mut lock = self.0.lock();
            lock.cursor_style = style;
            lock.platform.upgrade()
        };
        if let Some(platform) = platform {
            platform.set_window_cursor_style(self, style);
        }
    }

    fn modifiers(&self) -> crate::Modifiers {
        crate::Modifiers::default()
    }

    fn capslock(&self) -> crate::Capslock {
        crate::Capslock::default()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.0.lock().input_handler = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.0.lock().input_handler.take()
    }

    fn prompt(
        &self,
        _level: crate::PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        Some(
            self.0
                .lock()
                .platform
                .upgrade()
                .expect("platform dropped")
                .prompt(msg, detail, answers),
        )
    }

    fn activate(&self) {
        self.0
            .lock()
            .platform
            .upgrade()
            .unwrap()
            .set_active_window(Some(self.clone()))
    }

    fn is_active(&self) -> bool {
        false
    }

    fn is_hovered(&self) -> bool {
        let (platform, handle) = {
            let lock = self.0.lock();
            (lock.platform.upgrade(), lock.handle)
        };
        platform.is_some_and(|platform| platform.hovered_window().window() == Some(handle))
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    fn set_title(&mut self, title: &str) {
        self.0.lock().title = Some(title.to_owned());
    }

    fn set_app_id(&mut self, _app_id: &str) {}

    fn set_background_appearance(&self, _background: WindowBackgroundAppearance) {}

    fn set_edited(&mut self, edited: bool) {
        self.0.lock().edited = edited;
    }

    fn set_document_path(&self, path: Option<&std::path::Path>) {
        self.0.lock().document_path = path.map(|p| p.to_path_buf());
    }

    fn show_character_palette(&self) {
        unimplemented!()
    }

    fn minimize(&self) {
        self.0.lock().is_minimized = true;
    }

    fn zoom(&self) {
        unimplemented!()
    }

    fn toggle_fullscreen(&self) {
        let mut lock = self.0.lock();
        lock.is_fullscreen = !lock.is_fullscreen;
    }

    fn is_fullscreen(&self) -> bool {
        self.0.lock().is_fullscreen
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.0.lock().request_frame_callback = Some(callback);
    }

    fn on_input(&self, callback: Box<dyn FnMut(crate::PlatformInput) -> DispatchEventResult>) {
        self.0.lock().input_callback = Some(callback)
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.lock().active_status_change_callback = Some(callback)
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.0.lock().hover_status_change_callback = Some(callback)
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.0.lock().resize_callback = Some(callback)
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.0.lock().moved_callback = Some(callback)
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.0.lock().should_close_handler = Some(callback);
    }

    fn on_close(&self, _callback: Box<dyn FnOnce()>) {}

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.0.lock().hit_test_window_control_callback = Some(callback);
    }

    fn on_appearance_changed(&self, _callback: Box<dyn FnMut()>) {}

    fn draw(&self, _scene: &Scene) {}

    fn sprite_atlas(&self) -> sync::Arc<dyn crate::PlatformAtlas> {
        self.0.lock().sprite_atlas.clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    fn render_to_image(&self, scene: &Scene) -> anyhow::Result<RgbaImage> {
        let mut state = self.0.lock();
        let size = state.bounds.size;
        if let Some(renderer) = &mut state.renderer {
            let scale_factor = 2.0;
            let device_size: Size<DevicePixels> = size.to_device_pixels(scale_factor);
            renderer.render_scene_to_image(scene, device_size)
        } else {
            anyhow::bail!("render_to_image not available: no HeadlessRenderer configured")
        }
    }

    fn as_test(&mut self) -> Option<&mut TestWindow> {
        Some(self)
    }

    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        unimplemented!()
    }

    fn show_window_menu(&self, _position: Point<Pixels>) {
        unimplemented!()
    }

    fn start_window_move(&self) {
        unimplemented!()
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        self.0.lock().ime_position_history.push(bounds);
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }

    fn a11y_init(&self, callbacks: A11yCallbacks) {
        let mut state = self.0.lock();
        debug_assert!(
            state.accessibility.callbacks.is_none(),
            "accessibility callbacks initialized more than once for a test window"
        );
        state.accessibility.callbacks = Some(Rc::new(callbacks));
        state.accessibility.active = false;
        state.accessibility.updates.clear();
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        self.0
            .lock()
            .accessibility
            .record_platform_delivery(tree_update);
    }
}

fn normalize_accessibility_tree_update(mut update: accesskit::TreeUpdate) -> accesskit::TreeUpdate {
    update.nodes.sort_unstable_by_key(|(id, _)| *id);
    update
}

#[cfg(test)]
mod accessibility_tests {
    use super::*;
    use accesskit::{Node, NodeId, Role, Tree, TreeId, TreeUpdate};

    fn tree_update(nodes: Vec<(NodeId, Node)>) -> TreeUpdate {
        TreeUpdate {
            nodes,
            tree: Some(Tree::new(NodeId(0))),
            tree_id: TreeId::ROOT,
            focus: NodeId(0),
        }
    }

    fn callbacks() -> Rc<A11yCallbacks> {
        Rc::new(A11yCallbacks {
            activation: Box::new(|| None),
            action: Box::new(|_| {}),
            deactivation: Box::new(|| {}),
        })
    }

    #[test]
    fn accessibility_state_retains_only_current_active_activation_result() {
        let current = callbacks();
        let replacement = callbacks();
        let first = tree_update(vec![(NodeId(0), Node::new(Role::Window))]);
        let second = tree_update(vec![
            (NodeId(0), Node::new(Role::Window)),
            (NodeId(1), Node::new(Role::Button)),
        ]);
        let mut state = TestAccessibilityState {
            callbacks: Some(current.clone()),
            active: true,
            updates: Vec::new(),
        };

        state.retain_activation_result(&current, None);
        assert!(state.updates.is_empty());
        state.retain_activation_result(&current, Some(first.clone()));
        assert_eq!(state.updates, [first.clone()]);

        state.active = false;
        state.retain_activation_result(&current, Some(second.clone()));
        assert_eq!(state.updates, [first.clone()]);

        state.active = true;
        state.callbacks = Some(replacement);
        state.retain_activation_result(&current, Some(second));
        assert_eq!(state.updates, [first]);
    }

    #[test]
    fn accessibility_state_preserves_platform_delivery_order() {
        let first = tree_update(vec![(NodeId(0), Node::new(Role::Window))]);
        let second = tree_update(vec![
            (NodeId(0), Node::new(Role::Window)),
            (NodeId(1), Node::new(Role::Button)),
        ]);
        let mut state = TestAccessibilityState::default();

        state.record_platform_delivery(first.clone());
        state.record_platform_delivery(second.clone());
        assert_eq!(state.updates, [first.clone(), second.clone()]);

        state.active = true;
        state.record_platform_delivery(first.clone());
        assert_eq!(state.updates, [first.clone(), second, first]);
    }

    #[test]
    fn accessibility_normalization_preserves_relationship_order() {
        let mut root = Node::new(Role::Window);
        root.set_children([NodeId(2), NodeId(1)]);
        let update = tree_update(vec![
            (NodeId(2), Node::new(Role::Label)),
            (NodeId(0), root),
            (NodeId(1), Node::new(Role::Button)),
        ]);

        let normalized = normalize_accessibility_tree_update(update);
        assert_eq!(
            normalized
                .nodes
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            [NodeId(0), NodeId(1), NodeId(2)]
        );
        assert_eq!(normalized.nodes[0].1.children(), &[NodeId(2), NodeId(1)]);
    }
}

pub(crate) struct TestAtlasState {
    next_id: u32,
    tiles: HashMap<AtlasKey, AtlasTile>,
}

pub(crate) struct TestAtlas(Mutex<TestAtlasState>);

impl TestAtlas {
    pub fn new() -> Self {
        TestAtlas(Mutex::new(TestAtlasState {
            next_id: 0,
            tiles: HashMap::default(),
        }))
    }
}

impl PlatformAtlas for TestAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &crate::AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<crate::DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<crate::AtlasTile>> {
        let mut state = self.0.lock();
        if let Some(&tile) = state.tiles.get(key) {
            return Ok(Some(tile));
        }
        drop(state);

        let Some((size, _)) = build()? else {
            return Ok(None);
        };

        let mut state = self.0.lock();
        state.next_id += 1;
        let texture_id = state.next_id;
        state.next_id += 1;
        let tile_id = state.next_id;

        state.tiles.insert(
            key.clone(),
            crate::AtlasTile {
                texture_id: AtlasTextureId {
                    index: texture_id,
                    kind: key.texture_kind(),
                },
                tile_id: TileId(tile_id),
                padding: 0,
                bounds: crate::Bounds {
                    origin: Point::default(),
                    size,
                },
            },
        );

        Ok(Some(state.tiles[key]))
    }

    fn get_or_insert_with_diagnostics<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<AtlasAccess> {
        let mut state = self.0.lock();
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
        drop(state);

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
        let tile = crate::AtlasTile {
            texture_id: AtlasTextureId {
                index: texture_id,
                kind: key.texture_kind(),
            },
            tile_id: TileId(tile_id),
            padding: 0,
            bounds: crate::Bounds {
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
        let mut state = self.0.lock();
        state.tiles.remove(key);
    }

    fn remove_with_diagnostics(&self, key: &AtlasKey) -> AtlasRemoveDiagnostic {
        let mut state = self.0.lock();
        let removed = state.tiles.remove(key);
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
