//! Opt-in live GROBID capture test (S08/T03).
//!
//! **MISSING polarity.** The default endpoint alone never enables this test:
//! without an explicitly set `GROBID_URL`/`DA_GROBID_URL` the body prints
//! `MISSING` and returns *before any assertion runs*, so an absent service can
//! never be recorded as a green integration result. The test claims nothing in
//! that arm.
//!
//! With the environment set it performs exactly one capture and one receipt,
//! asserting structure only: state, `sections > 0`, count conservation, digest
//! binding. No measured number is asserted — the counts belong to the release
//! that answered, not to this test, and hardcoding them would turn a
//! measurement into a claim.
//!
//! The dump stays in memory: it carries token text and is never written here.

use da_cli::native_java_work::{MeasurementState, NativeJavaWorkReceipt, TapeVerificationState};
use da_parse::{CaptureState, GrobidParser, ParseBackendRequest, ParseEndpoint, StageTape};
use sha2::{Digest, Sha256};

const PINNED_PDF: &str = "data/sources/arxiv/cs-ai/2512.24601/source/original.pdf";
const PAPER_ID: &str = "s08-live-java-work";

fn explicitly_configured_endpoint() -> Option<String> {
    ["DA_GROBID_URL", "GROBID_URL"].iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[tokio::test]
async fn live_capture_yields_a_structural_receipt_or_reports_missing() {
    let Some(endpoint) = explicitly_configured_endpoint() else {
        println!(
            "MISSING: set DA_GROBID_URL or GROBID_URL to enable the live capture; \
             no assertion runs and no PASS is claimed"
        );
        return;
    };

    let pdf = std::fs::read(PINNED_PDF).expect("pinned corpus pdf is readable");
    let source_sha256 = sha256_hex(&pdf);
    let parser = GrobidParser::new(Some(&endpoint))
        .with_profile(da_cli::parse_profile::consult_corpus_grobid_profile());

    let version = parser
        .probe_version()
        .await
        .expect("live release probe")
        .version
        .trim()
        .to_string();
    assert!(
        !version.is_empty(),
        "a blank release must never bind a measurement"
    );

    let request = ParseBackendRequest::new(PAPER_ID.to_string(), pdf);
    let dump = parser
        .debug_capture(ParseEndpoint::Fulltext, &request, None)
        .await
        .expect("live debug capture");

    let tape = StageTape::from_capture(
        "fulltext",
        PAPER_ID,
        PINNED_PDF,
        &source_sha256,
        &version,
        None,
        dump.as_bytes(),
        None,
        Vec::new(),
    );
    assert_eq!(tape.capture_state, CaptureState::Captured);
    assert!(
        tape.sections > 0,
        "a capture that carries no model section proves nothing"
    );
    let dump_sha256 = tape.dump_sha256.clone().expect("dump digest");

    let receipt = NativeJavaWorkReceipt::from_tape(&tape).expect("receipt");
    match receipt.state() {
        MeasurementState::Measured | MeasurementState::Filtered => {}
        other => panic!("unexpected measurement state {other:?}"),
    }

    let counts = receipt.measured().expect("measured counts");
    assert!(counts.sections > 0);
    assert_eq!(counts.total_invocations, counts.sections);
    assert_eq!(counts.distinct_models, counts.models.len());
    assert_eq!(counts.source_sha256, source_sha256);
    assert_eq!(counts.source_pdf, PINNED_PDF);
    assert_eq!(counts.dump_sha256, dump_sha256);
    assert_eq!(counts.backend_version, version);
    assert_eq!(counts.endpoint, "fulltext");
    assert!(counts.tape_identity.starts_with("sha256:"));
    // Counting a tape is not verifying it: this test captures, it does not
    // re-measure the source and the release, so the receipt must say so.
    assert_eq!(counts.tape_verification, TapeVerificationState::NotObserved);
}
