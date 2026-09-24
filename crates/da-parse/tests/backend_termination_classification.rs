//! T04 (M511-j7wz2f/S06) — typed backend-termination classification (H19-N2).
//!
//! H19-N2 requires a refusal to be classified by TYPE, never by assumption,
//! and forbids recording a client-side transport timeout as if it were a
//! terminated Java process. This suite drives the REAL [`GrobidHttpBackend`]
//! against a local deterministic socket so the classes come out of the actual
//! client-side classification in `crates/da-parse/src/http.rs`, not out of a
//! hand-assembled failure:
//!
//! - a silent peer (accepts, never answers) → transport timeout;
//! - a peer that ends the connection before/during the response → a
//!   connection class that is NOT a timeout;
//! - a closed port → a connect class;
//! - a failure the client cannot attribute → `Unattributable`.
//!
//! Negatives: timeout and peer-close are distinguishable in BOTH directions,
//! no class claims a killed Java process, and the classification is
//! deterministic and order-independent.
//!
//! No variant is added to `FailureCode` / `FailurePhase` / `LimitKind` and the
//! `ParseBackendFailure::to_parser_error()` text/behaviour is untouched — the
//! classifier reads only the already-closed `code`/`phase`.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::time::Duration;

use da_parse::{
    BackendTerminationClass, FailureCode, FailurePhase, GrobidHttpBackend, GrobidProfile,
    LimitKind, ParseBackend, ParseBackendFailure, ParseBackendRequest, RetryAfter,
    classify_backend_termination,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PDF_FIXTURE: &[u8] = b"%PDF-1.4 termination-classification fixture";

fn request_fixture() -> ParseBackendRequest {
    ParseBackendRequest::new("termination-paper", PDF_FIXTURE.to_vec())
}

fn http_backend(addr: SocketAddr) -> GrobidHttpBackend {
    GrobidHttpBackend::new(Some(&format!("http://{addr}")))
        .with_profile(GrobidProfile::structural())
}

// ---------------------------------------------------------------------------
// Local deterministic peers (tokio TCP, no external deps)
// ---------------------------------------------------------------------------

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Read one complete HTTP/1.1 request (head + declared `Content-Length` body)
/// so a response is only written after the client has finished sending.
async fn read_full_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        let n = stream.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            return buf;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_subsequence(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > (1 << 20) {
            return buf;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let content_length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    while buf.len() < head_end + content_length {
        let n = stream.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    buf
}

/// A peer that accepts the connection, consumes the request and then never
/// writes a response byte (the client timeout is the only way out).
async fn spawn_silent_peer() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind silent peer");
    let addr = listener.local_addr().expect("silent peer address");
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let _ = read_full_request(&mut stream).await;
                // Hold the connection open and stay silent.
                tokio::time::sleep(Duration::from_secs(5)).await;
            });
        }
    });
    addr
}

/// A peer that accepts the connection, consumes the request, then closes
/// without writing any response (a peer-closed connection).
async fn spawn_closing_peer() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind closing peer");
    let addr = listener.local_addr().expect("closing peer address");
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let _ = read_full_request(&mut stream).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    addr
}

/// A peer that answers with a valid response head but truncates the body, then
/// closes (a connection ended DURING the response).
async fn spawn_truncating_peer() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind truncating peer");
    let addr = listener.local_addr().expect("truncating peer address");
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let _ = read_full_request(&mut stream).await;
                // Declared 4096 bytes, delivered 16: the peer ends mid-body.
                let head = "HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(b"<TEI>truncated").await;
                let _ = stream.shutdown().await;
            });
        }
    });
    addr
}

/// An address whose port is guaranteed closed: bind an ephemeral listener, note
/// its address, then drop the listener before connecting.
async fn closed_port_addr() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind then release");
    let addr = listener.local_addr().expect("released address");
    drop(listener);
    addr
}

// ---------------------------------------------------------------------------
// Real-socket classification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn silent_peer_classifies_as_transport_timeout() {
    let addr = spawn_silent_peer().await;
    let backend = http_backend(addr).with_timeout(Duration::from_millis(80));
    let err = backend
        .fetch_tei(request_fixture())
        .await
        .expect_err("a silent peer must time out, not hang forever");
    assert_eq!(
        err.code(),
        FailureCode::Timeout,
        "the client timeout is the only outcome for a silent peer (got {err})"
    );
    assert_eq!(err.phase(), FailurePhase::AwaitResponse);
    assert_eq!(
        classify_backend_termination(&err),
        BackendTerminationClass::TransportTimeout
    );
}

#[tokio::test]
async fn peer_closing_before_response_classifies_as_a_connection_class() {
    let addr = spawn_closing_peer().await;
    let backend = http_backend(addr).with_timeout(Duration::from_secs(10));
    let err = backend
        .fetch_tei(request_fixture())
        .await
        .expect_err("a peer that closes must not yield a success");
    let class = classify_backend_termination(&err);
    assert_ne!(
        class,
        BackendTerminationClass::TransportTimeout,
        "a closed peer is not a timeout (got {err})"
    );
    assert_eq!(
        class,
        BackendTerminationClass::PeerClosedConnection,
        "a peer-ended connection must classify as a connection class (got code={} phase={})",
        err.code(),
        err.phase()
    );
}

#[tokio::test]
async fn peer_closing_during_the_response_body_is_not_a_timeout() {
    let addr = spawn_truncating_peer().await;
    let backend = http_backend(addr).with_timeout(Duration::from_secs(10));
    let err = backend
        .fetch_tei(request_fixture())
        .await
        .expect_err("a truncated response body must fail closed");
    let class = classify_backend_termination(&err);
    assert_ne!(
        class,
        BackendTerminationClass::TransportTimeout,
        "a truncated body is not a timeout (got {err})"
    );
    assert_eq!(
        class,
        BackendTerminationClass::PeerClosedConnection,
        "a connection ended mid-body must classify as a connection class (got code={} phase={})",
        err.code(),
        err.phase()
    );
}

#[tokio::test]
async fn closed_port_classifies_as_connection_refused() {
    let addr = closed_port_addr().await;
    let backend = http_backend(addr).with_timeout(Duration::from_millis(500));
    let err = backend
        .fetch_tei(request_fixture())
        .await
        .expect_err("a closed port must fail to connect");
    assert_eq!(
        err.code(),
        FailureCode::Transport,
        "a refused connect is a transport failure (got {err})"
    );
    assert_eq!(err.phase(), FailurePhase::Connect);
    assert_eq!(
        classify_backend_termination(&err),
        BackendTerminationClass::ConnectionRefused
    );
}

#[tokio::test]
async fn unattributable_is_reachable_and_java_side_failures_are_not_process_death() {
    // A Java 5xx conversion failure says nothing about the process: it is
    // unattributable from the client side of the socket.
    let java_5xx = ParseBackendFailure::conversion(FailurePhase::AwaitResponse, Some(500));
    assert_eq!(
        classify_backend_termination(&java_5xx),
        BackendTerminationClass::Unattributable
    );

    let unattributable = [
        ParseBackendFailure::busy(503, Some(RetryAfter::Seconds(1))),
        ParseBackendFailure::limit(LimitKind::ResponseBytes, None),
        ParseBackendFailure::limit(LimitKind::RequestBytes, Some(413)),
        ParseBackendFailure::no_content(),
        ParseBackendFailure::invalid_request(400),
        ParseBackendFailure::invalid_response(FailurePhase::AwaitResponse, Some(418)),
        // Client-side multipart encode is not a peer event at all.
        ParseBackendFailure::transport(FailurePhase::MultipartEncode),
    ];
    for failure in &unattributable {
        assert_eq!(
            classify_backend_termination(failure),
            BackendTerminationClass::Unattributable,
            "`{failure}` must stay unattributable"
        );
    }
}

// ---------------------------------------------------------------------------
// Negatives (i): timeout and peer-close are distinguishable BOTH ways
// ---------------------------------------------------------------------------

#[tokio::test]
async fn timeout_and_peer_close_are_distinguishable_in_both_directions() {
    let timeout_head = ParseBackendFailure::timeout(FailurePhase::AwaitResponse);
    let timeout_body = ParseBackendFailure::timeout(FailurePhase::ReadBody);
    let peer_send = ParseBackendFailure::transport(FailurePhase::Send);
    let peer_await = ParseBackendFailure::transport(FailurePhase::AwaitResponse);
    let peer_body = ParseBackendFailure::conversion(FailurePhase::ReadBody, Some(200));
    let connect = ParseBackendFailure::transport(FailurePhase::Connect);

    // Timeouts are timeouts, in either phase.
    assert_eq!(
        classify_backend_termination(&timeout_head),
        BackendTerminationClass::TransportTimeout
    );
    assert_eq!(
        classify_backend_termination(&timeout_body),
        BackendTerminationClass::TransportTimeout
    );

    // A timeout is never a peer close...
    for failure in [&timeout_head, &timeout_body] {
        assert_ne!(
            classify_backend_termination(failure),
            BackendTerminationClass::PeerClosedConnection
        );
        assert_ne!(
            classify_backend_termination(failure),
            BackendTerminationClass::ConnectionRefused
        );
    }

    // ...and a peer close is never a timeout.
    for failure in [&peer_send, &peer_await, &peer_body, &connect] {
        assert_ne!(
            classify_backend_termination(failure),
            BackendTerminationClass::TransportTimeout,
            "`{failure}` must not be recorded as a timeout"
        );
    }

    // The connection classes are themselves distinct.
    assert_eq!(
        classify_backend_termination(&peer_send),
        BackendTerminationClass::PeerClosedConnection
    );
    assert_eq!(
        classify_backend_termination(&peer_await),
        BackendTerminationClass::PeerClosedConnection
    );
    assert_eq!(
        classify_backend_termination(&peer_body),
        BackendTerminationClass::PeerClosedConnection
    );
    assert_eq!(
        classify_backend_termination(&connect),
        BackendTerminationClass::ConnectionRefused
    );
    assert_ne!(
        classify_backend_termination(&connect),
        classify_backend_termination(&peer_send)
    );
}

// ---------------------------------------------------------------------------
// Negatives (ii): no class claims that a Java process was killed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_variant_claims_a_killed_java_process() {
    let names: BTreeSet<String> = BackendTerminationClass::ALL
        .iter()
        .map(|class| class.as_str().to_string())
        .collect();
    assert_eq!(
        names,
        BTreeSet::from([
            "transport_timeout".to_string(),
            "peer_closed_connection".to_string(),
            "connection_refused".to_string(),
            "unattributable".to_string(),
        ]),
        "the termination vocabulary is exactly these four classes"
    );
    assert_eq!(BackendTerminationClass::ALL.len(), 4);

    // The wire form of every class is free of process-death claims, and the
    // honest "cannot attribute" state exists.
    for class in BackendTerminationClass::ALL {
        let wire = serde_json::to_string(&class).expect("class serializes");
        let lower = wire.to_lowercase();
        for forbidden in [
            "process",
            "kill",
            "java",
            "grobid",
            "dead",
            "crash",
            "oom",
            "sigkill",
            "terminated",
        ] {
            assert!(
                !lower.contains(forbidden),
                "termination class `{wire}` claims process attribution via `{forbidden}`"
            );
        }
    }
    assert!(
        BackendTerminationClass::ALL.contains(&BackendTerminationClass::Unattributable),
        "the honest unattributable state must exist"
    );
}

// ---------------------------------------------------------------------------
// Negatives (iii): the classification is deterministic and order-independent
// ---------------------------------------------------------------------------

#[tokio::test]
async fn classification_is_deterministic_and_order_independent() {
    let failures = [
        ParseBackendFailure::timeout(FailurePhase::AwaitResponse),
        ParseBackendFailure::timeout(FailurePhase::ReadBody),
        ParseBackendFailure::transport(FailurePhase::Connect),
        ParseBackendFailure::transport(FailurePhase::Send),
        ParseBackendFailure::transport(FailurePhase::AwaitResponse),
        ParseBackendFailure::conversion(FailurePhase::ReadBody, Some(200)),
        ParseBackendFailure::conversion(FailurePhase::AwaitResponse, Some(500)),
        ParseBackendFailure::busy(503, None),
        ParseBackendFailure::limit(LimitKind::RequestBytes, Some(413)),
        ParseBackendFailure::invalid_response(FailurePhase::AwaitResponse, Some(418)),
    ];

    // Repeated calls on the same input agree.
    for failure in &failures {
        let first = classify_backend_termination(failure);
        let second = classify_backend_termination(failure);
        assert_eq!(first, second, "classification must be deterministic");
    }

    // Forward and reverse orders agree element-wise.
    let forward: Vec<_> = failures.iter().map(classify_backend_termination).collect();
    let mut backward: Vec<_> = failures
        .iter()
        .rev()
        .map(classify_backend_termination)
        .collect();
    backward.reverse();
    assert_eq!(
        forward, backward,
        "classification must not depend on the order of calls"
    );
}
