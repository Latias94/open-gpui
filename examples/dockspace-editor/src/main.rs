use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use anyhow::{Context as _, Result};
use open_gpui::{
    App, AppContext as _, Context, Entity, EntityId, IntoElement, PrepaintPublicationId, Render,
    Window, WindowId, WindowOptions, div, prelude::*, rgb,
};

mod recovery;
mod smoke;
use open_gpui_docking::{
    DockPane, DockSurface, GpuiDockspaceConfig,
    dockspace::{
        model::{
            DockAnchor, DockPlacement, DockspaceLayout, DockspaceNode, DockspaceRootLayout,
            DockspaceSurfaceLayout, ItemId, RootId, SurfaceId,
        },
        policy::DockPolicy,
        runtime::DockspaceSession,
    },
};

const EDITOR: SurfaceId = SurfaceId::new(1);
const INSPECTOR: SurfaceId = SurfaceId::new(2);
const FIRST: ItemId = ItemId::new(1);
const SECOND: ItemId = ItemId::new(2);

struct Document {
    edits: usize,
}
struct EditorView {
    document: Rc<RefCell<Document>>,
    draft: usize,
    name: &'static str,
    publication: PrepaintPublicationId,
    frames: Rc<RefCell<BTreeMap<WindowId, ViewFrame>>>,
}

#[derive(Clone, Copy, Debug)]
struct ViewFrame {
    generation: u64,
    entity: EntityId,
    draft: usize,
    document_edits: usize,
}

impl EditorView {
    fn edit(&mut self, cx: &mut Context<Self>) {
        self.draft += 1;
        self.document.borrow_mut().edits += 1;
        cx.notify();
        cx.refresh_windows();
    }
}

impl Render for EditorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frames = self.frames.clone();
        let entity = cx.entity().entity_id();
        let draft = self.draft;
        let document_edits = self.document.borrow().edits;
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(rgb(0xfafafa))
            .text_color(rgb(0x29313d))
            .child(self.name)
            .child(format!(
                "Local draft: {draft} · Shared document edits: {document_edits}"
            ))
            .child(
                div()
                    .id("edit-document")
                    .px_3()
                    .py_2()
                    .bg(rgb(0xe2edfc))
                    .on_click(cx.listener(|view, _, _, cx| view.edit(cx)))
                    .child("Edit this View"),
            )
            .child("Each tab retains its original entity when moved between windows.")
            .record_prepaint_window_transaction(
                self.publication,
                move |fence, window, _| {
                    frames.borrow_mut().insert(
                        window.window_handle().window_id(),
                        ViewFrame {
                            generation: fence.generation(),
                            entity,
                            draft,
                            document_edits,
                        },
                    );
                },
                |_, _, _| {},
            )
    }
}

struct Workspace {
    dock: DockSurface,
    views: [Entity<EditorView>; 2],
    document: Rc<RefCell<Document>>,
}

fn workspace(cx: &mut App) -> Result<Workspace> {
    let document = Rc::new(RefCell::new(Document { edits: 0 }));
    let first = cx.new(|_| EditorView {
        document: document.clone(),
        draft: 0,
        name: "Document / Editing View",
        publication: PrepaintPublicationId::new(),
        frames: Rc::default(),
    });
    let second = cx.new(|_| EditorView {
        document: document.clone(),
        draft: 0,
        name: "Document / Inspector View",
        publication: PrepaintPublicationId::new(),
        frames: Rc::default(),
    });
    let layout = DockspaceLayout::new([(EDITOR, FIRST), (INSPECTOR, SECOND)].map(
        |(surface, item)| {
            DockspaceSurfaceLayout::new(
                surface,
                DockspaceRootLayout::new(
                    RootId::new(surface.get()),
                    DockspaceNode::central_tabs([item]),
                ),
            )
        },
    ))?;
    let views = [first, second];
    let dock = create_dock(layout, &views, cx)?;
    Ok(Workspace {
        dock,
        views,
        document,
    })
}

fn create_dock(
    layout: DockspaceLayout,
    views: &[Entity<EditorView>; 2],
    cx: &mut App,
) -> Result<DockSurface> {
    let mut policy = DockPolicy::default();
    policy.set_allow_native_surfaces(true);
    let dock = DockSurface::new_managed(
        DockspaceSession::from_layout(layout, policy)?,
        [
            DockPane::new(FIRST, "Editing View", views[0].clone()),
            DockPane::new(SECOND, "Inspector View", views[1].clone()),
        ],
        GpuiDockspaceConfig::default(),
        cx,
    )?;
    dock.set_close_handler(
        |dock, plan, cx| {
            if let Err(error) = dock.update(
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
            ) {
                eprintln!("editor demo View close failed: {error:#}");
            }
        },
        cx,
    )?;
    Ok(dock)
}

fn open(cx: &mut App) -> Result<()> {
    let workspace = workspace(cx)?;
    let smoke_native = std::env::args().any(|argument| argument == "--smoke-native");
    let smoke_recovery = std::env::args().any(|argument| argument == "--smoke-recovery");
    if smoke_native || smoke_recovery {
        // Keep the process alive until each bounded smoke observes its final receipt.
        cx.set_quit_mode(open_gpui::QuitMode::Explicit);
    }
    if smoke_native {
        let first_window =
            workspace
                .dock
                .open_native_root(EDITOR, true, WindowOptions::default(), cx)?;
        smoke::run(workspace, first_window, cx)?;
        return Ok(());
    }
    let recovery = recovery::open(workspace, cx)?;
    if smoke_recovery {
        recovery::run_smoke(recovery, cx)?;
        return Ok(());
    }
    if std::env::args().any(|argument| argument == "--smoke-move") {
        let (observe, first_window, second_window) = {
            let recovery = recovery.read(cx);
            (
                recovery.workspace.dock.clone(),
                *recovery
                    .roots
                    .get(&EDITOR)
                    .context("editing root did not open")?,
                *recovery
                    .roots
                    .get(&INSPECTOR)
                    .context("inspector root did not open")?,
            )
        };
        first_window.update(cx, |_, window, _| {
            window.on_next_frame(move |_, cx| {
                if let Err(error) = observe.update(
                    |frame| {
                        frame.dock_item_current(
                            FIRST,
                            DockPlacement::Center(DockAnchor::Item(SECOND)),
                        )?;
                        frame.select_item_current(FIRST)?;
                        Ok(())
                    },
                    cx,
                ) {
                    log_error(&error);
                }
                let _ = second_window.update(cx, |_, window, _| window.refresh());
            });
        })?;
    }
    Ok(())
}

fn log_error(error: &dyn std::fmt::Display) {
    eprintln!("dockspace-editor: {error}");
}

fn main() {
    open_gpui_platform::application().run(|cx| {
        if let Err(error) = open(cx) {
            eprintln!("dockspace-editor startup failed: {error:#}");
            std::process::exit(1);
        }
        cx.activate(true);
    });
}
