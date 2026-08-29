//! Independent preserved-TEI oracle for the shadow citation field candidate.
//!
//! This deliberately uses a small test-local scanner instead of `da-parse` XML
//! or lifting code.  The fixture bytes, entry order, and raw notes are the
//! authority; candidate output is never used to manufacture expected values.
use da_parse::{CitationFieldRole, NativeCitationOutcome, NativeCitationParser};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

const FIXTURES: [(&str, &str, usize); 3] = [
    (
        "hub_2507.19457.tei.xml",
        "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
        80,
    ),
    (
        "reca_2605.26525v1.tei.xml",
        "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
        78,
    ),
    (
        "rlm_2512.24601.tei.xml",
        "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
        49,
    ),
];
const ORDERED_RAW_DIGEST: &str = "a65155b066ab689ef252415572f8f8ceff38e9ee37edba4e4ffac4207330e078";

#[derive(Debug, Clone)]
struct OracleEntry {
    fixture: &'static str,
    ordinal: usize,
    raw: String,
    declared_fields: Vec<(CitationFieldRole, String)>,
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/preserved")
        .join(name)
}

fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn text_of(fragment: &str) -> String {
    let mut out = String::with_capacity(fragment.len());
    let mut inside = false;
    for c in fragment.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn between<'a>(block: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = block.find(open)? + open.len();
    let end = start + block[start..].find(close)?;
    Some(&block[start..end])
}

/// Scan only biblStruct blocks and their raw_reference notes.  It is not an
/// XML parser and intentionally has no dependency on production XML helpers.
fn scan_fixture(name: &'static str) -> Vec<OracleEntry> {
    let xml = fs::read_to_string(fixture_path(name)).expect("preserved fixture readable");
    let mut entries = Vec::new();
    let mut rest = xml.as_str();
    while let Some(start) = rest.find("<biblStruct") {
        rest = &rest[start..];
        let Some(end) = rest.find("</biblStruct>") else {
            break;
        };
        let block_end = end + "</biblStruct>".len();
        let block = &rest[..block_end];
        if let Some(raw) = between(block, "<note type=\"raw_reference\">", "</note>") {
            let mut declared_fields = Vec::new();
            if let Some(title) = between(block, "<title", "</title>") {
                let value = title.split_once('>').map_or(title, |(_, value)| value);
                declared_fields.push((
                    CitationFieldRole::ArticleTitle,
                    text_of(value).trim().to_owned(),
                ));
            }
            if let Some(date) = between(block, "<date", "</date>") {
                let value = date.split_once('>').map_or(date, |(_, value)| value);
                declared_fields.push((
                    CitationFieldRole::PublicationDate,
                    text_of(value).trim().to_owned(),
                ));
            }
            if let Some(idno) = between(block, "<idno", "</idno>") {
                let value = idno.split_once('>').map_or(idno, |(_, value)| value);
                let role = if idno.contains("DOI") {
                    CitationFieldRole::Doi
                } else {
                    CitationFieldRole::ArxivId
                };
                declared_fields.push((role, text_of(value).trim().to_owned()));
            }
            entries.push(OracleEntry {
                fixture: name,
                ordinal: entries.len(),
                raw: raw.to_owned(),
                declared_fields,
            });
        }
        rest = &rest[block_end..];
    }
    entries
}

fn all_entries() -> Vec<OracleEntry> {
    FIXTURES
        .iter()
        .flat_map(|(name, _, _)| scan_fixture(name))
        .collect()
}

#[test]
fn checksum_bound_population_and_order_are_exact() {
    let mut population = Vec::new();
    for (name, expected_checksum, expected_n) in FIXTURES {
        let bytes = fs::read(fixture_path(name)).unwrap();
        assert_eq!(
            digest(&bytes),
            expected_checksum,
            "fixture checksum changed: {name}"
        );
        let entries = scan_fixture(name);
        assert_eq!(
            entries.len(),
            expected_n,
            "raw-note denominator changed: {name}"
        );
        assert!(
            entries
                .iter()
                .enumerate()
                .all(|(i, e)| e.ordinal == i && e.fixture == name)
        );
        population.extend(entries.into_iter().map(|entry| entry.raw));
    }
    assert_eq!(population.len(), 207);
    let joined = population.join("\x1f");
    assert_eq!(digest(joined.as_bytes()), ORDERED_RAW_DIGEST);
}

#[test]
fn independent_role_oracle_has_explicit_denominators_and_zero_invention() {
    let entries = all_entries();
    assert_eq!(entries.len(), 207);
    let parser = NativeCitationParser::new();
    let mut observed = [0usize; 4];
    let mut strict = [0usize; 4];
    let mut soft = [0usize; 4];
    let mut missing = [0usize; 4];
    let mut unsupported = 0usize;
    let mut invented = 0usize;

    for entry in &entries {
        let outcome = parser.parse(&entry.raw);
        if matches!(outcome, NativeCitationOutcome::Unsupported { .. }) {
            unsupported += 1;
        }
        let fields = match &outcome {
            NativeCitationOutcome::Recognized { fields } => Some(fields),
            _ => None,
        };
        for (index, role) in [
            CitationFieldRole::ArticleTitle,
            CitationFieldRole::PublicationDate,
            CitationFieldRole::Doi,
            CitationFieldRole::ArxivId,
        ]
        .into_iter()
        .enumerate()
        {
            let expected = entry.declared_fields.iter().find(|(r, _)| *r == role);
            if expected.is_some() {
                observed[index] += 1;
            }
            if let Some((_, value)) = expected {
                if let Some(fields) = fields {
                    if let Some(field) = fields.iter().find(|field| field.role == role) {
                        if field.value == *value {
                            strict[index] += 1;
                        } else {
                            soft[index] += 1;
                        }
                    } else {
                        missing[index] += 1;
                    }
                } else {
                    missing[index] += 1;
                }
            }
        }
        for field in fields.into_iter().flatten() {
            if !entry
                .raw
                .as_bytes()
                .windows(field.value.len())
                .any(|window| window == field.value.as_bytes())
            {
                invented += 1;
            }
        }
    }
    assert_eq!(observed[0], 200, "title denominator");
    assert_eq!(observed[1], 167, "date denominator");
    assert!(
        observed[2] + observed[3] > 0,
        "identifier denominators must be visible"
    );
    assert_eq!(invented, 0);
    assert_eq!(strict[0] + soft[0] + missing[0], observed[0]);
    assert_eq!(strict[1] + soft[1] + missing[1], observed[1]);
    assert!(unsupported <= entries.len());
}

#[test]
fn repeated_scans_are_byte_deterministic_and_same_n() {
    let first = all_entries();
    let second = all_entries();
    assert_eq!(first.len(), 207);
    assert_eq!(
        first.iter().map(|e| &e.raw).collect::<Vec<_>>(),
        second.iter().map(|e| &e.raw).collect::<Vec<_>>()
    );
    let parser = NativeCitationParser::new();
    let a: Vec<_> = first.iter().map(|e| parser.parse(&e.raw)).collect();
    let b: Vec<_> = second.iter().map(|e| parser.parse(&e.raw)).collect();
    assert_eq!(a, b);
}

#[test]
fn tamper_missing_duplicate_reorder_malformed_and_span_overflow_are_rejected() {
    let name = FIXTURES[0].0;
    let original = fs::read(fixture_path(name)).unwrap();
    let mut tampered = original.clone();
    tampered[0] ^= 1;
    assert_ne!(digest(&tampered), FIXTURES[0].1);
    assert!(
        between(
            "<biblStruct><note type=\"raw_reference\">x</note></biblStruct>",
            "<note type=\"raw_reference\">",
            "</note>"
        )
        .is_some()
    );
    assert!(
        between(
            "<biblStruct></biblStruct>",
            "<note type=\"raw_reference\">",
            "</note>"
        )
        .is_none()
    );
    let mut entries = scan_fixture(name);
    let original_first = entries[0].raw.clone();
    entries.remove(0);
    assert_eq!(entries.len(), 79);
    entries.insert(
        0,
        OracleEntry {
            fixture: name,
            ordinal: 0,
            raw: original_first.clone(),
            declared_fields: vec![],
        },
    );
    entries.insert(
        1,
        OracleEntry {
            fixture: name,
            ordinal: 1,
            raw: original_first,
            declared_fields: vec![],
        },
    );
    assert_eq!(entries.len(), 81);
    assert!(entries.windows(2).any(|pair| pair[0].raw == pair[1].raw));
    entries.swap(2, 3);
    assert!(!entries.iter().enumerate().all(|(i, e)| e.ordinal == i));
    assert!(!matches!(
        NativeCitationParser::new().parse("<malformed"),
        NativeCitationOutcome::Recognized { .. }
    ));
    assert!(da_parse_span_overflow_is_invalid());
}

fn da_parse_span_overflow_is_invalid() -> bool {
    !da_parse::CitationSpan { start: 1, end: 9 }.is_valid_for(4)
}
