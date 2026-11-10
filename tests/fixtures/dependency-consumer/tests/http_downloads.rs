use open_gpui_http_client::{AsyncBody, HttpRequestExt, Request, RequestTimeout};
use std::time::Duration;

#[test]
fn request_deadline_and_redirect_extensions_compose() {
    use open_gpui_http_client::RedirectPolicy;

    let request = Request::get("https://example.invalid/release")
        .follow_redirects(RedirectPolicy::FollowLimit(3))
        .timeout(Duration::from_secs(7))
        .body(AsyncBody::empty())
        .unwrap();
    assert_eq!(
        request.extensions().get::<RequestTimeout>(),
        Some(&RequestTimeout(Duration::from_secs(7)))
    );
    assert_eq!(
        request.extensions().get::<RedirectPolicy>(),
        Some(&RedirectPolicy::FollowLimit(3))
    );
}

#[cfg(not(target_family = "wasm"))]
mod native {
    use super::*;
    use open_gpui::{BackgroundExecutor, Task};
    use open_gpui_github_releases::{
        AssetKind, GithubReleaseAsset, build_asset_url, download_server_raw_binary,
        get_release_by_tag_name,
    };
    use open_gpui_http_client::{HttpClient, Response, Url, http::HeaderValue};
    use std::{path::Path, sync::Arc};

    struct ReleaseClient;

    impl HttpClient for ReleaseClient {
        fn user_agent(&self) -> Option<&HeaderValue> {
            None
        }

        fn proxy(&self) -> Option<&Url> {
            None
        }

        fn send(
            &self,
            request: Request<AsyncBody>,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = anyhow::Result<Response<AsyncBody>>> + Send + 'static>,
        > {
            assert_eq!(
                request.uri().to_string(),
                "https://api.github.com/repos/example/tool/releases/tags/v1"
            );
            assert_eq!(
                request.extensions().get::<RequestTimeout>(),
                Some(&RequestTimeout(Duration::from_secs(10)))
            );
            Box::pin(async {
                Ok(Response::new(AsyncBody::from(
                    r#"{"tag_name":"v1","prerelease":false,"assets":[{"name":"tool.zip","browser_download_url":"https://example.invalid/tool.zip","digest":"sha256:ABCDEF"}],"tarball_url":"tar","zipball_url":"zip"}"#,
                )))
            })
        }
    }

    #[test]
    fn public_release_metadata_uses_deadline_and_normalized_digest() {
        let release = smol::block_on(get_release_by_tag_name(
            "example/tool",
            "v1",
            Arc::new(ReleaseClient),
        ))
        .unwrap();
        assert_eq!(release.tag_name, "v1");
        assert_eq!(release.assets[0].digest.as_deref(), Some("ABCDEF"));
        let without_digest: GithubReleaseAsset = serde_json::from_str(
            r#"{"name":"tool.zip","browser_download_url":"https://example.invalid/tool.zip"}"#,
        )
        .unwrap();
        assert!(without_digest.digest.is_none());
    }

    #[test]
    fn imported_reqwest_client_follows_redirects_with_its_original_headers() {
        use open_gpui_reqwest_client::ReqwestClient;
        use smol::io::AsyncReadExt;
        use std::{
            io::{BufRead, BufReader, Write},
            net::TcpListener,
            time::Instant,
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = std::thread::spawn(move || {
            for redirected in [false, true] {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "request did not reach loopback");
                            std::thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => panic!("loopback accept failed: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(&stream);
                let mut headers = String::new();
                loop {
                    let mut line = String::new();
                    assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                    let terminal = line == "\r\n";
                    headers.push_str(&line);
                    if terminal {
                        break;
                    }
                }
                assert!(
                    headers
                        .to_lowercase()
                        .contains("user-agent: independent-client\r\n")
                );
                if redirected {
                    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nready").unwrap();
                } else {
                    stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: /target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                }
            }
        });
        let upstream = reqwest::Client::builder()
            .no_proxy()
            .user_agent("independent-client")
            .redirect_policy(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let client: ReqwestClient = upstream.into();
        let request = Request::get(format!("http://{address}/release"))
            .follow_redirects(open_gpui_http_client::RedirectPolicy::FollowAll)
            .timeout(Duration::from_secs(3))
            .body(AsyncBody::empty())
            .unwrap();
        let mut response = smol::block_on(client.send(request)).unwrap();
        let mut bytes = Vec::new();
        smol::block_on(response.body_mut().read_to_end(&mut bytes)).unwrap();
        assert_eq!(bytes, b"ready");
        server.join().unwrap();
    }

    #[test]
    fn release_downloads_use_public_background_tasks() {
        let _download =
            |executor: &BackgroundExecutor, client: Arc<dyn HttpClient>, destination: &Path| {
                let task: Task<anyhow::Result<()>> = download_server_raw_binary(
                    executor,
                    client,
                    "https://example.invalid/tool",
                    None,
                    destination,
                    "tool",
                );
                task
            };
        assert_eq!(
            build_asset_url("example/tool", "v1", AssetKind::Zip).unwrap(),
            "https://github.com/example/tool/archive/refs/tags/v1.zip"
        );
    }
}
