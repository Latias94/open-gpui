use open_gpui::{
    App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _, Window,
    WindowOptions, canvas, div, fill, px, rgb,
};
use open_gpui_docking::{
    DockPane, DockSurface, GpuiDockspaceConfig,
    dockspace::{
        model::{
            DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
            RootId, SurfaceId,
        },
        policy::DockPolicy,
        runtime::DockspaceSession,
    },
};
use open_gpui_ui_components::button::{Button, ButtonVariant};

struct PreviewPanel {
    extent: taffy::Size<f32>,
    format: wgpu::TextureFormat,
    highlighted: bool,
}

impl Render for PreviewPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let color = rgb(if self.highlighted { 0x2563eb } else { 0x334155 });
        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_4()
            .child(format!("Preview format: {:?}", self.format))
            .child(
                Button::new("toggle-highlight", "Toggle highlight")
                    .variant(ButtonVariant::Secondary)
                    .selected(self.highlighted)
                    .on_activate(cx.processor(|panel, _, _, cx| {
                        panel.highlighted = !panel.highlighted;
                        cx.notify();
                    })),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| window.paint_quad(fill(bounds, color)),
                )
                .w(px(self.extent.width))
                .h(px(self.extent.height)),
            )
    }
}

fn main() {
    open_gpui_platform::application().run(|cx: &mut App| {
        let surface_id = SurfaceId::new(1);
        let item = ItemId::new(1);
        let view = cx.new(|_| PreviewPanel {
            extent: taffy::Size {
                width: 320.0,
                height: 180.0,
            },
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            highlighted: false,
        });
        let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
            surface_id,
            DockspaceRootLayout::new(RootId::new(1), DockspaceNode::central_tabs([item])),
        )])
        .expect("preview dock layout should validate");
        let surface = DockSurface::new_managed(
            DockspaceSession::from_layout(layout, DockPolicy::default())
                .expect("the preview owns one shared docking session"),
            [DockPane::new(item, "Texture preview", view)],
            GpuiDockspaceConfig::default(),
            cx,
        )
        .expect("retained preview content should mount");
        surface
            .set_close_handler(
                |surface, plan, cx| {
                    surface
                        .update(
                            |frame| {
                                let items = frame
                                    .pending_close_decisions(plan.request())
                                    .collect::<Vec<_>>();
                                for item in items {
                                    frame.resolve_close(
                                        plan.request(),
                                        item.token(),
                                        open_gpui_docking::dockspace::close::CloseDecision::Allow,
                                    )?;
                                }
                                Ok(())
                            },
                            cx,
                        )
                        .expect("the application's preview close decision should commit");
                },
                cx,
            )
            .expect("the application owns preview close decisions");
        surface
            .open_native_root(surface_id, true, WindowOptions::default(), cx)
            .expect("the preview native root should open");
        cx.activate(true);
    });
}
