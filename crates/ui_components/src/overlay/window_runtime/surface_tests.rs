use super::*;

use open_gpui::{AnyView, AppContext as _, Context, ParentElement, Render, Styled, div};

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

        div().size_full()
    }
}

struct SurfaceProjectionProbe {
    surface_runtime: WindowOverlayRuntime,
    snapshot_runtime: WindowOverlayRuntime,
    surface_binding: Option<OverlayLayerBinding>,
    child: Option<Entity<NestedLayerProbe>>,
}

impl SurfaceProjectionProbe {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let runtime = WindowOverlayRuntime::for_window(window, cx);
        Self {
            surface_runtime: runtime.clone(),
            snapshot_runtime: runtime,
            surface_binding: None,
            child: None,
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
            root = root.child(self.surface_runtime.surface(
                binding,
                OverlayInsideRegionId::new("surface-parent-region"),
                "surface-parent-wrapper",
                AnyView::from(child.clone()),
            ));
        }
        root
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
