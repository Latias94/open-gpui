use super::*;
use image::{Frame, ImageBuffer, Rgba};
use open_gpui::{
    AtlasKey, AtlasTextureId, AtlasTextureInstanceId, AtlasTextureLeaseEpoch,
    AtlasTextureLeaseError, AtlasTile, DevicePixels, ImageSource, PlatformAtlas, Point,
    RenderImage, Size, TileId, img, size,
};
use std::borrow::Cow;
use std::sync::{Arc, Mutex};

struct RejectNextActivationAtlas {
    fail_next_lease: Mutex<bool>,
    next_texture_index: Mutex<u32>,
}

impl RejectNextActivationAtlas {
    fn new() -> Self {
        Self {
            fail_next_lease: Mutex::new(false),
            next_texture_index: Mutex::new(0),
        }
    }

    fn fail_next_lease(&self) {
        *self
            .fail_next_lease
            .lock()
            .expect("activation test atlas lock should remain available") = true;
    }

    fn tile(key: &AtlasKey, index: u32) -> AtlasTile {
        AtlasTile {
            texture_id: AtlasTextureId {
                index,
                kind: key.texture_kind(),
            },
            tile_id: TileId(1),
            padding: 0,
            bounds: open_gpui::Bounds::new(
                Point::default(),
                size(DevicePixels(1), DevicePixels(1)),
            ),
            texture_generation: 1,
            texture_generation_padding: 0,
        }
    }
}

impl PlatformAtlas for RejectNextActivationAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        _build: &mut dyn FnMut() -> open_gpui::Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> open_gpui::Result<Option<AtlasTile>> {
        let mut next_texture_index = self
            .next_texture_index
            .lock()
            .expect("activation test atlas index lock should remain available");
        *next_texture_index += 1;
        Ok(Some(Self::tile(key, *next_texture_index)))
    }

    fn remove(&self, _key: &AtlasKey) {}

    fn atlas_texture_lease_epoch(&self) -> AtlasTextureLeaseEpoch {
        AtlasTextureLeaseEpoch::INITIAL
    }

    unsafe fn acquire_atlas_texture_leases(
        &self,
        textures: &[AtlasTextureInstanceId],
    ) -> std::result::Result<AtlasTextureLeaseEpoch, AtlasTextureLeaseError> {
        let mut fail_next = self
            .fail_next_lease
            .lock()
            .expect("activation test atlas lock should remain available");
        if *fail_next {
            *fail_next = false;
            return Err(AtlasTextureLeaseError::TextureUnavailable {
                texture: *textures
                    .first()
                    .expect("an activation test atlas lease must name a texture"),
                epoch: AtlasTextureLeaseEpoch::INITIAL,
            });
        }
        Ok(AtlasTextureLeaseEpoch::INITIAL)
    }

    unsafe fn release_atlas_texture_leases(
        &self,
        _epoch: AtlasTextureLeaseEpoch,
        _textures: &[AtlasTextureInstanceId],
    ) {
    }
}

#[open_gpui::test]
fn button_routes_pointer_keyboard_and_accessibility_through_one_typed_activation(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        activations: Rc<RefCell<Vec<ActivationSource>>>,
        disabled_activations: Rc<RefCell<Vec<ActivationSource>>>,
        prevent_key_up: Rc<Cell<bool>>,
        stop_key_up: Rc<Cell<bool>>,
        activation_handle: ActivationHandle,
        disabled_activation_handle: ActivationHandle,
        disabled_control: bool,
        show_disabled_control: bool,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let disabled_activations = self.disabled_activations.clone();
            let prevent_key_up = self.prevent_key_up.clone();
            let stop_key_up = self.stop_key_up.clone();

            div()
                .id("semantic-activation-capture-owner")
                .capture_key_up(move |_, window, cx| {
                    if prevent_key_up.get() {
                        window.prevent_default();
                    }
                    if stop_key_up.get() {
                        cx.stop_propagation();
                    }
                })
                .size_full()
                .flex()
                .flex_col()
                .child(
                    Button::new("semantic-activation-button", "Run")
                        .on_activate(move |activation, _, _| {
                            activations.borrow_mut().push(activation.source());
                        })
                        .activation_handle(&self.activation_handle),
                )
                .when(self.show_disabled_control, |this| {
                    this.child(
                        Button::new("disabled-semantic-activation-button", "Disabled")
                            .disabled(self.disabled_control)
                            .on_activate(move |activation, _, _| {
                                disabled_activations.borrow_mut().push(activation.source());
                            })
                            .activation_handle(&self.disabled_activation_handle),
                    )
                })
                .child(Button::new("semantic-activation-other-button", "Other"))
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let disabled_activations = Rc::new(RefCell::new(Vec::new()));
    let prevent_key_up = Rc::new(Cell::new(false));
    let stop_key_up = Rc::new(Cell::new(false));
    let activation_handle = ActivationHandle::new();
    let disabled_activation_handle = ActivationHandle::new();
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        activations: activations.clone(),
        disabled_activations: disabled_activations.clone(),
        prevent_key_up: prevent_key_up.clone(),
        stop_key_up: stop_key_up.clone(),
        activation_handle: activation_handle.clone(),
        disabled_activation_handle: disabled_activation_handle.clone(),
        disabled_control: true,
        show_disabled_control: true,
    });
    cx.update(|window, cx| {
        window.draw(cx).clear();
    });

    let button_bounds = cx
        .debug_bounds("button:semantic-activation-button:root")
        .expect("Button should expose a stable root selector");
    cx.simulate_click(button_bounds.center(), Modifiers::none());
    assert_eq!(
        activations.borrow().as_slice(),
        &[ActivationSource::Pointer]
    );

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("Button should publish a final accessibility tree");
    let button_node = node_with_label(&update, "Run");
    let disabled_node = node_with_label(&update, "Disabled");
    let other_node = node_with_label(&update, "Other");
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, button_node,))
    );

    let unpaired_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(unpaired_enter_up.propagated());
    assert_eq!(
        activations.borrow().len(),
        1,
        "an unpaired key-up must not activate"
    );

    let modified = Modifiers {
        control: true,
        ..Modifiers::none()
    };
    let modified_enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", modified, false));
    let released_modifier_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(modified_enter_down.propagated());
    assert!(released_modifier_enter_up.propagated());
    assert_eq!(
        activations.borrow().len(),
        1,
        "a modified key-down must not become an activation when the modifier is released first"
    );

    let prevented_enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(prevented_enter_down.propagation_stopped());
    prevent_key_up.set(true);
    let prevented_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    prevent_key_up.set(false);
    assert!(prevented_enter_up.propagated());
    assert!(prevented_enter_up.default_prevented());
    assert_eq!(
        activations.borrow().len(),
        1,
        "a capture owner that prevents key-up must cancel activation"
    );

    let stopped_enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(stopped_enter_down.propagation_stopped());
    stop_key_up.set(true);
    let stopped_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    stop_key_up.set(false);
    assert!(stopped_enter_up.propagation_stopped());
    assert_eq!(
        activations.borrow().len(),
        1,
        "a capture owner that stops key-up must cancel activation"
    );

    let stale_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(stale_enter_up.propagated());
    assert_eq!(
        activations.borrow().len(),
        1,
        "a later unpaired key-up must not reuse an armed transaction whose release was stopped"
    );

    let focus_changed_enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(focus_changed_enter_down.propagation_stopped());
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, other_node,))
    );
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, button_node,))
    );
    let focus_changed_enter_up =
        cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(focus_changed_enter_up.propagated());
    assert_eq!(
        activations.borrow().len(),
        1,
        "focus changes must invalidate an armed key transaction"
    );

    let enter_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(enter_down.propagation_stopped());
    assert!(!enter_down.default_prevented());
    assert_eq!(activations.borrow().len(), 1, "key-down must not activate");
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    let enter_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(enter_up.propagation_stopped());
    assert!(!enter_up.default_prevented());
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            ActivationSource::Pointer,
            ActivationSource::Keyboard(ActivationKey::Enter),
        ]
    );

    let space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), false));
    assert!(space_down.propagation_stopped());
    assert!(space_down.default_prevented());
    assert_eq!(
        activations.borrow().len(),
        2,
        "Space key-down must not activate"
    );

    let repeated_space_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("space", Modifiers::none(), true));
    assert!(repeated_space_down.propagation_stopped());
    assert!(repeated_space_down.default_prevented());
    assert_eq!(
        activations.borrow().len(),
        2,
        "held-key repeats must not activate"
    );

    let space_up = cx.simulate_event_with_dispatch_snapshot(key_up("space", Modifiers::none()));
    assert!(space_up.propagation_stopped());
    assert!(space_up.default_prevented());
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            ActivationSource::Pointer,
            ActivationSource::Keyboard(ActivationKey::Enter),
            ActivationSource::Keyboard(ActivationKey::Space),
        ]
    );

    let modified_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", modified, false));
    let modified_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", modified));
    assert!(modified_down.propagated());
    assert!(!modified_down.default_prevented());
    assert!(modified_up.propagated());
    assert!(!modified_up.default_prevented());
    assert_eq!(activations.borrow().len(), 3);

    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, button_node,))
    );
    assert_eq!(
        activations.borrow().as_slice(),
        &[
            ActivationSource::Pointer,
            ActivationSource::Keyboard(ActivationKey::Enter),
            ActivationSource::Keyboard(ActivationKey::Space),
            ActivationSource::Accessibility,
        ],
        "AccessKit Click must dispatch directly instead of synthesizing a pointer activation"
    );

    cx.update(|window, cx| {
        assert_eq!(
            activation_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
        assert_eq!(
            disabled_activation_handle.request(window, cx),
            ActivationRequestResult::Blocked
        );
    });
    assert_eq!(
        activations.borrow().last(),
        Some(&ActivationSource::Programmatic)
    );

    let disabled_bounds = cx
        .debug_bounds("button:disabled-semantic-activation-button:root")
        .expect("disabled Button should keep a stable root selector");
    cx.simulate_click(disabled_bounds.center(), Modifiers::none());
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Click, disabled_node,))
    );
    assert!(disabled_activations.borrow().is_empty());

    cx.simulate_mouse_down(
        disabled_bounds.center(),
        MouseButton::Left,
        Modifiers::none(),
    );
    view.update(cx, |probe, cx| {
        probe.disabled_control = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let enabled_bounds = cx
        .debug_bounds("button:disabled-semantic-activation-button:root")
        .expect("enabled Button should retain its stable root selector");
    cx.simulate_mouse_up(
        enabled_bounds.center(),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert!(
        disabled_activations.borrow().is_empty(),
        "a pointer press that began while disabled must not activate after a gate change"
    );
    cx.update(|window, cx| {
        assert_eq!(
            disabled_activation_handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(
        disabled_activations.borrow().as_slice(),
        &[ActivationSource::Programmatic]
    );

    view.update(cx, |probe, cx| {
        probe.show_disabled_control = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            disabled_activation_handle.request(window, cx),
            ActivationRequestResult::Unavailable
        );
    });
}

#[open_gpui::test]
fn pointer_activation_survives_same_gate_owner_rerender(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        activations: Rc<RefCell<Vec<ActivationSource>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            Button::new("pointer-rerender-button", "Continue").on_activate(
                move |activation, _, _| activations.borrow_mut().push(activation.source()),
            )
        }
    }

    let activations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        activations: activations.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let bounds = cx
        .debug_bounds("button:pointer-rerender-button:root")
        .expect("Button should expose a stable root selector");
    cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    let bounds = cx
        .debug_bounds("button:pointer-rerender-button:root")
        .expect("Button should retain its root selector after rerender");
    cx.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::none());

    assert_eq!(
        activations.borrow().as_slice(),
        &[ActivationSource::Pointer]
    );
}

#[open_gpui::test]
fn activation_handle_publishes_only_from_accepted_frames(cx: &mut open_gpui::TestAppContext) {
    #[derive(Clone, Copy)]
    enum Control {
        First,
        Second,
        Absent,
    }

    struct Probe {
        control: Control,
        handle: ActivationHandle,
        first_activations: Rc<Cell<usize>>,
        second_activations: Rc<Cell<usize>>,
        first_image: Arc<RenderImage>,
        second_image: Arc<RenderImage>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let first_activations = self.first_activations.clone();
            let second_activations = self.second_activations.clone();
            let image = match self.control {
                Control::First => self.first_image.clone(),
                Control::Second | Control::Absent => self.second_image.clone(),
            };
            div()
                .child(
                    img(ImageSource::Render(image))
                        .w(open_gpui::px(1.0))
                        .h(open_gpui::px(1.0)),
                )
                .when(matches!(self.control, Control::First), |this| {
                    this.child(
                        Button::new("accepted-frame-first-button", "First")
                            .activation_handle(&self.handle)
                            .on_activate(move |_, _, _| {
                                first_activations.set(first_activations.get() + 1)
                            }),
                    )
                })
                .when(matches!(self.control, Control::Second), |this| {
                    this.child(
                        Button::new("accepted-frame-second-button", "Second")
                            .activation_handle(&self.handle)
                            .on_activate(move |_, _, _| {
                                second_activations.set(second_activations.get() + 1)
                            }),
                    )
                })
        }
    }

    let atlas = Arc::new(RejectNextActivationAtlas::new());
    let handle = ActivationHandle::new();
    let first_activations = Rc::new(Cell::new(0));
    let second_activations = Rc::new(Cell::new(0));
    let first_image = Arc::new(RenderImage::new([Frame::new(ImageBuffer::from_pixel(
        1,
        1,
        Rgba([0, 0, 0, 0xff]),
    ))]));
    let second_image = Arc::new(RenderImage::new([Frame::new(ImageBuffer::from_pixel(
        1,
        1,
        Rgba([0xff, 0, 0, 0xff]),
    ))]));
    let (view, cx) = cx.add_window_view({
        let atlas = atlas.clone();
        let handle = handle.clone();
        let first_activations = first_activations.clone();
        let second_activations = second_activations.clone();
        let first_image = first_image.clone();
        let second_image = second_image.clone();
        move |window, _| {
            window.set_sprite_atlas_for_test(atlas);
            Probe {
                control: Control::First,
                handle,
                first_activations,
                second_activations,
                first_image,
                second_image,
            }
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let committed_generation = cx.update(|window, _| window.rendered_frame_revision());
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(first_activations.get(), 1);
    assert_eq!(second_activations.get(), 0);

    cx.update(|window, cx| {
        atlas.fail_next_lease();
        view.update(cx, |view, cx| {
            view.control = Control::Second;
            cx.notify();
        });
        window.draw(cx).clear();
        assert_eq!(
            window.rendered_frame_revision(),
            committed_generation,
            "the fresh image lease must reject the second candidate"
        );
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched,
            "the rejected candidate must preserve the still-visible first control"
        );
        assert_eq!(
            (first_activations.get(), second_activations.get()),
            (2, 0),
            "the rejected candidate must not publish the second dispatcher"
        );

        window.draw(cx).clear();
        assert_eq!(window.rendered_frame_revision(), committed_generation + 1);
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(first_activations.get(), 2);
    assert_eq!(second_activations.get(), 1);

    view.update(cx, |view, cx| {
        view.control = Control::Absent;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Unavailable,
            "accepted absence must clear only the exact committed publication"
        );
    });
}

#[open_gpui::test]
fn atlas_rejected_candidate_preserves_armed_pointer_runtime(cx: &mut open_gpui::TestAppContext) {
    struct Probe {
        disabled: bool,
        use_second_image: bool,
        activations: Rc<RefCell<Vec<ActivationSource>>>,
        first_image: Arc<RenderImage>,
        second_image: Arc<RenderImage>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let image = if self.use_second_image {
                self.second_image.clone()
            } else {
                self.first_image.clone()
            };
            div()
                .child(
                    img(ImageSource::Render(image))
                        .w(open_gpui::px(1.0))
                        .h(open_gpui::px(1.0)),
                )
                .child(
                    Button::new("atlas-rejected-armed-button", "Armed")
                        .disabled(self.disabled)
                        .on_activate(move |activation, _, _| {
                            activations.borrow_mut().push(activation.source())
                        }),
                )
        }
    }

    let atlas = Arc::new(RejectNextActivationAtlas::new());
    let activations = Rc::new(RefCell::new(Vec::new()));
    let first_image = Arc::new(RenderImage::new([Frame::new(ImageBuffer::from_pixel(
        1,
        1,
        Rgba([0, 0, 0, 0xff]),
    ))]));
    let second_image = Arc::new(RenderImage::new([Frame::new(ImageBuffer::from_pixel(
        1,
        1,
        Rgba([0xff, 0, 0, 0xff]),
    ))]));
    let (view, cx) = cx.add_window_view({
        let atlas = atlas.clone();
        let activations = activations.clone();
        let first_image = first_image.clone();
        let second_image = second_image.clone();
        move |window, _| {
            window.set_sprite_atlas_for_test(atlas);
            Probe {
                disabled: false,
                use_second_image: false,
                activations,
                first_image,
                second_image,
            }
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("the accepted button should publish accessibility state");
    let button_node = node_with_label(&update, "Armed");
    assert!(
        cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, button_node,))
    );
    let bounds = cx
        .debug_bounds("button:atlas-rejected-armed-button:root")
        .expect("the accepted button should expose stable bounds");
    cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none());
    let committed_generation = cx.update(|window, _| window.rendered_frame_revision());

    cx.update(|window, cx| {
        atlas.fail_next_lease();
        view.update(cx, |view, cx| {
            view.disabled = true;
            view.use_second_image = true;
            cx.notify();
        });
        window.draw(cx).clear();
        assert_eq!(
            window.rendered_frame_revision(),
            committed_generation,
            "the disabled candidate must be rejected before it can replace the armed control"
        );
        let mouse_up = window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: bounds.center(),
                modifiers: Modifiers::none(),
                click_count: 1,
            }),
            cx,
        );
        assert!(!mouse_up.propagate);
    });

    assert_eq!(
        activations.borrow().as_slice(),
        &[ActivationSource::Pointer],
        "candidate-only gate changes must not cancel interactions owned by the visible frame"
    );
}

#[open_gpui::test]
fn activation_handle_publication_survives_cached_journal_replay(
    cx: &mut open_gpui::TestAppContext,
) {
    struct CachedButton {
        handle: ActivationHandle,
        activations: Rc<Cell<usize>>,
        renders: Rc<Cell<usize>>,
    }

    impl Render for CachedButton {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            let activations = self.activations.clone();
            Button::new("cached-activation-button", "Cached")
                .activation_handle(&self.handle)
                .on_activate(move |_, _, _| activations.set(activations.get() + 1))
        }
    }

    struct Root {
        child: Entity<CachedButton>,
        show_child: bool,
        parent_revision: usize,
    }

    impl Render for Root {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let _ = self.parent_revision;
            div().when(self.show_child, |this| {
                this.child(
                    AnyView::from(self.child.clone()).cached(
                        StyleRefinement::default()
                            .w(open_gpui::px(120.0))
                            .h(open_gpui::px(32.0)),
                    ),
                )
            })
        }
    }

    let handle = ActivationHandle::new();
    let activations = Rc::new(Cell::new(0));
    let renders = Rc::new(Cell::new(0));
    let (root, cx) = cx.add_window_view({
        let handle = handle.clone();
        let activations = activations.clone();
        let renders = renders.clone();
        move |_, cx| Root {
            child: cx.new(|_| CachedButton {
                handle,
                activations,
                renders,
            }),
            show_child: true,
            parent_revision: 0,
        }
    });

    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(renders.get(), 1);
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(activations.get(), 1);

    root.update(cx, |root, cx| {
        root.parent_revision += 1;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        renders.get(),
        1,
        "an unchanged cached child must replay its accepted publication journal"
    );
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched,
            "journal replay must retain the accepted dispatcher publication"
        );
    });
    assert_eq!(activations.get(), 2);

    root.update(cx, |root, cx| {
        root.show_child = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Unavailable,
            "an accepted frame without the cached child must discard its publication"
        );
    });
}

#[open_gpui::test]
fn reused_activation_handle_keeps_the_last_interactive_publication(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        handle: ActivationHandle,
        second_interactive: bool,
        first_activations: Rc<Cell<usize>>,
        second_activations: Rc<Cell<usize>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let first_activations = self.first_activations.clone();
            let second_activations = self.second_activations.clone();
            div()
                .child(
                    Button::new("duplicate-handle-first", "First")
                        .activation_handle(&self.handle)
                        .on_activate(move |_, _, _| {
                            first_activations.set(first_activations.get() + 1)
                        }),
                )
                .child(
                    Button::new("duplicate-handle-second", "Second")
                        .activation_handle(&self.handle)
                        .on_activate(move |_, _, _| {
                            second_activations.set(second_activations.get() + 1)
                        })
                        .with_subtree_presentation(if self.second_interactive {
                            SubtreePresentation::Visible
                        } else {
                            SubtreePresentation::Inert
                        }),
                )
        }
    }

    let handle = ActivationHandle::new();
    let first_activations = Rc::new(Cell::new(0));
    let second_activations = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view({
        let handle = handle.clone();
        let first_activations = first_activations.clone();
        let second_activations = second_activations.clone();
        move |_, _| Probe {
            handle,
            second_interactive: false,
            first_activations,
            second_activations,
        }
    });

    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched,
            "an inert later publication must not clear the earlier interactive control"
        );
    });
    assert_eq!((first_activations.get(), second_activations.get()), (1, 0));

    view.update(cx, |view, cx| {
        view.second_interactive = true;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched,
            "the later interactive publication should become the accepted winner"
        );
    });
    assert_eq!((first_activations.get(), second_activations.get()), (1, 1));
}

#[open_gpui::test]
fn accepted_handle_replacement_is_visible_to_focus_stable_callbacks(
    cx: &mut open_gpui::TestAppContext,
) {
    #[derive(Clone, Copy)]
    enum HandleSelection {
        First,
        Second,
        None,
    }

    struct Probe {
        selection: HandleSelection,
        observation_stage: Rc<Cell<usize>>,
        first_handle: ActivationHandle,
        second_handle: ActivationHandle,
        observations: Rc<RefCell<Vec<(usize, ActivationRequestResult, ActivationRequestResult)>>>,
        activations: Rc<Cell<usize>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            let button = Button::new("accepted-handle-replacement", "Replace")
                .on_activate(move |_, _, _| activations.set(activations.get() + 1));
            let button = match self.selection {
                HandleSelection::First => button.activation_handle(&self.first_handle),
                HandleSelection::Second => button.activation_handle(&self.second_handle),
                HandleSelection::None => button,
            };
            let observation_stage = self.observation_stage.clone();
            let first_handle = self.first_handle.clone();
            let second_handle = self.second_handle.clone();
            let observations = self.observations.clone();
            div().child(button).child(
                open_gpui::canvas(
                    move |_, window, _| {
                        let observation_stage = observation_stage.clone();
                        let first_handle = first_handle.clone();
                        let second_handle = second_handle.clone();
                        let observations = observations.clone();
                        window.record_prepaint_focus_stable_commit(move |_, window, cx| {
                            let stage = observation_stage.get();
                            if stage == 0 {
                                return;
                            }
                            observations.borrow_mut().push((
                                stage,
                                first_handle.request(window, cx),
                                second_handle.request(window, cx),
                            ));
                        });
                    },
                    |_, _, _, _| {},
                )
                .w(open_gpui::px(1.0))
                .h(open_gpui::px(1.0)),
            )
        }
    }

    let observation_stage = Rc::new(Cell::new(0));
    let first_handle = ActivationHandle::new();
    let second_handle = ActivationHandle::new();
    let observations = Rc::new(RefCell::new(Vec::new()));
    let activations = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view({
        let observation_stage = observation_stage.clone();
        let first_handle = first_handle.clone();
        let second_handle = second_handle.clone();
        let observations = observations.clone();
        let activations = activations.clone();
        move |_, _| Probe {
            selection: HandleSelection::First,
            observation_stage,
            first_handle,
            second_handle,
            observations,
            activations,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());

    observation_stage.set(1);
    view.update(cx, |view, cx| {
        view.selection = HandleSelection::Second;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        observations.borrow().as_slice(),
        &[((
            1,
            ActivationRequestResult::Unavailable,
            ActivationRequestResult::Dispatched,
        ))],
        "frame-stable callbacks must see the replacement handle and not the retired dispatcher"
    );
    assert_eq!(activations.get(), 1);

    observation_stage.set(2);
    view.update(cx, |view, cx| {
        view.selection = HandleSelection::None;
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        observations.borrow().last().copied(),
        Some((
            2,
            ActivationRequestResult::Unavailable,
            ActivationRequestResult::Unavailable,
        )),
        "accepted removal must be visible before focus-stable callbacks run"
    );
    assert_eq!(activations.get(), 1);
}

#[open_gpui::test]
fn presentation_suppression_unbinds_programmatic_activation_and_clears_armed_input(
    cx: &mut open_gpui::TestAppContext,
) {
    struct Probe {
        presentation: SubtreePresentation,
        handle: ActivationHandle,
        activations: Rc<RefCell<Vec<ActivationSource>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let activations = self.activations.clone();
            Button::new("presentation-activation-button", "Present")
                .on_activate(move |activation, _, _| {
                    activations.borrow_mut().push(activation.source())
                })
                .activation_handle(&self.handle)
                .with_subtree_presentation(self.presentation)
        }
    }

    let handle = ActivationHandle::new();
    let activations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view(|_, _| Probe {
        presentation: SubtreePresentation::Visible,
        handle: handle.clone(),
        activations: activations.clone(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let bounds = cx
        .debug_bounds("button:presentation-activation-button:root")
        .expect("visible button should expose its root bounds");

    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    activations.borrow_mut().clear();

    cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none());
    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Unavailable
        );
    });
    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::none());
    assert!(
        activations.borrow().is_empty(),
        "restoring visibility must not consume a pointer arm from before suppression"
    );

    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("visible button should publish accessibility");
    let node = node_with_label(&update, "Present");
    assert!(cx.dispatch_accessibility_action(action_request(accesskit::Action::Focus, node,)));
    let key_down =
        cx.simulate_event_with_dispatch_snapshot(key_down("enter", Modifiers::none(), false));
    assert!(key_down.propagation_stopped());
    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Hidden;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Unavailable
        );
    });
    view.update(cx, |view, cx| {
        view.presentation = SubtreePresentation::Visible;
        cx.notify();
    });
    cx.run_until_parked();
    let key_up = cx.simulate_event_with_dispatch_snapshot(key_up("enter", Modifiers::none()));
    assert!(key_up.propagated());
    assert!(
        activations.borrow().is_empty(),
        "restoring visibility must not consume a key arm from before suppression"
    );

    cx.update(|window, cx| {
        assert_eq!(
            handle.request(window, cx),
            ActivationRequestResult::Dispatched
        );
    });
    assert_eq!(
        activations.borrow().as_slice(),
        &[ActivationSource::Programmatic]
    );
}
