#![cfg(not(target_family = "wasm"))]

use futures::{AsyncRead, AsyncReadExt, future::BoxFuture, io::Cursor};
use open_gpui::BackgroundExecutor;
use open_gpui_github_releases::download_server_raw_binary;
use open_gpui_http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue};
use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

struct ReadFailure;

impl AsyncRead for ReadFailure {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "interrupted response",
        )))
    }
}

struct InterruptedClient;

impl HttpClient for InterruptedClient {
    fn send(
        &self,
        _request: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        Box::pin(async {
            let body = Cursor::new(vec![0x5a; 64 * 1024]).chain(ReadFailure);
            Ok(Response::new(AsyncBody::from_reader(body)))
        })
    }

    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }
}

#[open_gpui::test]
async fn interrupted_raw_download_releases_file_before_staging_cleanup(
    executor: BackgroundExecutor,
) {
    executor.allow_parking();
    let temp_dir = tempfile::tempdir().unwrap();
    let destination = temp_dir.path().join("v_1");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("agent"), b"previous binary").unwrap();

    let error = download_server_raw_binary(
        &executor,
        Arc::new(InterruptedClient),
        "https://example.invalid/agent",
        None,
        &destination,
        "agent",
    )
    .await
    .unwrap_err();

    assert!(
        error
            .chain()
            .any(|cause| cause.to_string() == "interrupted response")
    );
    assert_eq!(
        std::fs::read(destination.join("agent")).unwrap(),
        b"previous binary"
    );
    assert_eq!(std::fs::read_dir(temp_dir.path()).unwrap().count(), 1);
}
