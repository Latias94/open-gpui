use open_gpui::{QuitMode, TestAppContext};

use super::*;

fn settle(cx: &mut TestAppContext, shared: &Workspace, isolated: &Workspace) {
    cx.run_until_parked();
    cx.update(|cx| {
        shared.dock.pump_native(cx);
        isolated.dock.pump_native(cx);
    });
    cx.run_until_parked();
    for workspace in [shared, isolated] {
        assert!(
            workspace.dock.error().is_none(),
            "{:?}",
            workspace.dock.error()
        );
        assert!(!workspace.dock.is_update_pending());
    }
}

#[open_gpui::test]
fn shared_preview_moves_and_anchor_close_preserves_isolated_workspace(cx: &mut TestAppContext) {
    cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        open(cx).expect("the complete example should open its native roots");
    });
    let (shared, isolated) =
        cx.read_global::<DemoApp, _>(|app, _| (app.shared.clone(), app.isolated.clone()));
    settle(cx, &shared, &isolated);

    let primary = shared.roots.borrow()[&MAIN];
    let secondary = shared.roots.borrow()[&SECONDARY];
    let isolated_primary = isolated.roots.borrow()[&MAIN];
    assert_ne!(primary, secondary);
    assert_ne!(primary, isolated_primary);
    assert_ne!(secondary, isolated_primary);
    assert_eq!(shared.roots.borrow().len(), 2);
    assert_eq!(isolated.roots.borrow().len(), 1);
    assert_eq!(cx.windows().len(), 3);

    let preview = shared.item("preview").unwrap();
    let preview_view = cx.update(|cx| {
        shared
            .panels
            .iter()
            .find(|panel| panel.read(cx).title == "Preview")
            .expect("the application retains Preview's entity")
            .clone()
    });
    let entity = preview_view.entity_id();
    cx.update(|cx| {
        preview_view.update(cx, |view, cx| {
            view.edits = 7;
            cx.notify();
        });
    });
    settle(cx, &shared, &isolated);
    let assert_preview_retained = |cx: &mut TestAppContext| {
        assert_eq!(shared.item("preview").unwrap(), preview);
        cx.update(|cx| {
            let retained = shared
                .panels
                .iter()
                .find(|panel| panel.entity_id() == entity)
                .expect("Preview must retain its original application entity");
            assert_eq!(retained.read(cx).edits, 7);
        });
    };
    cx.update(|cx| {
        shared.dock.update(
            |frame| {
                frame.request_close_item_current(preview)?;
                Ok(())
            },
            cx,
        )
    })
    .expect("the tab close request should commit for application dispatch");
    settle(cx, &shared, &isolated);
    assert!(shared.dock.with_view(|view| view.item(preview).is_none()));
    assert_preview_retained(cx);
    for destination in [MAIN, SECONDARY] {
        cx.update(|cx| shared.move_preview(cx))
            .expect("Preview should reopen beside Editor and move to the secondary root");
        settle(cx, &shared, &isolated);
        assert_preview_retained(cx);
        assert_eq!(
            shared
                .dock
                .with_view(|view| view.item(preview).map(|item| item.surface())),
            Some(destination)
        );
        cx.update(|cx| shared.export_snapshot(cx))
            .expect("the shared snapshot should round-trip");
    }
    assert!(!shared.last_snapshot.borrow().as_ref().unwrap().is_empty());

    let close = cx.simulate_window_close_request(secondary);
    assert!(!close.native_close_allowed());
    settle(cx, &shared, &isolated);
    assert!(!cx.windows().contains(&secondary));
    assert_eq!(
        shared
            .dock
            .inspection()
            .unwrap()
            .native
            .unwrap()
            .root_window_count,
        1
    );
    assert!(cx.windows().contains(&primary));
    assert!(cx.windows().contains(&isolated_primary));
    assert!(
        shared
            .dock
            .native_binding_for_window(secondary.window_id())
            .is_none()
    );
    assert!(
        shared
            .dock
            .native_binding_for_window(primary.window_id())
            .is_some()
    );
    assert!(
        isolated
            .dock
            .native_binding_for_window(isolated_primary.window_id())
            .is_some()
    );
    assert!(!shared.dock.native_is_quiescent());
    assert!(!isolated.dock.native_is_quiescent());
    assert!(!cx.did_quit());
    assert_preview_retained(cx);
    assert_eq!(
        shared
            .dock
            .with_view(|view| view.item(preview).map(|item| item.surface())),
        Some(SECONDARY)
    );
    cx.update(|cx| shared.move_preview(cx))
        .expect("Preview should return from the closed secondary root to Editor");
    settle(cx, &shared, &isolated);
    assert_preview_retained(cx);
    assert_eq!(
        shared
            .dock
            .with_view(|view| view.item(preview).map(|item| item.surface())),
        Some(MAIN)
    );

    let secondary = cx
        .update(|cx| shared.open_root(SECONDARY, false, WindowOptions::default(), cx))
        .expect("the retained logical root should open another native window");
    settle(cx, &shared, &isolated);
    assert!(cx.windows().contains(&secondary));
    assert_eq!(cx.windows().len(), 3);

    let close = cx.simulate_window_close_request(primary);
    assert!(!close.native_close_allowed());
    settle(cx, &shared, &isolated);
    assert!(shared.dock.native_is_quiescent());
    assert_eq!(
        shared
            .dock
            .inspection()
            .unwrap()
            .native
            .unwrap()
            .root_window_count,
        0
    );
    assert!(!cx.windows().contains(&primary));
    assert!(!cx.windows().contains(&secondary));
    assert!(cx.windows().contains(&isolated_primary));
    assert!(
        shared
            .dock
            .native_binding_for_window(primary.window_id())
            .is_none()
    );
    assert!(
        shared
            .dock
            .native_binding_for_window(secondary.window_id())
            .is_none()
    );
    assert!(
        isolated
            .dock
            .native_binding_for_window(isolated_primary.window_id())
            .is_some()
    );
    assert!(!isolated.dock.native_is_quiescent());
    assert!(!cx.did_quit());

    cx.update(|cx| {
        isolated.panels[0].update(cx, |view, cx| {
            view.edits += 1;
            cx.notify();
        });
    });
    settle(cx, &shared, &isolated);
    cx.update(|cx| {
        assert_eq!(isolated.panels[0].read(cx).edits, 1);
        assert_eq!(preview_view.read(cx).edits, 7);
        isolated
            .export_snapshot(cx)
            .expect("the surviving workspace should still save");
    });
    assert!(!isolated.last_snapshot.borrow().as_ref().unwrap().is_empty());

    let close = cx.simulate_window_close_request(isolated_primary);
    assert!(!close.native_close_allowed());
    settle(cx, &shared, &isolated);
    assert!(isolated.dock.native_is_quiescent());
    assert_eq!(
        isolated
            .dock
            .inspection()
            .unwrap()
            .native
            .unwrap()
            .root_window_count,
        0
    );
    assert!(cx.windows().is_empty());
    assert!(
        !cx.did_quit(),
        "workspace shutdown must not request application exit"
    );
}
