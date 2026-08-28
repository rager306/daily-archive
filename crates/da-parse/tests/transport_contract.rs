//! Protocol contract for the versioned fulltext request vocabulary and the
//! closed typed failure vocabulary (M464-z84in2 S04/T01, R107).
//!
//! Asserted here over the public `da_parse` surface only:
//!
//! - serde roundtrip of the V1 request envelope; rejection of unknown
//!   versions, unknown fields and out-of-range values,
//! - every sanctioned consolidation mode accepted, out-of-range levels and
//!   invalid inclusive page windows rejected,
//! - exact default/rich/structural request projection and byte-identical
//!   legacy D204 fingerprints (S02 pins),
//! - versioned canonical identities for extended dialects: deterministic,
//!   distinct from every legacy preset identity, free of transport tuning
//!   (timeout/limits are not request fields),
//! - provider-neutral request fields (logical id + bytes + dialect only),
//! - typed failures: deterministic redacted Display/Debug, closed serde
//!   schema with unknown-version/unknown-field rejection, the fail-closed
//!   compatibility mapping to `ParserError`, bounded Retry-After parsing,
//! - the cached backend stays a deterministic success under the typed trait.
//!
//! No network, no filesystem, no env. Raw PDF/TEI fixture bytes must never
//! appear in any failure rendering.

use da_parse::{
    CachedTeiBackend, Consolidation, CoordinateFamily, CoordinateSet,
    FULLTEXT_REQUEST_SCHEMA_VERSION, FailureCode, FailurePhase, FulltextDialectV1,
    FulltextRequestVersioned, GrobidProfile, HEADER_REQUEST_SCHEMA_VERSION, HeaderDialectV1,
    HeaderRequestVersioned, LimitKind, PARSE_FAILURE_SCHEMA_VERSION, PageWindow, ParseBackend,
    ParseBackendFailure, ParseBackendRequest, ParseEndpoint, ParseResultKind,
    REFERENCES_REQUEST_SCHEMA_VERSION, ReferencesDialectV1, ReferencesRequestVersioned, RetryAfter,
    TeiIdentity, TeiResultIdentity,
};
use da_ports::parser::ParserError;

const TEI_FIXTURE: &str =
    "<TEI xmlns=\"http://www.tei-c.org/ns/1.0\"><text><body><p>contract</p></body></text></TEI>";

const PDF_FIXTURE: &[u8] = b"%PDF-1.4 transport contract fixture";

/// Canonical D204 fingerprints — S02 pins, byte-identical.
const RICH_FP: &str = "profile=rich;consolidateHeader=1;consolidateCitations=1;teiCoordinates=figure,table,note,formula";
const STRUCTURAL_FP: &str = "profile=structural;consolidateHeader=0;consolidateCitations=0;teiCoordinates=figure,table,note,formula";

/// Full V1 dialect wire shape (used to build valid/invalid JSON fixtures).
fn dialect_json(overrides: &str) -> String {
    format!(
        r#"{{"version":"v1","consolidation":{{"header":1,"citations":1,"funders":0}},"coordinates":["figure","table","note","formula"],"raw_citations":false,"raw_affiliations":false,"segment_sentences":false,"emit_ids":false,"page_window":null{overrides}}}"#
    )
}

// ---------------------------------------------------------------------------
// Versioned request envelope: serde roundtrip + closed evolution
// ---------------------------------------------------------------------------

#[test]
fn v1_request_serde_roundtrip_preserves_the_whole_dialect() {
    let extended = FulltextDialectV1::legacy_rich()
        .with_raw_citations(true)
        .with_raw_affiliations(true)
        .with_segment_sentences(true)
        .with_emit_ids(true)
        .with_page_window(Some(PageWindow::try_new(3, 9).expect("valid window")));
    let envelope = FulltextRequestVersioned::v1(extended);

    let json = serde_json::to_string(&envelope).expect("serialize");
    assert!(
        json.contains(r#""version":"v1""#),
        "explicit V1 discriminant on the wire: {json}"
    );

    let round: FulltextRequestVersioned = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round.version_label(), "v1");
    assert_eq!(round.dialect(), &extended, "lossless roundtrip");
    assert_eq!(
        serde_json::to_string(&envelope).unwrap(),
        serde_json::to_string(&round).unwrap(),
        "re-serialization is byte-stable"
    );
    assert_eq!(FULLTEXT_REQUEST_SCHEMA_VERSION, 1);
}

#[test]
fn unknown_request_versions_and_unknown_fields_are_rejected() {
    let err = serde_json::from_str::<FulltextRequestVersioned>(r#"{"version":"v9"}"#)
        .expect_err("unknown version must be rejected");
    assert!(
        err.to_string().contains("unknown variant"),
        "rejection names the unknown variant: {err}"
    );

    let missing_version = serde_json::from_str::<FulltextRequestVersioned>(
        r#"{"consolidation":{"header":1,"citations":1,"funders":0}}"#,
    );
    assert!(
        missing_version.is_err(),
        "a request without the version discriminant is not a valid envelope"
    );

    // Closed vocabulary: unknown fields inside V1 are rejected too.
    let unknown_field = dialect_json(r#","brand_new_knob":true"#);
    assert!(
        serde_json::from_str::<FulltextRequestVersioned>(&unknown_field).is_err(),
        "unknown dialect fields must be rejected: {unknown_field}"
    );

    // The exact same shape without the extra field is accepted.
    assert!(
        serde_json::from_str::<FulltextRequestVersioned>(&dialect_json("")).is_ok(),
        "the canonical V1 shape deserializes"
    );
}

// ---------------------------------------------------------------------------
// Sanctioned consolidation modes + page windows
// ---------------------------------------------------------------------------

#[test]
fn every_sanctioned_consolidation_mode_is_accepted_and_out_of_range_rejected() {
    for header in 0..3u8 {
        assert!(
            Consolidation::try_new(header, 0, 0).is_ok(),
            "header={header}"
        );
    }
    for citations in 0..2u8 {
        assert!(
            Consolidation::try_new(0, citations, 0).is_ok(),
            "citations={citations}"
        );
    }
    for funders in 0..2u8 {
        assert!(
            Consolidation::try_new(0, 0, funders).is_ok(),
            "funders={funders}"
        );
    }
    let too_high = Consolidation::try_new(3, 0, 0).expect_err("header 3 is outside 0..3");
    assert!(too_high.to_string().contains("header"), "{too_high}");
    assert!(
        Consolidation::try_new(0, 2, 0).is_err(),
        "citations 2 is outside 0..2"
    );
    assert!(
        Consolidation::try_new(0, 0, 2).is_err(),
        "funders 2 is outside 0..2"
    );

    // The same bounds hold on deserialization (validated vocabulary).
    let bad = r#"{"consolidation":{"header":3,"citations":0,"funders":0},"coordinates":[],"raw_citations":false,"raw_affiliations":false,"segment_sentences":false,"emit_ids":false,"page_window":null}"#;
    assert!(
        serde_json::from_str::<FulltextDialectV1>(bad).is_err(),
        "out-of-range levels must not deserialize"
    );
}

#[test]
fn page_windows_must_be_valid_inclusive_ranges() {
    for (start, end) in [(1u32, 1u32), (1, 12), (7, 9), (u32::MAX, u32::MAX)] {
        let window = PageWindow::try_new(start, end)
            .unwrap_or_else(|e| panic!("valid window {start}..{end}: {e}"));
        assert_eq!(window.start(), start);
        assert_eq!(window.end(), end);
    }
    for (start, end) in [(0u32, 5u32), (5, 4)] {
        let err =
            PageWindow::try_new(start, end).expect_err(&format!("{start}..{end} must be rejected"));
        assert!(err.to_string().contains("1 <= start <= end"), "{err}");
    }
    assert!(serde_json::from_str::<PageWindow>(r#"{"start":0,"end":5}"#).is_err());
    assert!(serde_json::from_str::<PageWindow>(r#"{"start":5,"end":4}"#).is_err());
    assert_eq!(
        serde_json::from_str::<PageWindow>(r#"{"start":3,"end":9}"#).expect("valid"),
        PageWindow::try_new(3, 9).unwrap()
    );
}

// ---------------------------------------------------------------------------
// Exact default/rich/structural projection + legacy fingerprints
// ---------------------------------------------------------------------------

#[test]
fn default_request_projects_the_exact_rich_wire_request() {
    let request = ParseBackendRequest::new("projection-paper", PDF_FIXTURE.to_vec());
    assert_eq!(
        request.dialect(),
        &FulltextDialectV1::legacy_rich(),
        "the legacy/default constructor carries the rich-equivalent dialect"
    );
    assert_eq!(
        request.dialect(),
        &GrobidProfile::rich().dialect(),
        "default projection reproduces the current rich wire request"
    );

    let dialect = request.dialect();
    assert_eq!(dialect.consolidation().citations().value(), 1);
    assert_eq!(dialect.consolidation().header().value(), 1);
    assert_eq!(dialect.consolidation().funders().value(), 0);
    assert!(!dialect.raw_citations());
    assert!(!dialect.raw_affiliations());
    assert!(!dialect.segment_sentences());
    assert!(!dialect.emit_ids());
    assert_eq!(dialect.page_window(), None);
    assert_eq!(
        dialect.coordinates().wire_value(),
        "figure,table,note,formula",
        "coordinates wire string stays the D204 constant"
    );
    assert_eq!(
        GrobidProfile::tei_coordinates_json(),
        "[\"figure\",\"table\",\"note\",\"formula\"]",
        "REST JSON rendering stays byte-identical"
    );
}

#[test]
fn legacy_profiles_project_their_exact_dialects() {
    let structural =
        ParseBackendRequest::from_legacy_profile(&GrobidProfile::structural(), "p", Vec::new());
    assert_eq!(
        structural.dialect(),
        &FulltextDialectV1::legacy_structural(),
        "structural profile projects its bounded dialect"
    );
    assert_eq!(structural.dialect().consolidation().header().value(), 0);
    assert_eq!(structural.dialect().consolidation().citations().value(), 0);

    // Provider-neutral fields flow verbatim through both constructors.
    assert_eq!(structural.paper_id(), "p");
    assert!(structural.pdf_bytes().is_empty());
    let rich =
        ParseBackendRequest::from_legacy_profile(&GrobidProfile::rich(), "q", PDF_FIXTURE.to_vec());
    assert_eq!(rich.pdf_bytes(), PDF_FIXTURE);
    assert_eq!(rich.paper_id(), "q");
}

#[test]
fn legacy_fingerprints_stay_byte_identical_to_the_s02_pins() {
    assert_eq!(GrobidProfile::rich().configuration_fingerprint(), RICH_FP);
    assert_eq!(
        GrobidProfile::structural().configuration_fingerprint(),
        STRUCTURAL_FP
    );
    assert_ne!(RICH_FP, STRUCTURAL_FP, "presets must never share identity");
}

#[test]
fn custom_profiles_keep_stable_names_fingerprints_and_accessors() {
    let custom = GrobidProfile::custom(Consolidation::try_new(2, 0, 0).expect("sanctioned"));
    assert_eq!(custom.name(), "custom");
    assert_eq!(
        custom.configuration_fingerprint(),
        "profile=custom;consolidateHeader=2;consolidateCitations=0;teiCoordinates=figure,table,note,formula"
    );
    assert!(
        custom.consolidate_header(),
        "compatibility bool accessor: enabled"
    );
    assert!(!custom.consolidate_citations());
    // header=2 leaves the legacy preset domain → the dialect owns an
    // extended identity (never colliding with a legacy fingerprint).
    assert!(custom.dialect().canonical_identity().is_some());
}

// ---------------------------------------------------------------------------
// Coordinate sets
// ---------------------------------------------------------------------------

#[test]
fn coordinate_sets_dedupe_into_canonical_wire_order() {
    let set = CoordinateSet::from_families([
        CoordinateFamily::Formula,
        CoordinateFamily::Figure,
        CoordinateFamily::Formula,
    ]);
    assert_eq!(set.wire_value(), "figure,formula", "deduped + canonical");
    assert!(set.contains(CoordinateFamily::Formula));
    assert!(!set.contains(CoordinateFamily::Table));
    assert_eq!(CoordinateSet::none().wire_value(), "");
    assert_eq!(
        CoordinateSet::all().wire_value(),
        "figure,table,note,formula"
    );
    // Roundtrip: deserialization normalizes identically.
    assert_eq!(
        serde_json::from_str::<CoordinateSet>(r#"["formula","figure","formula"]"#).expect("valid"),
        set
    );
}

// ---------------------------------------------------------------------------
// Versioned canonical identities vs legacy presets
// ---------------------------------------------------------------------------

#[test]
fn legacy_preset_dialects_claim_no_extended_identity() {
    assert!(
        FulltextDialectV1::legacy_rich()
            .canonical_identity()
            .is_none()
    );
    assert!(
        FulltextDialectV1::legacy_structural()
            .canonical_identity()
            .is_none()
    );
}

#[test]
fn extended_dialects_get_deterministic_distinct_versioned_identities() {
    let raw_on = FulltextDialectV1::legacy_rich().with_raw_citations(true);
    let header2 = FulltextDialectV1::legacy_rich()
        .with_consolidation(Consolidation::try_new(2, 1, 0).expect("sanctioned"));
    let windowed = FulltextDialectV1::legacy_structural()
        .with_page_window(Some(PageWindow::try_new(1, 4).expect("valid")));

    for dialect in [raw_on, header2, windowed] {
        let identity = dialect
            .canonical_identity()
            .expect("extended dialect identity");
        assert!(
            identity.starts_with("fulltext=v1;"),
            "versioned namespace, never a legacy fingerprint: {identity}"
        );
        assert_ne!(identity, RICH_FP);
        assert_ne!(identity, STRUCTURAL_FP);
        assert_eq!(
            identity,
            dialect.canonical_identity().expect("present"),
            "deterministic"
        );
    }

    assert_ne!(
        raw_on.canonical_identity(),
        header2.canonical_identity(),
        "different extended dialects stay distinguishable"
    );
    assert_ne!(windowed.canonical_identity(), header2.canonical_identity());
    assert!(
        windowed
            .canonical_identity()
            .unwrap()
            .contains("pages=1..4")
    );
    assert!(
        raw_on
            .canonical_identity()
            .unwrap()
            .contains("rawCitations=1")
    );

    // Identity survives a serde roundtrip unchanged.
    let json = serde_json::to_string(&FulltextRequestVersioned::v1(raw_on)).unwrap();
    let round: FulltextRequestVersioned = serde_json::from_str(&json).unwrap();
    assert_eq!(
        round.dialect().canonical_identity(),
        raw_on.canonical_identity()
    );
}

#[test]
fn transport_tuning_never_enters_the_request_vocabulary_or_identity() {
    let json = serde_json::to_value(FulltextRequestVersioned::v1(
        FulltextDialectV1::legacy_rich(),
    ))
    .expect("serialize");
    let text = json.to_string();
    assert!(
        !text.contains("timeout"),
        "timeout is not a request field: {text}"
    );
    assert!(
        !text.contains("limit"),
        "limits are not request fields: {text}"
    );
    // Limits live only on the typed failure after they are actually hit.
    let limit = ParseBackendFailure::limit(LimitKind::ResponseBytes, None);
    assert_eq!(limit.code(), FailureCode::LimitExceeded);
    assert_eq!(limit.enforced_limit(), Some(LimitKind::ResponseBytes));
    assert_eq!(
        limit.status(),
        None,
        "opt-in client bounds carry no provider status"
    );
    // Identity is a pure function of the dialect fields.
    let a = FulltextDialectV1::legacy_rich().with_raw_affiliations(true);
    let b = FulltextDialectV1::legacy_rich().with_raw_affiliations(true);
    assert_eq!(a.canonical_identity(), b.canonical_identity());
}

// ---------------------------------------------------------------------------
// Provider-neutral request fields
// ---------------------------------------------------------------------------

#[test]
fn request_stays_provider_neutral_logical_id_plus_bytes_plus_dialect() {
    let path_looking = "../../data/cache/root/paper.pdf";
    let request = ParseBackendRequest::new(path_looking, PDF_FIXTURE.to_vec());
    assert_eq!(
        request.paper_id(),
        path_looking,
        "logical id carried verbatim (never validated as a physical path)"
    );
    assert_eq!(request.pdf_bytes(), PDF_FIXTURE);

    let dialect = FulltextDialectV1::legacy_structural().with_emit_ids(true);
    let explicit = ParseBackendRequest::with_dialect("p2", Vec::new(), dialect);
    assert_eq!(explicit.dialect(), &dialect);
    assert_eq!(explicit.paper_id(), "p2");
    assert!(explicit.pdf_bytes().is_empty());
}

// ---------------------------------------------------------------------------
// Typed failures: deterministic, redacted, closed schema
// ---------------------------------------------------------------------------

#[test]
fn failures_render_deterministic_redacted_metadata_only() {
    let failure = ParseBackendFailure::busy(503, Some(RetryAfter::Seconds(30)));
    let display = failure.to_string();
    let debug = format!("{failure:?}");
    for rendering in [display.clone(), debug] {
        assert!(
            !rendering.contains("%PDF") && !rendering.contains("paper.pdf"),
            "no payload or physical path in a failure rendering: {rendering}"
        );
        assert!(
            !rendering.contains(TEI_FIXTURE),
            "no TEI in a failure rendering"
        );
    }
    assert!(
        display.contains("code=busy") && display.contains("status=503"),
        "Display carries the stable typed metadata: {display}"
    );
    assert_eq!(
        display,
        ParseBackendFailure::busy(503, Some(RetryAfter::Seconds(30))).to_string(),
        "identical failures render identically"
    );
    assert_eq!(failure.schema_version(), PARSE_FAILURE_SCHEMA_VERSION);

    // Every constructor keeps the stable schema version and safe fields only.
    let no_content = ParseBackendFailure::no_content();
    assert_eq!(no_content.code(), FailureCode::NoContent);
    assert_eq!(no_content.status(), Some(204));
    assert_eq!(no_content.phase(), FailurePhase::AwaitResponse);
    let invalid = ParseBackendFailure::invalid_request(400);
    assert_eq!(invalid.code(), FailureCode::InvalidRequest);
    let response_limit = ParseBackendFailure::limit(LimitKind::ResponseBytes, None);
    assert_eq!(response_limit.phase(), FailurePhase::ReadBody);
    let request_limit = ParseBackendFailure::limit(LimitKind::RequestBytes, Some(413));
    assert_eq!(request_limit.phase(), FailurePhase::Send);
    assert_eq!(request_limit.status(), Some(413));
    assert_eq!(
        ParseBackendFailure::timeout(FailurePhase::AwaitResponse).code(),
        FailureCode::Timeout
    );
    assert_eq!(
        ParseBackendFailure::transport(FailurePhase::Connect).code(),
        FailureCode::Transport
    );
}

#[test]
fn failure_serde_roundtrips_and_rejects_unknown_schema_versions_and_fields() {
    let failure = ParseBackendFailure::busy(503, Some(RetryAfter::Unparsable));
    let json = serde_json::to_string(&failure).expect("serialize");
    assert!(json.contains(r#""schema_version":1"#), "{json}");
    let round: ParseBackendFailure = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round, failure);

    let limit = ParseBackendFailure::limit(LimitKind::RequestBytes, Some(413));
    let round_limit: ParseBackendFailure =
        serde_json::from_str(&serde_json::to_string(&limit).unwrap()).unwrap();
    assert_eq!(round_limit, limit);

    let bad_version = json.replace(r#""schema_version":1"#, r#""schema_version":2"#);
    let err = serde_json::from_str::<ParseBackendFailure>(&bad_version)
        .expect_err("unknown schema versions must be rejected");
    assert!(
        err.to_string()
            .contains("unsupported parse failure schema version 2"),
        "{err}"
    );

    let unknown_field = r#"{"schema_version":1,"code":"busy","phase":"await_response","status":503,"retry_after":null,"limit":null,"extra":"x"}"#;
    assert!(
        serde_json::from_str::<ParseBackendFailure>(unknown_field).is_err(),
        "unknown fields must be rejected (closed schema)"
    );
}

#[test]
fn to_parser_error_maps_availability_open_and_everything_else_fail_closed() {
    let availability = [
        ParseBackendFailure::busy(503, Some(RetryAfter::Seconds(5))),
        ParseBackendFailure::timeout(FailurePhase::AwaitResponse),
        ParseBackendFailure::transport(FailurePhase::Connect),
    ];
    for failure in availability {
        let parser_error = failure.to_parser_error();
        assert!(
            matches!(parser_error, ParserError::Unavailable(ref message)
                if message.contains("code=") && message.contains(&format!("code={}", failure.code()))),
            "{failure} must stay Unavailable with stable metadata: {parser_error:?}"
        );
    }

    let fail_closed = [
        ParseBackendFailure::no_content(),
        ParseBackendFailure::limit(LimitKind::RequestBytes, Some(413)),
        ParseBackendFailure::invalid_request(400),
        ParseBackendFailure::conversion(FailurePhase::ReadBody, Some(500)),
        ParseBackendFailure::invalid_response(FailurePhase::AwaitResponse, Some(418)),
    ];
    for failure in fail_closed {
        let parser_error = failure.to_parser_error();
        assert!(
            matches!(parser_error, ParserError::ParseFailed(ref message)
                if message.contains(&format!("code={}", failure.code()))),
            "{failure} must fail closed as ParseFailed: {parser_error:?}"
        );
    }
}

#[test]
fn retry_after_header_parsing_is_bounded_and_honest() {
    assert_eq!(
        RetryAfter::parse_header("120"),
        Some(RetryAfter::Seconds(120))
    );
    assert_eq!(
        RetryAfter::parse_header("  45  "),
        Some(RetryAfter::Seconds(45))
    );
    assert_eq!(RetryAfter::parse_header(""), None);
    assert_eq!(RetryAfter::parse_header("   "), None);
    assert_eq!(
        RetryAfter::parse_header("Wed, 21 Oct 2026 07:28:00 GMT"),
        Some(RetryAfter::Unparsable),
        "HTTP-date form is honest unparsable, never a fabricated delay"
    );
    assert_eq!(
        RetryAfter::parse_header("12345678901"),
        Some(RetryAfter::Unparsable),
        ">10 digits stay bounded"
    );
    assert_eq!(
        RetryAfter::parse_header("9999999999"),
        Some(RetryAfter::Seconds(9_999_999_999))
    );
}

// ---------------------------------------------------------------------------
// Cached backend: deterministic success under the typed trait
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cached_backend_stays_a_deterministic_success_under_the_typed_trait() {
    let backend = CachedTeiBackend::new(TEI_FIXTURE, GrobidProfile::structural());
    let request = ParseBackendRequest::new("typed-trait-paper", PDF_FIXTURE.to_vec());

    let first = backend
        .fetch_tei(request.clone())
        .await
        .expect("cached replay is infallible");
    let second = backend
        .fetch_tei(request)
        .await
        .expect("cached replay is infallible");

    assert_eq!(first, second, "deterministic success");
    assert_eq!(first.raw_tei(), TEI_FIXTURE.as_bytes());
    assert_eq!(first.identity().backend_id(), "cached-tei");
}

// ---------------------------------------------------------------------------
// S05/T01: closed endpoint/result vocabulary, header/references dialects,
// endpoint result identities
// ---------------------------------------------------------------------------

#[test]
fn endpoint_vocabulary_is_closed_with_exact_static_routes_and_result_kinds() {
    let cases = [
        (
            ParseEndpoint::Fulltext,
            "fulltext",
            "/api/processFulltextDocument",
            ParseResultKind::FullDocument,
        ),
        (
            ParseEndpoint::Header,
            "header",
            "/api/processHeaderDocument",
            ParseResultKind::Header,
        ),
        (
            ParseEndpoint::References,
            "references",
            "/api/processReferences",
            ParseResultKind::ListBibl,
        ),
    ];
    for (endpoint, name, path, result_kind) in cases {
        assert_eq!(endpoint.as_str(), name);
        assert_eq!(endpoint.static_path(), path, "{name}: closed static route");
        assert_eq!(endpoint.result_kind(), result_kind);
        let wire = serde_json::to_string(&endpoint).expect("endpoint serializes");
        assert_eq!(wire, format!("\"{name}\""), "stable snake_case wire form");
        let back: ParseEndpoint = serde_json::from_str(&wire).expect("endpoint roundtrips");
        assert_eq!(back, endpoint);
    }
    assert_eq!(
        serde_json::to_string(&ParseResultKind::ListBibl).expect("result kind serializes"),
        "\"list_bibl\""
    );
    assert!(
        serde_json::from_str::<ParseEndpoint>("\"abstract\"").is_err(),
        "invented endpoints must be rejected (closed vocabulary)"
    );
    assert!(
        serde_json::from_str::<ParseResultKind>("\"bibtex\"").is_err(),
        "invented result kinds must be rejected (closed vocabulary)"
    );
}

#[test]
fn header_and_references_envelopes_roundtrip_and_reject_unknown_versions_and_fields() {
    let header = HeaderDialectV1::legacy_rich();
    let header_wire =
        serde_json::to_string(&HeaderRequestVersioned::v1(header)).expect("serialize");
    assert!(
        header_wire.contains(r#""version":"v1""#),
        "explicit V1 discriminant on the wire: {header_wire}"
    );
    assert!(header_wire.contains(r#""consolidate_header":1"#));
    assert!(header_wire.contains(r#""include_raw_affiliations":false"#));
    let round: HeaderRequestVersioned = serde_json::from_str(&header_wire).expect("deserialize");
    assert_eq!(round.version_label(), "v1");
    assert_eq!(*round.dialect(), header, "lossless roundtrip");

    let references = serde_json::to_string(&ReferencesRequestVersioned::v1(
        ReferencesDialectV1::legacy_structural(),
    ))
    .expect("serialize");
    let round_refs: ReferencesRequestVersioned =
        serde_json::from_str(&references).expect("deserialize");
    assert_eq!(
        *round_refs.dialect(),
        ReferencesDialectV1::legacy_structural()
    );
    assert_eq!(HEADER_REQUEST_SCHEMA_VERSION, 1);
    assert_eq!(REFERENCES_REQUEST_SCHEMA_VERSION, 1);

    // Unknown versions are rejected exactly like the fulltext envelope.
    for wire in [
        r#"{"version":"v9","consolidate_header":1,"include_raw_affiliations":false}"#,
        r#"{"version":"v9","consolidate_citations":1,"include_raw_citations":false}"#,
    ] {
        assert!(
            serde_json::from_str::<HeaderRequestVersioned>(wire).is_err(),
            "unknown version must fail on header: {wire}"
        );
        assert!(
            serde_json::from_str::<ReferencesRequestVersioned>(wire).is_err(),
            "unknown version must fail on references: {wire}"
        );
    }
    assert!(
        serde_json::from_str::<HeaderRequestVersioned>(r#"{"consolidate_header":1}"#).is_err(),
        "missing version discriminant is not a valid envelope"
    );

    // Unknown AND foreign fields are rejected: header knobs cannot ride the
    // references dialect and vice versa; fulltext-only knobs, invented knobs
    // and infrastructure tuning have no field anywhere.
    let header_foreign = [
        "include_raw_citations",
        "consolidate_citations",
        "segment_sentences",
        "emit_ids",
        "page_window",
        "coordinates",
        "brand_new_knob",
        "base_url",
        "timeout_ms",
        "request_byte_limit",
    ];
    for field in header_foreign {
        let wire = format!(
            r#"{{"version":"v1","consolidate_header":1,"include_raw_affiliations":false,"{field}":true}}"#
        );
        assert!(
            serde_json::from_str::<HeaderRequestVersioned>(&wire).is_err(),
            "header dialect must reject foreign/unknown field `{field}`"
        );
    }
    let references_foreign = [
        "include_raw_affiliations",
        "consolidate_header",
        "segment_sentences",
        "emit_ids",
        "page_window",
        "coordinates",
        "brand_new_knob",
        "base_url",
        "timeout_ms",
        "response_byte_limit",
    ];
    for field in references_foreign {
        let wire = format!(
            r#"{{"version":"v1","consolidate_citations":1,"include_raw_citations":false,"{field}":true}}"#
        );
        assert!(
            serde_json::from_str::<ReferencesRequestVersioned>(&wire).is_err(),
            "references dialect must reject foreign/unknown field `{field}`"
        );
    }

    // Closed shapes: required controls cannot be omitted.
    assert!(
        serde_json::from_str::<HeaderRequestVersioned>(
            r#"{"version":"v1","consolidate_header":1}"#
        )
        .is_err(),
        "include_raw_affiliations is a required V1 control"
    );
    assert!(
        serde_json::from_str::<HeaderRequestVersioned>(
            r#"{"version":"v1","include_raw_affiliations":false}"#
        )
        .is_err(),
        "consolidate_header is a required V1 control"
    );
}

#[test]
fn header_and_references_consolidation_stays_inside_sanctioned_ranges() {
    let bad_header = serde_json::from_str::<HeaderRequestVersioned>(
        r#"{"version":"v1","consolidate_header":3,"include_raw_affiliations":false}"#,
    );
    assert!(bad_header.is_err(), "header level 3 is outside 0..3");
    let bad_references = serde_json::from_str::<ReferencesRequestVersioned>(
        r#"{"version":"v1","consolidate_citations":2,"include_raw_citations":false}"#,
    );
    assert!(bad_references.is_err(), "citations level 2 is outside 0..2");

    for header in 0..3u8 {
        let wire = format!(
            r#"{{"version":"v1","consolidate_header":{header},"include_raw_affiliations":false}}"#
        );
        assert!(
            serde_json::from_str::<HeaderRequestVersioned>(&wire).is_ok(),
            "sanctioned header level {header} must deserialize"
        );
    }
    for citations in 0..2u8 {
        let wire = format!(
            r#"{{"version":"v1","consolidate_citations":{citations},"include_raw_citations":false}}"#
        );
        assert!(
            serde_json::from_str::<ReferencesRequestVersioned>(&wire).is_ok(),
            "sanctioned citations level {citations} must deserialize"
        );
    }
}

#[test]
fn legacy_profiles_map_only_onto_their_consolidation_axis() {
    let rich_header = HeaderDialectV1::from_legacy_profile(&GrobidProfile::rich());
    assert_eq!(rich_header.consolidate_header().value(), 1);
    assert!(!rich_header.include_raw_affiliations());
    let structural_header = HeaderDialectV1::from_legacy_profile(&GrobidProfile::structural());
    assert_eq!(structural_header.consolidate_header().value(), 0);
    assert_eq!(rich_header, HeaderDialectV1::legacy_rich());
    assert_eq!(structural_header, HeaderDialectV1::legacy_structural());

    let rich_references = ReferencesDialectV1::from_legacy_profile(&GrobidProfile::rich());
    assert_eq!(rich_references.consolidate_citations().value(), 1);
    assert!(!rich_references.include_raw_citations());
    let structural_references =
        ReferencesDialectV1::from_legacy_profile(&GrobidProfile::structural());
    assert_eq!(structural_references.consolidate_citations().value(), 0);
    assert_eq!(rich_references, ReferencesDialectV1::legacy_rich());
    assert_eq!(
        structural_references,
        ReferencesDialectV1::legacy_structural()
    );
}

#[test]
fn endpoint_requests_are_constructed_through_the_existing_seam() {
    let header_bytes = b"%PDF header proxy".to_vec();
    let header = ParseBackendRequest::header(
        "header-paper",
        header_bytes.clone(),
        HeaderDialectV1::legacy_rich(),
    );
    assert_eq!(header.endpoint(), ParseEndpoint::Header);
    assert_eq!(header.paper_id(), "header-paper");
    assert_eq!(header.pdf_bytes(), &header_bytes);
    assert_eq!(
        header.header_dialect(),
        Some(HeaderDialectV1::legacy_rich())
    );
    assert_eq!(header.references_dialect(), None);

    let references = ParseBackendRequest::references_from_legacy_profile(
        &GrobidProfile::structural(),
        "references-paper",
        Vec::new(),
    );
    assert_eq!(references.endpoint(), ParseEndpoint::References);
    assert_eq!(
        references.references_dialect(),
        Some(ReferencesDialectV1::legacy_structural())
    );
    assert_eq!(references.header_dialect(), None);

    // Trait seam: backends hand out closed operation requests too.
    let cached = CachedTeiBackend::new(TEI_FIXTURE, GrobidProfile::rich());
    let via_trait = cached.default_header_request("via-trait", Vec::new());
    assert_eq!(via_trait.endpoint(), ParseEndpoint::Header);
    let via_trait_refs = cached.default_references_request("via-trait", Vec::new());
    assert_eq!(via_trait_refs.endpoint(), ParseEndpoint::References);

    // Legacy fulltext constructors keep their exact surface.
    let legacy = ParseBackendRequest::from_legacy_profile(
        &GrobidProfile::structural(),
        "p",
        PDF_FIXTURE.to_vec(),
    );
    assert_eq!(legacy.endpoint(), ParseEndpoint::Fulltext);
    assert_eq!(legacy.header_dialect(), None);
    assert_eq!(legacy.references_dialect(), None);
    assert_eq!(legacy.dialect(), &FulltextDialectV1::legacy_structural());
    assert_eq!(legacy.pdf_bytes(), PDF_FIXTURE);

    // Redaction: source bytes never render through Debug (S05 redaction floor).
    let rendered = format!("{legacy:?}");
    assert!(
        !rendered.contains("%PDF"),
        "request Debug must not carry source bytes: {rendered}"
    );
    assert!(rendered.contains("pdf_bytes_len"));
}

#[test]
fn result_identities_are_pairwise_distinct_for_one_profile() {
    for profile in [GrobidProfile::rich(), GrobidProfile::structural()] {
        let fulltext = ParseBackendRequest::new("p", PDF_FIXTURE.to_vec()).result_identity();
        let header = ParseBackendRequest::header_from_legacy_profile(&profile, "p", Vec::new())
            .result_identity();
        let references =
            ParseBackendRequest::references_from_legacy_profile(&profile, "p", Vec::new())
                .result_identity();

        assert_eq!(fulltext.endpoint(), ParseEndpoint::Fulltext);
        assert_eq!(header.endpoint(), ParseEndpoint::Header);
        assert_eq!(references.endpoint(), ParseEndpoint::References);
        assert_ne!(fulltext, header);
        assert_ne!(header, references);
        assert_ne!(fulltext, references);
        let (f, h, r) = (
            fulltext.canonical(),
            header.canonical(),
            references.canonical(),
        );
        assert_ne!(f, h);
        assert_ne!(h, r);
        assert_ne!(f, r);

        // Deterministic under repetition.
        assert_eq!(
            ParseBackendRequest::header_from_legacy_profile(&profile, "again", Vec::new())
                .result_identity(),
            header
        );
        assert_eq!(
            ParseBackendRequest::references_from_legacy_profile(&profile, "again", Vec::new())
                .result_identity(),
            references
        );
    }

    // Stamped envelope identities stay separated across endpoints for one
    // profile.
    let base = TeiIdentity::grobid_http(GrobidProfile::rich());
    let stamped = [
        base.clone()
            .with_result(TeiResultIdentity::fulltext(FulltextDialectV1::legacy_rich())),
        base.clone()
            .with_result(TeiResultIdentity::header(HeaderDialectV1::legacy_rich())),
        base.clone().with_result(TeiResultIdentity::references(
            ReferencesDialectV1::legacy_rich(),
        )),
    ];
    assert_ne!(stamped[0], stamped[1]);
    assert_ne!(stamped[1], stamped[2]);
    assert_ne!(stamped[0], stamped[2]);
}

#[test]
fn raw_switches_rotate_only_their_own_identity() {
    let header_off = HeaderDialectV1::legacy_rich();
    let header_on = header_off.with_include_raw_affiliations(true);
    assert_ne!(
        header_off.canonical_identity(),
        header_on.canonical_identity()
    );
    assert!(
        header_off
            .canonical_identity()
            .contains("rawAffiliations=0")
    );
    assert!(header_on.canonical_identity().contains("rawAffiliations=1"));

    let references_off = ReferencesDialectV1::legacy_rich();
    let references_on = references_off.with_include_raw_citations(true);
    assert_ne!(
        references_off.canonical_identity(),
        references_on.canonical_identity()
    );
    assert!(
        references_off
            .canonical_identity()
            .contains("rawCitations=0")
    );
    assert!(
        references_on
            .canonical_identity()
            .contains("rawCitations=1")
    );

    // Versioned namespaces never collide with each other or with the legacy
    // D204 fingerprints.
    assert_ne!(
        header_off.canonical_identity(),
        references_off.canonical_identity()
    );
    assert_ne!(header_off.canonical_identity(), RICH_FP);
    assert_ne!(references_on.canonical_identity(), STRUCTURAL_FP);
}

#[test]
fn endpoint_result_identities_exclude_infrastructure_tuning_and_locations() {
    let identities = [
        TeiResultIdentity::fulltext(FulltextDialectV1::legacy_rich()).canonical(),
        TeiResultIdentity::header(HeaderDialectV1::legacy_rich()).canonical(),
        TeiResultIdentity::header(
            HeaderDialectV1::legacy_structural().with_include_raw_affiliations(true),
        )
        .canonical(),
        TeiResultIdentity::references(ReferencesDialectV1::legacy_rich()).canonical(),
        TeiResultIdentity::references(
            ReferencesDialectV1::legacy_structural().with_include_raw_citations(true),
        )
        .canonical(),
    ];
    for identity in &identities {
        for forbidden in [
            "http", "://", "timeout", "limit", "path", "cache", "url", "dir",
        ] {
            assert!(
                !identity.to_lowercase().contains(forbidden),
                "identity `{identity}` must never carry infrastructure tuning or locations (`{forbidden}`)"
            );
        }
    }

    // Stamping keeps exactly the provenance fields; tuning has no input
    // channel at all.
    let header_result = TeiResultIdentity::header(HeaderDialectV1::legacy_rich());
    let stamped =
        TeiIdentity::grobid_http(GrobidProfile::rich()).with_result(header_result.clone());
    assert_eq!(stamped.backend_id(), "grobid-http");
    assert_eq!(stamped.profile_fingerprint(), RICH_FP);
    assert_eq!(stamped.result(), Some(&header_result));
}

#[test]
fn pre_s05_identity_pins_stay_untouched_and_result_stamping_is_additive() {
    // The S02/S04 identity constructors keep their exact shape.
    let http = TeiIdentity::grobid_http(GrobidProfile::rich());
    assert_eq!(http.backend_id(), "grobid-http");
    assert_eq!(http.profile_fingerprint(), RICH_FP);
    assert!(
        http.result().is_none(),
        "pre-S05 identities own no result component"
    );
    let cached = TeiIdentity::cached_tei(GrobidProfile::structural());
    assert!(cached.result().is_none());
    assert_ne!(http, cached);

    // Result stamping is purely additive.
    let header_result = TeiResultIdentity::header(HeaderDialectV1::legacy_rich());
    let stamped = http.clone().with_result(header_result.clone());
    assert_eq!(stamped.backend_id(), http.backend_id());
    assert_eq!(stamped.profile_fingerprint(), http.profile_fingerprint());
    assert_eq!(stamped.result(), Some(&header_result));
    assert_ne!(stamped, http, "result stamping distinguishes the operation");

    // Fulltext result identity: legacy-equivalent dialects carry no extended
    // fulltext dialect identity (the D204 fingerprint stays their only key)
    // but still distinguish the fulltext operation.
    let legacy_fulltext = TeiResultIdentity::fulltext(FulltextDialectV1::legacy_rich());
    assert_eq!(
        legacy_fulltext.schema_version(),
        FULLTEXT_REQUEST_SCHEMA_VERSION
    );
    assert_eq!(legacy_fulltext.dialect_identity(), "legacy-equivalent");
    assert_eq!(
        legacy_fulltext.canonical(),
        "endpoint=fulltext;schema=v1;legacy-equivalent"
    );
    let extended_fulltext =
        TeiResultIdentity::fulltext(FulltextDialectV1::legacy_rich().with_segment_sentences(true));
    assert!(
        extended_fulltext
            .dialect_identity()
            .starts_with("fulltext=v1;"),
        "extended fulltext dialects keep their versioned namespace"
    );
    assert_ne!(legacy_fulltext, extended_fulltext);
}
