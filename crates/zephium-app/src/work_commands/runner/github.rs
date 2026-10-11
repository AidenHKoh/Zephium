//! The child can request one opaque loopback route; only the captured broker
//! closure can decide the remote resource. No provider credential enters gh.
use super::{Failure, Output};
use std::{
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
const BODY_LIMIT: usize = 60 * 1024;
const HEADER_LIMIT: usize = 8192;

pub(crate) fn available() -> bool {
    const QUALIFIED: bool = true; // Qualified native denial, broker and Stop matrix on 25G83.
    #[cfg(target_os = "macos")]
    {
        QUALIFIED && super::macos_github::present()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = QUALIFIED;
        false
    }
}
pub(crate) async fn run<C, CF, F, FF>(cancelled: &C, fetch: F) -> Result<Output, Failure>
where
    C: Fn() -> CF,
    CF: Future<Output = bool>,
    F: FnOnce() -> FF,
    FF: Future<Output = Result<Vec<u8>, Failure>>,
{
    if !available() && !cfg!(test) {
        return Err(Failure::GitHubUnavailable);
    }
    #[cfg(target_os = "macos")]
    {
        if super::cancellation_requested(cancelled).await {
            return Err(Failure::Stopped);
        }
        let mut worker = tokio::task::spawn_blocking(super::macos_github::Runtime::prepare);
        let deadline = tokio::time::sleep(Duration::from_secs(10));
        tokio::pin!(deadline);
        let mut tick = tokio::time::interval(Duration::from_millis(25));
        let runtime = loop {
            tokio::select! {
                ready = &mut worker => break ready.map_err(|_| Failure::GitHubUnavailable)??,
                _ = &mut deadline => return Err(Failure::Limit),
                _ = tick.tick() => if super::cancellation_requested(cancelled).await { return Err(Failure::Stopped); }
            }
        };
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| Failure::Launch)?;
        let port = listener.local_addr().map_err(|_| Failure::Launch)?.port();
        let nonce = format!(
            "{:032x}",
            u128::from_be_bytes(zephium_core::work::WorkArtifactId::generate().bytes())
        );
        let halt = AtomicBool::new(false);
        let external = AtomicBool::new(false);
        let cancellation = || async {
            let stopped = super::cancellation_requested(cancelled).await;
            if stopped {
                external.store(true, Ordering::Release);
            }
            stopped || halt.load(Ordering::Acquire)
        };
        let child = async {
            let result = runtime.run(port, &nonce, &cancellation).await;
            if result.is_err() || result.as_ref().is_ok_and(|out| !out.success) {
                halt.store(true, Ordering::Release);
            }
            result
        };
        let broker = async {
            let result = serve(listener, port, &nonce, &halt, fetch).await;
            if result.is_err() {
                halt.store(true, Ordering::Release);
            }
            result
        };
        let (child, broker) = tokio::join!(child, broker);
        if matches!(child, Err(Failure::StopUnconfirmed)) {
            return Err(Failure::StopUnconfirmed);
        }
        if external.load(Ordering::Acquire) {
            return Err(Failure::Stopped);
        }
        let expected = broker?;
        let output = child?;
        if !output.success
            || serde_json::from_str::<serde_json::Value>(&output.text).ok() != Some(expected)
        {
            return Err(Failure::Failed);
        }
        Ok(output)
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Windows LPAC without networking cannot reach a loopback broker. Do not
        // grant internetClient or a machine-wide loopback exemption as a fallback.
        // A pinned packaged gh plus a qualified per-endpoint network policy and
        // the existing Job/LPAC filesystem confinement are required before admission.
        let _ = (cancelled, fetch);
        Err(Failure::GitHubUnavailable)
    }
}
async fn serve<F, FF>(
    listener: TcpListener,
    port: u16,
    nonce: &str,
    halted: &AtomicBool,
    fetch: F,
) -> Result<serde_json::Value, Failure>
where
    F: FnOnce() -> FF,
    FF: Future<Output = Result<Vec<u8>, Failure>>,
{
    let exchange = async {
        let (mut stream, peer) = listener.accept().await.map_err(|_| Failure::Failed)?;
        if !peer.ip().is_loopback() {
            return Err(Failure::Failed);
        }
        let mut header = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let count = stream.read(&mut chunk).await.map_err(|_| Failure::Failed)?;
            if count == 0 || header.len() + count > HEADER_LIMIT {
                return Err(Failure::Failed);
            }
            header.extend_from_slice(&chunk[..count]);
            if header.windows(4).any(|part| part == b"\r\n\r\n") {
                break;
            }
        }
        if !exact_request(&header, port, nonce) {
            return Err(Failure::Failed);
        }
        let bytes = fetch().await?;
        if bytes.len() > BODY_LIMIT {
            return Err(Failure::Limit);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| Failure::Failed)?;
        if zephium_agentic::contains_recognized_secret(text) {
            return Err(Failure::Sensitive);
        }
        let value: serde_json::Value = serde_json::from_str(text).map_err(|_| Failure::Failed)?;
        if zephium_agentic::contains_recognized_secret(&value.to_string()) {
            return Err(Failure::Sensitive);
        }
        let header = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len());
        stream
            .write_all(header.as_bytes())
            .await
            .map_err(|_| Failure::Failed)?;
        stream
            .write_all(&bytes)
            .await
            .map_err(|_| Failure::Failed)?;
        stream.shutdown().await.map_err(|_| Failure::Failed)?;
        Ok(value)
    };
    tokio::pin!(exchange);
    let deadline = tokio::time::sleep(Duration::from_secs(12));
    tokio::pin!(deadline);
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    loop {
        tokio::select! {
            result = &mut exchange => return result,
            _ = &mut deadline => return Err(Failure::Limit),
            _ = tick.tick() => if halted.load(Ordering::Acquire) { return Err(Failure::Failed); }
        }
    }
}
fn exact_request(bytes: &[u8], port: u16, nonce: &str) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    if !text.ends_with("\r\n\r\n") || text[..text.len() - 4].contains("\r\n\r\n") {
        return false;
    }
    let mut lines = text[..text.len() - 4].split("\r\n");
    if lines.next() != Some(format!("GET /{nonce} HTTP/1.1").as_str()) {
        return false;
    }
    let mut host = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return false;
        }
        if name.eq_ignore_ascii_case("host") {
            if host || value.trim() != format!("127.0.0.1:{port}") {
                return false;
            }
            host = true;
        } else if name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("content-length") && value.trim() != "0"
        {
            return false;
        }
        if line.chars().any(|c| c.is_control() && c != '\t') {
            return false;
        }
    }
    host
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn broker_accepts_only_one_exact_bodyless_get() {
        let good = b"GET /nonce HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nUser-Agent: fixture\r\n\r\n";
        assert!(exact_request(good, 1234, "nonce"));
        for request in [
            "POST /nonce HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n",
            "GET /other HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n",
            "GET /nonce?q=x HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n",
            "GET /nonce HTTP/1.1\r\nHost: example.com\r\n\r\n",
            "GET /nonce HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nHost: 127.0.0.1:1234\r\n\r\n",
            "GET /nonce HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nContent-Length: 2\r\n\r\nxx",
            "GET /nonce HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nTransfer-Encoding: chunked\r\n\r\n",
        ] {
            assert!(!exact_request(request.as_bytes(), 1234, "nonce"));
        }
    }
    #[tokio::test]
    async fn malformed_request_never_starts_remote_fetch() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let called = AtomicBool::new(false);
        let halt = AtomicBool::new(false);
        let broker = serve(listener, port, "nonce", &halt, || async {
            called.store(true, Ordering::Release);
            Ok(b"{}".to_vec())
        });
        let client = async {
            let mut socket = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .unwrap();
            socket
                .write_all(
                    format!("POST /nonce HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes(),
                )
                .await
                .unwrap();
            let mut bytes = Vec::new();
            socket.read_to_end(&mut bytes).await.unwrap();
            bytes
        };
        let (reply, bytes) = tokio::join!(broker, client);
        assert!(reply.is_err());
        assert!(bytes.is_empty());
        assert!(!called.load(Ordering::Acquire));
    }
    #[tokio::test]
    async fn broker_withholds_raw_decoded_and_oversized_output_before_client_receives_it() {
        for content in [
            br#"{"text":"ghp_inertFixtureToken"}"#.to_vec(),
            br#"{"text":"\u0067hp_inertFixtureToken"}"#.to_vec(),
            vec![b'x'; BODY_LIMIT + 1],
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let halt = AtomicBool::new(false);
            let broker = serve(listener, port, "nonce", &halt, || async { Ok(content) });
            let client = async {
                let mut socket =
                    tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
                        .await
                        .unwrap();
                socket
                    .write_all(
                        format!("GET /nonce HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes(),
                    )
                    .await
                    .unwrap();
                let mut bytes = Vec::new();
                socket.read_to_end(&mut bytes).await.unwrap();
                bytes
            };
            let (reply, bytes) = tokio::join!(broker, client);
            assert!(matches!(reply, Err(Failure::Sensitive | Failure::Limit)));
            assert!(bytes.is_empty());
        }
    }
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "Native pinned-gh qualification; inert loopback broker only"]
    async fn native_github_fixed_get_returns_only_captured_public_fixture() {
        let output = run(&|| async { false }, || async {
            Ok(br#"{"number":3,"title":"Inert public fixture"}"#.to_vec())
        })
        .await
        .unwrap();
        assert!(output.success);
        let parsed: serde_json::Value = serde_json::from_str(&output.text).unwrap();
        assert_eq!(parsed["number"], 3);
        assert_eq!(parsed["title"], "Inert public fixture");
        let hidden = run(&|| async { false }, || async {
            Ok(br#"{"text":"ghp_inertFixtureToken"}"#.to_vec())
        })
        .await;
        assert!(matches!(hidden, Err(Failure::Sensitive)));
    }
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "Native pinned-gh Stop qualification; pending inert broker, no remote request"]
    async fn native_github_stop_interrupts_pending_broker_with_owned_child_exit() {
        let started = AtomicBool::new(false);
        let requested = std::sync::Mutex::new(None::<std::time::Instant>);
        let cancelled = || async {
            if started.load(Ordering::Acquire) {
                requested
                    .lock()
                    .unwrap()
                    .get_or_insert_with(std::time::Instant::now);
                true
            } else {
                false
            }
        };
        let outcome = run(&cancelled, || async {
            started.store(true, Ordering::Release);
            std::future::pending::<Result<Vec<u8>, Failure>>().await
        })
        .await;
        assert!(matches!(outcome, Err(Failure::Stopped)));
        assert!(requested.lock().unwrap().unwrap().elapsed() < Duration::from_secs(1));
    }
}
