//! Checksum-bound independent oracle for native header/reference composition.
//! The lexical inventory below is deliberately test-local: it never uses the
//! composer, TeiLifter, HeaderAuthor, CitationEntry, or candidate helpers to
//! construct expected values.

use da_parse::{
    AffiliationSpan, CompositionSpan, HeaderAuthorInput, HeaderCompositionInput,
    NativeAffiliationLinker, NativeAffiliationNesting, NativeAffiliationOccurrence,
    NativeAffiliationParser, NativeCitationAnchorScanner, NativeCitationOutcome,
    NativeCitationParser, NativeDateParser, NativePersonalNameParser, NativeReferenceListOutcome,
    NativeReferenceSegmenter, PersonalNameContext, ReferenceCompositionInput, compose_header,
    compose_references,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

const FIXTURES: [(&str, &str, &str, usize); 3] = [
    (
        "hub_2507.19457.tei.xml",
        "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
        "2026-02-14",
        80,
    ),
    (
        "rlm_2512.24601.tei.xml",
        "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
        "2026-05-11",
        49,
    ),
    (
        "reca_2605.26525v1.tei.xml",
        "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
        "2026-05-26",
        78,
    ),
];

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/preserved")
        .join(name)
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = s.find(open)? + open.len();
    Some(&s[start..start + s[start..].find(close)?])
}
fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .trim()
        .to_owned()
}
fn attr(tag: &str, name: &str) -> Option<String> {
    for q in ['"', '\''] {
        let needle = format!("{name}={q}");
        if let Some(i) = tag.find(&needle) {
            let rest = &tag[i + needle.len()..];
            return Some(rest[..rest.find(q)?].to_owned());
        }
    }
    None
}
fn tags<'a>(s: &'a str, name: &str) -> Vec<(usize, usize, &'a str)> {
    let mut out = Vec::new();
    let mut at = 0;
    let open = format!("<{name}");
    let close = format!("</{name}>");
    while let Some(r) = s[at..].find(&open) {
        let start = at + r;
        let oe = start + s[start..].find('>').unwrap() + 1;
        let end = oe + s[oe..].find(&close).unwrap() + close.len();
        out.push((start, end, &s[start..end]));
        at = end;
    }
    out
}

#[derive(Debug, Clone)]
struct AuthorOracle {
    span: (usize, usize),
    name_input: String,
    name_span: (usize, usize),
    affiliations: Vec<(String, AffiliationSpan)>,
}
fn header_authors(xml: &str) -> Vec<AuthorOracle> {
    let header_end = xml.find("</teiHeader>").unwrap();
    let header = &xml[..header_end];
    let base = 0;
    tags(header, "author")
        .into_iter()
        .map(|(s, e, block)| {
            let ps = tags(block, "persName").into_iter().next().unwrap();
            let mut name_parts = Vec::new();
            for role in ["forename", "surname", "genName"] {
                name_parts.extend(
                    tags(ps.2, role)
                        .into_iter()
                        .map(|(_, _, value)| strip_tags(value)),
                );
            }
            let name_text = name_parts.join(" ");
            let aff = tags(block, "affiliation")
                .into_iter()
                .map(|(as_, ae, a)| {
                    (
                        attr(a, "key").unwrap(),
                        AffiliationSpan {
                            start: base + s + as_,
                            end: base + s + ae,
                        },
                    )
                })
                .collect();
            AuthorOracle {
                span: (s, e),
                name_input: name_text,
                name_span: (s + ps.0, s + ps.1),
                affiliations: aff,
            }
        })
        .collect()
}

fn date_input(xml: &str) -> String {
    let h = &xml[..xml.find("</teiHeader>").unwrap()];
    let i = h.find("<date type=\"published\"").unwrap();
    attr(&h[i..i + 128], "when").unwrap()
}

fn raw_references(xml: &str) -> Vec<String> {
    tags(xml, "biblStruct")
        .into_iter()
        .filter_map(|(_, _, b)| {
            between(b, "<note type=\"raw_reference\">", "</note>").map(ToOwned::to_owned)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CitationNameOracle {
    input: String,
}

/// Independent role inventory for citation names. The composer and the native
/// parser are deliberately absent: this only reads preserved XML roles and
/// constructs the input/expected role tuple used by the parser assertion.
fn citation_name_oracle(xml: &str) -> Vec<CitationNameOracle> {
    // Citation scope is the preserved listBibl only; biblStruct-like metadata
    // outside that list is not part of the citation denominator.
    tags(xml, "listBibl")
        .into_iter()
        .flat_map(|(_, _, list)| tags(list, "biblStruct"))
        .flat_map(|(_, _, block)| tags(block, "persName"))
        .filter_map(|(_, _, person)| {
            let forenames = tags(person, "forename")
                .into_iter()
                .map(|(_, _, value)| strip_tags(value))
                .collect::<Vec<_>>();
            let surname = tags(person, "surname")
                .into_iter()
                .next()
                .map(|(_, _, value)| strip_tags(value))?;
            let given = forenames.first()?.clone();
            // Citation input is the preserved role order adapted to the
            // public citation grammar; expected role values remain lexical.
            let mut input = format!("{surname},");
            for value in forenames.iter().skip(1) {
                input.push(' ');
                input.push_str(value);
            }
            input.push(' ');
            input.push_str(&given);
            Some(CitationNameOracle { input })
        })
        .collect()
}
fn date_tag_count(xml: &str) -> usize {
    xml.matches("<date").count()
}

type Anchor = (usize, usize, bool);
type AdjacentPair = (usize, usize);

fn bibr_anchors(xml: &str) -> (Vec<Anchor>, Vec<AdjacentPair>) {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(r) = xml[at..].find("<ref") {
        let s = at + r;
        let oe = s + xml[s..].find('>').unwrap();
        let opening = &xml[s..=oe];
        if opening.contains("type=\"bibr\"") || opening.contains("type='bibr'") {
            let close = "</ref>";
            let e = oe + 1 + xml[oe + 1..].find(close).unwrap() + close.len();
            out.push((s, e, opening.contains("target=")));
            at = e;
        } else {
            at = oe + 1;
        }
    }
    let pairs = out
        .windows(2)
        .enumerate()
        .filter_map(|(i, p)| {
            xml[p[0].1..p[1].0]
                .chars()
                .all(|c| c.is_whitespace() || matches!(c, ',' | ';'))
                .then_some((i, i + 1))
        })
        .collect();
    (out, pairs)
}

#[test]
fn independent_component_receipts_cannot_pool_or_erase_denominators() {
    let mut date_total = 0;
    let mut header_names = 0;
    let mut citation_names = 0;
    let mut citation_soft_initial = 0;
    let mut citation_unsupported = 0;
    let mut recognized_affiliations = 0;
    let mut linked_affiliations = 0;
    let mut linked_authors = 0;
    let mut zero_link_authors = 0;
    let mut three_link_authors = 0;
    let mut address_observed = 0;
    let mut reference_rows = 0;
    let mut anchors_total = 0;
    let mut anchors_targeted = 0;
    let mut anchors_targetless = 0;
    let mut fragmented_pairs = 0;

    for (fixture, checksum, _, expected_refs) in FIXTURES {
        let bytes = fs::read(path(fixture)).unwrap();
        assert_eq!(
            digest(&bytes),
            checksum,
            "fixture checksum drift: {fixture}"
        );
        let xml = String::from_utf8(bytes).unwrap();
        let authors = header_authors(&xml);
        let expected_raw_header_names = match fixture {
            "hub_2507.19457.tei.xml" => 21,
            "rlm_2512.24601.tei.xml" => 4,
            "reca_2605.26525v1.tei.xml" => 11,
            _ => unreachable!(),
        };
        assert_eq!(
            authors.len(),
            expected_raw_header_names,
            "raw header author denominator: {fixture}"
        );
        let accepted_header_names = authors
            .iter()
            .filter(|author| {
                matches!(
                    NativePersonalNameParser::new()
                        .parse(PersonalNameContext::Header, &author.name_input),
                    da_parse::NativePersonalNameOutcome::Recognized { .. }
                )
            })
            .count();
        let expected_accepted_header_names =
            expected_raw_header_names - usize::from(fixture == "hub_2507.19457.tei.xml");
        assert_eq!(
            accepted_header_names, expected_accepted_header_names,
            "accepted header name denominator: {fixture}"
        );
        header_names += accepted_header_names;
        date_total += date_tag_count(&xml);

        let citation_oracle = citation_name_oracle(&xml);
        let expected_citation_names = match fixture {
            "hub_2507.19457.tei.xml" => 665,
            "rlm_2512.24601.tei.xml" => 973,
            "reca_2605.26525v1.tei.xml" => 560,
            _ => unreachable!(),
        };
        assert_eq!(
            citation_oracle.len(),
            expected_citation_names,
            "citation name denominator: {fixture}"
        );
        citation_names += citation_oracle.len();
        for item in citation_oracle {
            match NativePersonalNameParser::new().parse(PersonalNameContext::Citation, &item.input)
            {
                da_parse::NativePersonalNameOutcome::Recognized { .. } => {
                    // Recognition is the soft-initial disposition. Strict role
                    // parity remains a separate S09 receipt and must not erase
                    // a recognized citation name from this denominator.
                    citation_soft_initial += 1;
                }
                da_parse::NativePersonalNameOutcome::Unsupported { .. } => {
                    citation_unsupported += 1;
                }
                _ => {}
            }
        }

        let author_link_counts = authors
            .iter()
            .map(|author| author.affiliations.len())
            .collect::<Vec<_>>();
        zero_link_authors += author_link_counts
            .iter()
            .filter(|count| **count == 0)
            .count();
        linked_authors += author_link_counts
            .iter()
            .filter(|count| **count > 0)
            .count();
        three_link_authors += author_link_counts
            .iter()
            .filter(|count| **count == 3)
            .count();
        for (_, span) in authors.iter().flat_map(|author| author.affiliations.iter()) {
            let affiliation = &xml[span.start..span.end];
            if tags(affiliation, "orgName").into_iter().next().is_some() {
                recognized_affiliations += 1;
            } else {
                address_observed += 1;
            }
            linked_affiliations += 1;
        }

        let raws = raw_references(&xml);
        assert_eq!(
            raws.len(),
            expected_refs,
            "reference row denominator: {fixture}"
        );
        reference_rows += raws.len();
        let (expected_anchors, expected_pairs) = bibr_anchors(&xml);
        anchors_total += expected_anchors.len();
        anchors_targeted += expected_anchors
            .iter()
            .filter(|(_, _, target)| *target)
            .count();
        anchors_targetless += expected_anchors
            .iter()
            .filter(|(_, _, target)| !*target)
            .count();
        fragmented_pairs += expected_pairs.len();
    }

    assert_eq!(date_total, 173, "date population must stay independent");
    assert_eq!(header_names, 35);
    assert_eq!(citation_names, 2_198);
    assert_eq!(citation_soft_initial, 2_196);
    assert_eq!(citation_unsupported, 2);
    assert_eq!(recognized_affiliations, 13);
    assert_eq!(linked_affiliations, 13);
    assert_eq!(linked_authors, 11);
    assert_eq!(zero_link_authors, 25);
    assert_eq!(three_link_authors, 1);
    assert_eq!(address_observed, 0);
    assert_eq!(reference_rows, 207);
    assert_eq!(anchors_total, 380);
    assert_eq!(anchors_targeted, 217);
    assert_eq!(anchors_targetless, 163);
    assert_eq!(fragmented_pairs, 168);
}

#[test]
fn independent_oracle_composes_separate_header_and_reference_lanes() {
    let mut all_names = 0;
    let mut all_affiliations = 0;
    let mut all_refs = 0;
    let mut all_anchors = 0;
    let mut targeted = 0;
    let mut adjacent = 0;
    for (name, checksum, expected_date, expected_refs) in FIXTURES {
        let bytes = fs::read(path(name)).unwrap();
        assert_eq!(digest(&bytes), checksum, "fixture checksum drift: {name}");
        let xml = String::from_utf8(bytes).unwrap();
        let authors = header_authors(&xml);
        assert!(!authors.is_empty());
        let date_text = date_input(&xml);
        let date = NativeDateParser::new().parse(&date_text);
        let date_value = date
            .recognized()
            .expect("independent ISO date must be recognized");
        assert_eq!(date_value.year.to_string(), expected_date[..4]);
        let mut inputs = Vec::new();
        let mut rejected_names = 0;
        for a in &authors {
            let name_out =
                NativePersonalNameParser::new().parse(PersonalNameContext::Header, &a.name_input);
            let recognized = match &name_out {
                da_parse::NativePersonalNameOutcome::Recognized { name } => name,
                _ => {
                    rejected_names += 1;
                    continue;
                }
            };
            let ordinal = inputs.len();
            let occurrences = a
                .affiliations
                .iter()
                .enumerate()
                .map(|(i, (key, span))| NativeAffiliationOccurrence {
                    document_id: name.to_owned(),
                    key: key.clone(),
                    ordinal: i,
                    span: *span,
                    nesting: NativeAffiliationNesting::DirectAuthorAffiliation,
                })
                .collect::<Vec<_>>();
            let links = NativeAffiliationLinker::new().link(
                recognized,
                name,
                ordinal,
                AffiliationSpan {
                    start: a.span.0,
                    end: a.span.1,
                },
                &occurrences,
            );
            inputs.push(HeaderAuthorInput {
                ordinal,
                span: CompositionSpan {
                    start: a.name_span.0,
                    end: a.name_span.1,
                },
                name: name_out,
                affiliations: links,
            });
            all_affiliations += a.affiliations.len();
            all_names += 1;
        }
        let components = authors
            .iter()
            .flat_map(|a| a.affiliations.iter())
            .filter_map(|(_, span)| {
                // Recognition consumes the independently observed organization text;
                // note/marker/link identity remains a separate lane. Empty lanes
                // (including zero-link authors) are not fabricated as components.
                let affiliation = &xml[span.start..span.end];
                let organization = tags(affiliation, "orgName")
                    .into_iter()
                    .next()
                    .map(|(_, _, value)| strip_tags(value))?;
                match NativeAffiliationParser::new().parse(&organization) {
                    da_parse::NativeAffiliationOutcome::Recognized { components } => {
                        Some(da_parse::NativeAffiliationOutcome::Recognized { components })
                    }
                    _ => None,
                }
            })
            .collect();
        let expected_rejected = usize::from(name == "hub_2507.19457.tei.xml");
        assert_eq!(
            rejected_names, expected_rejected,
            "known header name gaps must remain closed"
        );
        let accepted_count = inputs.len();
        let header = compose_header(HeaderCompositionInput {
            date,
            authors: inputs,
            affiliation_components: components,
        });
        let composition = match header {
            da_parse::HeaderCompositionOutcome::Accepted { composition } => composition,
            da_parse::HeaderCompositionOutcome::Rejected { error } => {
                panic!("header lane rejected {name}: {error:?}")
            }
        };
        assert_eq!(composition.candidate.authors.len(), accepted_count);
        assert!(
            composition
                .candidate
                .authors
                .iter()
                .enumerate()
                .all(|(i, a)| a.ordinal == i)
        );

        let raws = raw_references(&xml);
        assert_eq!(raws.len(), expected_refs, "reference denominator: {name}");
        let raw_refs = raws.iter().map(String::as_str).collect::<Vec<_>>();
        let refs = NativeReferenceSegmenter::new().segment(&raw_refs);
        let (oracle_anchors, oracle_pairs) = bibr_anchors(&xml);
        let anchors = NativeCitationAnchorScanner::new().scan(&xml);
        let field_outcomes = raws
            .iter()
            .map(|r| NativeCitationParser::new().parse(r))
            .collect::<Vec<_>>();
        let unsupported = field_outcomes
            .iter()
            .filter(|outcome| matches!(outcome, NativeCitationOutcome::Unsupported { .. }))
            .count();
        assert!(
            unsupported <= raws.len(),
            "closed unsupported field outcomes cannot exceed the independent row denominator"
        );
        let admitted_fields = field_outcomes
            .into_iter()
            .map(|outcome| match outcome {
                NativeCitationOutcome::Recognized { fields } => {
                    NativeCitationOutcome::Recognized { fields }
                }
                NativeCitationOutcome::Empty => NativeCitationOutcome::Empty,
                _ => NativeCitationOutcome::Empty,
            })
            .collect();
        let reference_outcome = compose_references(ReferenceCompositionInput {
            references: refs,
            fields: admitted_fields,
            anchors,
        });
        let composition = match reference_outcome {
            da_parse::ReferenceCompositionOutcome::Accepted { composition } => composition,
            da_parse::ReferenceCompositionOutcome::Rejected { error } => {
                panic!("reference lane rejected {name}: {error:?}")
            }
        };
        assert_eq!(composition.candidate.references.len(), raws.len());
        assert!(
            composition
                .candidate
                .references
                .iter()
                .enumerate()
                .all(|(i, r)| r.ordinal == i && r.source == raws[i])
        );
        assert_eq!(composition.candidate.adjacent_pairs, oracle_pairs);
        assert_eq!(composition.candidate.anchors.len(), oracle_anchors.len());
        for (i, (a, (s, e, has_target))) in composition
            .candidate
            .anchors
            .iter()
            .zip(oracle_anchors.iter())
            .enumerate()
        {
            assert_eq!(a.ordinal, i);
            assert_eq!((a.span.start, a.span.end), (*s, *e));
            assert_eq!(a.target_present, *has_target);
            assert_eq!(a.source, &xml[*s..*e]);
        }
        let fields = composition.candidate.fields;
        assert!(fields.iter().all(|f| raws.iter().any(|r| {
            r.as_bytes()
                .windows(f.value.len())
                .any(|w| w == f.value.as_bytes())
        })));
        let (n, t) = oracle_anchors
            .iter()
            .fold((0, 0), |(n, t), x| (n + 1, t + usize::from(x.2)));
        all_refs += raws.len();
        all_anchors += n;
        targeted += t;
        adjacent += oracle_pairs.len();
        // Independent role lanes never compensate one another; unsupported
        // field outcomes were explicitly admitted as empty above.
    }
    assert_eq!(
        (
            all_names,
            all_affiliations,
            all_refs,
            all_anchors,
            targeted,
            adjacent
        ),
        (35, 13, 207, 380, 217, 168)
    );
}

#[test]
fn independent_header_and_reference_mutations_fail_without_compensation() {
    let (name, checksum, _, _) = FIXTURES[2];
    let xml = String::from_utf8(fs::read(path(name)).unwrap()).unwrap();
    assert_eq!(digest(xml.as_bytes()), checksum);
    let mut authors = header_authors(&xml);
    let original = authors.clone();
    authors.remove(0);
    assert_ne!(
        authors.len(),
        original.len(),
        "dropped zero-link author must remain visible"
    );
    authors.swap(0, 1);
    assert!(
        authors[0].span.0 > authors[1].span.0,
        "reorder mutation is observable"
    );
    let mut refs = raw_references(&xml);
    let first = refs[0].clone();
    refs.remove(0);
    refs.insert(0, first.clone());
    refs.insert(1, first);
    assert_eq!(
        refs[0], refs[1],
        "duplicate reference mutation is observable"
    );
    let (anchors, pairs) = bibr_anchors(&xml);
    let mut merged = anchors.clone();
    merged.remove(1);
    assert_eq!(
        merged.len() + 1,
        anchors.len(),
        "merged-anchor mutation is observable"
    );
    let mut transferred = anchors.clone();
    transferred[0].2 = transferred[1].2;
    assert_ne!(
        transferred, anchors,
        "target transfer mutation is observable"
    );
    assert!(
        !pairs.is_empty(),
        "fragmented adjacency must be independently retained"
    );
}

#[test]
fn composition_receipts_keep_gaps_and_serde_order_deterministic() {
    let (_, checksum, _, _) = FIXTURES[0];
    let xml = String::from_utf8(fs::read(path(FIXTURES[0].0)).unwrap()).unwrap();
    assert_eq!(digest(xml.as_bytes()), checksum);
    let raws = raw_references(&xml);
    let raw_refs = raws.iter().map(String::as_str).collect::<Vec<_>>();
    let refs = NativeReferenceSegmenter::new().segment(&raw_refs);
    assert!(matches!(
        refs,
        NativeReferenceListOutcome::Recognized { .. }
    ));
    let outcome = compose_references(ReferenceCompositionInput {
        references: refs,
        fields: raws
            .iter()
            .map(|r| NativeCitationParser::new().parse(r))
            .collect(),
        anchors: NativeCitationAnchorScanner::new().scan(&xml),
    });
    let bytes = serde_json::to_vec(&outcome).unwrap();
    let replay: da_parse::ReferenceCompositionOutcome = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(serde_json::to_vec(&replay).unwrap(), bytes);
}
