use open_gpui::{
    App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _, Window,
    WindowOptions, canvas, div, fill, px, rgb,
};
use open_gpui_docking::prelude::{
    DockPanelPlacement, DockSurface, DockSurfacePrimaryWindowOpenOutcome,
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
        let surface = DockSurface::builder("preview")
            .panel_placements([DockPanelPlacement::center("preview").selected()])
            .panel_factory("preview", "Texture preview", |cx| {
                cx.new(|_| PreviewPanel {
                    extent: taffy::Size {
                        width: 320.0,
                        height: 180.0,
                    },
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    highlighted: false,
                })
                .into()
            })
            .build(cx)
            .expect("preview dock layout should validate");
        match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(_) => {}
            outcome => panic!("failed to open preview window: {outcome:?}"),
        }
        cx.activate(true);
    });
}
