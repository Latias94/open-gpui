//! Exercises production Fetch routing, streaming and cancellation in a browser.
//!
//! The companion Node harness supplies Fetch streams on the main thread while
//! real workers consume response bodies. A wasm build alone does not run these
//! regressions or qualify the browser synchronization backend.

use anyhow::{Context as _, anyhow};
use futures::{AsyncReadExt as _, FutureExt as _, channel::oneshot};
use open_gpui::Platform as _;
use open_gpui_http_client::{AsyncBody, HttpClient as _, RedirectPolicy, Request};
use open_gpui_web::{FetchCredentials, FetchHttpClient, WebPlatform};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_name = "__openGpuiFetchWaitForBodyState")]
    fn wait_for_body_state(url: &str) -> Result<js_sys::Promise, JsValue>;
}

fn main() {}

#[wasm_bindgen]
pub async fn test_worker_fetch(url: String) -> Result<(), JsValue> {
    let platform = WebPlatform::new(true);
    let client = Arc::new(FetchHttpClient::new(
        platform.foreground_executor().to_send(),
    ));
    let background = platform.background_executor();
    let main_thread = std::thread::current().id();
    let readers = (0..8).map(|_| {
        let client = client.clone();
        let url = url.clone();
        background.spawn(async move {
            anyhow::ensure!(
                std::thread::current().id() != main_thread,
                "response body must be read on a real worker"
            );
            let mut response = client.get(&url, Default::default(), true).await?;
            let mut bytes = Vec::new();
            response.body_mut().read_to_end(&mut bytes).await?;
            anyhow::ensure!(bytes.len() == 32768 * 3, "response did not reach EOF");
            for (index, chunk) in bytes.chunks_exact(3).enumerate() {
                let expected = [
                    (index % 251) as u8,
                    (index % 239) as u8,
                    (index % 227) as u8,
                ];
                anyhow::ensure!(chunk == expected, "response chunks must remain ordered");
            }
            anyhow::Ok(())
        })
    });
    futures::future::try_join_all(readers)
        .await
        .map(|_| ())
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub async fn test_fetch_controls(url: String, allow_multi_threading: bool) -> Result<(), JsValue> {
    run_fetch_controls(url, allow_multi_threading)
        .await
        .map_err(|error| JsValue::from_str(&format!("{error:#}")))
}

#[wasm_bindgen]
pub async fn test_missing_wait_async_fallback() -> Result<(), JsValue> {
    let platform = WebPlatform::new(true);
    if platform.dispatcher_mode()
        != (open_gpui_web::WebDispatcherMode::SingleThreaded {
            reason: open_gpui_web::WebDispatcherSingleThreadedReason::WaitAsyncUnavailable,
        })
    {
        return Err(JsValue::from_str(
            "missing waitAsync must refuse worker startup",
        ));
    }
    let owner = std::thread::current().id();
    let foreground = platform.foreground_executor().to_send();
    let result = platform
        .background_executor()
        .spawn(async move {
            if std::thread::current().id() != owner {
                return Err("fallback background work must stay on the browser main thread");
            }
            foreground
                .spawn(async move {
                    if std::thread::current().id() != owner {
                        return Err("fallback foreground work must keep its captured destination");
                    }
                    Ok(42)
                })
                .await
        })
        .await;
    match result {
        Ok(42) => Ok(()),
        Ok(_) => Err(JsValue::from_str("fallback task returned the wrong result")),
        Err(error) => Err(JsValue::from_str(error)),
    }
}

async fn run_fetch_controls(url: String, allow_multi_threading: bool) -> anyhow::Result<()> {
    let platform = WebPlatform::new(allow_multi_threading);
    anyhow::ensure!(
        platform.dispatcher_mode().supports_background_workers() == allow_multi_threading,
        "the requested worker mode must be active"
    );
    let background = platform.background_executor();
    let main_thread = std::thread::current().id();
    let client = Arc::new(platform.fetch_http_client());
    anyhow::ensure!(client.user_agent().is_none());
    anyhow::ensure!(client.proxy().is_none());
    anyhow::ensure!(FetchCredentials::default() == FetchCredentials::SameOrigin);
    anyhow::ensure!(
        platform
            .fetch_http_client_with_user_agent("invalid\nagent")
            .is_err()
    );
    anyhow::ensure!(
        FetchHttpClient::with_user_agent(
            platform.foreground_executor().to_send(),
            "invalid\nagent"
        )
        .is_err()
    );

    let omit = Arc::new(
        FetchHttpClient::with_user_agent(platform.foreground_executor().to_send(), "fetch-test")?
            .with_credentials(FetchCredentials::Omit),
    );
    let include = Arc::new(
        platform
            .fetch_http_client_with_user_agent("fetch-test")?
            .with_credentials(FetchCredentials::Include),
    );
    anyhow::ensure!(omit.user_agent().unwrap().to_str()? == "fetch-test");
    anyhow::ensure!(include.user_agent().unwrap().to_str()? == "fetch-test");

    let request_client = client.clone();
    let request_url = url.clone();
    background
        .spawn(async move {
            anyhow::ensure!(
                (std::thread::current().id() != main_thread) == allow_multi_threading,
                "requests must execute on the requested consumer thread"
            );
            for (case, client, redirects) in [
                (
                    "options-default",
                    request_client.clone(),
                    RedirectPolicy::FollowAll,
                ),
                ("options-omit", omit, RedirectPolicy::NoFollow),
                ("options-include", include, RedirectPolicy::FollowLimit(2)),
            ] {
                let request = Request::builder()
                    .method("POST")
                    .uri(format!("{request_url}/{case}"))
                    .header("content-type", "application/octet-stream")
                    .header("x-fetch-test", "request")
                    .extension(redirects)
                    .body(AsyncBody::from_reader(futures::io::Cursor::new(
                        b"request body".to_vec(),
                    )))?;
                let mut response = client.send(request).await?;
                anyhow::ensure!(response.status().as_u16() == 201);
                anyhow::ensure!(response.headers()["x-fetch-test"] == "response");
                let mut bytes = Vec::new();
                response.body_mut().read_to_end(&mut bytes).await?;
                anyhow::ensure!(bytes == b"response body");
            }

            let mut response = request_client
                .get(&format!("{request_url}/empty"), AsyncBody::empty(), true)
                .await?;
            anyhow::ensure!(response.status().as_u16() == 204);
            let mut bytes = Vec::new();
            response.body_mut().read_to_end(&mut bytes).await?;
            anyhow::ensure!(bytes.is_empty(), "a null response body must reach EOF");

            for (case, expected) in [
                ("read-error", "fixture stream failure"),
                ("non-byte", "response stream yielded a non-byte chunk"),
            ] {
                let mut response = request_client
                    .get(&format!("{request_url}/{case}"), AsyncBody::empty(), true)
                    .await?;
                let mut bytes = Vec::new();
                let error = response
                    .body_mut()
                    .read_to_end(&mut bytes)
                    .await
                    .expect_err("stream failures must reach the body reader");
                anyhow::ensure!(error.kind() == std::io::ErrorKind::Other);
                anyhow::ensure!(error.to_string().contains(expected), "{error}");
                if case == "read-error" {
                    anyhow::ensure!(bytes == b"prefix", "streamed bytes must precede the error");
                }
            }

            for (case, expected) in [
                ("rejected", "fetch failed"),
                ("thrown", "fetch threw an error"),
                ("wrong-response", "fetch result is not a Response"),
            ] {
                let result = request_client
                    .get(&format!("{request_url}/{case}"), AsyncBody::empty(), true)
                    .await;
                let Err(error) = result else {
                    anyhow::bail!("{case} must fail before returning a response");
                };
                anyhow::ensure!(error.to_string().contains(expected), "{error}");
            }

            let request = Request::builder()
                .uri(format!("{request_url}/invalid-header"))
                .header("x-invalid-value", [0x80_u8].as_slice())
                .body(AsyncBody::empty())?;
            let Err(error) = request_client.send(request).await else {
                anyhow::bail!("non-ASCII headers must fail before calling Fetch");
            };
            anyhow::ensure!(error.to_string().contains("non-ASCII header value"));
            anyhow::Ok(())
        })
        .await?;

    for case in ["drop-read", "drop-backpressure"] {
        let request_url = format!("{url}/{case}");
        let worker_url = request_url.clone();
        let client = client.clone();
        let (ready, body_ready) = oneshot::channel();
        let (drop_body, drop_requested) = oneshot::channel();
        let reader = background.spawn(async move {
            let response = client.get(&worker_url, AsyncBody::empty(), true).await?;
            ready
                .send(())
                .map_err(|_| anyhow!("body readiness receiver was dropped"))?;
            drop_requested
                .await
                .context("body drop was not requested")?;
            drop(response);
            anyhow::Ok(())
        });
        body_ready
            .await
            .context("response headers were not delivered")?;
        wait_for_fixture_state(&request_url).await?;
        drop_body
            .send(())
            .map_err(|_| anyhow!("body drop receiver was dropped"))?;
        reader.await?;
    }

    let request_url = format!("{url}/drop-request");
    let worker_url = request_url.clone();
    let (cancel, canceled) = oneshot::channel::<()>();
    let request = background.spawn(async move {
        let response = client.get(&worker_url, AsyncBody::empty(), true).fuse();
        let canceled = canceled.fuse();
        futures::pin_mut!(response, canceled);
        futures::select_biased! {
            _ = canceled => anyhow::Ok(()),
            _ = response => Err(anyhow!("the fixture must hold the Fetch response until cancellation")),
        }
    });
    wait_for_fixture_state(&request_url).await?;
    cancel
        .send(())
        .map_err(|_| anyhow!("request cancellation receiver was dropped"))?;
    request.await
}

async fn wait_for_fixture_state(url: &str) -> anyhow::Result<()> {
    let promise = wait_for_body_state(url)
        .map_err(|error| anyhow!("failed to wait for browser fixture state: {error:?}"))?;
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| anyhow!("browser fixture did not reach the requested state: {error:?}"))?;
    Ok(())
}
