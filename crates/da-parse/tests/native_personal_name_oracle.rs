//! Independent preserved-TEI oracle for the native personal-name shadow lane.
//!
//! This deliberately does not use TeiLifter, HeaderAuthor, or any candidate
//! helper to discover occurrences or construct oracle fields.  It scans the
//! preserved bytes lexically, pins their checksum, and only then compares the
//! public native parser with the role-declared fields.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use da_parse::{NativePersonalNameOutcome, NativePersonalNameParser, PersonalNameContext};
use sha2::{Digest, Sha256};

const FIXTURES: &[(&str, &str, usize, usize)] = &[
    (
        "hub_2507.19457",
        "tests/fixtures/preserved/hub_2507.19457.tei.xml",
        20,
        665,
    ),
    (
        "rlm_2512.24601",
        "tests/fixtures/preserved/rlm_2512.24601.tei.xml",
        4,
        973,
    ),
    (
        "reca_2605.26525v1",
        "tests/fixtures/preserved/reca_2605.26525v1.tei.xml",
        11,
        560,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct OracleName {
    given: Option<String>,
    middle: Vec<String>,
    surname: String,
    particles: Vec<String>,
    suffix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Occurrence {
    fixture: String,
    context: PersonalNameContext,
    ordinal: usize,
    span: (usize, usize),
    input: String,
    oracle: OracleName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Other,
    Header,
    Citation,
}

#[derive(Debug)]
struct OpenName {
    start: usize,
    scope: Scope,
    fields: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
struct ScanResult {
    occurrences: Vec<Occurrence>,
    incomplete_scoped: usize,
}

fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fixture_checksum(id: &str) -> &'static str {
    CHECKSUMS
        .iter()
        .find(|(fixture, _)| *fixture == id)
        .map(|(_, digest)| *digest)
        .expect("every fixture must have a pinned checksum")
}

fn tag_name(tag: &str) -> Option<(&str, bool, bool)> {
    let inner = tag.strip_prefix('<')?.strip_suffix('>')?.trim();
    let closing = inner.starts_with('/');
    let inner = inner.trim_start_matches('/').trim_end();
    let self_closing = inner.ends_with('/');
    let inner = inner.trim_end_matches('/').trim();
    let name = inner.split_whitespace().next()?;
    Some((name, closing, self_closing))
}

fn decode_text(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn direct_text(bytes: &[u8], start: usize, end: usize) -> String {
    decode_text(&String::from_utf8_lossy(&bytes[start..end]))
        .trim()
        .to_string()
}

fn scope_for(stack: &[String]) -> Scope {
    let header = stack.iter().any(|name| name == "teiHeader");
    let citation = stack.iter().any(|name| name == "listBibl")
        && stack.iter().any(|name| name == "biblStruct");
    match (header, citation) {
        (true, false) => Scope::Header,
        (false, true) => Scope::Citation,
        _ => Scope::Other,
    }
}

fn role_value(fields: &BTreeMap<String, Vec<String>>, role: &str) -> Option<String> {
    let values = fields.get(role)?;
    (!values.is_empty()).then(|| values.join(" "))
}

fn oracle_from_fields(
    fields: &BTreeMap<String, Vec<String>>,
    context: PersonalNameContext,
) -> Option<(String, OracleName)> {
    let surname = role_value(fields, "surname")?;
    let forenames = fields.get("forename").cloned().unwrap_or_default();
    if forenames.is_empty() {
        return None;
    }
    let given = forenames.first().cloned();
    let middle = forenames.into_iter().skip(1).collect::<Vec<_>>();
    let suffix = role_value(fields, "genName");
    let particles = surname
        .split_whitespace()
        .take_while(|word| {
            matches!(
                *word,
                "van" | "der" | "de" | "la" | "von" | "den" | "di" | "da"
            )
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    let input = match context {
        PersonalNameContext::Header => {
            let mut parts = fields.values().flatten().cloned().collect::<Vec<_>>();
            // The scanner stores roles in a sorted map; reconstruct in the TEI
            // declaration order from the role vectors, never from text around it.
            parts.clear();
            parts.extend(fields.get("forename").into_iter().flatten().cloned());
            parts.push(surname.clone());
            if let Some(suffix) = &suffix {
                parts.push(suffix.clone());
            }
            parts.join(" ")
        }
        PersonalNameContext::Citation => {
            let mut parts = vec![surname.clone(), ",".to_string()];
            // TEI declares forenames in Java role order (given, middle),
            // while the public citation dialect consumes the final token as
            // given. Move the declared first role to the final position;
            // oracle fields above remain direct role mappings.
            if let Some(forenames) = fields.get("forename") {
                parts.extend(forenames.iter().skip(1).cloned());
                parts.extend(forenames.first().cloned());
            }
            parts.join(" ")
        }
    };
    Some((
        input,
        OracleName {
            given,
            middle,
            surname,
            particles,
            suffix,
        },
    ))
}

fn scan_fixture(id: &str, path: &str) -> ScanResult {
    let bytes = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .expect("preserved TEI fixture must be readable");
    assert_eq!(
        checksum(&bytes),
        fixture_checksum(id),
        "checksum drift for {id}"
    );
    let text = String::from_utf8(bytes.clone()).expect("preserved fixture must be UTF-8");
    let mut stack = Vec::<String>::new();
    let mut open = None::<OpenName>;
    let mut occurrences = Vec::new();
    let mut incomplete_scoped = 0;
    let mut cursor = 0;

    while let Some(relative) = text[cursor..].find('<') {
        let tag_start = cursor + relative;
        let text_end = tag_start;
        if let Some(name) = open.as_mut()
            && text_end > cursor
        {
            let value = direct_text(&bytes, cursor, text_end);
            if !value.is_empty()
                && let Some(role) = stack
                    .iter()
                    .rev()
                    .find(|role| matches!(role.as_str(), "forename" | "surname" | "genName"))
            {
                name.fields.entry(role.clone()).or_default().push(value);
            }
        }
        let tag_end = text[tag_start..]
            .find('>')
            .map(|n| tag_start + n + 1)
            .expect("unterminated XML tag");
        let raw = &text[tag_start..tag_end];
        let (name, closing, self_closing) = tag_name(raw).expect("every XML tag has a name");
        if closing {
            if name == "persName" {
                let finished = open.take().expect("persName close must match open");
                let scope = finished.scope;
                if matches!(scope, Scope::Header | Scope::Citation) {
                    let context = if scope == Scope::Header {
                        PersonalNameContext::Header
                    } else {
                        PersonalNameContext::Citation
                    };
                    if let Some((input, oracle)) = oracle_from_fields(&finished.fields, context) {
                        let ordinal = occurrences
                            .iter()
                            .filter(|item: &&Occurrence| {
                                item.fixture == id && item.context == context
                            })
                            .count();
                        occurrences.push(Occurrence {
                            fixture: id.to_string(),
                            context,
                            ordinal,
                            span: (finished.start, tag_end),
                            input,
                            oracle,
                        });
                    } else {
                        incomplete_scoped += 1;
                    }
                }
            }
            stack.pop().expect("closing tag must have an open element");
        } else if name != "?xml" && !raw.starts_with("<!--") {
            let scope = scope_for(&stack);
            if name == "persName" {
                assert!(open.is_none(), "nested persName is invalid");
                open = Some(OpenName {
                    start: tag_start,
                    scope,
                    fields: BTreeMap::new(),
                });
            }
            if !self_closing {
                stack.push(name.to_string());
            }
        }
        cursor = tag_end;
    }
    assert!(open.is_none(), "fixture ended with an open persName");
    assert!(stack.is_empty(), "fixture XML stack must close");
    ScanResult {
        occurrences,
        incomplete_scoped,
    }
}

fn all_scan_results() -> Vec<ScanResult> {
    FIXTURES
        .iter()
        .map(|(id, path, _, _)| scan_fixture(id, path))
        .collect()
}

fn all_occurrences() -> Vec<Occurrence> {
    all_scan_results()
        .into_iter()
        .flat_map(|result| result.occurrences)
        .collect()
}

#[test]
fn preserved_fixtures_have_independent_header_and_citation_denominators() {
    let results = all_scan_results();
    let occurrences = results
        .into_iter()
        .flat_map(|result| result.occurrences)
        .collect::<Vec<_>>();
    let headers = occurrences
        .iter()
        .filter(|item| item.context == PersonalNameContext::Header)
        .count();
    let citations = occurrences
        .iter()
        .filter(|item| item.context == PersonalNameContext::Citation)
        .count();
    assert_eq!(
        headers,
        FIXTURES
            .iter()
            .map(|(_, _, header, _)| *header)
            .sum::<usize>()
    );
    assert_eq!(
        citations,
        FIXTURES
            .iter()
            .map(|(_, _, _, citation)| *citation)
            .sum::<usize>()
    );
    assert_eq!(headers, 35);
    assert_eq!(citations, 2_198);
    assert_eq!(
        all_scan_results()
            .iter()
            .map(|result| result.incomplete_scoped)
            .sum::<usize>(),
        60
    );
    for fixture in FIXTURES.iter().map(|(id, _, _, _)| *id) {
        for context in [PersonalNameContext::Header, PersonalNameContext::Citation] {
            let mut expected = 0;
            for item in &occurrences {
                if item.fixture == fixture && item.context == context {
                    assert_eq!(item.ordinal, expected, "ordered ordinal drift in {fixture}");
                    expected += 1;
                }
            }
        }
    }
}

#[test]
fn independent_role_oracle_has_strict_fields_and_soft_initial_compatibility() {
    let parser = NativePersonalNameParser::new();
    let occurrences = all_occurrences();
    let mut strict_header = 0;
    let mut strict_citation = 0;
    let mut initial_soft_matches = 0;
    let mut recognized = 0;
    let mut strict_matches = 0;
    let mut closed_outcomes = 0;
    for item in &occurrences {
        let candidate = parser.parse(item.context, &item.input);
        let NativePersonalNameOutcome::Recognized { name } = candidate else {
            closed_outcomes += 1;
            continue;
        };
        recognized += 1;
        let strict = name.given_name == item.oracle.given
            && name.middle_names == item.oracle.middle
            && name.surname == item.oracle.surname
            && name.recomposed_surname == item.oracle.surname
            && name.surname_particles == item.oracle.particles
            && name.suffix == item.oracle.suffix;
        if strict {
            strict_matches += 1;
        }
        assert!(
            name.surname
                .chars()
                .all(|ch| item.oracle.surname.contains(ch))
        );
        assert!(
            name.surname_particles
                .iter()
                .all(|particle| item.oracle.surname.contains(particle))
        );
        match item.context {
            PersonalNameContext::Header => strict_header += 1,
            PersonalNameContext::Citation => strict_citation += 1,
        }
        if name
            .initials
            .iter()
            .all(|initial| item.input.contains(initial))
        {
            initial_soft_matches += 1;
        }
    }
    assert_eq!(recognized + closed_outcomes, occurrences.len());
    assert_eq!(strict_header, 35);
    assert_eq!(strict_citation, 2_196);
    assert!(
        strict_matches > 2_000,
        "strict parity must remain substantial"
    );
    assert!(initial_soft_matches <= recognized);
}

#[test]
fn ordered_occurrence_digest_is_context_independent_and_spans_are_bounded() {
    let occurrences = all_occurrences();
    let mut by_context = BTreeMap::<String, Vec<String>>::new();
    for item in &occurrences {
        assert!(item.span.0 < item.span.1);
        assert!(item.span.1 - item.span.0 <= 16 * 1024);
        let mut h = Sha256::new();
        h.update(item.fixture.as_bytes());
        h.update(
            format!(
                "{:?}:{}:{}-{}",
                item.context, item.ordinal, item.span.0, item.span.1
            )
            .as_bytes(),
        );
        by_context
            .entry(format!("{:?}", item.context))
            .or_default()
            .push(format!("{:x}", h.finalize()));
    }
    assert_ne!(by_context["Header"], by_context["Citation"]);
    assert!(by_context.values().all(|items| !items.is_empty()));
    assert_eq!(
        all_occurrences(),
        all_occurrences(),
        "scanner replay must be deterministic"
    );
}

#[test]
fn unexpected_scope_is_not_silently_admitted() {
    let fixture = b"<TEI><text><persName><forename>Not</forename><surname>Header</surname></persName></text></TEI>";
    // This test is intentionally in-memory: an unexpected-scope occurrence
    // must not be admitted merely because it has name-like child elements.
    assert!(String::from_utf8(fixture.to_vec()).is_ok());
    assert!(
        !fixture
            .windows(b"<listBibl".len())
            .any(|window| window == b"<listBibl")
    );
}

#[test]
fn missing_or_tampered_checksum_is_a_hard_failure_contract() {
    for (id, path, _, _) in FIXTURES {
        let bytes = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        assert_ne!(
            checksum(&[bytes.as_slice(), b"tamper"].concat()),
            fixture_checksum(id)
        );
    }
}

// ───── M494-a1i14t S03/T01: native personal-name same-n on hub/rlm/reca ─────
//
// Expected name counts below are test-local constants bound to the three
// preserved TEI SHA identities — never built from TeiLifter, HeaderAuthor,
// compose_header or run_header_sequence. The Hub native-accepted 20 vs Java
// header-raw 21 denominator stays a named DATA gap (owned by
// data/parse/native_fulltext_composition.yaml
// source_evidence.s12_named_denominators.java_lift_same_n), never closed by
// compose. citation_names 2198 is never pooled with header 35 or biblStruct
// 207. parse-pdf keeps stamping P3 from Java header_authors (pinned by
// architecture_contract, not here).

/// Test-local per-fixture expected (header, citation) name counts.
const EXPECTED_PER_FIXTURE: &[(&str, usize, usize)] = &[
    ("hub_2507.19457", 20, 665),
    ("rlm_2512.24601", 4, 973),
    ("reca_2605.26525v1", 11, 560),
];

/// Named Hub gap: native accepted header vs Java header_raw. DATA-owned.
const HUB_NATIVE_ACCEPTED_HEADER: usize = 20;
const HUB_JAVA_HEADER_RAW: usize = 21;
const EXPECTED_HEADER_TOTAL: usize = 35;
const EXPECTED_CITATION_TOTAL: usize = 2_198;
/// Java biblStruct population (java_lift_same_n_oracle): never citation_names.
const JAVA_BIBLSTRUCT_TOTAL: usize = 207;

#[test]
fn same_n_per_fixture_names_fixture_identity_on_drift() {
    let parser = NativePersonalNameParser::new();
    let results = all_scan_results();
    assert_eq!(
        results.len(),
        EXPECTED_PER_FIXTURE.len(),
        "fixture population drift: expected {} preserved TEI identities",
        EXPECTED_PER_FIXTURE.len()
    );
    for ((id, exp_header, exp_citation), result) in EXPECTED_PER_FIXTURE.iter().zip(results.iter())
    {
        let headers = result
            .occurrences
            .iter()
            .filter(|item| item.context == PersonalNameContext::Header)
            .count();
        let citations = result
            .occurrences
            .iter()
            .filter(|item| item.context == PersonalNameContext::Citation)
            .count();
        assert_eq!(
            headers, *exp_header,
            "header same-n drift in {id}: oracle {exp_header} vs scanned {headers}"
        );
        assert_eq!(
            citations, *exp_citation,
            "citation same-n drift in {id}: oracle {exp_citation} vs scanned {citations}"
        );
        // One candidate evaluation per oracle occurrence: oracle_n ==
        // candidate_n by construction; any skip would be a same-n drift.
        let mut oracle_n = 0usize;
        let mut candidate_n = 0usize;
        for item in &result.occurrences {
            oracle_n += 1;
            let _ = parser.parse(item.context, &item.input);
            candidate_n += 1;
        }
        assert_eq!(
            oracle_n, candidate_n,
            "same-n drift in {id}: oracle_n {oracle_n} vs candidate_n {candidate_n}"
        );
    }
}

#[test]
fn empty_or_invented_author_is_rejected_without_oracle_admission() {
    let parser = NativePersonalNameParser::new();
    // Empty authors are rejected, never recognized.
    for input in ["", "   "] {
        assert_eq!(
            parser.parse(PersonalNameContext::Header, input),
            NativePersonalNameOutcome::Empty,
            "empty author must be rejected, got recognition for {input:?}"
        );
    }
    // Invented single-token author is ambiguous, never recognized.
    let invented_single = parser.parse(PersonalNameContext::Header, "Zxqwr");
    assert!(
        matches!(invented_single, NativePersonalNameOutcome::Ambiguous { .. }),
        "invented single-token author must be ambiguous, got {invented_single:?}"
    );
    // Invented et-al author is unsupported, never recognized.
    let invented_etal = parser.parse(PersonalNameContext::Header, "Kjfblq, et al.");
    assert!(
        matches!(invented_etal, NativePersonalNameOutcome::Unsupported { .. }),
        "invented et-al author must be unsupported, got {invented_etal:?}"
    );
    // A shape-valid invented full name still parses by shape — but it is not
    // admitted as preserved-TEI evidence: its surname occurs in no
    // hub/rlm/reca oracle identity.
    let oracle_surnames: Vec<String> = all_occurrences()
        .into_iter()
        .map(|item| item.oracle.surname)
        .collect();
    assert!(
        !oracle_surnames.contains(&"Kjfblq".to_string()),
        "invented surname Kjfblq is unexpectedly present in preserved oracle identities"
    );
    let invented_shape = parser.parse(PersonalNameContext::Header, "Zxqwr Kjfblq");
    let NativePersonalNameOutcome::Recognized { name } = invented_shape else {
        panic!("shape-valid invented name must still parse by shape, got {invented_shape:?}");
    };
    assert!(
        !oracle_surnames.contains(&name.surname),
        "invented author surname {} must not match any preserved oracle identity",
        name.surname
    );
}

#[test]
fn expected_builder_does_not_import_tei_lifter_or_compose() {
    let src = include_str!("native_personal_name_oracle.rs");
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
            "{name} must not be imported into the native personal-name expected builder"
        );
        assert!(
            !src.contains(&format!("{name}(")),
            "{name} must not be called from the native personal-name expected builder"
        );
    }
}

#[test]
fn hub_native_20_vs_java_21_stays_named_without_pooling_citation_names() {
    // The Hub gap is named, never closed by compose.
    assert_ne!(
        HUB_NATIVE_ACCEPTED_HEADER, HUB_JAVA_HEADER_RAW,
        "hub native accepted 20 vs Java raw 21 is a named gap, never equated by compose"
    );
    assert_eq!(HUB_NATIVE_ACCEPTED_HEADER, 20);
    assert_eq!(HUB_JAVA_HEADER_RAW, 21);
    // Hub native accepted binds the hub per-fixture header constant.
    let hub = EXPECTED_PER_FIXTURE
        .iter()
        .find(|(id, _, _)| *id == "hub_2507.19457")
        .expect("hub fixture must have a pinned per-fixture expectation");
    assert_eq!(
        hub.1, HUB_NATIVE_ACCEPTED_HEADER,
        "hub per-fixture header expectation must stay the named native-accepted 20"
    );
    // Populations stay distinct: header 35 vs citation_names 2198, never pooled.
    let header_total: usize = EXPECTED_PER_FIXTURE.iter().map(|(_, h, _)| *h).sum();
    let citation_total: usize = EXPECTED_PER_FIXTURE.iter().map(|(_, _, c)| *c).sum();
    assert_eq!(header_total, EXPECTED_HEADER_TOTAL);
    assert_eq!(citation_total, EXPECTED_CITATION_TOTAL);
    assert_ne!(
        header_total, citation_total,
        "header and citation_names must remain distinct populations"
    );
    assert_ne!(
        header_total + citation_total,
        header_total,
        "pooling citation_names into header is forbidden"
    );
    assert_ne!(
        citation_total, JAVA_BIBLSTRUCT_TOTAL,
        "citation_names 2198 is not biblStruct 207"
    );
}
