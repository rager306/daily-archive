//! M307 Wave 3 thin smoke: in-memory Samyama RespServer answers RESP PING.
//!
//! Scope (anti-goals):
//! - No RocksDB / PersistenceManager (`RespServer::new`, `data_path: None`)
//! - No remote GraphStore client wiring (GraphMode::Remote still NotConfigured)
//! - D127 / import_eligible unchanged (no graph import path)
//!
//! Reuses vendor RESP wire format from samyama-graph server tests.

#![cfg(feature = "samyama")]

use std::sync::Arc;
use std::time::Duration;

use samyama::{GraphStore, RespServer, ServerConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;

/// Pick an ephemeral free port on loopback (brief bind-and-release).
async fn free_loopback_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral");
    let port = listener.local_addr().expect("local_addr").port();
    drop(listener);
    port
}

#[tokio::test]
async fn in_memory_resp_server_answers_ping_with_pong() {
    let port = free_loopback_port().await;
    let config = ServerConfig {
        address: "127.0.0.1".to_string(),
        port,
        max_connections: 8,
        data_path: None, // in-memory only — no RocksDB path
    };
    let store = Arc::new(RwLock::new(GraphStore::new()));
    let server = RespServer::new(config, store);

    // start() blocks in accept loop; run as background task and abort after.
    let server_task = tokio::spawn(async move {
        server
            .start()
            .await
            .expect("RespServer::start should bind and accept");
    });

    // Wait until the port is accepting connections (poll, not fixed sleep-only).
    let addr = format!("127.0.0.1:{port}");
    let mut stream = None;
    for _ in 0..50 {
        match TcpStream::connect(&addr).await {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
    let mut stream = stream.unwrap_or_else(|| {
        panic!("RespServer did not accept connections on {addr} within timeout")
    });

    // RESP array: *1\r\n$4\r\nPING\r\n  → SimpleString +PONG\r\n
    stream
        .write_all(b"*1\r\n$4\r\nPING\r\n")
        .await
        .expect("write PING");
    let mut buf = vec![0u8; 256];
    let n = stream.read(&mut buf).await.expect("read PONG");
    assert!(n > 0, "empty response from RespServer");
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(
        response.contains("PONG"),
        "expected PONG in response, got: {response:?}"
    );

    drop(stream);
    server_task.abort();
    let _ = server_task.await;
}
