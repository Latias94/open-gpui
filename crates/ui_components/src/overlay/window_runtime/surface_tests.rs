use super::*;

use open_gpui::{
    AccessibleAction, AnyView, AppContext as _, Bounds, Context, InteractiveElement, ParentElement,
    Render, Role, StatefulInteractiveElement, Styled, accesskit, deferred, div, point, px, size,
};

const PARENT_LAYER: &str = "surface-parent";
const LOCAL_CHILD_LAYER: &str = "local-surface-child";
const FOREIGN_CHILD_LAYER: &str = "foreign-surface-child";

struct NestedLayerProbe {
    runtime: WindowOverlayRuntime,
    layer_id: &'static str,
    binding: Option<OverlayLayerBinding>,
}

impl NestedLayerProbe {
    fn new(runtime: WindowOverlayRuntime, layer_id: &'static str) -> Self {
        Self {
            runtime,
            layer_id,
            binding: None,
        }
    }
}

impl Render for NestedLayerProbe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let binding = self
            .runtime
            .bind_component_layer(
                &cx.entity(),
                self.binding.as_ref(),
                layer_registration(self.layer_id),
                window,
                cx,
            )
            .expect("nested layer should bind in its own window");
        self.binding = Some(binding);

        div()
            .id(self.layer_id)
            .role(Role::Button)
            .aria_label(self.layer_id)
            .size_full()
    }
}

struct SurfaceProjectionProbe {
    surface_runtime: WindowOverlayRuntime,
    snapshot_runtime: WindowOverlayRuntime,
    surface_binding: Option<OverlayLayerBinding>,
    child: Option<Entity<NestedLayerProbe>>,
    projects_parent: bool,
}

impl SurfaceProjectionProbe {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let runtime = WindowOverlayRuntime::for_window(window, cx);
        Self {
            surface_runtime: runtime.clone(),
            snapshot_runtime: runtime,
            surface_binding: None,
            child: None,
            projects_parent: true,
        }
    }

    fn foreign(
        surface_runtime: WindowOverlayRuntime,
        surface_binding: OverlayLayerBinding,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            surface_runtime,
            snapshot_runtime: WindowOverlayRuntime::for_window(window, cx),
            surface_binding: Some(surface_binding),
            child: None,
            projects_parent: true,
        }
    }

    fn mount_local_surface(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let parent = self
            .surface_runtime
            .register_layer(layer_registration(PARENT_LAYER), window, cx)
            .expect("surface parent should register in its own window");
        self.surface_binding = Some(parent);
        self.mount_child(LOCAL_CHILD_LAYER, cx);
    }

    fn mount_local_inside_region(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mount_local_surface(window, cx);
        self.projects_parent = false;
    }

    fn mount_child(&mut self, layer_id: &'static str, cx: &mut Context<Self>) {
        let runtime = self.snapshot_runtime.clone();
        self.child = Some(cx.new(|_| NestedLayerProbe::new(runtime, layer_id)));
        cx.notify();
    }
}

impl Render for SurfaceProjectionProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut root = div().size_full();
        if let (Some(binding), Some(child)) = (&self.surface_binding, &self.child) {
            let child = AnyView::from(child.clone());
            root = if self.projects_parent {
                root.child(self.surface_runtime.surface(
                    binding,
                    OverlayInsideRegionId::new("surface-parent-region"),
                    "surface-parent-wrapper",
                    child,
                ))
            } else {
                root.child(self.surface_runtime.inside_region(
                    binding,
                    OverlayInsideRegionId::new("inside-only-parent-region"),
                    "inside-only-parent-wrapper",
                    child,
                ))
            };
        }
        root
    }
}

struct A11yOverlaySurfaceProbe {
    runtime: WindowOverlayRuntime,
    modal: OverlayLayerBinding,
    modal_descendant: OverlayLayerBinding,
    unrelated: OverlayLayerBinding,
    underlay_activations: usize,
}

impl A11yOverlaySurfaceProbe {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let runtime = WindowOverlayRuntime::for_window(window, cx);
        let modal = runtime
            .register_layer(
                modal_registration("tree-modal", OverlayPresence::hidden()),
                window,
                cx,
            )
            .expect("hidden modal should register");
        let modal_descendant = runtime
            .register_layer(
                OverlayLayerRegistration::new(
                    "tree-modal-descendant",
                    OverlayLayerPolicy::new(
                        OverlayLayerKind::NonModalDismissible,
                        OverlayPresence::hidden(),
                    ),
                    OverlayOwnership::Controlled,
                )
                .focus_mode(OverlayFocusMode::None)
                .parent("tree-modal"),
                window,
                cx,
            )
            .expect("hidden modal descendant should register");
        let unrelated = runtime
            .register_layer(layer_registration("tree-unrelated"), window, cx)
            .expect("unrelated overlay should register");

        Self {
            runtime,
            modal,
            modal_descendant,
            unrelated,
            underlay_activations: 0,
        }
    }

    fn open_modal(&self, window: &mut Window, cx: &mut App) {
        self.runtime
            .rebind_layer(
                &self.modal,
                modal_registration("tree-modal", OverlayPresence::open()),
                window,
                cx,
            )
            .expect("modal should open");
        self.runtime
            .rebind_layer(
                &self.modal_descendant,
                child_registration("tree-modal-descendant", "tree-modal"),
                window,
                cx,
            )
            .expect("modal descendant should open");
    }

    fn begin_modal_close(&self, window: &mut Window, cx: &mut App) {
        self.runtime
            .rebind_layer(
                &self.modal,
                modal_registration("tree-modal", OverlayPresence::closing()),
                window,
                cx,
            )
            .expect("modal subtree should enter closing");
    }
}

impl Render for A11yOverlaySurfaceProbe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        div()
            .id("a11y-overlay-surface-probe")
            .role(Role::Group)
            .size_full()
            .child(
                div()
                    .id("tree-underlay")
                    .role(Role::Button)
                    .aria_label("Runtime underlay")
                    .aria_value(format!("activation-{}", self.underlay_activations))
                    .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            this.underlay_activations += 1;
                            cx.notify();
                        })
                        .ok();
                    }),
            )
            .child(
                self.runtime.surface(
                    &self.modal,
                    OverlayInsideRegionId::new("tree-modal-region"),
                    "tree-modal-wrapper",
                    div()
                        .id("tree-modal-node")
                        .role(Role::Dialog)
                        .aria_label("Runtime modal")
                        .aria_modal(true),
                ),
            )
            .child(deferred(
                self.runtime.surface(
                    &self.modal_descendant,
                    OverlayInsideRegionId::new("tree-modal-descendant-region"),
                    "tree-modal-descendant-wrapper",
                    div()
                        .id("tree-modal-descendant-node")
                        .role(Role::Menu)
                        .aria_label("Runtime deferred modal descendant"),
                ),
            ))
            .child(
                self.runtime.surface(
                    &self.unrelated,
                    OverlayInsideRegionId::new("tree-unrelated-region"),
                    "tree-unrelated-wrapper",
                    div()
                        .id("tree-unrelated-node")
                        .role(Role::List)
                        .aria_label("Runtime unrelated overlay"),
                ),
            )
    }
}

#[derive(Clone, Copy)]
enum OpenChangeEffect {
    Redraw,
    Commit,
    Supersede,
    Unregister,
}

struct OpenChangeEffectProbe {
    runtime: WindowOverlayRuntime,
    binding: Option<OverlayLayerBinding>,
    render_layer: bool,
    committed_open: bool,
    effect: OpenChangeEffect,
    events: Rc<RefCell<Vec<(bool, DismissReason)>>>,
}

impl OpenChangeEffectProbe {
    fn new(
        effect: OpenChangeEffect,
        events: Rc<RefCell<Vec<(bool, DismissReason)>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            runtime: WindowOverlayRuntime::for_window(window, cx),
            binding: None,
            render_layer: true,
            committed_open: false,
            effect,
            events,
        }
    }
}

impl Render for OpenChangeEffectProbe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.render_layer {
            return div().into_any_element();
        }
        let presence = if self.committed_open {
            OverlayPresence::open()
        } else {
            OverlayPresence::hidden()
        };
        let events = self.events.clone();
        let registration = OverlayLayerRegistration::new(
            "controlled-open-effect",
            OverlayLayerPolicy::new(OverlayLayerKind::NonModalDismissible, presence),
            OverlayOwnership::Controlled,
        )
        .focus_mode(OverlayFocusMode::None)
        .on_open_change(move |intent, _, _| {
            events
                .borrow_mut()
                .push((intent.desired_open(), intent.reason()));
        });
        let binding = self
            .runtime
            .bind_component_layer(
                &cx.entity(),
                self.binding.as_ref(),
                registration,
                window,
                cx,
            )
            .expect("controlled effect probe should bind its layer");
        self.binding = Some(binding.clone());

        let runtime = self.runtime.clone();
        let effect_runtime = self.runtime.clone();
        let effect_binding = binding.clone();
        let owner = cx.entity().downgrade();
        let effect = self.effect;
        div()
            .id("controlled-open-effect-trigger")
            .role(Role::Button)
            .aria_label("Controlled open effect trigger")
            .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                let effect_runtime = effect_runtime.clone();
                let effect_binding = effect_binding.clone();
                let owner = owner.clone();
                runtime
                    .request_open_change_with_effect(
                        &binding,
                        true,
                        DismissReason::Trigger,
                        window,
                        cx,
                        move |window, cx| match effect {
                            OpenChangeEffect::Redraw => {
                                window.draw(cx).clear();
                            }
                            OpenChangeEffect::Commit | OpenChangeEffect::Supersede => {
                                owner
                                    .update(cx, |probe, cx| {
                                        probe.committed_open = true;
                                        cx.notify();
                                    })
                                    .expect("controlled effect owner should remain live");
                                window.draw(cx).clear();
                                if matches!(effect, OpenChangeEffect::Supersede) {
                                    effect_runtime
                                        .request_open_change(
                                            &effect_binding,
                                            false,
                                            DismissReason::Programmatic,
                                            window,
                                            cx,
                                        )
                                        .expect("new close request should supersede open observer");
                                }
                            }
                            OpenChangeEffect::Unregister => {
                                owner
                                    .update(cx, |probe, cx| {
                                        probe.render_layer = false;
                                        probe.binding = None;
                                        cx.notify();
                                    })
                                    .expect("controlled effect owner should remain live");
                                effect_runtime
                                    .unregister_layer(&effect_binding, window, cx)
                                    .expect("effect should unregister its current layer");
                            }
                        },
                    )
                    .expect("controlled effect should request open");
            })
            .into_any_element()
    }
}

fn layer_registration(id: &'static str) -> OverlayLayerRegistration {
    OverlayLayerRegistration::new(
        id,
        OverlayLayerPolicy::new(
            OverlayLayerKind::NonModalDismissible,
            OverlayPresence::open(),
        ),
        OverlayOwnership::Controlled,
    )
    .focus_mode(OverlayFocusMode::None)
}

fn modal_registration(id: &'static str, presence: OverlayPresence) -> OverlayLayerRegistration {
    OverlayLayerRegistration::new(
        id,
        OverlayLayerPolicy::new(OverlayLayerKind::Modal, presence),
        OverlayOwnership::Controlled,
    )
}

fn child_registration(id: &'static str, parent: &'static str) -> OverlayLayerRegistration {
    layer_registration(id).parent(parent)
}

fn a11y_disposition(
    runtime: &WindowOverlayRuntime,
    binding: &OverlayLayerBinding,
    cx: &App,
) -> AccessibilityTreeScope {
    runtime
        .state
        .read(cx)
        .accessibility_tree_scope(binding.lease(), runtime.window_id)
}

fn a11y_node_id_with_label(update: &accesskit::TreeUpdate, label: &str) -> accesskit::NodeId {
    update
        .nodes
        .iter()
        .find_map(|(id, node)| (node.label() == Some(label)).then_some(*id))
        .unwrap_or_else(|| panic!("missing accessibility node labelled {label:?}"))
}

fn run_controlled_open_effect(
    cx: &mut open_gpui::TestAppContext,
    effect: OpenChangeEffect,
) -> (
    Vec<(bool, DismissReason)>,
    Option<(OverlayLayerPhase, Option<bool>, Option<DismissReason>)>,
) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let probe_events = events.clone();
    let window = cx
        .add_window(move |window, cx| OpenChangeEffectProbe::new(effect, probe_events, window, cx));
    let any_window = window.clone().into();
    cx.update_window(any_window, |_, window, cx| window.draw(cx).clear())
        .expect("controlled effect window should draw");

    assert!(cx.activate_accessibility(any_window));
    let tree = cx
        .latest_accessibility_tree_update(any_window)
        .expect("controlled effect trigger should publish accessibility");
    let trigger = a11y_node_id_with_label(&tree, "Controlled open effect trigger");
    assert!(cx.dispatch_accessibility_action(
        any_window,
        accesskit::ActionRequest {
            action: AccessibleAction::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node: trigger,
            data: None,
        },
    ));

    let projection = window
        .update(cx, |probe, window, cx| {
            probe
                .runtime
                .snapshot(window, cx)
                .expect("controlled effect snapshot should resolve")
                .layers()
                .iter()
                .find(|layer| layer.id().as_str() == "controlled-open-effect")
                .map(|layer| (layer.phase(), layer.pending_open(), layer.pending_intent()))
        })
        .expect("controlled effect window should remain open");
    let events = events.borrow().clone();
    (events, projection)
}

#[open_gpui::test]
fn controlled_open_observer_survives_effect_redraw_rebind(cx: &mut open_gpui::TestAppContext) {
    let (events, projection) = run_controlled_open_effect(cx, OpenChangeEffect::Redraw);

    assert_eq!(events, [(true, DismissReason::Trigger)]);
    assert_eq!(
        projection,
        Some((
            OverlayLayerPhase::Hidden,
            Some(true),
            Some(DismissReason::Trigger),
        )),
        "an unrelated redraw must preserve the unresolved controlled-open request"
    );
}

#[open_gpui::test]
fn controlled_open_observer_is_invalidated_by_owner_commit(cx: &mut open_gpui::TestAppContext) {
    let (events, projection) = run_controlled_open_effect(cx, OpenChangeEffect::Commit);

    assert!(events.is_empty());
    assert_eq!(
        projection,
        Some((OverlayLayerPhase::Open, None, None)),
        "owner commit must resolve the request before its stale observer runs"
    );
}

#[open_gpui::test]
fn controlled_open_observer_is_invalidated_by_superseding_request(
    cx: &mut open_gpui::TestAppContext,
) {
    let (events, projection) = run_controlled_open_effect(cx, OpenChangeEffect::Supersede);

    assert_eq!(events, [(false, DismissReason::Programmatic)]);
    assert_eq!(
        projection,
        Some((
            OverlayLayerPhase::CloseRequested,
            Some(false),
            Some(DismissReason::Programmatic),
        )),
        "only the newest controlled request may reach an observer"
    );
}

#[open_gpui::test]
fn controlled_open_observer_is_invalidated_by_unregister(cx: &mut open_gpui::TestAppContext) {
    let (events, _) = run_controlled_open_effect(cx, OpenChangeEffect::Unregister);

    assert!(events.is_empty());
}

#[open_gpui::test]
fn controlled_child_pending_close_observer_survives_ancestor_commit(
    cx: &mut open_gpui::TestAppContext,
) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let child_events = events.clone();
    let window = cx.add_window(SurfaceProjectionProbe::new);

    let child_projection = window
        .update(cx, |probe, window, cx| {
            let runtime = probe.surface_runtime.clone();
            let parent = runtime
                .register_layer(
                    OverlayLayerRegistration::new(
                        "pending-close-parent",
                        OverlayLayerPolicy::new(
                            OverlayLayerKind::NonModalDismissible,
                            OverlayPresence::open(),
                        ),
                        OverlayOwnership::Controlled,
                    )
                    .focus_mode(OverlayFocusMode::None),
                    window,
                    cx,
                )
                .expect("pending-close parent should register");
            let child = runtime
                .register_layer(
                    OverlayLayerRegistration::new(
                        "pending-close-child",
                        OverlayLayerPolicy::new(
                            OverlayLayerKind::NonModalDismissible,
                            OverlayPresence::open(),
                        ),
                        OverlayOwnership::Controlled,
                    )
                    .focus_mode(OverlayFocusMode::None)
                    .parent("pending-close-parent")
                    .on_open_change(move |intent, _, _| {
                        child_events
                            .borrow_mut()
                            .push((intent.desired_open(), intent.reason()));
                    }),
                    window,
                    cx,
                )
                .expect("pending-close child should register");

            let effect_runtime = runtime.clone();
            runtime
                .request_open_change_with_effect(
                    &child,
                    false,
                    DismissReason::Selection,
                    window,
                    cx,
                    move |window, cx| {
                        effect_runtime
                            .rebind_layer(
                                &parent,
                                OverlayLayerRegistration::new(
                                    "pending-close-parent",
                                    OverlayLayerPolicy::new(
                                        OverlayLayerKind::NonModalDismissible,
                                        OverlayPresence::hidden(),
                                    ),
                                    OverlayOwnership::Controlled,
                                )
                                .focus_mode(OverlayFocusMode::None),
                                window,
                                cx,
                            )
                            .expect("parent owner should commit hidden presence");
                    },
                )
                .expect("child selection should request close");

            runtime
                .snapshot(window, cx)
                .expect("pending-close snapshot should resolve")
                .layers()
                .iter()
                .find(|layer| layer.id().as_str() == "pending-close-child")
                .map(|layer| (layer.phase(), layer.pending_open(), layer.pending_intent()))
                .expect("pending-close child should remain registered")
        })
        .expect("pending-close window should remain open");

    assert_eq!(
        events.borrow().as_slice(),
        [(false, DismissReason::Selection)]
    );
    assert_eq!(
        child_projection,
        (
            OverlayLayerPhase::Hidden,
            Some(false),
            Some(DismissReason::Selection),
        ),
        "ancestor commit must preserve the already queued child close request"
    );
}

#[open_gpui::test]
fn controlled_reopen_observer_survives_closing_presence_rebind(cx: &mut open_gpui::TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let open_events = events.clone();
    let window = cx.add_window(SurfaceProjectionProbe::new);

    let projection = window
        .update(cx, |probe, window, cx| {
            let runtime = probe.surface_runtime.clone();
            let rebound_events = events.clone();
            let binding = runtime
                .register_layer(
                    OverlayLayerRegistration::new(
                        "controlled-closing-reopen",
                        OverlayLayerPolicy::new(
                            OverlayLayerKind::NonModalDismissible,
                            OverlayPresence::closing(),
                        ),
                        OverlayOwnership::Controlled,
                    )
                    .focus_mode(OverlayFocusMode::None)
                    .on_open_change(move |intent, _, _| {
                        open_events
                            .borrow_mut()
                            .push((intent.desired_open(), intent.reason()));
                    }),
                    window,
                    cx,
                )
                .expect("controlled closing layer should register");

            let effect_runtime = runtime.clone();
            let effect_binding = binding.clone();
            runtime
                .request_open_change_with_effect(
                    &binding,
                    true,
                    DismissReason::Programmatic,
                    window,
                    cx,
                    move |window, cx| {
                        effect_runtime
                            .rebind_layer(
                                &effect_binding,
                                OverlayLayerRegistration::new(
                                    "controlled-closing-reopen",
                                    OverlayLayerPolicy::new(
                                        OverlayLayerKind::NonModalDismissible,
                                        OverlayPresence::closing(),
                                    ),
                                    OverlayOwnership::Controlled,
                                )
                                .focus_mode(OverlayFocusMode::None)
                                .on_open_change(
                                    move |intent, _, _| {
                                        rebound_events
                                            .borrow_mut()
                                            .push((intent.desired_open(), intent.reason()));
                                    },
                                ),
                                window,
                                cx,
                            )
                            .expect("same closing presence should rebind");
                    },
                )
                .expect("closing layer should request reopen");

            runtime
                .snapshot(window, cx)
                .expect("controlled reopen snapshot should resolve")
                .layers()
                .iter()
                .find(|layer| layer.id().as_str() == "controlled-closing-reopen")
                .map(|layer| (layer.phase(), layer.pending_open(), layer.pending_intent()))
                .expect("controlled closing layer should remain registered")
        })
        .expect("controlled reopen window should remain open");

    assert_eq!(
        events.borrow().as_slice(),
        [(true, DismissReason::Programmatic)]
    );
    assert_eq!(
        projection,
        (
            OverlayLayerPhase::Closing,
            Some(true),
            Some(DismissReason::Programmatic),
        ),
    );
}

#[open_gpui::test]
fn overlay_surface_projects_modal_authority_into_final_accessibility_tree(
    cx: &mut open_gpui::TestAppContext,
) {
    let window = cx.add_window(A11yOverlaySurfaceProbe::new);
    let any_window = window.clone().into();

    assert!(cx.activate_accessibility(any_window));
    let initial = cx
        .latest_accessibility_tree_update(any_window)
        .expect("initial accessibility tree should publish");
    let underlay_id = a11y_node_id_with_label(&initial, "Runtime underlay");
    a11y_node_id_with_label(&initial, "Runtime unrelated overlay");
    assert!(
        !initial
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Runtime modal"))
    );

    window
        .update(cx, |probe, window, cx| {
            probe.open_modal(window, cx);
            cx.notify();
        })
        .expect("overlay accessibility window should remain open");
    cx.run_until_parked();

    let modal = cx
        .latest_accessibility_tree_update(any_window)
        .expect("modal accessibility tree should publish");
    a11y_node_id_with_label(&modal, "Runtime modal");
    a11y_node_id_with_label(&modal, "Runtime deferred modal descendant");
    for excluded in ["Runtime underlay", "Runtime unrelated overlay"] {
        assert!(
            !modal
                .nodes
                .iter()
                .any(|(_, node)| node.label() == Some(excluded)),
            "{excluded:?} must not remain in the modal accessibility tree"
        );
    }

    let modal_history_len = cx.accessibility_tree_update_history(any_window).len();
    assert!(cx.dispatch_accessibility_action(
        any_window,
        accesskit::ActionRequest {
            action: AccessibleAction::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node: underlay_id,
            data: None,
        },
    ));
    assert_eq!(
        cx.read(|cx| window.read(cx).unwrap().underlay_activations),
        0
    );
    assert_eq!(
        cx.accessibility_tree_update_history(any_window).len(),
        modal_history_len,
        "rejected underlay action must not schedule a new frame"
    );

    window
        .update(cx, |probe, window, cx| {
            probe.begin_modal_close(window, cx);
            cx.notify();
        })
        .expect("overlay accessibility window should remain open");
    cx.run_until_parked();

    let closing = cx
        .latest_accessibility_tree_update(any_window)
        .expect("closing accessibility tree should publish");
    assert_eq!(
        a11y_node_id_with_label(&closing, "Runtime underlay"),
        underlay_id
    );
    a11y_node_id_with_label(&closing, "Runtime unrelated overlay");
    for excluded in ["Runtime modal", "Runtime deferred modal descendant"] {
        assert!(
            !closing
                .nodes
                .iter()
                .any(|(_, node)| node.label() == Some(excluded)),
            "closing surface {excluded:?} must not own accessibility"
        );
    }
}

#[open_gpui::test]
fn accessibility_tree_scope_tracks_modal_lifecycle(cx: &mut open_gpui::TestAppContext) {
    let window = cx.add_window(SurfaceProjectionProbe::new);
    window
        .update(cx, |probe, window, cx| {
            let runtime = &probe.surface_runtime;
            let underlay = runtime
                .register_layer(layer_registration("a11y-underlay"), window, cx)
                .expect("underlay should register");
            assert_eq!(
                a11y_disposition(runtime, &underlay, cx),
                AccessibilityTreeScope::Unrestricted,
            );
            let hidden = runtime
                .register_layer(
                    OverlayLayerRegistration::new(
                        "a11y-hidden",
                        OverlayLayerPolicy::new(
                            OverlayLayerKind::NonModalDismissible,
                            OverlayPresence::hidden(),
                        ),
                        OverlayOwnership::Controlled,
                    )
                    .focus_mode(OverlayFocusMode::None),
                    window,
                    cx,
                )
                .expect("hidden surface should register");
            assert_eq!(
                a11y_disposition(runtime, &hidden, cx),
                AccessibilityTreeScope::Excluded,
            );

            let lower_modal = runtime
                .register_layer(
                    modal_registration("a11y-lower-modal", OverlayPresence::open()),
                    window,
                    cx,
                )
                .expect("lower modal should register");
            assert_eq!(
                a11y_disposition(runtime, &lower_modal, cx),
                AccessibilityTreeScope::ModalRoot,
            );
            assert_eq!(
                a11y_disposition(runtime, &underlay, cx),
                AccessibilityTreeScope::Excluded,
            );

            let top_modal = runtime
                .register_layer(
                    modal_registration("a11y-top-modal", OverlayPresence::open()),
                    window,
                    cx,
                )
                .expect("top modal should register");
            assert_eq!(
                a11y_disposition(runtime, &top_modal, cx),
                AccessibilityTreeScope::ModalRoot,
            );
            assert_eq!(
                a11y_disposition(runtime, &lower_modal, cx),
                AccessibilityTreeScope::Excluded,
            );

            runtime
                .request_open_change(&top_modal, false, DismissReason::Programmatic, window, cx)
                .expect("controlled top modal should enter close-requested");
            assert_eq!(
                a11y_disposition(runtime, &top_modal, cx),
                AccessibilityTreeScope::ModalRoot,
            );

            runtime
                .rebind_layer(
                    &top_modal,
                    modal_registration("a11y-top-modal", OverlayPresence::closing()),
                    window,
                    cx,
                )
                .expect("top modal owner should commit closing presence");
            assert_eq!(
                a11y_disposition(runtime, &top_modal, cx),
                AccessibilityTreeScope::Excluded,
            );
            assert_eq!(
                a11y_disposition(runtime, &lower_modal, cx),
                AccessibilityTreeScope::ModalRoot,
            );
            let snapshot = runtime
                .snapshot(window, cx)
                .expect("modal lifecycle snapshot should remain readable");
            let closing_top = snapshot
                .layers()
                .iter()
                .find(|layer| layer.id().as_str() == "a11y-top-modal")
                .expect("closing top modal should remain projected");
            assert_eq!(closing_top.phase(), OverlayLayerPhase::Closing);
            assert!(closing_top.modal_pointer_barrier());

            runtime
                .rebind_layer(
                    &top_modal,
                    modal_registration("a11y-top-modal", OverlayPresence::open()),
                    window,
                    cx,
                )
                .expect("same top modal lease should reopen");
            assert_eq!(
                a11y_disposition(runtime, &top_modal, cx),
                AccessibilityTreeScope::ModalRoot,
            );
            assert_eq!(
                a11y_disposition(runtime, &lower_modal, cx),
                AccessibilityTreeScope::Excluded,
            );
            let reopened_snapshot = runtime
                .snapshot(window, cx)
                .expect("reopened modal snapshot should remain readable");
            assert_eq!(
                reopened_snapshot
                    .layers()
                    .iter()
                    .filter(|layer| layer.phase() != OverlayLayerPhase::Hidden)
                    .last()
                    .expect("reopened modal should remain in the active stack")
                    .id()
                    .as_str(),
                "a11y-top-modal",
            );
        })
        .expect("modal lifecycle window should remain open");
}

#[open_gpui::test]
fn accessibility_tree_scope_tracks_nested_modal_descendants(cx: &mut open_gpui::TestAppContext) {
    let window = cx.add_window(SurfaceProjectionProbe::new);
    window
        .update(cx, |probe, window, cx| {
            let runtime = &probe.surface_runtime;
            let outer_modal = runtime
                .register_layer(
                    modal_registration("a11y-outer-modal", OverlayPresence::open()),
                    window,
                    cx,
                )
                .expect("outer modal should register");
            let nested_modal = runtime
                .register_layer(
                    modal_registration("a11y-nested-modal", OverlayPresence::open())
                        .parent("a11y-outer-modal"),
                    window,
                    cx,
                )
                .expect("nested modal should register");
            let descendant = runtime
                .register_layer(
                    child_registration("a11y-modal-descendant", "a11y-nested-modal"),
                    window,
                    cx,
                )
                .expect("modal descendant should register");
            let unrelated = runtime
                .register_layer(layer_registration("a11y-unrelated"), window, cx)
                .expect("unrelated overlay should register");

            assert_eq!(
                a11y_disposition(runtime, &outer_modal, cx),
                AccessibilityTreeScope::Excluded,
            );
            assert_eq!(
                a11y_disposition(runtime, &nested_modal, cx),
                AccessibilityTreeScope::ModalRoot,
            );
            assert_eq!(
                a11y_disposition(runtime, &descendant, cx),
                AccessibilityTreeScope::ModalDescendant,
            );
            assert_eq!(
                a11y_disposition(runtime, &unrelated, cx),
                AccessibilityTreeScope::Excluded,
            );
        })
        .expect("nested modal window should remain open");
}

#[open_gpui::test]
fn accessibility_tree_scope_rejects_pending_and_stale_leases(cx: &mut open_gpui::TestAppContext) {
    let window = cx.add_window(SurfaceProjectionProbe::new);
    let (runtime, stale_binding) = window
        .update(cx, |probe, window, cx| {
            let runtime = probe.surface_runtime.clone();
            let stale_binding = runtime
                .register_layer(layer_registration("a11y-aba-surface"), window, cx)
                .expect("initial surface incarnation should register");
            runtime
                .unregister_component_subtree(&stale_binding, window, cx)
                .expect("initial modal incarnation should begin unregistering");
            assert_eq!(
                runtime
                    .component_binding_status(&stale_binding, window, cx)
                    .expect("pending lease should remain window-local"),
                OverlayLayerLeaseStatus::PendingUnregister,
            );
            assert_eq!(
                a11y_disposition(&runtime, &stale_binding, cx),
                AccessibilityTreeScope::Excluded,
            );
            (runtime, stale_binding)
        })
        .expect("ABA modal window should remain open");

    cx.run_until_parked();

    window
        .update(cx, |_, window, cx| {
            assert_eq!(
                runtime
                    .component_binding_status(&stale_binding, window, cx)
                    .expect("released lease should remain window-local"),
                OverlayLayerLeaseStatus::Released,
            );
            let replacement = runtime
                .register_layer(layer_registration("a11y-aba-surface"), window, cx)
                .expect("replacement surface incarnation should register");
            assert_ne!(replacement.lease().token, stale_binding.lease().token);
            assert_eq!(
                a11y_disposition(&runtime, &stale_binding, cx),
                AccessibilityTreeScope::Excluded,
            );
            assert_eq!(
                a11y_disposition(&runtime, &replacement, cx),
                AccessibilityTreeScope::Unrestricted,
            );
        })
        .expect("ABA modal window should remain open");
}

#[open_gpui::test]
fn accessibility_tree_scope_rejects_foreign_window_lease_collisions(
    cx: &mut open_gpui::TestAppContext,
) {
    let first_window = cx.add_window(SurfaceProjectionProbe::new);
    let (first_runtime, first_binding) = first_window
        .update(cx, |probe, window, cx| {
            let binding = probe
                .surface_runtime
                .register_layer(
                    modal_registration("a11y-window-modal", OverlayPresence::open()),
                    window,
                    cx,
                )
                .expect("first-window modal should register");
            (probe.surface_runtime.clone(), binding)
        })
        .expect("first modal window should remain open");

    let second_window = cx.add_window(SurfaceProjectionProbe::new);
    let foreign_binding = second_window
        .update(cx, |probe, window, cx| {
            probe
                .surface_runtime
                .register_layer(
                    modal_registration("a11y-window-modal", OverlayPresence::open()),
                    window,
                    cx,
                )
                .expect("second-window modal should register")
        })
        .expect("second modal window should remain open");

    assert_eq!(first_binding.lease().token, foreign_binding.lease().token);
    assert_ne!(
        first_binding.lease().window_id,
        foreign_binding.lease().window_id,
    );
    first_window
        .update(cx, |_, _, cx| {
            assert_eq!(
                first_runtime
                    .state
                    .read(cx)
                    .accessibility_tree_scope(foreign_binding.lease(), first_runtime.window_id),
                AccessibilityTreeScope::Excluded,
            );
            assert_eq!(
                a11y_disposition(&first_runtime, &first_binding, cx),
                AccessibilityTreeScope::ModalRoot,
            );
        })
        .expect("first modal window should remain open");
}

#[open_gpui::test]
fn foreign_surface_does_not_project_parentage_into_child_window_runtime(
    cx: &mut open_gpui::TestAppContext,
) {
    let first_window = cx.add_window(SurfaceProjectionProbe::new);
    first_window
        .update(cx, |probe, window, cx| {
            probe.mount_local_surface(window, cx);
        })
        .expect("first window should remain open");
    let first_any = first_window.clone().into();
    cx.update_window(first_any, |_, window, cx| window.draw(cx).clear())
        .expect("local surface window should draw");

    let (surface_runtime, surface_binding, local_parent) = first_window
        .update(cx, |probe, window, cx| {
            let child_binding = probe
                .child
                .as_ref()
                .expect("local child should be mounted")
                .read(cx)
                .binding
                .clone()
                .expect("local child should own a binding after draw");
            let snapshot = probe
                .snapshot_runtime
                .snapshot(window, cx)
                .expect("local snapshot should belong to the first window");
            let child = snapshot
                .layers()
                .iter()
                .find(|layer| layer.id() == child_binding.lease().layer_id())
                .expect("local child binding should be present in the snapshot");
            (
                probe.surface_runtime.clone(),
                probe
                    .surface_binding
                    .clone()
                    .expect("local surface should own a binding"),
                child.parent().cloned(),
            )
        })
        .expect("first window should remain open");
    assert_eq!(local_parent, Some(OverlayLayerId::new(PARENT_LAYER)));

    let second_window = cx.add_window(move |window, cx| {
        SurfaceProjectionProbe::foreign(surface_runtime, surface_binding, window, cx)
    });
    second_window
        .update(cx, |probe, _, cx| {
            probe.mount_child(FOREIGN_CHILD_LAYER, cx);
        })
        .expect("second window should remain open");
    let second_any = second_window.clone().into();
    cx.update_window(second_any, |_, window, cx| window.draw(cx).clear())
        .expect("foreign surface window should draw");

    let foreign_parent = second_window
        .update(cx, |probe, window, cx| {
            let child_binding = probe
                .child
                .as_ref()
                .expect("foreign child should be mounted")
                .read(cx)
                .binding
                .clone()
                .expect("foreign child should bind to its own window runtime");
            let snapshot = probe
                .snapshot_runtime
                .snapshot(window, cx)
                .expect("foreign child snapshot should belong to the second window");
            let child = snapshot
                .layers()
                .iter()
                .find(|layer| layer.id() == child_binding.lease().layer_id())
                .expect("foreign child binding should be present in its window snapshot");
            child.parent().cloned()
        })
        .expect("second window should remain open");
    assert_eq!(foreign_parent, None);

    assert!(cx.activate_accessibility(first_any));
    let local_tree = cx
        .latest_accessibility_tree_update(first_any)
        .expect("valid local surface should publish an accessibility tree");
    a11y_node_id_with_label(&local_tree, LOCAL_CHILD_LAYER);

    assert!(cx.activate_accessibility(second_any));
    let foreign_tree = cx
        .latest_accessibility_tree_update(second_any)
        .expect("foreign surface window should publish an accessibility tree");
    assert!(
        !foreign_tree
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some(FOREIGN_CHILD_LAYER)),
        "a foreign surface binding must exclude its accessible subtree"
    );

    let first_has_foreign_child = first_window
        .update(cx, |probe, window, cx| {
            probe
                .snapshot_runtime
                .snapshot(window, cx)
                .expect("first window snapshot should remain readable")
                .layers()
                .iter()
                .any(|layer| layer.id().as_str() == FOREIGN_CHILD_LAYER)
        })
        .expect("first window should remain open");
    assert!(!first_has_foreign_child);
}

#[open_gpui::test]
fn inside_region_does_not_project_parentage_into_nested_children(
    cx: &mut open_gpui::TestAppContext,
) {
    let window = cx.add_window(SurfaceProjectionProbe::new);
    window
        .update(cx, |probe, window, cx| {
            probe.mount_local_inside_region(window, cx);
        })
        .expect("inside-region window should remain open");
    let any_window = window.clone().into();
    cx.update_window(any_window, |_, window, cx| window.draw(cx).clear())
        .expect("inside-region window should draw");

    let nested_parent = window
        .update(cx, |probe, window, cx| {
            let child_binding = probe
                .child
                .as_ref()
                .expect("inside-region child should be mounted")
                .read(cx)
                .binding
                .clone()
                .expect("inside-region child should bind after draw");
            let snapshot = probe
                .snapshot_runtime
                .snapshot(window, cx)
                .expect("inside-region snapshot should belong to its window");
            snapshot
                .layers()
                .iter()
                .find(|layer| layer.id() == child_binding.lease().layer_id())
                .expect("inside-region child should appear in the snapshot")
                .parent()
                .cloned()
        })
        .expect("inside-region window should remain open");

    assert_eq!(nested_parent, None);
}

#[open_gpui::test]
fn inside_region_refresh_evicts_expired_dynamic_ids(cx: &mut open_gpui::TestAppContext) {
    let window = cx.add_window(SurfaceProjectionProbe::new);
    window
        .update(cx, |probe, window, cx| {
            let binding = probe
                .surface_runtime
                .register_layer(layer_registration("dynamic-regions"), window, cx)
                .expect("dynamic region layer should register");
            let bounds = Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(10.0), px(10.0)),
            };
            probe.surface_runtime.state.update(cx, |state, _| {
                for revision in 1..=32 {
                    state
                        .refresh_inside_region(
                            binding.lease(),
                            OverlayInsideRegionId::new(format!("dynamic-{revision}")),
                            bounds,
                            None,
                            revision,
                        )
                        .expect("current layer lease should refresh geometry");
                }
                let entry = &state.entries[binding.lease().layer_id()];
                assert_eq!(entry.inside_regions.len(), 1);

                state
                    .refresh_inside_region(
                        binding.lease(),
                        OverlayInsideRegionId::new("same-frame"),
                        bounds,
                        None,
                        32,
                    )
                    .expect("same-frame geometry should remain valid");
                assert_eq!(
                    state.entries[binding.lease().layer_id()]
                        .inside_regions
                        .len(),
                    2
                );

                state
                    .record_component_bind(binding.lease(), 33)
                    .expect("component bind should keep the current lease alive");
                assert!(
                    state.entries[binding.lease().layer_id()]
                        .inside_regions
                        .is_empty()
                );
            });
        })
        .expect("dynamic region window should remain open");
}

#[open_gpui::test]
fn component_binding_status_tracks_registered_unregistering_and_released(
    cx: &mut open_gpui::TestAppContext,
) {
    let window = cx.add_window(SurfaceProjectionProbe::new);
    let binding = window
        .update(cx, |probe, window, cx| {
            let binding = probe
                .surface_runtime
                .register_layer(layer_registration("component-status"), window, cx)
                .expect("component status layer should register");
            assert_eq!(
                probe
                    .surface_runtime
                    .component_binding_status(&binding, window, cx)
                    .expect("registered binding should belong to its window"),
                OverlayLayerLeaseStatus::Registered {
                    phase: OverlayLayerPhase::Open,
                }
            );
            binding
        })
        .expect("component status window should remain open");

    window
        .update(cx, |probe, window, cx| {
            probe
                .surface_runtime
                .unregister_component_subtree(&binding, window, cx)
                .expect("component status layer should begin unregistering");
            assert_eq!(
                probe
                    .surface_runtime
                    .component_binding_status(&binding, window, cx)
                    .expect("pending binding should remain window-local"),
                OverlayLayerLeaseStatus::PendingUnregister,
            );
        })
        .expect("component status window should remain open");

    cx.run_until_parked();
    window
        .update(cx, |probe, window, cx| {
            assert_eq!(
                probe
                    .surface_runtime
                    .component_binding_status(&binding, window, cx)
                    .expect("released binding should remain window-local"),
                OverlayLayerLeaseStatus::Released,
            );
        })
        .expect("component status window should remain open");
}
