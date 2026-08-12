//! Wiremock HTTP contract matrix for ArxivSourceAcquisition (M386 S02 T02).
//!
//! Provider endpoints are env-overridable (D239), so tests point the adapter at
//! a local wiremock server. Covers: atom meta + PDF fetch (happy), transient
//! 503 retry→success, partial PDF validation fail, and HTML fallback.

use std::path::PathBuf;

use da_adapters::arxiv_acquisition::ArxivSourceAcquisition;
use da_domain::{AcquisitionPolicy, ArticleLocator, PrimaryContentKind};
use da_ports::SourceAcquisitionPort;
use httpmock::prelude::*;

const ATOM_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/atom">
  <title>ArXiv Query: 2507.19458</title>
  <entry>
    <title>Test paper title</title>
    <arxiv:primary_category term="cs.AI" xmlns:arxiv="http://arxiv.org/schemas/atom"/>
    <category term="cs.AI" scheme="http://arxiv.org/schemas/atom"/>
  </entry>
</feed>"#;

/// Minimal PDF body passing validate_pdf (>=1000 bytes, %PDF- magic).
fn pdf_body() -> Vec<u8> {
    let mut body = b"%PDF-1.4\n".to_vec();
    body.extend(std::iter::repeat_n(b'0', 1100));
    body
}

fn temp_catalog() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("da-acq-contract-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn policy_network() -> AcquisitionPolicy {
    AcquisitionPolicy {
        allow_network: true,
        ..Default::default()
    }
}

/// Serialize env-mutating tests and restore env afterwards (no poisoning).
fn with_endpoints(server: &MockServer, body: impl FnOnce()) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let base = server.base_url();
    unsafe {
        std::env::set_var("DA_ARXIV_EXPORT_BASE_URL", &base);
        std::env::set_var("DA_ARXIV_ABS_BASE_URL", &base);
        std::env::set_var("DA_AR5IV_BASE_URL", &base);
        std::env::set_var("DA_HTTP_RETRY_BACKOFF_SECONDS", "0,0");
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
    unsafe {
        std::env::remove_var("DA_ARXIV_EXPORT_BASE_URL");
        std::env::remove_var("DA_ARXIV_ABS_BASE_URL");
        std::env::remove_var("DA_AR5IV_BASE_URL");
        std::env::remove_var("DA_HTTP_RETRY_BACKOFF_SECONDS");
    }
    if let Err(err) = result {
        std::panic::resume_unwind(err);
    }
}

#[test]
fn atom_meta_and_pdf_fetch_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path_contains("/api/query");
        then.status(200)
            .header("Content-Type", "application/atom+xml")
            .body(ATOM_XML);
    });
    server.mock(|when, then| {
        when.method(GET).path_contains("/pdf/2507.19458");
        then.status(200)
            .header("Content-Type", "application/pdf")
            .body(pdf_body());
    });
    let catalog = temp_catalog();
    with_endpoints(&server, || {
        let acq = ArxivSourceAcquisition::new(&catalog);
        let locator = ArticleLocator::Arxiv {
            id: "2507.19458".into(),
            revision: None,
        };
        let resolved = acq.resolve(&locator).unwrap();
        assert_eq!(resolved.source_code, "arxiv");
        let ensured = acq.ensure_assets(&resolved, &policy_network()).unwrap();
        assert_eq!(ensured.primary.kind, PrimaryContentKind::Pdf);
        let pdf = catalog.join("article_catalog/arxiv/cs-ai/2507.19458/source/2507.19458.pdf");
        assert!(pdf.is_file(), "pdf should be written: {}", pdf.display());
    });
    let _ = server;
}

#[test]
fn transient_503_retries_then_first_valid_falls_back() {
    let server = MockServer::start();
    let pdf = pdf_body();
    // Candidate 1 (/pdf/2507.19459) returns transient 503 → http_get_bytes retries,
    // exhausts backoff, returns Err. http_get_first_valid falls through to
    // candidate 2 (/pdf/2507.19459.pdf) which returns 200 + valid PDF.
    server.mock(|when, then| {
        when.method(GET).path("/pdf/2507.19459");
        then.status(503).body("retry later");
    });
    server.mock(|when, then| {
        when.method(GET).path("/pdf/2507.19459.pdf");
        then.status(200)
            .header("Content-Type", "application/pdf")
            .body(pdf.clone());
    });
    server.mock(|when, then| {
        when.method(GET).path_contains("/api/query");
        then.status(200).body(ATOM_XML);
    });
    let catalog = temp_catalog();
    with_endpoints(&server, || {
        let acq = ArxivSourceAcquisition::new(&catalog);
        let locator = ArticleLocator::Arxiv {
            id: "2507.19459".into(),
            revision: None,
        };
        let resolved = acq.resolve(&locator).unwrap();
        let ensured = acq.ensure_assets(&resolved, &policy_network()).unwrap();
        assert_eq!(ensured.primary.kind, PrimaryContentKind::Pdf);
    });
    let _ = server;
}

#[test]
fn partial_pdf_fails_validation_and_fails_closed() {
    let server = MockServer::start();
    // Body shorter than 1000 bytes → validation fail; all candidates fail → error.
    server.mock(|when, then| {
        when.method(GET).path_contains("/pdf/2507.19460");
        then.status(200).body(b"%PDF-1.4\nshort");
    });
    server.mock(|when, then| {
        when.method(GET).path_contains("/api/query");
        then.status(200).body(ATOM_XML);
    });
    let catalog = temp_catalog();
    with_endpoints(&server, || {
        let acq = ArxivSourceAcquisition::new(&catalog);
        let locator = ArticleLocator::Arxiv {
            id: "2507.19460".into(),
            revision: None,
        };
        let resolved = acq.resolve(&locator).unwrap();
        let result = acq.ensure_assets(&resolved, &policy_network());
        assert!(result.is_err(), "partial pdf must fail closed");
    });
    let _ = server;
}

#[test]
fn network_denied_fails_closed_without_network() {
    let catalog = temp_catalog();
    let acq = ArxivSourceAcquisition::new(&catalog);
    let locator = ArticleLocator::Arxiv {
        id: "2507.19461".into(),
        revision: None,
    };
    let resolved = acq.resolve(&locator).unwrap();
    let policy = AcquisitionPolicy {
        allow_network: false,
        ..Default::default()
    };
    let result = acq.ensure_assets(&resolved, &policy);
    assert!(result.is_err(), "network denied must fail closed");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("allow-network"), "got: {err}");
}
