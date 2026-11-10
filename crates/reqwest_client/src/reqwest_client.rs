use std::error::Error;
use std::sync::{LazyLock, OnceLock};
use std::{borrow::Cow, mem, pin::Pin, task::Poll, time::Duration};

use open_gpui_core_util::defer;

use anyhow::anyhow;
use bytes::{BufMut, Bytes, BytesMut};
use futures::{AsyncRead, FutureExt as _, Stream as _, TryStreamExt as _};
use open_gpui_http_client::{RedirectPolicy, RequestTimeout, Url, http};
use regex::Regex;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    redirect,
};

const DEFAULT_CAPACITY: usize = 4096;
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static REDACT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"key=[^&]+").unwrap());

pub struct ReqwestClient {
    client: reqwest::Client,
    proxy: Option<Url>,
    user_agent: Option<HeaderValue>,
    handle: tokio::runtime::Handle,
}

impl ReqwestClient {
    /// Shared connection-management configuration for every client this type
    /// builds. `read_timeout` sets an idle timeout on each body read (see
    /// [`ReqwestClient::proxy_user_agent_and_read_timeout`]); `None` leaves
    /// reads without a timeout.
    fn builder(read_timeout: Option<Duration>) -> reqwest::ClientBuilder {
        let builder = reqwest::Client::builder()
            .use_rustls_tls()
            .connect_timeout(Duration::from_secs(10))
            // Detect and drop connections that have silently gone bad on a
            // flaky path (NAT timeouts, resets) instead of reusing them. A
            // stale reused HTTP/2 connection is a common source of
            // `BadRecordMac` TLS errors against long-lived endpoints.
            .tcp_keepalive(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(30))
            .http2_keep_alive_interval(Duration::from_secs(15))
            .http2_keep_alive_timeout(Duration::from_secs(10))
            .http2_keep_alive_while_idle(true);
        match read_timeout {
            Some(read_timeout) => builder.read_timeout(read_timeout),
            None => builder,
        }
    }

    pub fn new() -> Self {
        Self::builder(None)
            .build()
            .expect("Failed to initialize HTTP client")
            .into()
    }

    pub fn user_agent(agent: &str) -> anyhow::Result<Self> {
        let mut map = HeaderMap::new();
        map.insert(http::header::USER_AGENT, HeaderValue::from_str(agent)?);
        let client = Self::builder(None).default_headers(map).build()?;
        Ok(client.into())
    }

    pub fn proxy_and_user_agent(proxy: Option<Url>, user_agent: &str) -> anyhow::Result<Self> {
        Self::proxy_user_agent_and_read_timeout(proxy, user_agent, None)
    }

    /// Like [`ReqwestClient::proxy_and_user_agent`], but also applies a
    /// per-read idle timeout. `read_timeout` fires only after that long with no
    /// bytes received on a response body and resets on every chunk, so it
    /// aborts a silently stalled stream without disturbing a healthy one that
    /// merely goes quiet between chunks. Callers streaming long-lived responses
    /// (e.g. LLM completions, which can pause for tens of seconds during
    /// provider-side reasoning while keep-alive bytes still flow) should size
    /// it comfortably above the provider's keep-alive interval.
    ///
    /// Note: on macOS the timeout is measured against a monotonic clock that
    /// pauses during system sleep, so it does not fire from a suspend alone;
    /// callers that need prompt detection of a connection killed while
    /// suspended must re-validate the stream on wake themselves.
    pub fn proxy_user_agent_and_read_timeout(
        proxy: Option<Url>,
        user_agent: &str,
        read_timeout: Option<Duration>,
    ) -> anyhow::Result<Self> {
        let user_agent = HeaderValue::from_str(user_agent)?;

        let mut map = HeaderMap::new();
        map.insert(http::header::USER_AGENT, user_agent.clone());
        let mut client = Self::builder(read_timeout).default_headers(map);
        let client_has_proxy;

        if let Some(proxy) = proxy.as_ref().and_then(|proxy_url| {
            reqwest::Proxy::all(proxy_url.clone())
                .inspect_err(|e| {
                    log::error!(
                        "Failed to parse proxy URL '{}': {}",
                        proxy_url,
                        e.source().unwrap_or(&e as &_)
                    )
                })
                .ok()
        }) {
            // Respect NO_PROXY env var
            client = client.proxy(proxy.no_proxy(reqwest::NoProxy::from_env()));
            client_has_proxy = true;
        } else {
            client_has_proxy = false;
        };

        let client = client
            .use_preconfigured_tls(open_gpui_http_client_tls::tls_config())
            .build()?;
        let mut client: ReqwestClient = client.into();
        client.proxy = client_has_proxy.then_some(proxy).flatten();
        client.user_agent = Some(user_agent);
        Ok(client)
    }
}

pub fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            // Since we now have two executors, let's try to keep our footprint small
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("Failed to initialize HTTP client")
    })
}

impl From<reqwest::Client> for ReqwestClient {
    fn from(client: reqwest::Client) -> Self {
        let handle = tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
            log::debug!("no tokio runtime found, creating one for Reqwest...");
            runtime().handle().clone()
        });
        Self {
            client,
            handle,
            proxy: None,
            user_agent: None,
        }
    }
}

// This struct is essentially a re-implementation of
// https://docs.rs/tokio-util/0.7.12/tokio_util/io/struct.ReaderStream.html
// except outside of Tokio's aegis
struct StreamReader {
    reader: Option<Pin<Box<dyn futures::AsyncRead + Send + Sync>>>,
    buf: BytesMut,
    capacity: usize,
}

impl StreamReader {
    fn new(reader: Pin<Box<dyn futures::AsyncRead + Send + Sync>>) -> Self {
        Self {
            reader: Some(reader),
            buf: BytesMut::new(),
            capacity: DEFAULT_CAPACITY,
        }
    }
}

impl futures::Stream for StreamReader {
    type Item = std::io::Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut();

        let mut reader = match this.reader.take() {
            Some(r) => r,
            None => return Poll::Ready(None),
        };

        if this.buf.capacity() == 0 {
            let capacity = this.capacity;
            this.buf.reserve(capacity);
        }

        match poll_read_buf(&mut reader, cx, &mut this.buf) {
            Poll::Pending => {
                self.reader = Some(reader);

                Poll::Pending
            }
            Poll::Ready(Err(err)) => {
                self.reader = None;

                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(Ok(0)) => {
                self.reader = None;
                Poll::Ready(None)
            }
            Poll::Ready(Ok(_)) => {
                let chunk = this.buf.split();
                self.reader = Some(reader);
                Poll::Ready(Some(Ok(chunk.freeze())))
            }
        }
    }
}

/// Implementation from <https://docs.rs/tokio-util/0.7.12/src/tokio_util/util/poll_buf.rs.html>
/// Specialized for this use case
fn poll_read_buf(
    io: &mut Pin<Box<dyn futures::AsyncRead + Send + Sync>>,
    cx: &mut std::task::Context<'_>,
    buf: &mut BytesMut,
) -> Poll<std::io::Result<usize>> {
    if !buf.has_remaining_mut() {
        return Poll::Ready(Ok(0));
    }

    let n = {
        let dst = buf.chunk_mut();

        // Safety: `chunk_mut()` returns a `&mut UninitSlice`, and `UninitSlice` is a
        // transparent wrapper around `[std::mem::MaybeUninit<u8>]`.
        let dst = unsafe { &mut *(dst as *mut _ as *mut [std::mem::MaybeUninit<u8>]) };
        let mut read_buf = tokio::io::ReadBuf::uninit(dst);
        let unfilled_portion = read_buf.initialize_unfilled();
        // SAFETY: Pin projection
        let io_pin = unsafe { Pin::new_unchecked(io) };
        // `futures::AsyncRead` reports the byte count as the poll's return
        // value; `read_buf.filled()` stays empty because the reader writes
        // through the initialized slice without advancing the `ReadBuf`.
        std::task::ready!(io_pin.poll_read(cx, unfilled_portion)?)
    };

    // Safety: `initialize_unfilled()` zero-initialized the entire spare
    // capacity, so the first `n` bytes are initialized no matter how many the
    // reader actually wrote, and `advance_mut` panics rather than exceeding
    // the capacity if `n` overstates the slice length.
    unsafe {
        buf.advance_mut(n);
    }

    Poll::Ready(Ok(n))
}

fn redact_error(mut error: reqwest::Error) -> reqwest::Error {
    if let Some(url) = error.url_mut()
        && let Some(query) = url.query()
        && let Cow::Owned(redacted) = REDACT_REGEX.replace_all(query, "key=REDACTED")
    {
        url.set_query(Some(redacted.as_str()));
    }
    error
}

impl open_gpui_http_client::HttpClient for ReqwestClient {
    fn proxy(&self) -> Option<&Url> {
        self.proxy.as_ref()
    }

    fn user_agent(&self) -> Option<&HeaderValue> {
        self.user_agent.as_ref()
    }

    fn send(
        &self,
        req: http::Request<open_gpui_http_client::AsyncBody>,
    ) -> futures::future::BoxFuture<
        'static,
        anyhow::Result<open_gpui_http_client::Response<open_gpui_http_client::AsyncBody>>,
    > {
        let (parts, body) = req.into_parts();

        let mut request_builder = self.client.request(parts.method, parts.uri.to_string());
        request_builder = request_builder.headers(parts.headers);
        if let Some(redirect_policy) = parts.extensions.get::<RedirectPolicy>() {
            request_builder = request_builder.redirect_policy(match redirect_policy {
                RedirectPolicy::NoFollow => redirect::Policy::none(),
                RedirectPolicy::FollowLimit(limit) => redirect::Policy::limited(*limit as usize),
                RedirectPolicy::FollowAll => redirect::Policy::limited(100),
            });
        }
        if let Some(timeout) = parts.extensions.get::<RequestTimeout>() {
            request_builder = request_builder.timeout(timeout.0);
        }
        let request = request_builder.body(match body.0 {
            open_gpui_http_client::Inner::Empty => reqwest::Body::default(),
            open_gpui_http_client::Inner::Bytes(cursor) => cursor.into_inner().into(),
            open_gpui_http_client::Inner::AsyncReader(stream) => {
                reqwest::Body::wrap_stream(StreamReader::new(stream))
            }
        });

        let handle = self.handle.clone();
        async move {
            let join_handle = handle.spawn(async { request.send().await });
            let abort_handle = join_handle.abort_handle();
            let _abort_on_drop = defer(move || abort_handle.abort());

            let mut response = join_handle.await?.map_err(redact_error)?;

            let headers = mem::take(response.headers_mut());
            let mut builder = http::Response::builder()
                .status(response.status().as_u16())
                .version(response.version());
            *builder.headers_mut().unwrap() = headers;

            // Keep the original Tokio context when reading on other executors:
            // Reqwest's per-read timeout creates a new timer on each chunk.
            let mut bytes = Box::pin(response.bytes_stream());
            let bytes = futures::stream::poll_fn(move |cx| {
                let _guard = handle.enter();
                bytes.as_mut().poll_next(cx)
            })
            .map_err(futures::io::Error::other)
            .into_async_read();
            let body = open_gpui_http_client::AsyncBody::from_reader(bytes);

            builder.body(body).map_err(|e| anyhow!(e))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};
    use std::net::{TcpListener, TcpStream};
    use std::time::{Duration, Instant};

    use futures::AsyncReadExt as _;
    use open_gpui_http_client::{
        AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request as HttpRequest,
        StatusCode, Url,
    };

    use crate::ReqwestClient;

    fn loopback_client(read_timeout: Option<Duration>) -> ReqwestClient {
        let mut builder = reqwest::Client::builder().no_proxy();
        if let Some(read_timeout) = read_timeout {
            builder = builder.read_timeout(read_timeout);
        }
        builder.build().unwrap().into()
    }

    fn read_request(listener: &TcpListener) -> (TcpStream, Vec<String>) {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("failed to accept a loopback request: {error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            assert_ne!(reader.read_line(&mut line).unwrap(), 0);
            if line == "\r\n" {
                break;
            }
            headers.push(line);
        }
        drop(reader);
        (stream, headers)
    }

    fn assert_connection_closed(mut stream: TcpStream) {
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut buffer = [0; 1];
        match stream.read(&mut buffer) {
            Ok(0) => {}
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
            result => panic!("the client should close the connection: {result:?}"),
        }
    }

    /// Regression test: `StreamReader::poll_next` used to drop the reader it
    /// `take()`s whenever the reader returned `Poll::Pending`, so the next
    /// poll reported end-of-stream and streamed request bodies were silently
    /// truncated. Readers backed by real I/O return `Pending` on their first
    /// read, so their uploads sent zero bytes.
    #[test]
    fn test_streamed_body_survives_pending_reader() {
        let payload: Vec<u8> = (0..30_000usize).map(|byte| (byte % 251) as u8).collect();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let expected_payload = payload.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                assert_ne!(read, 0, "client closed the connection mid-request");
                request.extend_from_slice(&buffer[..read]);
                if let Some(position) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let body_start = position + 4;
                    while request.len() - body_start < expected_payload.len() {
                        let read = stream.read(&mut buffer).unwrap();
                        assert_ne!(read, 0, "client closed the connection mid-body");
                        request.extend_from_slice(&buffer[..read]);
                    }
                    assert_eq!(&request[body_start..], &expected_payload);
                    break;
                }
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
                .unwrap();
        });

        struct PendingFirstReader {
            data: std::io::Cursor<Vec<u8>>,
            ready: bool,
        }

        impl futures::AsyncRead for PendingFirstReader {
            fn poll_read(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buf: &mut [u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                if self.ready {
                    self.ready = false;
                    std::task::Poll::Ready(self.data.read(buf))
                } else {
                    self.ready = true;
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
            }
        }

        let reader = PendingFirstReader {
            data: std::io::Cursor::new(payload.clone()),
            ready: false,
        };

        let client = loopback_client(None);
        let request = HttpRequest::builder()
            .method(Method::PUT)
            .uri(format!("http://{address}/upload"))
            .header("Content-Length", payload.len().to_string())
            .body(AsyncBody::from_reader(reader))
            .unwrap();
        let response = futures::executor::block_on(client.send(request)).unwrap();
        assert!(response.status().is_success());
        server.join().unwrap();
    }

    #[test]
    fn test_request_timeout_applies_while_reading_response_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = read_request(&listener);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 1\r\n\r\n")
                .unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut buffer = [0; 1];
            match stream.read(&mut buffer) {
                Ok(_) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(error) => panic!("failed while waiting for the client to close: {error}"),
            }
        });

        let client = loopback_client(None);
        let request = HttpRequest::get(format!("http://{address}/"))
            .timeout(Duration::from_millis(100))
            .body(AsyncBody::default())
            .unwrap();
        let started_at = Instant::now();
        let mut response = futures::executor::block_on(client.send(request)).unwrap();
        let mut body = Vec::new();
        let result = futures::executor::block_on(response.body_mut().read_to_end(&mut body));
        assert!(result.is_err(), "the response body should time out");
        assert!(
            started_at.elapsed() < Duration::from_millis(500),
            "the request timeout should not wait for the server to close the connection"
        );
        drop(response);
        server.join().unwrap();
    }

    #[test]
    fn test_request_timeout_applies_after_redirect_before_response_headers() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = read_request(&listener);
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nlocation: /target\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                )
                .unwrap();
            drop(stream);
            let (stream, headers) = read_request(&listener);
            assert!(headers[0].starts_with("GET /target "));
            assert_connection_closed(stream);
        });

        let client = loopback_client(Some(Duration::from_secs(5)));
        let request = HttpRequest::get(format!("http://{address}/redirect"))
            .follow_redirects(RedirectPolicy::FollowLimit(3))
            .timeout(Duration::from_millis(200))
            .body(AsyncBody::default())
            .unwrap();
        let error = futures::executor::block_on(client.send(request))
            .err()
            .unwrap();
        assert!(error.downcast_ref::<reqwest::Error>().unwrap().is_timeout());
        server.join().unwrap();
    }

    #[test]
    fn test_imported_client_redirect_policies_preserve_proxy_user_agent_and_read_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let server = std::thread::spawn(move || {
            for follow in [false, false, true, true] {
                let (mut stream, headers) = read_request(&listener);
                assert!(headers[0].starts_with("GET http://127.0.0.1:1/redirect "));
                assert!(
                    headers
                        .iter()
                        .any(|header| header.eq_ignore_ascii_case("user-agent: imported-agent\r\n"))
                );
                let content_length = if follow { 0 } else { 1 };
                let response = format!(
                    "HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:1/target\r\ncontent-length: {content_length}\r\nconnection: close\r\n\r\n"
                );
                stream.write_all(response.as_bytes()).unwrap();
                let stream = if follow {
                    drop(stream);
                    let (mut stream, headers) = read_request(&listener);
                    assert!(headers[0].starts_with("GET http://127.0.0.1:1/target "));
                    assert!(headers.iter().any(|header| {
                        header.eq_ignore_ascii_case("user-agent: imported-agent\r\n")
                    }));
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 1\r\n\r\n")
                        .unwrap();
                    stream
                } else {
                    stream
                };
                assert_connection_closed(stream);
            }
        });

        let client = ReqwestClient::from(
            reqwest::Client::builder()
                .proxy(reqwest::Proxy::all(proxy).unwrap().no_proxy(None))
                .user_agent("imported-agent")
                .read_timeout(Duration::from_millis(100))
                .redirect_policy(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        );

        for (policy, expected_status) in [
            (None, StatusCode::FOUND),
            (Some(RedirectPolicy::NoFollow), StatusCode::FOUND),
            (Some(RedirectPolicy::FollowLimit(3)), StatusCode::OK),
            (Some(RedirectPolicy::FollowAll), StatusCode::OK),
        ] {
            let request = HttpRequest::get("http://127.0.0.1:1/redirect")
                .when_some(policy, |request, policy| request.follow_redirects(policy))
                .timeout(Duration::from_secs(3))
                .body(AsyncBody::default())
                .unwrap();
            let mut response = futures::executor::block_on(client.send(request)).unwrap();
            assert_eq!(response.status(), expected_status);
            let mut body = Vec::new();
            let started_at = Instant::now();
            let error = futures::executor::block_on(response.body_mut().read_to_end(&mut body))
                .unwrap_err();
            assert!(
                error
                    .get_ref()
                    .unwrap()
                    .downcast_ref::<reqwest::Error>()
                    .unwrap()
                    .is_timeout()
            );
            assert!(started_at.elapsed() < Duration::from_secs(1));
            drop(response);
        }
        server.join().unwrap();
    }

    #[test]
    fn test_imported_client_redirect_policy_preserves_proxy_bypass() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, headers) = read_request(&listener);
                assert!(headers[0].starts_with("GET /direct "));
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
                    .unwrap();
            }
        });

        let client = ReqwestClient::from(
            reqwest::Client::builder()
                .proxy(
                    reqwest::Proxy::all("http://127.0.0.1:1")
                        .unwrap()
                        .no_proxy(reqwest::NoProxy::from_string("127.0.0.1")),
                )
                .build()
                .unwrap(),
        );
        for policy in [None, Some(RedirectPolicy::NoFollow)] {
            let request = HttpRequest::get(format!("http://{address}/direct"))
                .when_some(policy, |request, policy| request.follow_redirects(policy))
                .timeout(Duration::from_secs(3))
                .body(AsyncBody::default())
                .unwrap();
            let response = futures::executor::block_on(client.send(request)).unwrap();
            assert!(response.status().is_success());
        }
        server.join().unwrap();
    }

    #[test]
    fn test_read_timeout_resets_between_body_chunks_outside_tokio() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = read_request(&listener);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 6\r\nconnection: close\r\n\r\n")
                .unwrap();
            for byte in b"abcdef" {
                std::thread::sleep(Duration::from_millis(100));
                stream.write_all(&[*byte]).unwrap();
            }
        });

        let client = loopback_client(Some(Duration::from_millis(300)));
        let request = HttpRequest::get(format!("http://{address}/stream"))
            .follow_redirects(RedirectPolicy::FollowAll)
            .timeout(Duration::from_secs(3))
            .body(AsyncBody::default())
            .unwrap();
        let mut response = futures::executor::block_on(client.send(request)).unwrap();
        let mut body = Vec::new();
        futures::executor::block_on(response.body_mut().read_to_end(&mut body)).unwrap();
        assert_eq!(body, b"abcdef");
        server.join().unwrap();
    }

    #[test]
    fn test_no_follow_redirect_does_not_replay_streamed_body() {
        let payload = b"streamed-secret".to_vec();
        let redirect_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let redirect_address = redirect_listener.local_addr().unwrap();
        let target_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let target_address = target_listener.local_addr().unwrap();

        let target_server = std::thread::spawn(move || {
            target_listener.set_nonblocking(true).unwrap();
            let deadline = Instant::now() + Duration::from_millis(500);
            loop {
                match target_listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .write_all(
                                b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                            )
                            .unwrap();
                        return true;
                    }
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        return false;
                    }
                    Err(error) => panic!("target listener failed: {error}"),
                }
            }
        });

        let expected_payload = payload.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = redirect_listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                assert_ne!(read, 0, "client closed the connection mid-request");
                request.extend_from_slice(&buffer[..read]);
                if let Some(position) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let body_start = position + 4;
                    while request.len() - body_start < expected_payload.len() {
                        let read = stream.read(&mut buffer).unwrap();
                        assert_ne!(read, 0, "client closed the connection mid-body");
                        request.extend_from_slice(&buffer[..read]);
                    }
                    assert_eq!(&request[body_start..], &expected_payload);
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 307 Temporary Redirect\r\nlocation: http://{target_address}/target\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        let client = loopback_client(None);
        let request = HttpRequest::builder()
            .method(Method::PUT)
            .uri(format!("http://{redirect_address}/upload"))
            .extension(RedirectPolicy::NoFollow)
            .header("Content-Length", payload.len().to_string())
            .body(AsyncBody::from_reader(futures::io::Cursor::new(payload)))
            .unwrap();
        let response = futures::executor::block_on(client.send(request)).unwrap();
        assert_eq!(
            response.status(),
            open_gpui_http_client::StatusCode::TEMPORARY_REDIRECT
        );
        server.join().unwrap();
        assert!(
            !target_server.join().unwrap(),
            "NoFollow must not connect to the redirect target"
        );
    }

    #[test]
    fn test_proxy_uri() {
        let client = ReqwestClient::new();
        assert_eq!(client.proxy(), None);

        let proxy = Url::parse("http://localhost:10809").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));

        let proxy = Url::parse("https://localhost:10809").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));

        let proxy = Url::parse("socks4://localhost:10808").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));

        let proxy = Url::parse("socks4a://localhost:10808").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));

        let proxy = Url::parse("socks5://localhost:10808").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));

        let proxy = Url::parse("socks5h://localhost:10808").unwrap();
        let client = ReqwestClient::proxy_and_user_agent(Some(proxy.clone()), "test").unwrap();
        assert_eq!(client.proxy(), Some(&proxy));
    }

    #[test]
    fn test_unsupported_proxy_schemes() {
        for proxy in ["socks://127.0.0.1:20170", "ftp://127.0.0.1:20170"] {
            let proxy = Url::parse(proxy).unwrap();
            let client = ReqwestClient::proxy_and_user_agent(Some(proxy), "test").unwrap();
            assert!(
                client.proxy.is_none(),
                "An unsupported proxy URL should add no proxy to the client!"
            )
        }
    }
}
