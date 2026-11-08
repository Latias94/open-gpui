use crate::{App, AppContext as _, AsyncApp, Global, Task};
use anyhow::{Context as _, Result};
use async_tungstenite::tungstenite::Message;
use dioxus_devtools_types::DevserverMsg;
use futures::StreamExt;
use std::net::SocketAddr;
use subsecond::JumpTable;

struct HotReload {
    _task: Task<()>,
}

impl Global for HotReload {}

pub(crate) fn init(cx: &mut App) {
    if cx.has_global::<HotReload>() {
        return;
    }
    let (Some(address), Some(endpoint)) = (
        dioxus_cli_config::devserver_raw_addr(),
        dioxus_cli_config::devserver_ws_endpoint(),
    ) else {
        log::warn!(
            "GPUI hot reload is enabled, but no dx development server was found. \
             Launch the app with `dx serve --hot-patch` to enable hot reloading."
        );
        return;
    };

    let build_id = dioxus_cli_config::build_id();
    let process_id = std::process::id();
    // Resolve the executable anchor on the foreground thread before any patch can load.
    let reference = subsecond::aslr_reference();
    let endpoint =
        format!("{endpoint}?aslr_reference={reference}&build_id={build_id}&pid={process_id}");
    let (sender, receiver) = async_channel::bounded(16);
    let connection = cx.background_spawn(async move {
        if let Err(error) = receive_patches(address, endpoint, build_id, process_id, sender).await {
            log::error!("GPUI hot reload disconnected: {error:#}. Restart dx serve to reconnect.");
        }
    });
    let task = cx.spawn(async move |cx| {
        let _connection = connection;
        apply_patches(cx, receiver).await;
    });
    cx.set_global(HotReload { _task: task });
}

async fn apply_patches(cx: &mut AsyncApp, receiver: async_channel::Receiver<JumpTable>) {
    while let Ok(table) = receiver.recv().await {
        if cx
            .update_when_available(|cx| {
                // Patches are executable code from the developer's trusted dx server. Applying
                // them between foreground updates avoids replacing code during a render.
                match unsafe { subsecond::apply_patch(table) } {
                    Ok(()) => {
                        cx.refresh_windows();
                        log::info!("GPUI hot reload applied");
                    }
                    Err(error) => log::error!("GPUI hot reload failed: {error}"),
                }
            })
            .await
            .is_err()
        {
            break;
        }
    }
}

async fn receive_patches(
    address: SocketAddr,
    endpoint: String,
    build_id: u64,
    process_id: u32,
    sender: async_channel::Sender<JumpTable>,
) -> Result<()> {
    let stream = smol::net::TcpStream::connect(address)
        .await
        .context("connecting to the dx development server")?;
    let (mut websocket, _) = async_tungstenite::client_async(endpoint, stream)
        .await
        .context("opening the dx development websocket")?;
    log::info!("GPUI hot reload connected");

    while let Some(message) = websocket.next().await {
        match message.context("reading the dx development websocket")? {
            Message::Text(text) => {
                let message = serde_json::from_str::<DevserverMsg>(&text)
                    .context("decoding a dx development message")?;
                if let Some(table) = patch_for_process(message, build_id, process_id) {
                    if sender.send(table).await.is_err() {
                        return Ok(());
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    log::info!("GPUI hot reload disconnected");
    Ok(())
}

fn patch_for_process(message: DevserverMsg, build_id: u64, process_id: u32) -> Option<JumpTable> {
    match message {
        DevserverMsg::HotReload(message)
            if message.for_build_id == Some(build_id) && message.for_pid == Some(process_id) =>
        {
            message.jump_table
        }
        DevserverMsg::FullReloadCommand => {
            log::warn!("GPUI hot reload requires a full application restart");
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnyView, Context, Entity, IntoElement, Render, RenderOnce, StyleRefinement, TestAppContext,
        ViewElement, Window, div, prelude::*,
    };
    use dioxus_devtools_types::HotReloadMsg;
    use futures::FutureExt as _;
    use std::{cell::Cell, rc::Rc};

    fn message(build_id: Option<u64>, process_id: Option<u32>) -> DevserverMsg {
        DevserverMsg::HotReload(HotReloadMsg {
            for_build_id: build_id,
            for_pid: process_id,
            jump_table: Some(JumpTable {
                lib: "unused-test-patch".into(),
                map: Default::default(),
                aslr_reference: 0,
                new_base_address: 0,
                ifunc_count: 0,
            }),
            ..Default::default()
        })
    }

    #[test]
    fn receives_only_matching_patches_over_the_websocket() -> Result<()> {
        smol::block_on(async {
            let listener = smol::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let (sender, receiver) = async_channel::bounded(16);
            let server = async {
                let (stream, _) = listener.accept().await?;
                let mut websocket = async_tungstenite::accept_async(stream).await?;
                for message in [
                    DevserverMsg::HotPatchStart,
                    DevserverMsg::HotReload(HotReloadMsg::default()),
                    message(Some(3), Some(2)),
                    message(Some(1), Some(3)),
                    message(None, Some(2)),
                    message(Some(1), None),
                    message(Some(1), Some(2)),
                ] {
                    websocket
                        .send(Message::Text(serde_json::to_string(&message)?.into()))
                        .await?;
                }
                websocket.close(None).await?;
                anyhow::Ok(())
            };
            futures::try_join!(
                server,
                receive_patches(address, format!("ws://{address}/_dioxus"), 1, 2, sender)
            )?;
            assert_eq!(
                receiver.recv().await?.lib,
                std::path::Path::new("unused-test-patch")
            );
            assert!(receiver.recv().await.is_err());
            Ok(())
        })
    }

    #[test]
    fn full_restart_and_non_text_messages_do_not_enqueue_patches() -> Result<()> {
        smol::block_on(async {
            let listener = smol::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let (sender, receiver) = async_channel::bounded(16);
            let server = async {
                let (stream, _) = listener.accept().await?;
                let mut websocket = async_tungstenite::accept_async(stream).await?;
                for message in [
                    Message::Ping(Vec::new().into()),
                    Message::Binary(b"not a development message".to_vec().into()),
                    Message::Text(serde_json::to_string(&DevserverMsg::FullReloadCommand)?.into()),
                    Message::Text(serde_json::to_string(&message(Some(1), Some(2)))?.into()),
                ] {
                    websocket.send(message).await?;
                }
                websocket.close(None).await?;
                anyhow::Ok(())
            };
            futures::try_join!(
                server,
                receive_patches(address, format!("ws://{address}/_dioxus"), 1, 2, sender)
            )?;
            assert_eq!(
                receiver.recv().await?.lib,
                std::path::Path::new("unused-test-patch")
            );
            assert!(receiver.recv().await.is_err());
            Ok(())
        })
    }

    #[test]
    fn websocket_disconnect_closes_the_patch_queue() -> Result<()> {
        smol::block_on(async {
            for graceful in [true, false] {
                let listener = smol::net::TcpListener::bind("127.0.0.1:0").await?;
                let address = listener.local_addr()?;
                let (sender, receiver) = async_channel::bounded(16);
                let server = async {
                    let (stream, _) = listener.accept().await?;
                    let mut websocket = async_tungstenite::accept_async(stream).await?;
                    if graceful {
                        websocket.close(None).await?;
                    }
                    anyhow::Ok(())
                };
                let client = async {
                    let result =
                        receive_patches(address, format!("ws://{address}/_dioxus"), 1, 2, sender)
                            .await;
                    if graceful {
                        result?;
                    } else {
                        let error = result.expect_err("an unannounced disconnect must be reported");
                        assert_eq!(error.to_string(), "reading the dx development websocket");
                    }
                    anyhow::Ok(())
                };
                futures::try_join!(server, client)?;
                assert!(receiver.recv().await.is_err());
            }
            Ok(())
        })
    }

    #[test]
    fn invalid_development_messages_disconnect_without_enqueuing_a_patch() -> Result<()> {
        smol::block_on(async {
            for invalid in ["{", "null", r#"{"not-a-development-message":true}"#] {
                let listener = smol::net::TcpListener::bind("127.0.0.1:0").await?;
                let address = listener.local_addr()?;
                let (sender, receiver) = async_channel::bounded(16);
                let server = async {
                    let (stream, _) = listener.accept().await?;
                    let mut websocket = async_tungstenite::accept_async(stream).await?;
                    websocket.send(Message::Text(invalid.into())).await?;
                    anyhow::Ok(())
                };
                let client = async {
                    let error =
                        receive_patches(address, format!("ws://{address}/_dioxus"), 1, 2, sender)
                            .await
                            .expect_err("invalid development messages must end the connection");
                    assert_eq!(error.to_string(), "decoding a dx development message");
                    anyhow::Ok(())
                };
                futures::try_join!(server, client)?;
                assert!(receiver.recv().await.is_err());
            }
            Ok(())
        })
    }

    #[test]
    fn rejected_websocket_handshake_closes_the_patch_queue() -> Result<()> {
        use futures::io::{AsyncReadExt as _, AsyncWriteExt as _};

        smol::block_on(async {
            let listener = smol::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let (sender, receiver) = async_channel::bounded(16);
            let server = async {
                let (mut stream, _) = listener.accept().await?;
                let mut request = [0; 2048];
                assert!(stream.read(&mut request).await? > 0);
                stream
                    .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
                    .await?;
                anyhow::Ok(())
            };
            let client = async {
                let error =
                    receive_patches(address, format!("ws://{address}/_dioxus"), 1, 2, sender)
                        .await
                        .expect_err("a rejected handshake must end the connection");
                assert_eq!(error.to_string(), "opening the dx development websocket");
                anyhow::Ok(())
            };
            futures::try_join!(server, client)?;
            assert!(receiver.recv().await.is_err());
            Ok(())
        })
    }

    #[test]
    fn retiring_the_patch_receiver_ends_the_transport() -> Result<()> {
        smol::block_on(async {
            let listener = smol::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let (sender, receiver) = async_channel::bounded(16);
            drop(receiver);
            let server = async {
                let (stream, _) = listener.accept().await?;
                let mut websocket = async_tungstenite::accept_async(stream).await?;
                websocket
                    .send(Message::Text(
                        serde_json::to_string(&message(Some(1), Some(2)))?.into(),
                    ))
                    .await?;
                anyhow::Ok(())
            };
            futures::try_join!(
                server,
                receive_patches(address, format!("ws://{address}/_dioxus"), 1, 2, sender)
            )?;
            Ok(())
        })
    }

    #[test]
    fn foreground_patch_waits_for_the_active_borrow_and_retires_with_the_app() -> Result<()> {
        let cx = TestAppContext::single();
        let mut async_cx = cx.to_async();
        let app = cx.app.clone();
        drop(cx);
        let borrowed_app = app.borrow_mut();
        let (sender, receiver) = async_channel::bounded(16);
        sender.try_send(patch_for_process(message(Some(1), Some(2)), 1, 2).unwrap())?;
        let mut patches = Box::pin(apply_patches(&mut async_cx, receiver));
        assert!(patches.as_mut().now_or_never().is_none());
        assert!(!sender.is_closed());
        assert_eq!(sender.len(), 0);
        assert!(patches.as_mut().now_or_never().is_none());

        drop(borrowed_app);
        drop(app);
        assert_eq!(patches.as_mut().now_or_never(), Some(()));
        assert!(sender.is_closed());
        Ok(())
    }

    #[open_gpui::test]
    fn retiring_the_app_owner_cancels_both_patch_tasks(cx: &mut TestAppContext) {
        let (sender, receiver) = async_channel::bounded(16);
        let (connection_lifetime, connection_retired) = async_channel::bounded::<()>(1);
        let connection = cx.background_spawn(async move {
            let _connection_lifetime = connection_lifetime;
            futures::future::pending::<()>().await;
        });
        cx.update(|cx| {
            let task = cx.spawn(async move |cx| {
                let _connection = connection;
                apply_patches(cx, receiver).await;
            });
            cx.set_global(HotReload { _task: task });
        });
        cx.run_until_parked();
        assert!(!sender.is_closed());
        assert!(!connection_retired.is_closed());

        cx.update(|cx| drop(cx.remove_global::<HotReload>()));
        cx.run_until_parked();
        assert!(sender.is_closed());
        assert!(connection_retired.is_closed());
    }

    #[open_gpui::test]
    fn restart_requests_and_failed_patches_preserve_live_state(cx: &mut TestAppContext) {
        let renders = Rc::new(Cell::new(0));
        let component_renders = Rc::new(Cell::new(0));
        let child = cx.new(|_| Child {
            count: 7,
            renders: renders.clone(),
        });
        let window = cx.add_window(|_, _| Root {
            child: child.clone(),
            erased: true,
            component_renders: component_renders.clone(),
        });
        cx.run_until_parked();
        let baseline = cx
            .update_window(window.into(), |_, window, cx| {
                window.draw(cx).clear(cx);
                assert!(!window.refreshing);
                assert!(!window.refresh_pending_for_test());
                (
                    window.rendered_frame_revision(),
                    window.invalidator.update_count(),
                    renders.get(),
                    component_renders.get(),
                )
            })
            .unwrap();
        let assert_unchanged = |cx: &mut TestAppContext| {
            cx.update_window(window.into(), |_, window, _| {
                assert!(!window.refreshing);
                assert!(!window.refresh_pending_for_test());
                assert_eq!(
                    (
                        window.rendered_frame_revision(),
                        window.invalidator.update_count(),
                        renders.get(),
                        component_renders.get(),
                    ),
                    baseline
                );
            })
            .unwrap();
            assert_eq!(child.read_with(cx, |child, _| child.count), 7);
        };
        let (sender, receiver) = async_channel::bounded(16);
        assert!(patch_for_process(DevserverMsg::FullReloadCommand, 1, 2).is_none());
        let mut async_cx = cx.to_async();
        let mut patches = Box::pin(apply_patches(&mut async_cx, receiver));
        assert!(patches.as_mut().now_or_never().is_none());
        assert_unchanged(cx);

        let mut table = patch_for_process(message(Some(1), Some(2)), 1, 2).unwrap();
        table.lib = std::env::current_exe()
            .unwrap()
            .join("invalid-gpui-hot-reload-patch");
        assert!(!table.lib.exists());
        sender.try_send(table).unwrap();
        assert!(patches.as_mut().now_or_never().is_none());
        assert_eq!(sender.len(), 0);
        assert_unchanged(cx);

        sender.close();
        assert_eq!(patches.as_mut().now_or_never(), Some(()));
        cx.run_until_parked();
        assert_unchanged(cx);
    }

    struct Child {
        count: usize,
        renders: Rc<Cell<usize>>,
    }

    impl Render for Child {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            div()
        }
    }

    struct Component(Rc<Cell<usize>>);

    impl RenderOnce for Component {
        fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
            self.0.set(self.0.get() + 1);
            div()
        }
    }

    impl IntoElement for Component {
        type Element = ViewElement<Self>;

        fn into_element(self) -> Self::Element {
            ViewElement::new(self)
        }
    }

    struct Root {
        child: Entity<Child>,
        erased: bool,
        component_renders: Rc<Cell<usize>>,
    }

    impl Render for Root {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let element = div().child(Component(self.component_renders.clone()));
            if self.erased {
                element.child(AnyView::from(self.child.clone()).cached(StyleRefinement::default()))
            } else {
                element.child(self.child.clone().cached(StyleRefinement::default()))
            }
        }
    }

    #[open_gpui::test]
    fn refresh_renders_cached_views_without_recreating_state(cx: &mut TestAppContext) {
        for erased in [false, true] {
            let renders = Rc::new(Cell::new(0));
            let component_renders = Rc::new(Cell::new(0));
            let child = cx.new(|_| Child {
                count: 7,
                renders: renders.clone(),
            });
            let window = cx.add_window(|_, _| Root {
                child: child.clone(),
                erased,
                component_renders: component_renders.clone(),
            });
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .unwrap();
            let initial_renders = renders.get();
            let initial_component_renders = component_renders.get();
            assert!(initial_renders > 0);
            assert!(initial_component_renders > 0);

            cx.update(|cx| cx.refresh_windows());
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .unwrap();
            assert!(renders.get() > initial_renders);
            assert!(component_renders.get() > initial_component_renders);
            assert_eq!(child.read_with(cx, |child, _| child.count), 7);
        }
    }
}
