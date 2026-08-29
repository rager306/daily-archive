//! Independent preserved-TEI oracle for the native affiliation shadow lane.
//!
//! The oracle is intentionally lexical and test-local. It pins the preserved
//! bytes before scanning direct `<author>` nesting, and never calls a parser,
//! lifter, candidate helper, or production TEI representation to discover
//! affiliations or links.

use std::fs;
use std::path::Path;

use da_parse::{
    AffiliationSpan, NativeAffiliationLinkOutcome, NativeAffiliationLinker,
    NativeAffiliationNesting, NativeAffiliationOccurrence, RecognizedPersonalName,
};
use sha2::{Digest, Sha256};

const FIXTURES: &[(&str, &str)] = &[
    (
        "hub_2507.19457",
        "tests/fixtures/preserved/hub_2507.19457.tei.xml",
    ),
    (
        "rlm_2512.24601",
        "tests/fixtures/preserved/rlm_2512.24601.tei.xml",
    ),
    (
        "reca_2605.26525v1",
        "tests/fixtures/preserved/reca_2605.26525v1.tei.xml",
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
struct OracleAffiliation {
    fixture: String,
    author_ordinal: usize,
    author_span: AffiliationSpan,
    key: String,
    ordinal: usize,
    affiliation_span: AffiliationSpan,
    note_span: AffiliationSpan,
    marker_span: Option<AffiliationSpan>,
    organization_span: Option<AffiliationSpan>,
    organization_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OracleLink {
    fixture: String,
    author_ordinal: usize,
    affiliation_ordinal: usize,
    key: String,
    author_span: AffiliationSpan,
    affiliation_span: AffiliationSpan,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn expected_checksum(fixture: &str) -> &'static str {
    CHECKSUMS
        .iter()
        .find(|(id, _)| *id == fixture)
        .map(|(_, checksum)| *checksum)
        .expect("every fixture has a pinned checksum")
}

fn tag_end(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .position(|b| *b == b'>')
        .map(|offset| from + offset + 1)
        .expect("preserved XML tags are terminated")
}

fn attribute(tag: &[u8], name: &[u8]) -> Option<String> {
    let marker = [name, b"=\""].concat();
    let start = tag.windows(marker.len()).position(|w| w == marker)? + marker.len();
    let end = tag[start..].iter().position(|b| *b == b'\"')? + start;
    Some(String::from_utf8(tag[start..end].to_vec()).expect("XML attributes are UTF-8"))
}

fn direct_tag_span(
    bytes: &[u8],
    block_start: usize,
    block_end: usize,
    name: &[u8],
) -> Option<AffiliationSpan> {
    let open = [b"<".as_slice(), name].concat();
    let start = bytes[block_start..block_end]
        .windows(open.len())
        .position(|w| w == open)
        .map(|offset| block_start + offset)?;
    let close = [b"</".as_slice(), name, b">"].concat();
    let close_start = bytes[start..block_end]
        .windows(close.len())
        .position(|w| w == close)
        .map(|offset| start + offset)?;
    Some(AffiliationSpan {
        start,
        end: start + close_start + close.len(),
    })
}

fn direct_affiliation_spans(author: &[u8], author_start: usize) -> Vec<(String, AffiliationSpan)> {
    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = author[cursor..]
        .windows(b"<affiliation".len())
        .position(|w| w == b"<affiliation")
    {
        let start = cursor + relative;
        let open_end = tag_end(author, start);
        let close = b"</affiliation>";
        let close_start = author[open_end..]
            .windows(close.len())
            .position(|w| w == close)
            .map(|offset| open_end + offset)
            .expect("affiliation must close");
        let key = attribute(&author[start..open_end], b"key").expect("affiliation key is declared");
        out.push((
            key,
            AffiliationSpan {
                start: author_start + start,
                end: author_start + close_start + close.len(),
            },
        ));
        cursor = close_start + close.len();
    }
    out
}

fn scan_fixture(
    fixture: &str,
    relative_path: &str,
) -> (Vec<OracleAffiliation>, Vec<OracleLink>, usize) {
    let bytes = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path))
        .expect("preserved fixture must be readable");
    assert_eq!(
        sha256(&bytes),
        expected_checksum(fixture),
        "checksum drift for {fixture}"
    );
    let header = direct_tag_span(&bytes, 0, bytes.len(), b"teiHeader").expect("teiHeader exists");
    let header_bytes = &bytes[header.start..header.end];
    let mut affiliations = Vec::new();
    let mut links = Vec::new();
    let mut author_ordinal = 0;
    let mut cursor = 0;
    while let Some(relative) = header_bytes[cursor..]
        .windows(b"<author>".len())
        .position(|w| w == b"<author>")
    {
        let author_start = cursor + relative;
        let close = b"</author>";
        let author_close = header_bytes[author_start..]
            .windows(close.len())
            .position(|w| w == close)
            .map(|offset| author_start + offset + close.len())
            .expect("author must close");
        let absolute_author = AffiliationSpan {
            start: header.start + author_start,
            end: header.start + author_close,
        };
        let author = &header_bytes[author_start..author_close];
        let occurrences = direct_affiliation_spans(author, absolute_author.start);
        for (ordinal, (key, affiliation_span)) in occurrences.into_iter().enumerate() {
            let aff = &bytes[affiliation_span.start..affiliation_span.end];
            let note = direct_tag_span(aff, 0, aff.len(), b"note").expect("raw note exists");
            let note = AffiliationSpan {
                start: affiliation_span.start + note.start,
                end: affiliation_span.start + note.end,
            };
            assert!(
                bytes[note.start..note.end]
                    .windows(b"type=\"raw_affiliation\"".len())
                    .any(|window| window == b"type=\"raw_affiliation\""),
                "note must be raw_affiliation"
            );
            let marker = direct_tag_span(
                &bytes[note.start..note.end],
                0,
                note.end - note.start,
                b"label",
            )
            .map(|s| AffiliationSpan {
                start: note.start + s.start,
                end: note.start + s.end,
            });
            let organization =
                direct_tag_span(aff, 0, aff.len(), b"orgName").map(|s| AffiliationSpan {
                    start: affiliation_span.start + s.start,
                    end: affiliation_span.start + s.end,
                });
            let organization_type = organization.and_then(|org| {
                let open_end = tag_end(&bytes, org.start);
                attribute(&bytes[org.start..open_end], b"type")
            });
            affiliations.push(OracleAffiliation {
                fixture: fixture.to_owned(),
                author_ordinal,
                author_span: absolute_author,
                key: key.clone(),
                ordinal,
                affiliation_span,
                note_span: note,
                marker_span: marker,
                organization_span: organization,
                organization_type,
            });
            links.push(OracleLink {
                fixture: fixture.to_owned(),
                author_ordinal,
                affiliation_ordinal: ordinal,
                key,
                author_span: absolute_author,
                affiliation_span,
            });
        }
        author_ordinal += 1;
        cursor = author_close;
    }
    (affiliations, links, author_ordinal)
}

fn all() -> (Vec<OracleAffiliation>, Vec<OracleLink>, Vec<usize>) {
    let mut affiliations = Vec::new();
    let mut links = Vec::new();
    let mut authors = Vec::new();
    for (fixture, path) in FIXTURES {
        let (mut a, mut l, n) = scan_fixture(fixture, path);
        affiliations.append(&mut a);
        links.append(&mut l);
        authors.push(n);
    }
    (affiliations, links, authors)
}

fn digest<T: std::fmt::Debug>(items: &[T]) -> String {
    sha256(format!("{items:?}").as_bytes())
}

fn accepted_name() -> RecognizedPersonalName {
    RecognizedPersonalName {
        given_name: Some("Accepted".into()),
        middle_names: vec![],
        surname: "Name".into(),
        recomposed_surname: "Name".into(),
        surname_particles: vec![],
        suffix: None,
        initials: vec![],
    }
}

#[test]
fn frozen_oracle_keeps_recognition_and_linking_populations_separate() {
    let (affiliations, links, authors) = all();
    assert_eq!(authors, vec![21, 4, 11]);
    assert_eq!(affiliations.len(), 13);
    assert_eq!(links.len(), 13);
    assert_eq!(
        affiliations
            .iter()
            .filter(|a| a.organization_type.as_deref() == Some("institution"))
            .count(),
        9
    );
    assert_eq!(
        affiliations
            .iter()
            .filter(|a| a.organization_type.as_deref() == Some("laboratory"))
            .count(),
        4
    );
    assert_eq!(
        affiliations
            .iter()
            .filter(|a| a.organization_span.is_some())
            .count(),
        13
    );
    assert_eq!(
        affiliations
            .iter()
            .filter(|a| a.marker_span.is_some())
            .count(),
        13
    );
    assert_eq!(
        affiliations
            .iter()
            .filter(|a| a.note_span.start < a.note_span.end)
            .count(),
        13
    );
    assert_eq!(affiliations.iter().filter(|a| a.key == "aff0").count(), 2);
    assert_eq!(affiliations.iter().filter(|a| a.key == "aff1").count(), 4);
    assert_eq!(affiliations.iter().filter(|a| a.key == "aff2").count(), 6);
    assert_eq!(affiliations.iter().filter(|a| a.key == "aff3").count(), 1);
    assert_eq!(
        affiliations
            .iter()
            .map(|a| a.author_ordinal)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        11
    );
    assert_eq!(
        authors.iter().sum::<usize>() - 11,
        25,
        "zero-link header authors"
    );
    assert_eq!(
        affiliations
            .iter()
            .filter(|a| a.affiliation_span.start < a.affiliation_span.end
                && a.organization_span.is_none())
            .count(),
        0,
        "address-only affiliation is not admitted"
    );
    assert_eq!(digest(&affiliations), digest(&all().0));
    assert_eq!(digest(&links), digest(&all().1));
}

#[test]
fn lexical_oracle_proves_direct_spans_and_independent_linker_parity() {
    let (affiliations, oracle_links, _) = all();
    let mut candidate_links = Vec::new();
    for author in affiliations
        .iter()
        .filter(|a| a.fixture == "reca_2605.26525v1")
        .map(|a| a.author_ordinal)
        .collect::<std::collections::BTreeSet<_>>()
    {
        let rows = affiliations
            .iter()
            .filter(|a| a.fixture == "reca_2605.26525v1" && a.author_ordinal == author)
            .collect::<Vec<_>>();
        let occurrences = rows
            .iter()
            .map(|a| NativeAffiliationOccurrence {
                document_id: "reca_2605.26525v1".into(),
                key: a.key.clone(),
                ordinal: a.ordinal,
                span: a.affiliation_span,
                nesting: NativeAffiliationNesting::DirectAuthorAffiliation,
            })
            .collect::<Vec<_>>();
        let outcome = NativeAffiliationLinker::new().link(
            &accepted_name(),
            "reca_2605.26525v1",
            author,
            rows[0].author_span,
            &occurrences,
        );
        let NativeAffiliationLinkOutcome::Linked {
            author: evidence,
            links,
        } = outcome
        else {
            panic!("oracle rows must link")
        };
        candidate_links.extend(links.into_iter().map(|link| OracleLink {
            fixture: "reca_2605.26525v1".into(),
            author_ordinal: evidence.ordinal,
            affiliation_ordinal: link.affiliation_ordinal,
            key: link.key,
            author_span: evidence.span,
            affiliation_span: link.affiliation_span,
        }));
    }
    let expected_links = oracle_links
        .iter()
        .filter(|link| link.fixture == "reca_2605.26525v1")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(candidate_links, expected_links);
    for item in affiliations.iter().chain(affiliations.iter()) {
        assert!(item.author_span.start < item.author_span.end);
        assert!(item.affiliation_span.start < item.affiliation_span.end);
        assert!(item.note_span.start >= item.affiliation_span.start);
        assert!(item.note_span.end <= item.affiliation_span.end);
    }
}

#[test]
fn hub_and_rlm_are_explicit_zero_link_negative_controls() {
    let (affiliations, links, _) = all();
    assert!(
        affiliations
            .iter()
            .all(|a| a.fixture == "reca_2605.26525v1")
    );
    assert!(links.iter().all(|l| l.fixture == "reca_2605.26525v1"));
}

fn compare_keys(
    expected: &[OracleAffiliation],
    actual: &[OracleAffiliation],
) -> Result<(), String> {
    if expected.len() != actual.len() {
        return Err("lost_or_extra_occurrence".into());
    }
    for (left, right) in expected.iter().zip(actual) {
        if left.fixture != right.fixture || left.ordinal != right.ordinal {
            return Err("reordered_or_relabelled_occurrence".into());
        }
        if left.key != right.key {
            return Err("relabelled_key".into());
        }
        if left.affiliation_span != right.affiliation_span {
            return Err("span_drift".into());
        }
    }
    Ok(())
}

#[test]
fn synthetic_drop_duplicate_reorder_relabel_and_invented_controls_fail_closed() {
    let (expected, _, _) = all();
    let mut dropped = expected.clone();
    dropped.pop();
    assert!(compare_keys(&expected, &dropped).is_err());
    let mut duplicate = expected.clone();
    duplicate.push(expected[0].clone());
    assert!(compare_keys(&expected, &duplicate).is_err());
    let mut reordered = expected.clone();
    reordered.swap(0, 1);
    assert!(compare_keys(&expected, &reordered).is_err());
    let mut relabelled = expected.clone();
    relabelled[0].key = "foreign".into();
    assert!(compare_keys(&expected, &relabelled).is_err());
    let mut invented = expected.clone();
    invented[0].affiliation_span = AffiliationSpan { start: 1, end: 2 };
    assert!(compare_keys(&expected, &invented).is_err());
}
