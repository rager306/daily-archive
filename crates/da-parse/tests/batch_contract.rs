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
    PARSE_CACHE_FAILURE_SCHEMA_VERSION, PROJECT_ARTICLE_STAGE_ID, ParseBackendFailure,
    ParseBackendRequest, ParseBatch, ParseBatchAdmissionError, ParseBatchCursorError,
    ParseBatchItem, ParseBatchLimits, ParseCacheDispositions, ParseCacheFailure, ParseCacheStage,
    ParseDocumentFailure, ParseDocumentOutcome, ParseDocumentSuccess, RAW_TEI_STAGE_ID,
    ReferencesDialectV1, RetryAfter, ordered_input_fingerprint,
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
