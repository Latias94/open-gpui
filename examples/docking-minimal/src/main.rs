use anyhow::Result;
use open_gpui::{
    App, Bounds, Context, Entity, Global, IntoElement, ParentElement, Render, Styled,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use open_gpui_docking::{
    dockspace::close::CloseDecision,
    prelude::{
        DockPane, DockPolicy, DockSurface, DockspaceAxis, DockspaceLayout, DockspaceNode,
        DockspaceRootLayout, DockspaceSession, DockspaceSurfaceLayout, GpuiDockspaceConfig, ItemId,
        RootId, SurfaceId,
    },
};
use open_gpui_platform::application;

const MAIN: SurfaceId = SurfaceId::new(1);
const EXPLORER: ItemId = ItemId::new(1);
const EDITOR: ItemId = ItemId::new(2);
const TERMINAL: ItemId = ItemId::new(3);

struct Workspace {
    dock: DockSurface,
    _panels: [Entity<DemoPanel>; 3],
}

impl Global for Workspace {}

struct DemoPanel {
    title: &'static str,
    subtitle: &'static str,
    accent: u32,
    lines: &'static [&'static str],
}

impl DemoPanel {
    fn new(
        title: &'static str,
        subtitle: &'static str,
        accent: u32,
        lines: &'static [&'static str],
    ) -> Self {
        Self {
            title,
            subtitle,
            accent,
            lines,
        }
    }
}

impl Render for DemoPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_4()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x111827))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(4.0)).h(px(30.0)).bg(rgb(self.accent)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_lg().child(self.title))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x5f6b7a))
                                    .child(self.subtitle),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.lines.iter().map(|line| {
                        div()
                            .px_2()
                            .py_1()
                            .bg(rgb(0xf4f6f8))
                            .text_color(rgb(0x253041))
                            .child(*line)
                    })),
            )
    }
}

fn explorer_panel(cx: &mut App) -> Entity<DemoPanel> {
    cx.new(|_| {
        DemoPanel::new(
            "Explorer",
            "A retained application Entity registered with DockPane",
            0x2563eb,
            &[
                "crates/gpui_docking",
                "examples/docking-minimal",
                "README.md",
            ],
        )
    })
}

fn editor_panel(cx: &mut App) -> Entity<DemoPanel> {
    cx.new(|_| {
        DemoPanel::new(
            "Editor",
            "DockspaceSession owns layout and selection",
            0x7c3aed,
            &[
                "DockSurface binds the shared session to GPUI",
                "Weighted splits define relative layout shares",
                "Application Entities stay outside serialized layouts",
            ],
        )
    })
}

fn terminal_panel(cx: &mut App) -> Entity<DemoPanel> {
    cx.new(|_| {
        DemoPanel::new(
            "Terminal",
            "Single-window docking keeps platform viewport support optional",
            0x0f766e,
            &[
                "Contained floating is enabled",
                "This window is an ordinary managed native root",
                "Root shutdown does not request application exit",
            ],
        )
    })
}

fn build_workspace(cx: &mut App) -> Result<Workspace> {
    let editor_and_terminal = DockspaceNode::split(
        DockspaceAxis::Vertical,
        [
            (DockspaceNode::central_tabs([EDITOR]), 0.70),
            (DockspaceNode::tabs([TERMINAL]), 0.30),
        ],
    )?;
    let content = DockspaceNode::split(
        DockspaceAxis::Horizontal,
        [
            (DockspaceNode::tabs([EXPLORER]), 0.24),
            (editor_and_terminal, 0.76),
        ],
    )?;
    let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
        MAIN,
        DockspaceRootLayout::new(RootId::new(1), content),
    )])?;
    let mut policy = DockPolicy::default();
    policy.set_allow_contained_floating(true);
    let panels = [explorer_panel(cx), editor_panel(cx), terminal_panel(cx)];
    let dock = DockSurface::new_managed(
        DockspaceSession::from_layout(layout, policy)?,
        [
            DockPane::new(EXPLORER, "Explorer", panels[0].clone()),
            DockPane::new(EDITOR, "Editor", panels[1].clone()),
            DockPane::new(TERMINAL, "Terminal", panels[2].clone()),
        ],
        GpuiDockspaceConfig::default(),
        cx,
    )?;
    dock.set_close_handler(
        |dock, plan, cx| {
            if let Err(error) = dock.update(
                |frame| {
                    let pending = frame
                        .pending_close_decisions(plan.request())
                        .collect::<Vec<_>>();
                    for item in pending {
                        frame.resolve_close(plan.request(), item.token(), CloseDecision::Allow)?;
                    }
                    Ok(())
                },
                cx,
            ) {
                eprintln!("docking-minimal demo pane close failed: {error:#}");
            }
        },
        cx,
    )?;
    Ok(Workspace {
        dock,
        _panels: panels,
    })
}

fn open(cx: &mut App) -> Result<()> {
    let workspace = build_workspace(cx)?;
    let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
    workspace.dock.open_native_root(
        MAIN,
        true,
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Open GPUI Docking Minimal".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            ..Default::default()
        },
        cx,
    )?;
    cx.set_global(workspace);
    cx.activate(true);
    Ok(())
}

fn main() {
    application().run(|cx| {
        if let Err(error) = open(cx) {
            eprintln!("docking-minimal startup failed: {error:#}");
            std::process::exit(1);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{QuitMode, TestAppContext};

    #[open_gpui::test]
    fn primary_close_converges_without_app_quit(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_quit_mode(QuitMode::Explicit));
        cx.defer_next_window_frame_requests();
        let (workspace, anchor) = cx.update(|cx| {
            let workspace = build_workspace(cx).expect("minimal workspace should validate");
            let anchor = workspace
                .dock
                .open_native_root(MAIN, true, WindowOptions::default(), cx)
                .expect("minimal managed native root should open");
            (workspace, anchor)
        });
        cx.run_until_parked();

        let close = cx.simulate_window_close_request(anchor);
        assert!(!close.native_close_allowed());
        cx.run_until_parked();
        cx.update(|cx| workspace.dock.pump_native(cx));
        cx.run_until_parked();

        assert!(!cx.windows().contains(&anchor));
        assert!(
            !cx.did_quit(),
            "DockSurface teardown must not call App::quit"
        );
        assert!(
            workspace.dock.native_is_quiescent(),
            "root shutdown must retire native bindings, outputs and callbacks"
        );
        assert!(
            workspace.dock.error().is_none(),
            "{:?}",
            workspace.dock.error()
        );
    }
}
