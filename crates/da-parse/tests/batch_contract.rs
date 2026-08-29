//! Typed batch & cache contract gate for the scholarly parse plane
//! (M464-z84in2 / S06 / T01).
//!
//! Falsifiable assertions over the FROZEN S06 contract (before any executor
//! exists), each one phrased so a violation fails with a specific message:
//!
//! - the closed cache-mode vocabulary (`off`, `read-only`, `write-through`,
//!   `refresh`) round-trips exactly and rejects unknown modes; the mode
//!   truth table is pinned as code (off never reads or writes; read-only
//!   reads both stages and never publishes; write-through reads and
//!   publishes; refresh bypasses hits and replaces both stages);
//! - the two cache stages keep their stable schema-bound identities
//!   (`parse.raw_tei.v1`, `parse.project_article.v1`);
//! - cache integrity failures are closed typed ERRORS (never silent
//!   misses) with strict schema versions;
//! - [`ParseBatchLimits`] rejects zero bounds and foreign knobs (timeout,
//!   roots, redb settings have no field and cannot be expressed);
//! - bounds, cache mode and locations are excluded from the ordered input
//!   fingerprint; content (order, bytes, paper id, full effective dialect)
//!   is what the fingerprint binds;
//! - admission fails closed on empty/duplicate/oversized/empty-id inputs
//!   and on a cursor of another ordered input fingerprint — batch-wide
//!   `Err` exists only at admission;
//! - the schema-bound cursor rejects foreign versions, foreign fields and
//!   out-of-range ordinals;
//! - exactly one typed terminal outcome per admitted ordinal, round-trip
//!   stable, preserving the closed backend failure metadata verbatim
//!   (status / phase / bounded retry-after — never a `ParserError`
//!   collapse);
//! - the batch wire forms carry no location, tuning or retry material.
//!
//! No network, no GROBID, no graph store, no wall-clock inputs; all
//! fixtures are inline values.

use std::num::NonZeroUsize;

use da_parse::{
    BatchCursor, CacheIntegrityCode, CacheMode, CacheStageDisposition, FailureCode,
    FulltextDialectV1, HeaderDialectV1, PARSE_BATCH_CURSOR_SCHEMA_VERSION,
    PARSE_BATCH_SCHEMA_VERSION, PARSE_CACHE_DISPOSITION_SCHEMA_VERSION,
    PARSE_CACHE_FAILURE_SCHEMA_VERSION, PROJECT_ARTICLE_STAGE_ID, ParseBackend,
    ParseBackendFailure, ParseBackendRequest, ParseBatch, ParseBatchAdmissionError,
    ParseBatchCursorError, ParseBatchItem, ParseBatchLimits, ParseCacheDispositions,
    ParseCacheFailure, ParseCacheStage, ParseDocumentFailure, ParseDocumentOutcome,
    ParseDocumentSuccess, RAW_TEI_STAGE_ID, ReferencesDialectV1, RetryAfter,
    ordered_input_fingerprint,
};
use da_ports::parser::ParsedArticle;
use serde_json::{Value, json};

// ───────────────────────────── fixtures ───────────────────────────────

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("test bounds are nonzero")
}

fn pdf(seed: u8) -> Vec<u8> {
    let mut bytes = b"%PDF-1.7 batch-contract-fixture".to_vec();
    bytes.push(seed);
    bytes
}

fn request(paper_id: &str, seed: u8) -> ParseBackendRequest {
    ParseBackendRequest::new(paper_id, pdf(seed))
}

fn limits() -> ParseBatchLimits {
    ParseBatchLimits::new(nz(2), nz(4), nz(2), nz(16))
}

fn ordered_items(requests: Vec<ParseBackendRequest>) -> Vec<ParseBatchItem> {
    requests
        .into_iter()
        .enumerate()
        .map(|(ordinal, request)| ParseBatchItem::new(ordinal as u32, request))
        .collect()
}

fn fixture_article(paper_id: &str) -> ParsedArticle {
    ParsedArticle {
        paper_id: paper_id.to_string(),
        title: "Hub bibliography contract fixture".to_string(),
        abstract_text: "fixture abstract".to_string(),
        body_text: String::new(),
        sections: Vec::new(),
        citations: Vec::new(),
        layout_json: None,
        tei_xml: None,
        pdf_hash: "fixture-hash".to_string(),
        tables: Vec::new(),
        table_missingness: None,
        formulas: Vec::new(),
        formula_missingness: None,
        page_text_missingness: None,
    }
}

// ───────────────────── cache mode: closed vocabulary ──────────────────

#[test]
fn cache_mode_wire_vocabulary_is_closed() {
    let wire = [
        (CacheMode::Off, "off"),
        (CacheMode::ReadOnly, "read-only"),
        (CacheMode::WriteThrough, "write-through"),
        (CacheMode::Refresh, "refresh"),
    ];
    for (mode, name) in wire {
        assert_eq!(mode.as_str(), name, "as_str must equal the wire form");
        assert_eq!(
            serde_json::to_string(&mode).unwrap(),
            format!("\"{name}\""),
            "{name}: wire form drifted"
        );
        let back: CacheMode = serde_json::from_str(&format!("\"{name}\"")).unwrap();
        assert_eq!(back, mode, "{name}: roundtrip must be exact");
        assert_eq!(back.to_string(), name);
    }
    for foreign in [
        "\"write-back\"",
        "\"OFF\"",
        "\"readonly\"",
        "\"\"",
        "\"2\"",
        "\"refresh_all\"",
    ] {
        let error = serde_json::from_str::<CacheMode>(foreign)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("unknown variant"),
            "foreign mode {foreign} must be rejected as unknown variant, got: {error}"
        );
    }
}

#[test]
fn cache_mode_truth_table_is_pinned() {
    let expected = [
        // (mode, reads_stages, publishes_results, bypasses_hits)
        (CacheMode::Off, false, false, false),
        (CacheMode::ReadOnly, true, false, false),
        (CacheMode::WriteThrough, true, true, false),
        (CacheMode::Refresh, false, true, true),
    ];
    for (mode, reads, publishes, bypass) in expected {
        assert_eq!(mode.reads_stages(), reads, "{mode}: reads_stages drifted");
        assert_eq!(
            mode.publishes_results(),
            publishes,
            "{mode}: publishes_results drifted"
        );
        assert_eq!(
            mode.bypasses_hits(),
            bypass,
            "{mode}: bypasses_hits drifted"
        );
    }
}

#[test]
fn stage_dispositions_cover_the_full_mode_truth_table() {
    use CacheStageDisposition::Hit;
    use CacheStageDisposition::{ComputedPublished, ComputedUncached, ComputedUnpublished};
    let expected = [
        // (mode, disposition on a miss, disposition on a hit)
        (CacheMode::Off, ComputedUncached, ComputedUncached),
        (CacheMode::ReadOnly, ComputedUnpublished, Hit),
        (CacheMode::WriteThrough, ComputedPublished, Hit),
        (CacheMode::Refresh, ComputedPublished, ComputedPublished),
    ];
    for (mode, miss, hit) in expected {
        assert_eq!(
            mode.stage_disposition_for(false),
            miss,
            "{mode}: miss disposition drifted"
        );
        assert_eq!(
            mode.stage_disposition_for(true),
            hit,
            "{mode}: hit disposition drifted"
        );
    }
    for disposition in [
        Hit,
        ComputedPublished,
        ComputedUnpublished,
        ComputedUncached,
    ] {
        let wire = serde_json::to_string(&disposition).unwrap();
        assert_eq!(wire, format!("\"{}\"", disposition.as_str()));
        assert_eq!(
            serde_json::from_str::<CacheStageDisposition>(&wire).unwrap(),
            disposition
        );
    }
    let error = serde_json::from_str::<CacheStageDisposition>("\"revalidated\"")
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown variant"), "{error}");
}

#[test]
fn cache_dispositions_are_strictly_versioned() {
    let dispositions = ParseCacheDispositions::new(
        CacheStageDisposition::Hit,
        CacheStageDisposition::ComputedUnpublished,
    );
    let wire = serde_json::to_value(dispositions).unwrap();
    assert_eq!(
        wire,
        json!({
            "schema_version": PARSE_CACHE_DISPOSITION_SCHEMA_VERSION,
            "raw": "hit",
            "projector": "computed_unpublished",
        }),
        "disposition wire shape drifted"
    );
    assert_eq!(
        serde_json::from_value::<ParseCacheDispositions>(wire.clone()).unwrap(),
        dispositions
    );
    let mut foreign = wire.clone();
    foreign["schema_version"] = json!(PARSE_CACHE_DISPOSITION_SCHEMA_VERSION + 1);
    let error = serde_json::from_value::<ParseCacheDispositions>(foreign)
        .unwrap_err()
        .to_string();
    assert!(error.contains("schema version"), "{error}");
    let mut extra = wire;
    extra["storage_root"] = json!("never-expressed");
    assert!(
        serde_json::from_value::<ParseCacheDispositions>(extra).is_err(),
        "unknown fields must be rejected"
    );
}

// ─────────────────── cache stages: stable identities ──────────────────

#[test]
fn cache_stage_identities_are_stable_and_closed() {
    assert_eq!(RAW_TEI_STAGE_ID, "parse.raw_tei.v1");
    assert_eq!(PROJECT_ARTICLE_STAGE_ID, "parse.project_article.v1");
    assert_eq!(ParseCacheStage::RawTei.stage_id(), "parse.raw_tei.v1");
    assert_eq!(
        ParseCacheStage::ProjectArticle.stage_id(),
        "parse.project_article.v1"
    );
    for (stage, wire) in [
        (ParseCacheStage::RawTei, "\"raw_tei\""),
        (ParseCacheStage::ProjectArticle, "\"project_article\""),
    ] {
        assert_eq!(serde_json::to_string(&stage).unwrap(), wire);
        assert_eq!(
            serde_json::from_str::<ParseCacheStage>(wire).unwrap(),
            stage
        );
    }
    assert!(serde_json::from_str::<ParseCacheStage>("\"layout\"").is_err());
}

#[test]
fn cache_integrity_failures_are_typed_errors_not_misses() {
    let codes = [
        (CacheIntegrityCode::ChecksumMismatch, "checksum_mismatch"),
        (CacheIntegrityCode::ManifestCorrupt, "manifest_corrupt"),
        (CacheIntegrityCode::IdentityMismatch, "identity_mismatch"),
        (CacheIntegrityCode::PayloadUnreadable, "payload_unreadable"),
        (CacheIntegrityCode::StageWriteFailed, "stage_write_failed"),
    ];
    for (code, name) in codes {
        assert_eq!(code.as_str(), name);
        let failure = ParseCacheFailure::new(ParseCacheStage::RawTei, code);
        let wire = serde_json::to_value(failure).unwrap();
        assert_eq!(
            wire["schema_version"],
            json!(PARSE_CACHE_FAILURE_SCHEMA_VERSION)
        );
        assert_eq!(wire["stage"], json!("raw_tei"));
        assert_eq!(wire["code"], json!(name));
        let restored: ParseCacheFailure = serde_json::from_value(wire).unwrap();
        assert_eq!(restored, failure);
        assert_eq!(restored.code(), code);
        assert_eq!(restored.stage(), ParseCacheStage::RawTei);
    }
    // foreign schema version fails closed
    let mut foreign = serde_json::to_value(ParseCacheFailure::new(
        ParseCacheStage::ProjectArticle,
        CacheIntegrityCode::ManifestCorrupt,
    ))
    .unwrap();
    foreign["schema_version"] = json!(PARSE_CACHE_FAILURE_SCHEMA_VERSION + 1);
    let error = serde_json::from_value::<ParseCacheFailure>(foreign)
        .unwrap_err()
        .to_string();
    assert!(error.contains("schema version"), "{error}");
    // foreign code and field are rejected
    assert!(
        serde_json::from_value::<ParseCacheFailure>(json!({
            "schema_version": PARSE_CACHE_FAILURE_SCHEMA_VERSION,
            "stage": "raw_tei",
            "code": "silently_ignored",
        }))
        .is_err(),
        "unknown integrity codes must be rejected"
    );
    assert!(
        serde_json::from_value::<ParseCacheFailure>(json!({
            "schema_version": PARSE_CACHE_FAILURE_SCHEMA_VERSION,
            "stage": "raw_tei",
            "code": "checksum_mismatch",
            "payload": "raw bytes must have nowhere to live",
        }))
        .is_err(),
        "payload material must have no field"
    );
}

// ──────────────────── bounded limits: zero + tuning ───────────────────

#[test]
fn batch_limits_reject_zero_bounds_and_foreign_knobs() {
    for field in [
        "workers",
        "queue_capacity",
        "max_in_flight",
        "max_documents",
    ] {
        let mut wire = serde_json::to_value(limits()).unwrap();
        wire[field] = json!(0);
        let error = serde_json::from_value::<ParseBatchLimits>(wire)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("nonzero") || error.contains("non-zero"),
            "`{field}`=0 must be a type-level rejection, got: {error}"
        );
    }
    let wire = serde_json::to_value(limits()).unwrap();
    assert_eq!(
        wire,
        json!({
            "schema_version": PARSE_BATCH_SCHEMA_VERSION,
            "workers": 2,
            "queue_capacity": 4,
            "max_in_flight": 2,
            "max_documents": 16,
        }),
        "limits wire shape drifted (only the four bounds + schema version may exist)"
    );
    // Foreign schema versions fail closed.
    let mut foreign_version = serde_json::to_value(limits()).unwrap();
    foreign_version["schema_version"] = json!(PARSE_BATCH_SCHEMA_VERSION + 1);
    let version_error = serde_json::from_value::<ParseBatchLimits>(foreign_version)
        .unwrap_err()
        .to_string();
    assert!(
        version_error.contains("schema version"),
        "foreign limits schema version must be rejected, got: {version_error}"
    );
    // Transport tuning, roots and accelerator settings cannot even be
    // expressed: every foreign field is rejected.
    let mut timeout = serde_json::to_value(limits()).unwrap();
    timeout["timeout_secs"] = json!(30);
    assert!(serde_json::from_value::<ParseBatchLimits>(timeout).is_err());
    let mut redb = serde_json::to_value(limits()).unwrap();
    redb["redb_path"] = json!("accelerator.redb");
    assert!(serde_json::from_value::<ParseBatchLimits>(redb).is_err());
    let mut root = serde_json::to_value(limits()).unwrap();
    root["cache_dir"] = json!("somewhere");
    assert!(serde_json::from_value::<ParseBatchLimits>(root).is_err());
}

#[test]
fn bounds_and_modes_are_excluded_from_content_identity() {
    let requests = || vec![request("2507.19457", 1), request("2512.24601", 2)];
    let tight = ParseBatchLimits::new(nz(1), nz(1), nz(1), nz(2));
    let loose = ParseBatchLimits::new(nz(8), nz(64), nz(8), nz(2));
    let a = ParseBatch::admit(CacheMode::WriteThrough, tight, requests(), None).unwrap();
    let b = ParseBatch::admit(CacheMode::WriteThrough, loose, requests(), None).unwrap();
    assert_eq!(
        a.input_fingerprint(),
        b.input_fingerprint(),
        "bounds must never enter the input fingerprint"
    );
    // The same cursor lineage validates across differently bounded runs.
    let cursor = BatchCursor::at(a.input_fingerprint(), 1);
    assert!(
        cursor
            .validate_for(b.input_fingerprint(), b.total_documents())
            .is_ok()
    );
    // The cache mode is cost strategy, not content: same identity.
    let c = ParseBatch::admit(CacheMode::Refresh, tight, requests(), None).unwrap();
    assert_eq!(a.input_fingerprint(), c.input_fingerprint());
}

// ─────────────── ordered input fingerprint: content bound ─────────────

#[test]
fn input_fingerprint_is_stable_order_sensitive_and_content_bound() {
    let fingerprint = ordered_input_fingerprint(&ordered_items(vec![
        request("2507.19457", 1),
        request("2512.24601", 2),
    ]));
    assert_eq!(fingerprint.len(), 64, "sha256 hex: {fingerprint}");
    assert!(
        fingerprint
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "lowercase hex only: {fingerprint}"
    );
    assert_eq!(
        fingerprint,
        ordered_input_fingerprint(&ordered_items(vec![
            request("2507.19457", 1),
            request("2512.24601", 2),
        ])),
        "same ordered content must reproduce the fingerprint"
    );

    let reordered = ordered_input_fingerprint(&ordered_items(vec![
        request("2512.24601", 2),
        request("2507.19457", 1),
    ]));
    assert_ne!(fingerprint, reordered, "order is content");

    let byte_changed = ordered_input_fingerprint(&ordered_items(vec![
        request("2507.19457", 9),
        request("2512.24601", 2),
    ]));
    assert_ne!(fingerprint, byte_changed, "source bytes are content");

    let id_changed = ordered_input_fingerprint(&ordered_items(vec![
        request("2507.19458", 1),
        request("2512.24601", 2),
    ]));
    assert_ne!(fingerprint, id_changed, "paper ids are content");

    // The rich and structural legacy presets share NO extended canonical
    // identity, but they are different effective dialects: the fingerprint
    // must separate them.
    let structural =
        ordered_input_fingerprint(&ordered_items(vec![ParseBackendRequest::with_dialect(
            "2507.19457",
            pdf(1),
            FulltextDialectV1::legacy_structural(),
        )]));
    let rich = ordered_input_fingerprint(&ordered_items(vec![ParseBackendRequest::with_dialect(
        "2507.19457",
        pdf(1),
        FulltextDialectV1::legacy_rich(),
    )]));
    assert_ne!(rich, structural, "legacy presets are distinct content");
    let extended =
        ordered_input_fingerprint(&ordered_items(vec![ParseBackendRequest::with_dialect(
            "2507.19457",
            pdf(1),
            FulltextDialectV1::legacy_rich().with_raw_citations(true),
        )]));
    assert_ne!(rich, extended, "extended dialects are distinct content");

    // Endpoint changes are content changes even for identical ids/bytes.
    let header = ordered_input_fingerprint(&ordered_items(vec![ParseBackendRequest::header(
        "2507.19457",
        pdf(1),
        HeaderDialectV1::legacy_rich(),
    )]));
    assert_ne!(rich, header, "endpoint is content");
    let references =
        ordered_input_fingerprint(&ordered_items(vec![ParseBackendRequest::references(
            "2507.19457",
            pdf(1),
            ReferencesDialectV1::legacy_rich(),
        )]));
    assert_ne!(rich, references);
    assert_ne!(header, references);
}

// ───────────────────── admission: the only Err gate ───────────────────

#[test]
fn admission_fails_closed_on_invalid_config_before_any_work() {
    assert!(
        matches!(
            ParseBatch::admit(CacheMode::Off, limits(), vec![], None),
            Err(ParseBatchAdmissionError::EmptyBatch)
        ),
        "an empty run is a config error, not an empty success"
    );
    match ParseBatch::admit(
        CacheMode::Off,
        limits(),
        vec![request("a", 1), request("", 2)],
        None,
    ) {
        Err(ParseBatchAdmissionError::EmptyPaperId { index }) => assert_eq!(index, 1),
        other => panic!("empty paper id must be rejected, got {other:?}"),
    }
    match ParseBatch::admit(
        CacheMode::Off,
        limits(),
        vec![request("dup", 1), request("dup", 2)],
        None,
    ) {
        Err(ParseBatchAdmissionError::DuplicatePaperId { paper_id }) => {
            assert_eq!(paper_id, "dup")
        }
        other => panic!("duplicate paper id must be rejected, got {other:?}"),
    }
    match ParseBatch::admit(
        CacheMode::Off,
        ParseBatchLimits::new(nz(1), nz(1), nz(1), nz(1)),
        vec![request("a", 1), request("b", 2)],
        None,
    ) {
        Err(ParseBatchAdmissionError::DocumentLimitExceeded {
            requested,
            max_documents,
        }) => {
            assert_eq!(requested, 2);
            assert_eq!(max_documents, 1);
        }
        other => panic!("document bound must be enforced, got {other:?}"),
    }
}

#[test]
fn cursor_of_another_input_fingerprint_is_rejected_before_work() {
    let inputs = || vec![request("2507.19457", 1), request("2512.24601", 2)];
    let batch_a = ParseBatch::admit(CacheMode::WriteThrough, limits(), inputs(), None).unwrap();
    let batch_b = ParseBatch::admit(
        CacheMode::WriteThrough,
        limits(),
        vec![request("2507.19457", 3)],
        None,
    )
    .unwrap();
    assert_ne!(
        batch_a.input_fingerprint(),
        batch_b.input_fingerprint(),
        "different content must produce different fingerprints"
    );

    // A fresh admission starts bound to its own fingerprint at ordinal 0.
    assert_eq!(batch_a.cursor().next_ordinal(), 0);
    assert_eq!(
        batch_a.cursor().input_fingerprint(),
        batch_a.input_fingerprint()
    );

    // Valid mid-run resume: the cursor is adopted verbatim.
    let cursor = BatchCursor::at(batch_a.input_fingerprint(), 1);
    let resumed = ParseBatch::admit(
        CacheMode::WriteThrough,
        limits(),
        inputs(),
        Some(cursor.clone()),
    )
    .unwrap();
    assert_eq!(resumed.cursor(), &cursor);
    assert_eq!(resumed.cursor().next_ordinal(), 1);

    // Foreign fingerprint fails closed (never a silent restart-from-zero).
    let foreign_cursor = BatchCursor::at(batch_a.input_fingerprint(), 1);
    match ParseBatch::admit(
        CacheMode::WriteThrough,
        limits(),
        vec![request("2507.19457", 3)],
        Some(foreign_cursor),
    ) {
        Err(ParseBatchAdmissionError::InvalidCursor(
            ParseBatchCursorError::FingerprintMismatch {
                cursor_fingerprint,
                batch_fingerprint,
            },
        )) => {
            assert_eq!(cursor_fingerprint, batch_a.input_fingerprint());
            assert_eq!(batch_fingerprint, batch_b.input_fingerprint());
        }
        other => panic!("foreign-fingerprint cursor must fail closed, got {other:?}"),
    }

    // Out-of-range ordinals fail closed.
    let beyond = BatchCursor::at(batch_a.input_fingerprint(), 3);
    match beyond.validate_for(batch_a.input_fingerprint(), batch_a.total_documents()) {
        Err(ParseBatchCursorError::OrdinalBeyondInput {
            next_ordinal,
            total_documents,
        }) => {
            assert_eq!(next_ordinal, 3);
            assert_eq!(total_documents, 2);
        }
        other => panic!("out-of-range ordinal must fail closed, got {other:?}"),
    }
}

#[test]
fn cursor_is_strictly_schema_bound() {
    let cursor = BatchCursor::at("fingerprint-hex", 2);
    let wire = serde_json::to_value(&cursor).unwrap();
    assert_eq!(
        wire,
        json!({
            "schema_version": PARSE_BATCH_CURSOR_SCHEMA_VERSION,
            "input_fingerprint": "fingerprint-hex",
            "next_ordinal": 2,
        }),
        "cursor wire shape drifted"
    );
    assert_eq!(
        serde_json::from_value::<BatchCursor>(wire.clone()).unwrap(),
        cursor
    );
    let mut foreign = wire.clone();
    foreign["schema_version"] = json!(PARSE_BATCH_CURSOR_SCHEMA_VERSION + 1);
    let error = serde_json::from_value::<BatchCursor>(foreign)
        .unwrap_err()
        .to_string();
    assert!(error.contains("schema version"), "{error}");
    let mut extra = wire;
    extra["checkpoint_root"] = json!("never-expressed");
    assert!(serde_json::from_value::<BatchCursor>(extra).is_err());
}

// ─────────────── terminal outcomes: one per admission ─────────────────

#[test]
fn success_outcome_roundtrips_and_preserves_result_identity() {
    let outcome = ParseDocumentOutcome::new(
        0,
        "2507.19457".to_string(),
        CacheMode::ReadOnly,
        ParseCacheDispositions::new(
            CacheStageDisposition::ComputedUncached,
            CacheStageDisposition::ComputedUncached,
        ),
        Ok(ParseDocumentSuccess::new(
            fixture_article("2507.19457"),
            "endpoint=fulltext;schema=v1;legacy-equivalent",
        )),
    );
    let wire = serde_json::to_value(&outcome).unwrap();
    assert_eq!(wire["schema_version"], json!(PARSE_BATCH_SCHEMA_VERSION));
    assert_eq!(wire["ordinal"], json!(0));
    assert_eq!(wire["paper_id"], json!("2507.19457"));
    assert_eq!(wire["cache_mode"], json!("read-only"));
    assert_eq!(wire["dispositions"]["raw"], json!("computed_uncached"));
    assert_eq!(
        wire["dispositions"]["projector"],
        json!("computed_uncached")
    );
    let restored: ParseDocumentOutcome = serde_json::from_value(wire).unwrap();
    assert!(restored.is_success());
    let success = match restored.result() {
        Ok(success) => success,
        other => panic!("expected success, got {other:?}"),
    };
    assert_eq!(
        success.result_identity(),
        "endpoint=fulltext;schema=v1;legacy-equivalent"
    );
    // Semantic payload survived the roundtrip byte-stably (wire equality).
    let original_wire = serde_json::to_value(&outcome).unwrap();
    let restored_wire = serde_json::to_value(&restored).unwrap();
    assert_eq!(
        original_wire["result"], restored_wire["result"],
        "article payload must round-trip exactly"
    );
}

#[test]
fn failure_outcome_preserves_closed_backend_metadata_verbatim() {
    let failure = ParseBackendFailure::busy(503, Some(RetryAfter::Seconds(30)));
    let outcome = ParseDocumentOutcome::new(
        7,
        "2507.19457".to_string(),
        CacheMode::WriteThrough,
        ParseCacheDispositions::new(
            CacheStageDisposition::ComputedPublished,
            CacheStageDisposition::ComputedUncached,
        ),
        Err(ParseDocumentFailure::Backend(failure.clone())),
    );
    let wire = serde_json::to_value(&outcome).unwrap();
    assert_eq!(
        wire["result"],
        json!({"Err": {"backend": {
            "schema_version": 1,
            "code": "busy",
            "phase": "await_response",
            "status": 503,
            "retry_after": {"seconds": 30},
            "limit": null,
        }}}),
        "the closed backend failure metadata must ride verbatim"
    );
    let restored: ParseDocumentOutcome = serde_json::from_value(wire).unwrap();
    assert!(!restored.is_success());
    match restored.result() {
        Err(ParseDocumentFailure::Backend(restored_failure)) => {
            assert_eq!(restored_failure, &failure, "metadata must survive verbatim");
            assert_eq!(restored_failure.code(), FailureCode::Busy);
            assert_eq!(restored_failure.status(), Some(503));
            assert_eq!(
                restored_failure.retry_after(),
                Some(RetryAfter::Seconds(30))
            );
        }
        other => panic!("expected a backend failure, got {other:?}"),
    }
}

#[test]
fn failure_outcome_carries_typed_cache_integrity_errors() {
    for (stage, code) in [
        (
            ParseCacheStage::RawTei,
            CacheIntegrityCode::ChecksumMismatch,
        ),
        (
            ParseCacheStage::ProjectArticle,
            CacheIntegrityCode::ManifestCorrupt,
        ),
    ] {
        let outcome = ParseDocumentOutcome::new(
            1,
            "2512.24601".to_string(),
            CacheMode::WriteThrough,
            ParseCacheDispositions::new(
                CacheStageDisposition::ComputedUncached,
                CacheStageDisposition::ComputedUncached,
            ),
            Err(ParseDocumentFailure::Cache(ParseCacheFailure::new(
                stage, code,
            ))),
        );
        let wire = serde_json::to_value(&outcome).unwrap();
        let restored: ParseDocumentOutcome = serde_json::from_value(wire).unwrap();
        match restored.result() {
            Err(ParseDocumentFailure::Cache(restored_failure)) => {
                assert_eq!(restored_failure.stage(), stage);
                assert_eq!(restored_failure.code(), code);
            }
            other => panic!("expected a cache failure, got {other:?}"),
        }
        assert!(!restored.is_success());
    }
}

#[test]
fn outcome_is_strictly_versioned_and_shape_bound() {
    let base = json!({
        "schema_version": PARSE_BATCH_SCHEMA_VERSION,
        "ordinal": 0,
        "paper_id": "2507.19457",
        "cache_mode": "off",
        "dispositions": {
            "schema_version": PARSE_CACHE_DISPOSITION_SCHEMA_VERSION,
            "raw": "computed_uncached",
            "projector": "computed_uncached",
        },
        "result": {"Ok": {
            "article": {
                "paper_id": "2507.19457",
                "title": "t",
                "abstract_text": "",
                "body_text": "",
                "sections": [],
                "citations": [],
                "layout_json": null,
                "tei_xml": null,
                "pdf_hash": "h",
            },
            "result_identity": "endpoint=fulltext;schema=v1;legacy-equivalent",
        }},
    });
    assert!(
        serde_json::from_value::<ParseDocumentOutcome>(base.clone()).is_ok(),
        "the documented wire form must deserialize"
    );
    let mut foreign_version = base.clone();
    foreign_version["schema_version"] = json!(PARSE_BATCH_SCHEMA_VERSION + 1);
    assert!(
        serde_json::from_value::<ParseDocumentOutcome>(foreign_version).is_err(),
        "foreign outcome schema versions must be rejected"
    );
    let mut extra_field = base;
    extra_field["attempts"] = json!(2);
    let error = serde_json::from_value::<ParseDocumentOutcome>(extra_field)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("unknown field"),
        "a retry counter has no field and must be rejected: {error}"
    );
}

// ─────────────── admitted items: identity passthrough ─────────────────

#[test]
fn admitted_items_preserve_endpoint_and_result_dialect_identity() {
    let fulltext = request("2507.19457", 1);
    let header = ParseBackendRequest::header("2512.24601", pdf(2), HeaderDialectV1::legacy_rich());
    let references = ParseBackendRequest::references(
        "2605.26525",
        pdf(3),
        ReferencesDialectV1::legacy_structural(),
    );
    let expected_identities = [
        fulltext.result_identity().canonical(),
        header.result_identity().canonical(),
        references.result_identity().canonical(),
    ];
    let expected_endpoints = [
        fulltext.endpoint(),
        header.endpoint(),
        references.endpoint(),
    ];
    let batch = ParseBatch::admit(
        CacheMode::ReadOnly,
        limits(),
        vec![fulltext, header, references],
        None,
    )
    .unwrap();
    assert_eq!(batch.total_documents(), 3);
    assert_eq!(batch.mode(), CacheMode::ReadOnly);
    for (position, item) in batch.items().iter().enumerate() {
        assert_eq!(item.ordinal(), position as u32, "ordinals are stable 0..n");
        assert_eq!(
            item.result_identity().canonical(),
            expected_identities[position],
            "result dialect identity must pass through unmodified"
        );
        assert_eq!(item.request().endpoint(), expected_endpoints[position]);
    }
    // And a success outcome stamps exactly the same canonical identity.
    let outcome = ParseDocumentOutcome::new(
        1,
        "2512.24601".to_string(),
        CacheMode::ReadOnly,
        ParseCacheDispositions::new(
            CacheStageDisposition::Hit,
            CacheStageDisposition::ComputedUnpublished,
        ),
        Ok(ParseDocumentSuccess::new(
            fixture_article("2512.24601"),
            expected_identities[1].clone(),
        )),
    );
    match outcome.result() {
        Ok(success) => {
            assert_eq!(success.result_identity(), expected_identities[1]);
        }
        other => panic!("expected success, got {other:?}"),
    }
}

// ─────────────── wire hygiene: no locations/tuning/retry ──────────────

#[test]
fn batch_wire_forms_carry_no_location_tuning_or_retry_material() {
    let success_outcome = ParseDocumentOutcome::new(
        0,
        "2507.19457".to_string(),
        CacheMode::Refresh,
        ParseCacheDispositions::new(
            CacheStageDisposition::ComputedPublished,
            CacheStageDisposition::ComputedPublished,
        ),
        Ok(ParseDocumentSuccess::new(
            fixture_article("2507.19457"),
            "endpoint=fulltext;schema=v1;legacy-equivalent",
        )),
    );
    // Success-path surfaces only: the Err case legitimately carries the
    // bounded `retry_after` metadata of the closed backend failure (that
    // metadata is provider-advertised state, not a retry surface).
    let surfaces: Vec<String> = vec![
        serde_json::to_string(&CacheMode::Refresh).unwrap(),
        serde_json::to_string(&limits()).unwrap(),
        serde_json::to_string(&BatchCursor::at("f".repeat(64), 3)).unwrap(),
        serde_json::to_string(&ParseCacheDispositions::new(
            CacheStageDisposition::Hit,
            CacheStageDisposition::ComputedUnpublished,
        ))
        .unwrap(),
        serde_json::to_string(&ParseCacheFailure::new(
            ParseCacheStage::RawTei,
            CacheIntegrityCode::IdentityMismatch,
        ))
        .unwrap(),
        serde_json::to_string(&success_outcome).unwrap(),
    ];
    let forbidden = [
        "path",
        "url",
        "http",
        "root",
        "env",
        "dir",
        "retry",
        "backoff",
        "attempt",
        "timeout",
        "queue_wait",
        "redb",
    ];
    for surface in &surfaces {
        for marker in forbidden {
            assert!(
                !surface.contains(marker),
                "batch wire form leaked `{marker}`: {surface}"
            );
        }
    }
    // And the success wire carries no slash-separated locations either.
    let _unused: Value = serde_json::from_str(surfaces.last().unwrap()).unwrap();
}

// ═══════════════════════════════════════════════════════════════════════
// S06/T02: bounded two-stage executor over stateful fakes
// ═══════════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::future::pending;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::time::Duration;

use async_trait::async_trait;
use da_parse::{
    BatchCheckpoint, PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION, PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    ParseArtifactCache, ParseBatchExecutor, ParseBatchMemoryCheckpoint, ParseBatchResultSink,
    ParseBatchRunSummary, ParseBatchSinkError, ProjectArticlePayload, RawStageRead,
    RawTeiEnvelopePayload, TeiEnvelope, TeiIdentity, project_article_stage_key, raw_tei_stage_key,
};

const RAW_KEY_PREFIX: &str = "parse.raw_tei.v1:";
const PROJECTOR_KEY_PREFIX: &str = "parse.project_article.v1:";

// ───────────────────────── stateful fakes ─────────────────────────

#[derive(Default)]
struct BackendState {
    calls: std::sync::Mutex<Vec<String>>,
    counts: std::sync::Mutex<HashMap<String, usize>>,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
}

#[derive(Clone)]
struct FakeBackend {
    state: Arc<BackendState>,
    tei: Arc<HashMap<String, String>>,
    failures: Arc<HashMap<String, ParseBackendFailure>>,
    delay: Duration,
}

impl FakeBackend {
    fn new() -> Self {
        Self {
            state: Arc::default(),
            tei: Arc::default(),
            failures: Arc::default(),
            delay: Duration::ZERO,
        }
    }

    fn with_tei(mut self, paper: &str, tei: &str) -> Self {
        let mut map = HashMap::clone(&self.tei);
        map.insert(paper.to_string(), tei.to_string());
        self.tei = Arc::new(map);
        self
    }

    fn with_busy(mut self, paper: &str, status: u16, seconds: u64) -> Self {
        let mut failures = HashMap::clone(&self.failures);
        failures.insert(
            paper.to_string(),
            ParseBackendFailure::busy(status, Some(RetryAfter::Seconds(seconds))),
        );
        self.failures = Arc::new(failures);
        self
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    fn call_count(&self, paper: &str) -> usize {
        self.state
            .counts
            .lock()
            .unwrap()
            .get(paper)
            .copied()
            .unwrap_or(0)
    }
}

#[async_trait]
impl ParseBackend for FakeBackend {
    fn default_request(&self, paper_id: &str, pdf_bytes: Vec<u8>) -> ParseBackendRequest {
        ParseBackendRequest::new(paper_id, pdf_bytes)
    }

    async fn fetch_tei(
        &self,
        request: ParseBackendRequest,
    ) -> Result<TeiEnvelope, ParseBackendFailure> {
        let paper = request.paper_id().to_string();
        self.state.calls.lock().unwrap().push(paper.clone());
        *self.state.counts.lock().unwrap().entry(paper).or_insert(0) += 1;
        let now = self.state.in_flight.fetch_add(1, AtomicOrdering::SeqCst) + 1;
        self.state
            .max_in_flight
            .fetch_max(now, AtomicOrdering::SeqCst);
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        self.state.in_flight.fetch_sub(1, AtomicOrdering::SeqCst);
        if let Some(failure) = self.failures.get(request.paper_id()) {
            return Err(failure.clone());
        }
        let tei = self
            .tei
            .get(request.paper_id())
            .cloned()
            .unwrap_or_else(|| format!("<TEI>body-for-{}</TEI>", request.paper_id()));
        let identity = TeiIdentity::new("fake-test-backend", "profile=fake;test=1");
        Ok(TeiEnvelope::new(
            tei,
            identity.with_result(request.result_identity()),
        ))
    }
}

#[derive(Default)]
struct CacheState {
    store: std::sync::Mutex<HashMap<String, Vec<u8>>>,
    calls: std::sync::Mutex<Vec<String>>,
    fail_projector_writes: AtomicBool,
    legacy_raw: AtomicBool,
}

#[derive(Clone, Default)]
struct FakeCache {
    state: Arc<CacheState>,
}

impl FakeCache {
    fn calls(&self) -> Vec<String> {
        self.state.calls.lock().unwrap().clone()
    }

    fn keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        self.state
            .store
            .lock()
            .unwrap()
            .keys()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect()
    }

    fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
        let mut entries: Vec<(String, Vec<u8>)> = self
            .state
            .store
            .lock()
            .unwrap()
            .iter()
            .map(|(key, payload)| (key.clone(), payload.clone()))
            .collect();
        entries.sort();
        entries
    }

    fn mutate_payload<F: Fn(&mut serde_json::Value)>(&self, prefix: &str, mutate: F) {
        let mut store = self.state.store.lock().unwrap();
        let key = store
            .keys()
            .find(|key| key.starts_with(prefix))
            .cloned()
            .expect("stored entry with prefix");
        let mut value: serde_json::Value =
            serde_json::from_slice(&store[&key]).expect("stored payload parses");
        mutate(&mut value);
        store.insert(key, serde_json::to_vec(&value).unwrap());
    }

    fn scramble_payload(&self, prefix: &str) {
        let mut store = self.state.store.lock().unwrap();
        let key = store
            .keys()
            .find(|key| key.starts_with(prefix))
            .cloned()
            .expect("stored entry with prefix");
        store.insert(key, b"{not json at all".to_vec());
    }
}

#[async_trait]
impl ParseArtifactCache for FakeCache {
    async fn read_raw(&self, key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        self.state
            .calls
            .lock()
            .unwrap()
            .push(format!("read_raw:{key}"));
        let store = self.state.store.lock().unwrap();
        Ok(store.get(key).map(|payload| {
            if self.state.legacy_raw.load(AtomicOrdering::SeqCst) {
                RawStageRead::from_legacy(payload.clone())
            } else {
                RawStageRead::canonical(payload.clone())
            }
        }))
    }

    async fn write_raw(&self, key: &str, payload: &[u8]) -> Result<(), ParseCacheFailure> {
        self.state
            .calls
            .lock()
            .unwrap()
            .push(format!("write_raw:{key}"));
        self.state
            .store
            .lock()
            .unwrap()
            .insert(key.to_string(), payload.to_vec());
        Ok(())
    }

    async fn read_projected(&self, key: &str) -> Result<Option<RawStageRead>, ParseCacheFailure> {
        self.state
            .calls
            .lock()
            .unwrap()
            .push(format!("read_projected:{key}"));
        let store = self.state.store.lock().unwrap();
        Ok(store
            .get(key)
            .map(|payload| RawStageRead::canonical(payload.clone())))
    }

    async fn write_projected(&self, key: &str, payload: &[u8]) -> Result<(), ParseCacheFailure> {
        self.state
            .calls
            .lock()
            .unwrap()
            .push(format!("write_projected:{key}"));
        if self
            .state
            .fail_projector_writes
            .load(AtomicOrdering::SeqCst)
        {
            return Err(ParseCacheFailure::new(
                ParseCacheStage::ProjectArticle,
                CacheIntegrityCode::StageWriteFailed,
            ));
        }
        self.state
            .store
            .lock()
            .unwrap()
            .insert(key.to_string(), payload.to_vec());
        Ok(())
    }
}

struct FakeSink {
    outcomes: std::sync::Mutex<Vec<ParseDocumentOutcome>>,
    fail_at: Option<u32>,
    park_at: Option<u32>,
}

impl FakeSink {
    fn new() -> Self {
        Self {
            outcomes: std::sync::Mutex::new(Vec::new()),
            fail_at: None,
            park_at: None,
        }
    }

    fn failing_at(ordinal: u32) -> Self {
        Self {
            fail_at: Some(ordinal),
            ..Self::new()
        }
    }

    fn parking_at(ordinal: u32) -> Self {
        Self {
            park_at: Some(ordinal),
            ..Self::new()
        }
    }

    fn recorded_ordinals(&self) -> Vec<u32> {
        let mut ordinals: Vec<u32> = self
            .outcomes
            .lock()
            .unwrap()
            .iter()
            .map(|outcome| outcome.ordinal())
            .collect();
        ordinals.sort();
        ordinals
    }

    fn recorded(&self) -> Vec<ParseDocumentOutcome> {
        let mut outcomes = self.outcomes.lock().unwrap().clone();
        outcomes.sort_by_key(|outcome| outcome.ordinal());
        outcomes
    }
}

#[async_trait]
impl ParseBatchResultSink for FakeSink {
    async fn record(&self, outcome: ParseDocumentOutcome) -> Result<(), ParseBatchSinkError> {
        if self.park_at == Some(outcome.ordinal()) {
            pending::<()>().await;
        }
        if self.fail_at == Some(outcome.ordinal()) {
            return Err(ParseBatchSinkError("injected sink failure".to_string()));
        }
        self.outcomes.lock().unwrap().push(outcome);
        Ok(())
    }
}

struct FakeCheckpoint {
    saved: std::sync::Mutex<Vec<BatchCursor>>,
    fail: bool,
}

impl FakeCheckpoint {
    fn new() -> Self {
        Self {
            saved: std::sync::Mutex::new(Vec::new()),
            fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            fail: true,
            ..Self::new()
        }
    }

    fn last_saved(&self) -> Option<BatchCursor> {
        self.saved.lock().unwrap().last().cloned()
    }
}

#[async_trait]
impl BatchCheckpoint for FakeCheckpoint {
    async fn save(&self, cursor: &BatchCursor) -> Result<(), ParseBatchSinkError> {
        if self.fail {
            return Err(ParseBatchSinkError(
                "injected checkpoint failure".to_string(),
            ));
        }
        self.saved.lock().unwrap().push(cursor.clone());
        Ok(())
    }
}

fn exec_limits(max_documents: usize) -> ParseBatchLimits {
    ParseBatchLimits::new(nz(2), nz(4), nz(2), nz(max_documents))
}

fn requests_for(ids: &[&str]) -> Vec<ParseBackendRequest> {
    ids.iter()
        .enumerate()
        .map(|(index, id)| request(id, index as u8 + 1))
        .collect()
}

fn admitted(mode: CacheMode, ids: &[&str]) -> ParseBatch {
    ParseBatch::admit(mode, exec_limits(64), requests_for(ids), None).expect("test batch admits")
}

// ───────────────────────── executor behavior ─────────────────────────

#[tokio::test(start_paused = true)]
async fn executor_off_mode_makes_zero_cache_calls() {
    let backend = FakeBackend::new();
    let backend_handle = backend.clone();
    let cache = FakeCache::default();
    let cache_handle = cache.clone();
    let executor = ParseBatchExecutor::new(backend, cache);
    let sink = Arc::new(FakeSink::new());
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let summary = executor
        .run(
            admitted(CacheMode::Off, &["doc-a", "doc-b"]),
            sink.clone(),
            checkpoint.clone(),
        )
        .await;
    assert_eq!(summary.completed(), 2);
    assert_eq!(summary.succeeded(), 2);
    assert_eq!(summary.failed(), 0);
    assert_eq!(summary.backend_calls(), 2);
    assert!(
        cache_handle.calls().is_empty(),
        "off mode never consults caches"
    );
    assert!(
        cache_handle.snapshot().is_empty(),
        "off mode never publishes"
    );
    assert_eq!(backend_handle.call_count("doc-a"), 1);
    let outcomes = sink.outcomes.lock().unwrap().clone();
    for outcome in outcomes {
        assert!(outcome.is_success());
        let dispositions = outcome.dispositions();
        assert_eq!(dispositions.raw(), CacheStageDisposition::ComputedUncached);
        assert_eq!(
            dispositions.projector(),
            CacheStageDisposition::ComputedUncached
        );
    }
    assert!(checkpoint.cursor().await.unwrap().next_ordinal() >= 2);
}

#[tokio::test(start_paused = true)]
async fn articles_are_byte_equal_across_all_four_modes() {
    let expected = [
        (
            CacheMode::Off,
            CacheStageDisposition::ComputedUncached,
            CacheStageDisposition::ComputedUncached,
        ),
        (
            CacheMode::ReadOnly,
            CacheStageDisposition::ComputedUnpublished,
            CacheStageDisposition::ComputedUnpublished,
        ),
        (
            CacheMode::WriteThrough,
            CacheStageDisposition::ComputedPublished,
            CacheStageDisposition::ComputedPublished,
        ),
        (
            CacheMode::Refresh,
            CacheStageDisposition::ComputedPublished,
            CacheStageDisposition::ComputedPublished,
        ),
    ];
    let mut baseline: Option<serde_json::Value> = None;
    for (mode, raw, projector) in expected {
        let executor = ParseBatchExecutor::new(FakeBackend::new(), FakeCache::default());
        let sink = Arc::new(FakeSink::new());
        let summary = executor
            .run(
                admitted(mode, &["mode-doc"]),
                sink.clone(),
                Arc::new(ParseBatchMemoryCheckpoint::new()),
            )
            .await;
        assert_eq!(summary.succeeded(), 1, "{mode}: fresh compute succeeds");
        let outcome = &sink.recorded()[0];
        assert!(outcome.is_success());
        assert_eq!(
            outcome.dispositions().raw(),
            raw,
            "{mode}: raw disposition drifted"
        );
        assert_eq!(
            outcome.dispositions().projector(),
            projector,
            "{mode}: projector disposition drifted"
        );
        let article_wire =
            serde_json::to_value(outcome.result().as_ref().unwrap().article()).unwrap();
        match &baseline {
            None => baseline = Some(article_wire),
            Some(baseline) => assert_eq!(
                &article_wire, baseline,
                "{mode}: semantic drift across modes"
            ),
        }
    }
}

#[tokio::test(start_paused = true)]
async fn write_through_miss_commits_raw_before_projector_with_exact_sequence() {
    let cache = FakeCache::default();
    let cache_handle = cache.clone();
    let executor = ParseBatchExecutor::new(FakeBackend::new(), cache);
    let sink = Arc::new(FakeSink::new());
    let summary = executor
        .run(
            admitted(CacheMode::WriteThrough, &["seq-doc"]),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary.succeeded(), 1);
    assert_eq!(summary.raw_writes(), 1);
    assert_eq!(summary.projector_writes(), 1);
    let calls = cache_handle.calls();
    let ops: Vec<&str> = calls
        .iter()
        .map(|call| call.split(':').next().unwrap())
        .collect();
    assert_eq!(
        ops,
        vec!["read_raw", "write_raw", "read_projected", "write_projected"],
        "write-through miss order drifted: {calls:?}"
    );
    assert!(calls[0].starts_with("read_raw:parse.raw_tei.v1:"));
    assert!(calls[2].starts_with("read_projected:parse.project_article.v1:"));
}

#[tokio::test(start_paused = true)]
async fn read_only_full_hit_avoids_backend_and_keeps_cache_byte_identical() {
    // Populate through write-through first.
    let cache = FakeCache::default();
    let cache_handle = cache.clone();
    let populate = ParseBatchExecutor::new(FakeBackend::new(), cache.clone());
    let first = populate
        .run(
            admitted(CacheMode::WriteThrough, &["hit-doc"]),
            Arc::new(FakeSink::new()),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(first.succeeded(), 1);
    let before = cache_handle.snapshot();

    // Read-only replay: zero backend calls, byte-identical cache, full hit.
    let replay = ParseBatchExecutor::new(FakeBackend::new(), cache);
    let sink = Arc::new(FakeSink::new());
    let summary = replay
        .run(
            admitted(CacheMode::ReadOnly, &["hit-doc"]),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(
        summary.backend_calls(),
        0,
        "a full hit must not call the backend"
    );
    assert_eq!(summary.raw_hits(), 1);
    assert_eq!(summary.projector_hits(), 1);
    assert_eq!(summary.raw_writes(), 0);
    assert_eq!(
        cache_handle.snapshot(),
        before,
        "read-only never mutates the cache"
    );
    let outcome = &sink.recorded()[0];
    assert_eq!(outcome.dispositions().raw(), CacheStageDisposition::Hit);
    assert_eq!(
        outcome.dispositions().projector(),
        CacheStageDisposition::Hit
    );
}

#[tokio::test(start_paused = true)]
async fn raw_hit_reprojects_after_projector_schema_rotation_without_backend() {
    let cache = FakeCache::default();
    let cache_handle = cache.clone();
    let populate = ParseBatchExecutor::new(FakeBackend::new(), cache.clone());
    let populate_sink = Arc::new(FakeSink::new());
    let first = populate
        .run(
            admitted(CacheMode::WriteThrough, &["rot-doc"]),
            populate_sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(first.succeeded(), 1);
    let first_wire = serde_json::to_value(
        populate_sink.recorded()[0]
            .result()
            .as_ref()
            .unwrap()
            .article(),
    )
    .unwrap();

    // A rotated projector fingerprint orphans projector entries; raw survives.
    let replay = ParseBatchExecutor::new(FakeBackend::new(), cache)
        .with_projector_schema_fingerprint("ParsedArticle@rotated-for-test");
    let sink = Arc::new(FakeSink::new());
    let summary = replay
        .run(
            admitted(CacheMode::ReadOnly, &["rot-doc"]),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(
        summary.backend_calls(),
        0,
        "raw hit must re-project without the backend"
    );
    assert_eq!(summary.raw_hits(), 1);
    assert_eq!(summary.projector_hits(), 0);
    assert_eq!(
        cache_handle.keys_with_prefix(PROJECTOR_KEY_PREFIX).len(),
        1,
        "old entries are orphaned, not deleted"
    );
    let outcome = &sink.recorded()[0];
    assert_eq!(outcome.dispositions().raw(), CacheStageDisposition::Hit);
    assert_eq!(
        outcome.dispositions().projector(),
        CacheStageDisposition::ComputedUnpublished
    );
    assert_eq!(
        serde_json::to_value(outcome.result().as_ref().unwrap().article()).unwrap(),
        first_wire,
        "re-projection is byte-equal"
    );
}

#[tokio::test(start_paused = true)]
async fn projector_write_failure_leaves_honest_reusable_raw_state() {
    let cache = FakeCache::default();
    cache
        .state
        .fail_projector_writes
        .store(true, AtomicOrdering::SeqCst);
    let cache_handle = cache.clone();
    let executor = ParseBatchExecutor::new(FakeBackend::new(), cache);
    let sink = Arc::new(FakeSink::new());
    let summary = executor
        .run(
            admitted(CacheMode::WriteThrough, &["projfail-doc"]),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(
        summary.succeeded(),
        1,
        "a failed publish never fails the document"
    );
    assert_eq!(summary.projector_write_failures(), 1);
    assert_eq!(
        summary.raw_writes(),
        1,
        "raw committed before the projector stage"
    );
    let outcome = &sink.recorded()[0];
    assert_eq!(
        outcome.dispositions().raw(),
        CacheStageDisposition::ComputedPublished
    );
    assert_eq!(
        outcome.dispositions().projector(),
        CacheStageDisposition::ComputedUnpublished
    );

    // The committed raw state is reusable: replay hits raw, re-projects.
    let replay = ParseBatchExecutor::new(FakeBackend::new(), cache_handle);
    let replay_sink = Arc::new(FakeSink::new());
    let replay_summary = replay
        .run(
            admitted(CacheMode::ReadOnly, &["projfail-doc"]),
            replay_sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(replay_summary.backend_calls(), 0);
    assert_eq!(replay_summary.raw_hits(), 1);
    let replay_outcome = &replay_sink.recorded()[0];
    assert!(replay_outcome.is_success());
    assert_eq!(
        replay_outcome.dispositions().raw(),
        CacheStageDisposition::Hit
    );
}

#[tokio::test(start_paused = true)]
async fn busy_503_is_typed_exactly_once_and_the_run_continues() {
    let backend = FakeBackend::new().with_busy("busy-doc", 503, 7);
    let backend_handle = backend.clone();
    let executor = ParseBatchExecutor::new(backend, FakeCache::default());
    let sink = Arc::new(FakeSink::new());
    let summary = executor
        .run(
            admitted(
                CacheMode::WriteThrough,
                &["ok-before", "busy-doc", "ok-after"],
            ),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary.completed(), 3);
    assert_eq!(summary.succeeded(), 2);
    assert_eq!(summary.failed(), 1);
    assert_eq!(
        backend_handle.call_count("busy-doc"),
        1,
        "a typed busy is exactly one backend invocation"
    );
    let outcomes = sink.recorded();
    let ordinals: Vec<u32> = outcomes.iter().map(|outcome| outcome.ordinal()).collect();
    assert_eq!(
        ordinals,
        vec![0, 1, 2],
        "later documents survive earlier failures"
    );
    match outcomes[1].result() {
        Err(ParseDocumentFailure::Backend(failure)) => {
            assert_eq!(failure.status(), Some(503));
            assert_eq!(failure.retry_after(), Some(RetryAfter::Seconds(7)));
        }
        other => panic!("expected a typed busy failure, got {other:?}"),
    }
    let cursor = summary.cursor();
    assert_eq!(
        cursor.next_ordinal(),
        3,
        "terminal failures are part of the durable prefix"
    );
}

#[tokio::test(start_paused = true)]
async fn raw_corruption_is_a_typed_error_never_a_miss() {
    let cache = FakeCache::default();
    let cache_handle = cache.clone();
    let populate = ParseBatchExecutor::new(FakeBackend::new(), cache.clone());
    let first = populate
        .run(
            admitted(CacheMode::WriteThrough, &["corrupt-a", "corrupt-b"]),
            Arc::new(FakeSink::new()),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(first.succeeded(), 2);

    cache_handle.mutate_payload(RAW_KEY_PREFIX, |value| {
        let hex_payload = value["tei_xml_hex"].as_str().unwrap().to_string();
        let mut bytes = hex_payload.into_bytes();
        let last = bytes.len() - 1;
        bytes[last] = if bytes[last] == b'0' { b'1' } else { b'0' };
        value["tei_xml_hex"] = serde_json::Value::String(String::from_utf8(bytes).unwrap());
    });

    let replay = ParseBatchExecutor::new(FakeBackend::new(), cache);
    let sink = Arc::new(FakeSink::new());
    let summary = replay
        .run(
            admitted(CacheMode::ReadOnly, &["corrupt-a", "corrupt-b"]),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(
        summary.backend_calls(),
        0,
        "corruption never falls back to the backend"
    );
    assert_eq!(summary.failed(), 1);
    assert_eq!(
        summary.succeeded(),
        1,
        "the run continues past one corrupt document"
    );
    let recorded = sink.recorded();
    let failures: Vec<&ParseDocumentOutcome> = recorded
        .iter()
        .filter(|outcome| !outcome.is_success())
        .collect();
    match failures[0].result() {
        Err(ParseDocumentFailure::Cache(failure)) => {
            assert_eq!(failure.stage(), ParseCacheStage::RawTei);
            assert_eq!(failure.code(), CacheIntegrityCode::ChecksumMismatch);
        }
        other => panic!("expected a typed checksum failure, got {other:?}"),
    }
}

#[tokio::test(start_paused = true)]
async fn foreign_raw_identity_and_broken_payloads_fail_visible() {
    // Foreign result identity stamped into a stored raw payload.
    let cache = FakeCache::default();
    let cache_handle = cache.clone();
    let populate = ParseBatchExecutor::new(FakeBackend::new(), cache.clone());
    populate
        .run(
            admitted(CacheMode::WriteThrough, &["identity-doc"]),
            Arc::new(FakeSink::new()),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    cache_handle.mutate_payload(RAW_KEY_PREFIX, |value| {
        value["result_identity"] =
            serde_json::Value::String("endpoint=header;schema=v1;header=v1;foreign=1".to_string());
    });
    let replay = ParseBatchExecutor::new(FakeBackend::new(), cache);
    let sink = Arc::new(FakeSink::new());
    let summary = replay
        .run(
            admitted(CacheMode::ReadOnly, &["identity-doc"]),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary.backend_calls(), 0);
    match sink.recorded()[0].result() {
        Err(ParseDocumentFailure::Cache(failure)) => {
            assert_eq!(failure.stage(), ParseCacheStage::RawTei);
            assert_eq!(failure.code(), CacheIntegrityCode::IdentityMismatch);
        }
        other => panic!("expected an identity mismatch, got {other:?}"),
    }

    // Uninterpretable projector wrapper (manifest corrupt).
    let cache = FakeCache::default();
    let cache_handle = cache.clone();
    let populate = ParseBatchExecutor::new(FakeBackend::new(), cache.clone());
    populate
        .run(
            admitted(CacheMode::WriteThrough, &["scramble-doc"]),
            Arc::new(FakeSink::new()),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    cache_handle.scramble_payload(PROJECTOR_KEY_PREFIX);
    let replay = ParseBatchExecutor::new(FakeBackend::new(), cache);
    let sink = Arc::new(FakeSink::new());
    let summary = replay
        .run(
            admitted(CacheMode::ReadOnly, &["scramble-doc"]),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary.backend_calls(), 0, "no recompute past corruption");
    match sink.recorded()[0].result() {
        Err(ParseDocumentFailure::Cache(failure)) => {
            assert_eq!(failure.stage(), ParseCacheStage::ProjectArticle);
            assert_eq!(failure.code(), CacheIntegrityCode::ManifestCorrupt);
        }
        other => panic!("expected manifest corrupt, got {other:?}"),
    }

    // Interpretable wrapper, undecodable article payload.
    let cache = FakeCache::default();
    let cache_handle = cache.clone();
    let populate = ParseBatchExecutor::new(FakeBackend::new(), cache.clone());
    populate
        .run(
            admitted(CacheMode::WriteThrough, &["scalar-doc"]),
            Arc::new(FakeSink::new()),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    cache_handle.mutate_payload(PROJECTOR_KEY_PREFIX, |value| {
        value["article"] = serde_json::Value::Number(serde_json::Number::from(42));
    });
    let replay = ParseBatchExecutor::new(FakeBackend::new(), cache);
    let sink = Arc::new(FakeSink::new());
    let summary = replay
        .run(
            admitted(CacheMode::ReadOnly, &["scalar-doc"]),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary.backend_calls(), 0);
    match sink.recorded()[0].result() {
        Err(ParseDocumentFailure::Cache(failure)) => {
            assert_eq!(failure.stage(), ParseCacheStage::ProjectArticle);
            assert_eq!(failure.code(), CacheIntegrityCode::PayloadUnreadable);
        }
        other => panic!("expected payload unreadable, got {other:?}"),
    }
}

#[tokio::test(start_paused = true)]
async fn bounded_load_gauges_never_exceed_config_and_true_overlap_happens() {
    let backend = FakeBackend::new().with_delay(Duration::from_millis(5));
    let backend_handle = backend.clone();
    let executor = ParseBatchExecutor::new(backend, FakeCache::default());
    let sink = Arc::new(FakeSink::new());
    let ids: Vec<String> = (0..6).map(|index| format!("load-{index}")).collect();
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let limits = ParseBatchLimits::new(nz(3), nz(2), nz(2), nz(6));
    let batch = ParseBatch::admit(CacheMode::Off, limits, requests_for(&id_refs), None).unwrap();
    let summary = executor
        .run(
            batch,
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary.succeeded(), 6);
    assert!(
        summary.max_in_flight_observed() <= 2,
        "in-flight bound drifted: {}",
        summary.max_in_flight_observed()
    );
    assert_eq!(
        summary.max_in_flight_observed(),
        2,
        "with six delayed documents the bound must actually be reached"
    );
    assert!(summary.max_queue_depth_observed() <= 6);
    assert!(backend_handle.call_count("load-0") == 1);
}

#[tokio::test(start_paused = true)]
async fn sink_failure_freezes_cursor_and_resume_redoes_only_unwritten_ordinals() {
    let backend = FakeBackend::new();
    let backend_handle = backend.clone();
    let executor = ParseBatchExecutor::new(backend, FakeCache::default());
    let batch = admitted(CacheMode::WriteThrough, &["sink-a", "sink-b"]);
    let fingerprint = batch.input_fingerprint().to_string();
    let sink = Arc::new(FakeSink::failing_at(1));
    let summary = executor
        .run(
            batch,
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary.sink_errors(), 1);
    assert_eq!(
        summary.cursor().next_ordinal(),
        1,
        "cursor never passes an unacknowledged ordinal"
    );
    assert_eq!(summary.succeeded(), 2, "both documents still terminate");

    // Resume from the frozen cursor: ordinal 0 is skipped (already durable),
    // ordinal 1 is redone.
    let replay = ParseBatchExecutor::new(FakeBackend::new(), FakeCache::default());
    let replay_batch = ParseBatch::admit(
        CacheMode::WriteThrough,
        exec_limits(64),
        requests_for(&["sink-a", "sink-b"]),
        Some(BatchCursor::at(&fingerprint, 1)),
    )
    .unwrap();
    let replay_sink = Arc::new(FakeSink::new());
    let replay_summary = replay
        .run(
            replay_batch,
            replay_sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(replay_summary.skipped_on_resume(), 1);
    let ordinals: Vec<u32> = replay_sink
        .recorded()
        .iter()
        .map(|outcome| outcome.ordinal())
        .collect();
    assert_eq!(
        ordinals,
        vec![1],
        "resume redoes exactly the unwritten ordinal"
    );
    let _ = backend_handle;
}

#[tokio::test(start_paused = true)]
async fn interrupted_run_never_skips_unwritten_ordinals_and_resume_completes() {
    let backend = FakeBackend::new().with_delay(Duration::from_millis(5));
    let cache = FakeCache::default();
    let executor = ParseBatchExecutor::new(backend, cache);
    let batch = admitted(
        CacheMode::WriteThrough,
        &["int-0", "int-1", "int-2", "int-3"],
    );
    let fingerprint = batch.input_fingerprint().to_string();
    let sink = Arc::new(FakeSink::parking_at(2));
    let checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let run = executor.run(batch, sink.clone(), checkpoint.clone());
    tokio::select! {
        _ = run => panic!("the run must be interrupted at the parked sink"),
        _ = tokio::time::sleep(Duration::from_secs(3600)) => {}
    }
    assert_eq!(sink.recorded_ordinals(), vec![0, 1]);
    let cursor = checkpoint.cursor().await.expect("durable cursor");
    assert_eq!(
        cursor.next_ordinal(),
        2,
        "cancellation never skips unwritten ordinals"
    );

    // Resume against the exact input fingerprint.
    let resume = ParseBatchExecutor::new(
        FakeBackend::new().with_delay(Duration::from_millis(5)),
        FakeCache::default(),
    );
    let resume_batch = ParseBatch::admit(
        CacheMode::WriteThrough,
        exec_limits(64),
        requests_for(&["int-0", "int-1", "int-2", "int-3"]),
        Some(BatchCursor::at(&fingerprint, 2)),
    )
    .unwrap();
    let resume_sink = Arc::new(FakeSink::new());
    let resume_checkpoint = Arc::new(ParseBatchMemoryCheckpoint::new());
    let resume_summary = resume
        .run(resume_batch, resume_sink.clone(), resume_checkpoint.clone())
        .await;
    assert_eq!(resume_summary.skipped_on_resume(), 2);
    assert_eq!(resume_summary.completed(), 2);
    assert_eq!(resume_sink.recorded_ordinals(), vec![2, 3]);
    assert_eq!(resume_checkpoint.cursor().await.unwrap().next_ordinal(), 4);
}

#[tokio::test(start_paused = true)]
async fn refresh_bypasses_hits_and_replaces_both_stages() {
    let cache = FakeCache::default();
    let cache_handle = cache.clone();
    let populate = ParseBatchExecutor::new(FakeBackend::new(), cache.clone());
    populate
        .run(
            admitted(CacheMode::WriteThrough, &["refresh-doc"]),
            Arc::new(FakeSink::new()),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    let before = cache_handle.snapshot();

    let backend = FakeBackend::new().with_tei("refresh-doc", "<TEI>replaced-body</TEI>");
    let backend_handle = backend.clone();
    let refresh = ParseBatchExecutor::new(backend, cache);
    let sink = Arc::new(FakeSink::new());
    let summary = refresh
        .run(
            admitted(CacheMode::Refresh, &["refresh-doc"]),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary.backend_calls(), 1, "refresh always re-fetches");
    assert_eq!(summary.raw_hits(), 0);
    assert_eq!(summary.raw_writes(), 1);
    assert_eq!(summary.projector_writes(), 1);
    let outcome = &sink.recorded()[0];
    assert_eq!(
        outcome.dispositions().raw(),
        CacheStageDisposition::ComputedPublished
    );
    assert_eq!(
        outcome.dispositions().projector(),
        CacheStageDisposition::ComputedPublished
    );
    assert_ne!(
        cache_handle.snapshot(),
        before,
        "refresh replaced the stages"
    );
    assert!(backend_handle.call_count("refresh-doc") == 1);
}

#[tokio::test(start_paused = true)]
async fn legacy_raw_hits_are_counted_not_promoted() {
    let cache = FakeCache::default();
    let cache_handle = cache.clone();
    let populate = ParseBatchExecutor::new(FakeBackend::new(), cache.clone());
    populate
        .run(
            admitted(CacheMode::WriteThrough, &["legacy-doc"]),
            Arc::new(FakeSink::new()),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    cache_handle
        .state
        .legacy_raw
        .store(true, AtomicOrdering::SeqCst);
    let replay = ParseBatchExecutor::new(FakeBackend::new(), cache);
    let sink = Arc::new(FakeSink::new());
    let summary = replay
        .run(
            admitted(CacheMode::ReadOnly, &["legacy-doc"]),
            sink.clone(),
            Arc::new(ParseBatchMemoryCheckpoint::new()),
        )
        .await;
    assert_eq!(summary.raw_hits(), 1);
    assert_eq!(summary.raw_legacy_hits(), 1);
    assert_eq!(
        summary.raw_writes(),
        0,
        "a legacy read never creates a canonical record"
    );
}

#[tokio::test(start_paused = true)]
async fn checkpoint_failure_freezes_the_durable_cursor() {
    let executor = ParseBatchExecutor::new(FakeBackend::new(), FakeCache::default());
    let sink = Arc::new(FakeSink::new());
    let checkpoint = Arc::new(FakeCheckpoint::failing());
    let summary = executor
        .run(
            admitted(CacheMode::WriteThrough, &["ckpt-a", "ckpt-b"]),
            sink.clone(),
            checkpoint.clone(),
        )
        .await;
    assert_eq!(summary.succeeded(), 2);
    assert_eq!(summary.checkpoint_errors(), 1);
    assert_eq!(
        summary.cursor().next_ordinal(),
        0,
        "the durable cursor stays at the last durably saved position"
    );
    assert_eq!(
        sink.recorded_ordinals(),
        vec![0, 1],
        "outcomes stay recorded"
    );
    assert!(checkpoint.last_saved().is_none());
}

#[test]
fn run_summary_wire_is_strictly_versioned() {
    let wire = json!({
        "schema_version": PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION,
        "mode": "write-through",
        "limits": serde_json::to_value(limits()).unwrap(),
        "input_fingerprint": "f".repeat(64),
        "resumed_from_ordinal": 0,
        "cursor": serde_json::to_value(BatchCursor::at("f".repeat(64), 2)).unwrap(),
        "admitted": 3,
        "skipped_on_resume": 0,
        "completed": 3,
        "succeeded": 2,
        "failed": 1,
        "backend_calls": 3,
        "raw_hits": 0,
        "raw_legacy_hits": 0,
        "raw_writes": 2,
        "raw_write_failures": 0,
        "projector_hits": 0,
        "projector_writes": 2,
        "projector_write_failures": 0,
        "max_in_flight_observed": 2,
        "max_queue_depth_observed": 2,
        "sink_errors": 0,
        "checkpoint_errors": 0,
    });
    let summary: ParseBatchRunSummary = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(summary.mode(), CacheMode::WriteThrough);
    assert_eq!(summary.succeeded(), 2);
    assert_eq!(summary.cursor().next_ordinal(), 2);
    let mut foreign = wire.clone();
    foreign["schema_version"] = json!(PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION + 1);
    let error = serde_json::from_value::<ParseBatchRunSummary>(foreign)
        .unwrap_err()
        .to_string();
    assert!(error.contains("schema version"), "{error}");
    let mut extra = wire;
    extra["retries"] = json!(3);
    assert!(
        serde_json::from_value::<ParseBatchRunSummary>(extra).is_err(),
        "the summary has no field for call amplification"
    );
}

#[tokio::test(start_paused = true)]
async fn stage_keys_and_payloads_keep_their_s06_identity_pins() {
    let requests = requests_for(&["key-doc"]);
    let raw_key = raw_tei_stage_key(&requests[0]);
    assert!(raw_key.starts_with(RAW_KEY_PREFIX));
    assert_eq!(raw_key.len(), RAW_KEY_PREFIX.len() + 64);
    let projector_key = project_article_stage_key(
        "key-doc",
        "pdf-sha",
        "raw-sha",
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    );
    assert!(projector_key.starts_with(PROJECTOR_KEY_PREFIX));

    // A stored raw payload round-trips and verifies against its request.
    let identity = TeiIdentity::new("fake-test-backend", "profile=fake;test=1");
    let envelope = TeiEnvelope::new(
        "<TEI>identity</TEI>",
        identity.with_result(requests[0].result_identity()),
    );
    let payload = RawTeiEnvelopePayload::build(&envelope, &requests[0].result_identity());
    let encoded = payload.encode();
    let (verified, sha) =
        RawTeiEnvelopePayload::decode_and_verify(&encoded, &requests[0].result_identity()).unwrap();
    assert_eq!(verified.raw_tei(), b"<TEI>identity</TEI>");
    assert_eq!(sha, payload.payload_sha256());

    // The projection payload binds its verified dependencies.
    let article = da_parse::lift_and_project_envelope("key-doc", "pdf-hash", &verified);
    let projection = ProjectArticlePayload::build(
        &article,
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        &sha,
        "key-doc",
        "pdf-sha",
    );
    let restored = ProjectArticlePayload::decode_and_verify(
        &projection.encode(),
        PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
        &sha,
        "key-doc",
        "pdf-sha",
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(&article).unwrap(),
        "cached projection is wire-equal to the fresh one"
    );
}
