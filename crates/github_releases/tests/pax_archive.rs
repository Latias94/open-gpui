#![cfg(not(target_family = "wasm"))]

use async_compression::futures::bufread::GzipEncoder;
use async_tar::{Builder, EntryType, Header};
use futures::{AsyncReadExt, future::BoxFuture, io::Cursor};
use open_gpui::BackgroundExecutor;
use open_gpui_github_releases::{AssetKind, download_server_binary};
use open_gpui_http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue};
use sha2::{Digest, Sha256};
use std::sync::Arc;

struct ArchiveClient(Vec<u8>);

impl HttpClient for ArchiveClient {
    fn send(
        &self,
        request: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        assert_eq!(request.uri(), "https://example.invalid/agent.tar.gz");
        let body = self.0.clone();
        Box::pin(async move { Ok(Response::new(AsyncBody::from(body))) })
    }

    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }
}

#[open_gpui::test]
async fn downloads_tar_gz_with_embedded_newline_pax_value_and_following_path(
    executor: BackgroundExecutor,
) {
    executor.allow_parking();
    let pax = b"24 comment=first\nsecond\n18 path=bin/agent\n";
    let contents = b"agent binary";
    let mut archive = Builder::new(Cursor::new(Vec::new()));
    let mut pax_header = Header::new_ustar();
    pax_header.set_entry_type(EntryType::XHeader);
    pax_header.set_size(pax.len() as u64);
    pax_header.set_mode(0o644);
    archive
        .append_data(&mut pax_header, "pax", Cursor::new(pax))
        .await
        .unwrap();
    let mut file_header = Header::new_ustar();
    file_header.set_size(contents.len() as u64);
    file_header.set_mode(0o755);
    archive
        .append_data(&mut file_header, "fallback-agent", Cursor::new(contents))
        .await
        .unwrap();
    let tar = archive.into_inner().await.unwrap().into_inner();
    let mut compressed = Vec::new();
    GzipEncoder::new(Cursor::new(tar))
        .read_to_end(&mut compressed)
        .await
        .unwrap();
    let digest = format!("{:x}", Sha256::digest(&compressed));
    let temp_dir = tempfile::tempdir().unwrap();
    let destination = temp_dir.path().join("v_1");

    download_server_binary(
        &executor,
        Arc::new(ArchiveClient(compressed)),
        "https://example.invalid/agent.tar.gz",
        Some(&digest),
        &destination,
        AssetKind::TarGz,
    )
    .await
    .unwrap();

    assert_eq!(
        std::fs::read(destination.join("bin/agent")).unwrap(),
        contents
    );
    assert!(!destination.join("fallback-agent").exists());
    assert_eq!(std::fs::read_dir(temp_dir.path()).unwrap().count(), 1);
}
