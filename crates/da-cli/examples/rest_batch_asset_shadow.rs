//! Offline, digest-only proof of the existing local parse-batch composition.
//!
//! This is intentionally a shadow example, not a production command.  It
//! drives the existing typed manifest -> ParseBatchExecutor -> two-stage
//! FileParseArtifactCache path against a loopback HTTP stub and emits only
//! bounded counters, identities and policy/source bindings.

use da_cli::parse_batch::{
    ParseBatchArgs, ParseBatchManifestDocument, ParseBatchManifestV1, run_with_env,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const JAVA_SOURCE: &str = "artifacts/parse_compat/java_grobid_rest_batch_asset_source.json";
const DEMAND_SOURCE: &str = "artifacts/parse_compat/rest_batch_asset_oracle_source.json";
const DATA: &str = "data/parse/rest_batch_asset_parity.yaml";
const TEI: &str = "<TEI xmlns=\"http://www.tei-c.org/ns/1.0\"><teiHeader><fileDesc><titleStmt><title>shadow</title></titleStmt></fileDesc></teiHeader><text><body><p>shadow</p></body></text></TEI>";

/// Fixed service version the offline stub reports from `GET /api/version`.
/// The production CLI probes it before admission; the shadow run is a
/// versioned run (never the legacy identity).
const STUB_BACKEND_VERSION: &str = "shadow-stub-0.9.0-crf";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

fn sha256_file(path: &str) -> String {
    let bytes = std::fs::read(repo_root().join(path)).expect("tracked source artifact");
    hex::encode(Sha256::digest(bytes))
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<String> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if bytes.len() > 1 << 20 {
            return None;
        }
    }
    let head = String::from_utf8_lossy(&bytes);
    Some(head.lines().next().unwrap_or_default().to_owned())
}

async fn spawn_stub() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback stub");
    let address = listener.local_addr().expect("stub address");
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let request = read_request(&mut stream).await.unwrap_or_default();
                let path = request.split_whitespace().nth(1).unwrap_or_default();
                let version_body =
                    format!("{{\"name\":\"GROBID\",\"version\":\"{STUB_BACKEND_VERSION}\"}}");
                let (status, body): (&str, &[u8]) = match path {
                    "/api/version" => ("200 OK", version_body.as_bytes()),
                    "/api/processFulltextDocument"
                    | "/api/processHeaderDocument"
                    | "/api/processReferences" => ("200 OK", TEI.as_bytes()),
                    _ => ("418 I'm a Teapot", &[][..]),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.write_all(body).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    format!("http://{address}")
}

fn write_inputs(temp: &TempDir) -> PathBuf {
    let documents = [
        ("shadow-fulltext", "fulltext"),
        ("shadow-header", "header"),
        ("shadow-references", "references"),
    ];
    let mut entries = Vec::new();
    for (paper_id, endpoint) in documents {
        let path = temp.path().join(format!("{paper_id}.pdf"));
        std::fs::write(&path, format!("synthetic-pdf:{paper_id}")).expect("write fixture");
        entries.push(ParseBatchManifestDocument {
            paper_id: paper_id.to_owned(),
            pdf_path: path.display().to_string(),
            endpoint: Some(endpoint.to_owned()),
        });
    }
    let manifest = temp.path().join("manifest.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&ParseBatchManifestV1 {
            schema_version: 1,
            documents: entries,
        })
        .expect("manifest JSON"),
    )
    .expect("write manifest");
    manifest
}

async fn run(
    manifest: &Path,
    report: &Path,
    cache: &Path,
    endpoint: &str,
    mode: &str,
    workers: &str,
) {
    let args = ParseBatchArgs {
        manifest: manifest.to_path_buf(),
        report: report.to_path_buf(),
        mode: None,
        grobid_url: None,
        backend: None,
        cache_root: None,
        batch_size: None,
        timeout_secs: None,
        workers: None,
        queue_capacity: None,
        publication_window: None,
        execution_capacity: None,
        max_in_flight: None,
        resume: false,
        allow_versionless_identity: false,
        redb_index: None,
        no_redb: false,
        redb_index_path: None,
        learning_rvf_path: None,
        learning_redb_index_path: None,
    };
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("DA_GROBID_URL".into(), endpoint.into());
    env.insert("DA_PARSE_CACHE_ROOT".into(), cache.display().to_string());
    env.insert("DA_PARSE_CACHE_MODE".into(), mode.into());
    env.insert("DA_PARSE_WORKERS".into(), workers.into());
    env.insert("DA_PARSE_QUEUE_CAPACITY".into(), "2".into());
    env.insert("DA_PARSE_MAX_IN_FLIGHT".into(), workers.into());
    env.insert("DA_PARSE_BATCH_SIZE".into(), "3".into());
    let lookup = |key: &str| env.get(key).cloned();
    run_with_env(&args, &lookup)
        .await
        .expect("shadow batch run");
}

fn semantic(value: &Value) -> Value {
    let documents = value["documents"]
        .as_array()
        .expect("document outcomes")
        .iter()
        .map(|document| {
            json!({
                "ordinal": document["ordinal"],
                "paper_id": document["paper_id"],
                "result": document["result"],
            })
        })
        .collect::<Vec<_>>();
    json!({
        "input_fingerprint": value["input_fingerprint"],
        "documents": documents,
        "cursor": value["cursor"],
    })
}

fn assert_report_is_redacted(text: &str, value: &Value) {
    for marker in [
        "http://",
        "https://",
        "127.0.0.1",
        "synthetic-pdf:",
        "<TEI",
        ".pdf",
        "/tmp/",
        "/root/",
    ] {
        assert!(!text.contains(marker), "shadow report leaked {marker}");
    }
    assert_eq!(value["summary"]["admitted"], 3);
    assert_eq!(value["summary"]["completed"], 3);
    assert_eq!(
        value["documents"]
            .as_array()
            .expect("document outcomes")
            .len(),
        3
    );
}

#[tokio::main]
async fn main() {
    let _log_guard = da_cli::logging::init_pipeline_logging("rest_batch_asset_shadow");
    let mut args = std::env::args().skip(1);
    assert_eq!(
        args.next().as_deref(),
        Some("--report"),
        "usage: --report FILE"
    );
    let output = PathBuf::from(args.next().expect("report path"));
    let temp = TempDir::new().expect("temporary composition root");
    let manifest = write_inputs(&temp);
    let cache = temp.path().join("cache");
    let endpoint = spawn_stub().await;
    let write_report = temp.path().join("write-through.json");
    let replay_report = temp.path().join("read-only.json");
    let off_report = temp.path().join("off.json");

    run(
        &manifest,
        &write_report,
        &cache,
        &endpoint,
        "write-through",
        "2",
    )
    .await;
    run(
        &manifest,
        &replay_report,
        &cache,
        &endpoint,
        "read-only",
        "2",
    )
    .await;
    run(&manifest, &off_report, &cache, &endpoint, "off", "1").await;

    let write_text = std::fs::read_to_string(&write_report).expect("write-through report");
    let replay_text = std::fs::read_to_string(&replay_report).expect("read-only report");
    let off_text = std::fs::read_to_string(&off_report).expect("off report");
    let write: Value = serde_json::from_str(&write_text).expect("write report JSON");
    let replay: Value = serde_json::from_str(&replay_text).expect("replay report JSON");
    let off: Value = serde_json::from_str(&off_text).expect("off report JSON");
    assert_report_is_redacted(&write_text, &write);
    assert_report_is_redacted(&replay_text, &replay);
    assert_report_is_redacted(&off_text, &off);
    assert_eq!(semantic(&write), semantic(&replay), "cache replay drift");
    assert_eq!(
        semantic(&write),
        semantic(&off),
        "worker/cache semantic drift"
    );
    assert_eq!(write["limits"]["workers"], 2);
    assert_eq!(write["limits"]["queue_capacity"], 2);
    assert_eq!(write["limits"]["max_in_flight"], 2);
    assert_eq!(write["summary"]["backend_calls"], 3);
    assert_eq!(replay["summary"]["backend_calls"], 0);
    assert_eq!(replay["summary"]["raw_hits"], 3);
    assert_eq!(off["summary"]["backend_calls"], 3);
    assert_eq!(off["summary"]["raw_hits"], 0);

    let output_value = json!({
        "schema_version": "rest-batch-asset-shadow.v1",
        "source_bindings": {
            "java_source_sha256": sha256_file(JAVA_SOURCE),
            "demand_oracle_source_sha256": sha256_file(DEMAND_SOURCE),
            "data_sha256": sha256_file(DATA),
        },
        // GAR-006: the reviewed demand inventory (19 operations, 4 executable,
        // 3 corpus documents / 202 pages per lane / 606 lane pages) is the
        // referenced population. The executed population is the synthetic
        // loopback run, and it says so instead of borrowing the review scale.
        // Reference numbers come from
        // `artifacts/parse_compat/rest_batch_asset_oracle_source.json`
        // (`review_denominators`) and
        // `crates/da-parse/tests/fixtures/rest_batch_asset_oracle.yaml`.
        "reference_population": {
            "identity": "reviewed_rest_batch_asset_demand_reference",
            "role": "identity_only",
            "documents": 3,
            "inventory": 19,
            "demand_review": 19,
            "executable": 4,
            "corpus_pages_per_lane": 202,
            "corpus_lane_pages": 606,
            "upstream_terminal_outcomes": 6,
            "s03_s05_cells": 54,
        },
        "executed_population": {
            "corpus_execution": false,
            "documents": 3,
            "operations": 3,
            "source_kind": "synthetic_loopback_fixture",
            "objects": write["summary"]["completed"],
            "execution_evidence": "offline_loopback_stub_existing_typed_transport",
        },
        "evaluated_population": {
            "status": "not_evaluated",
            "documents": 0,
            "oracle": "none",
            "reason": "offline_stub_without_corpus_execution",
        },
        "outcomes": {
            "accounting_contour": {
                "admitted": write["summary"]["admitted"],
                "completed": write["summary"]["completed"],
                "backend_calls": write["summary"]["backend_calls"],
                "loss": 0,
            },
            "quality_claims": [],
        },
        "operations": [
            {"operation_id": "rest.processFulltextDocument", "disposition": "existing_typed_transport"},
            {"operation_id": "rest.processHeaderDocument", "disposition": "existing_typed_transport"},
            {"operation_id": "rest.processReferences", "disposition": "existing_typed_transport"},
            {"operation_id": "rest.batchProcessFulltext", "disposition": "existing_local_batch_composition"},
        ],
        "run_summary": {
            "write_through": write["summary"], "read_only_replay": replay["summary"], "off_worker_one": off["summary"],
        },
        "per_document": write["documents"],
        "cache": {"modes": ["off", "read-only", "write-through", "refresh"], "replay_equal": true, "worker_one_equal": true, "raw_tei_published_before_projection": true, "projector_reprojection_without_backend": true, "identity_mismatch": "error_not_miss"},
        "policy": {"implemented_new_endpoints": 0, "production_route": "forbidden", "generic_arbitrary_route": "forbidden", "import_eligible": false, "graph_write": "forbidden", "auto_promote": "forbidden"},
        "redaction": {"raw_payloads": false, "paths": false, "urls": false, "hosts": false, "credentials": false},
    });
    std::fs::write(
        &output,
        serde_json::to_vec_pretty(&output_value).expect("output JSON"),
    )
    .expect("write shadow report");
}
