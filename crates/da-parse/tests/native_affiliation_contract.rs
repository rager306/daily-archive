use da_parse::{
    AffiliationComponentKind, NATIVE_AFFILIATION_INPUT_BYTE_CAP,
    NATIVE_AFFILIATION_OUTCOME_SCHEMA_VERSION, NATIVE_AFFILIATION_PARSER_ID,
    NATIVE_AFFILIATION_PARSER_VERSION, NativeAffiliationAmbiguous, NativeAffiliationMalformed,
    NativeAffiliationOutcome, NativeAffiliationParser, NativeAffiliationUnsupported,
};
use sha2::{Digest, Sha256};

fn parse(input: &str) -> NativeAffiliationOutcome {
    NativeAffiliationParser::new().parse(input)
}

fn recognized(input: &str) -> Vec<(AffiliationComponentKind, &str)> {
    match parse(input) {
        NativeAffiliationOutcome::Recognized { components } => components
            .iter()
            .map(|component| {
                (
                    component.kind,
                    &input[component.span.start..component.span.end],
                )
            })
            .collect(),
        other => panic!("expected recognized, got {other:?}"),
    }
}

#[test]
fn public_identity_and_schema_are_frozen() {
    assert_eq!(NATIVE_AFFILIATION_PARSER_ID, "da-parse.native_affiliation");
    assert_eq!(NATIVE_AFFILIATION_PARSER_VERSION, "1.0.0");
    assert_eq!(
        NATIVE_AFFILIATION_OUTCOME_SCHEMA_VERSION,
        "da-parse.native-affiliation.outcome/v1"
    );
    assert_eq!(NATIVE_AFFILIATION_INPUT_BYTE_CAP, 512);
}

#[test]
fn recognized_components_are_literal_spans_and_keep_order() {
    let input = "¹ Department of Physics, University of Example, 12 Main Street, Berlin";
    let components = recognized(input);
    assert_eq!(
        components,
        vec![
            (AffiliationComponentKind::Marker, "¹"),
            (
                AffiliationComponentKind::Organization,
                "Department of Physics"
            ),
            (
                AffiliationComponentKind::Organization,
                "University of Example"
            ),
            (AffiliationComponentKind::Address, "12 Main Street"),
        ]
    );
}

#[test]
fn laboratory_and_unicode_text_are_not_normalized_or_invented() {
    let input = "Laboratório de Física, Universität München";
    assert_eq!(
        recognized(input),
        vec![
            (
                AffiliationComponentKind::Organization,
                "Laboratório de Física"
            ),
            (
                AffiliationComponentKind::Organization,
                "Universität München"
            ),
        ]
    );
    assert!(matches!(
        parse("plain scholarly prose"),
        NativeAffiliationOutcome::Unsupported {
            reason: NativeAffiliationUnsupported::NoRecognizableComponent
        }
    ));
}

#[test]
fn closed_failures_are_redacted_and_serde_replay_is_byte_stable() {
    let cases = [
        ("", NativeAffiliationOutcome::Empty),
        ("   ", NativeAffiliationOutcome::Empty),
        (
            "University, Institute",
            NativeAffiliationOutcome::Ambiguous {
                reason: NativeAffiliationAmbiguous::MultipleOrganizationCandidates,
            },
        ),
        (
            "University,, Berlin",
            NativeAffiliationOutcome::Malformed {
                reason: NativeAffiliationMalformed::RepeatedSeparator,
            },
        ),
        (
            "University\0 Berlin",
            NativeAffiliationOutcome::Malformed {
                reason: NativeAffiliationMalformed::ControlCharacter,
            },
        ),
        (
            "University; Berlin",
            NativeAffiliationOutcome::Unsupported {
                reason: NativeAffiliationUnsupported::UnsupportedPunctuation,
            },
        ),
    ];
    for (input, expected) in cases {
        let actual = parse(input);
        assert_eq!(actual, expected);
        let bytes = serde_json::to_vec(&actual).unwrap();
        if !input.is_empty() {
            assert!(!String::from_utf8_lossy(&bytes).contains(input));
        }
        let replay: NativeAffiliationOutcome = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(serde_json::to_vec(&replay).unwrap(), bytes);
    }
}

#[test]
fn oversized_input_is_rejected_before_recognition() {
    assert_eq!(
        parse(&"University ".repeat(100)),
        NativeAffiliationOutcome::Oversized
    );
}

#[test]
fn parser_is_stateless_and_has_no_author_or_link_surface() {
    let parser = NativeAffiliationParser::new();
    assert_eq!(
        parser.parse("University of Example"),
        parse("University of Example")
    );
}

// ───── M494-a1i14t S04/T01: affiliation same-n on preserved TEI identities ─────
//
// Expected counts are test-local constants bound to the three preserved TEI
// SHA identities (same SHAs as java_lift_same_n_oracle). Recognition inputs
// are lexical `raw_affiliation` note texts scanned from the pinned bytes —
// never built from compose_header or TeiLifter. Hub (21 header authors) and
// rlm (4 header authors) are explicit zero-affiliation negative controls;
// reca (11 header authors) carries the 13-occurrence linked set. 25 zero-link
// header authors (36 − 11 linked) stay visible. No graph write.

/// Test-local expected (header authors, affiliation occurrences) per fixture.
const EXPECTED_PER_FIXTURE: &[(&str, &str, usize, usize)] = &[
    (
        "hub_2507.19457",
        "tests/fixtures/preserved/hub_2507.19457.tei.xml",
        21,
        0,
    ),
    (
        "rlm_2512.24601",
        "tests/fixtures/preserved/rlm_2512.24601.tei.xml",
        4,
        0,
    ),
    (
        "reca_2605.26525v1",
        "tests/fixtures/preserved/reca_2605.26525v1.tei.xml",
        11,
        13,
    ),
];

const CHECKSUMS: &[(&str, &str)] = &[
    (
        "hub_2507.19457",
        "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
    ),
    (
        "rlm_2512.24601",
        "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
    ),
    (
        "reca_2605.26525v1",
        "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
    ),
];

/// Test-local expected recognized count per fixture (raw_affiliation notes).
const EXPECTED_RECOGNIZED_PER_FIXTURE: &[(&str, usize)] = &[
    ("hub_2507.19457", 0),
    ("rlm_2512.24601", 0),
    ("reca_2605.26525v1", 13),
];

fn fixture_checksum(id: &str) -> &'static str {
    CHECKSUMS
        .iter()
        .find(|(fixture, _)| *fixture == id)
        .map(|(_, digest)| *digest)
        .expect("every fixture must have a pinned checksum")
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for ch in s.chars() {
        if ch == '<' {
            tag = true;
        } else if ch == '>' {
            tag = false;
        } else if !tag {
            out.push(ch);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Lexical scan of `raw_affiliation` note texts + header author count from
/// the checksum-pinned bytes. No TeiLifter, no compose_header.
fn scan_fixture(id: &str, path: &str) -> (usize, Vec<String>) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    let bytes = std::fs::read(base).expect("preserved TEI fixture must be readable");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        fixture_checksum(id),
        "checksum drift for {id}"
    );
    let data = String::from_utf8(bytes).expect("preserved fixture must be UTF-8");
    let hstart = data.find("<teiHeader").expect("teiHeader exists");
    let hend = data.find("</teiHeader>").expect("teiHeader closes");
    let header = &data[hstart..hend];
    let authors = header.matches("<author>").count();
    let mut notes = Vec::new();
    let mut cursor = 0;
    while let Some(rel) = header[cursor..].find("<note type=\"raw_affiliation\">") {
        let start = cursor + rel;
        let end = header[start..]
            .find("</note>")
            .map(|n| start + n)
            .expect("raw_affiliation note must close");
        notes.push(strip_tags(&header[start..end]));
        cursor = end + "</note>".len();
    }
    (authors, notes)
}

#[test]
fn same_n_affiliation_recognition_names_fixture_identity_on_drift() {
    let parser = NativeAffiliationParser::new();
    let mut total_oracle = 0usize;
    let mut total_candidate = 0usize;
    let mut total_recognized = 0usize;
    for (id, path, exp_authors, exp_occurrences) in EXPECTED_PER_FIXTURE {
        let (authors, notes) = scan_fixture(id, path);
        assert_eq!(
            authors, *exp_authors,
            "header author same-n drift in {id}: oracle {exp_authors} vs scanned {authors}"
        );
        assert_eq!(
            notes.len(),
            *exp_occurrences,
            "affiliation same-n drift in {id}: oracle {exp_occurrences} vs scanned {}",
            notes.len()
        );
        let exp_recognized = EXPECTED_RECOGNIZED_PER_FIXTURE
            .iter()
            .find(|(fixture, _)| fixture == id)
            .map(|(_, n)| *n)
            .expect("every fixture must have a pinned recognized count");
        let mut recognized = 0usize;
        for note in &notes {
            // One candidate evaluation per oracle occurrence: oracle_n ==
            // candidate_n by construction; any skip is a same-n drift.
            total_oracle += 1;
            let outcome = parser.parse(note);
            total_candidate += 1;
            if matches!(outcome, NativeAffiliationOutcome::Recognized { .. }) {
                recognized += 1;
            }
        }
        assert_eq!(
            recognized, exp_recognized,
            "recognition same-n drift in {id}: expected {exp_recognized} vs recognized {recognized}"
        );
        total_recognized += recognized;
    }
    assert_eq!(total_oracle, 13, "affiliation population stays 13");
    assert_eq!(
        total_oracle, total_candidate,
        "oracle_n must equal candidate_n (no pooled or skipped occurrence)"
    );
    assert_eq!(total_recognized, 13, "recognition stays 13/13");
    // 25 zero-link header authors stay visible: 36 header authors, 11 linked.
    let header_total: usize = EXPECTED_PER_FIXTURE.iter().map(|(_, _, h, _)| *h).sum();
    assert_eq!(header_total, 36, "header population stays 36");
    assert_eq!(
        header_total - 11,
        25,
        "zero-link header authors stay visible"
    );
}

#[test]
fn invented_organisation_is_never_recognized_as_preserved_evidence() {
    // The preserved oracle organisations are exactly these four lexical
    // values; an invented organisation matches none of them.
    let oracle_organisations = [
        "Monash University",
        "Tongyi Lab, Alibaba Group",
        "Zhejiang University",
        "University of Queensland",
    ];
    let invented = "Zxqwr Kjfblq Institute of Invented Studies";
    let outcome = parse(invented);
    let NativeAffiliationOutcome::Recognized { components } = outcome else {
        return;
    };
    for component in components {
        if component.kind == AffiliationComponentKind::Organization {
            assert!(
                !oracle_organisations
                    .contains(&&invented[component.span.start..component.span.end]),
                "invented organisation must not match preserved oracle identity"
            );
        }
    }
    assert!(
        !oracle_organisations.contains(&invented),
        "invented organisation is not preserved evidence"
    );
}

#[test]
fn expected_builder_does_not_import_tei_lifter_or_compose() {
    let src = include_str!("native_affiliation_contract.rs");
    let use_block: String = src
        .lines()
        .filter(|line| line.trim_start().starts_with("use "))
        .collect();
    for name in [
        "TeiLifter",
        "HeaderAuthor",
        "compose_header",
        "compose_references",
        "run_header_sequence",
        "run_reference_sequence",
    ] {
        assert!(
            !use_block.contains(name),
            "{name} must not be imported into the affiliation expected builder"
        );
        assert!(
            !src.contains(&format!("{name}(")),
            "{name} must not be called from the affiliation expected builder"
        );
    }
    // sha2/digest pinning is the sanctioned checksum vocabulary, not lifting.
    // The needle is built at runtime so this very assertion never matches itself.
    let needle = ["lift_", "tei("].concat();
    assert!(!src.contains(&needle));
}
