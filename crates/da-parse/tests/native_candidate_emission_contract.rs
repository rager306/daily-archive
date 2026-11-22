use da_parse::*;
use proptest::prelude::*;

fn hash() -> String {
    "a".repeat(64)
}
fn identity() -> NativeCandidateIdentity {
    NativeCandidateIdentity {
        document_id: "fixture".into(),
        source_pdf_sha256: hash(),
        carrier_digest: "fixture-carrier".into(),
    }
}
fn header() -> NativeHeaderCandidate {
    NativeHeaderCandidate {
        schema_version: NATIVE_SCHOLARLY_CANDIDATE_SCHEMA_VERSION,
        identity: identity(),
        feature_digest: "fixture".into(),
        rows: vec![],
        decoded: NativeHeaderDecodeCandidate {
            labels: vec![],
            fields: vec![],
        },
        fields: NativeHeaderFields {
            title: vec!["Title & <evidence>".into()],
            email: vec!["a&b@example.org".into()],
            ids: vec!["untyped-123".into()],
        },
        dates: vec![],
        authors: vec![],
        affiliations: vec![],
    }
}
fn role(role: CitationRole, text: &str) -> CitationRoleSpan {
    CitationRoleSpan {
        role,
        label: format!("<{}>", role.as_str()),
        start: 0,
        end: 1,
        byte_start: 0,
        byte_end: text.len(),
        text: text.into(),
    }
}
fn record(ordinal: usize, label: &str, article: &str, journal: &str) -> NativeReferenceRecord {
    NativeReferenceRecord {
        ordinal,
        printed_label: NativePrintedReferenceLabel {
            text: format!("[{label}]"),
            rows: 0..1,
            arabic_key: Some(label.into()),
        },
        source_rows: vec![],
        source_text: format!("[{label}] {article} in {journal}"),
        citation_source_rows: vec![],
        citation: CitationClusterSequence {
            labels: vec![],
            cluster_text: format!("{article} {journal}"),
            roles: vec![
                role(CitationRole::Title, article),
                role(CitationRole::Journal, journal),
                role(CitationRole::Pubnum, "generic-id"),
                role(CitationRole::Author, "An uncertain author list"),
            ],
        },
    }
}
fn references() -> NativeReferenceCandidate {
    NativeReferenceCandidate {
        schema_version: NATIVE_SCHOLARLY_CANDIDATE_SCHEMA_VERSION,
        identity: identity(),
        reference_feature_digest: "fixture".into(),
        citation_feature_digest: "fixture".into(),
        records: vec![
            record(0, "12", "First article", "First journal"),
            record(1, "1", "Second article", "Second journal"),
        ],
        lines: vec![],
    }
}
fn text_of(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect()
}
fn inline_bindings(xml: &str) -> Vec<(String, String)> {
    let doc = roxmltree::Document::parse(xml).unwrap();
    doc.descendants()
        .filter(|n| n.has_tag_name("ref") && n.attribute("type") == Some("bibr"))
        .map(|n| (text_of(n), n.attribute("target").unwrap().into()))
        .collect()
}

#[test]
fn optional_date_header_and_candidate_locks_survive_lifting() {
    let mut header = header();
    header.authors.push(NativeHeaderLeaf {
        span: HeaderFieldSpan {
            label: "<author>".into(),
            start: 0,
            end: 2,
        },
        source_rows: vec![],
        source_text: "Ada Lovelace".into(),
        outcome: NativePersonalNameOutcome::Recognized {
            name: RecognizedPersonalName {
                given_name: Some("Ada".into()),
                middle_names: vec![],
                surname: "Lovelace".into(),
                recomposed_surname: "Lovelace".into(),
                surname_particles: vec![],
                suffix: None,
                initials: vec![],
            },
        },
    });
    let output =
        emit_native_candidate(Some(&header), None, "A < b & c", &hash(), "config").unwrap();
    let xml = std::str::from_utf8(output.envelope.raw_tei()).unwrap();
    let doc = roxmltree::Document::parse(xml).unwrap();
    assert_eq!(
        doc.descendants().filter(|n| n.has_tag_name("date")).count(),
        0
    );
    assert_eq!(
        doc.descendants()
            .filter(|n| n.has_tag_name("persName") || n.has_tag_name("author"))
            .count(),
        0
    );
    assert_eq!(
        doc.descendants()
            .find(|n| n.has_tag_name("note") && n.attribute("type") == Some("native_header_author"))
            .unwrap()
            .text(),
        Some("Ada Lovelace")
    );
    assert!(
        doc.descendants()
            .find(|n| n.has_tag_name("note")
                && n.attribute("type") == Some("native_header_author_leaf_outcome"))
            .unwrap()
            .text()
            .unwrap()
            .contains("Recognized")
    );
    assert_eq!(
        doc.descendants()
            .find(|n| n.has_tag_name("title"))
            .unwrap()
            .text(),
        Some("Title & <evidence>")
    );
    assert_eq!(
        doc.descendants()
            .find(|n| n.has_tag_name("idno"))
            .unwrap()
            .attribute("type"),
        Some("pubnum")
    );
    let lifted = TeiLifter::new().lift(&output.envelope, "paper", hash());
    assert_eq!(lifted.title, "Title &amp; &lt;evidence&gt;");
    assert!(!output.manifest.import_eligible);
    assert!(!output.manifest.deployed);
    assert!(output.manifest.diagnostic_only);
    assert_eq!(output.manifest.graph_write, "forbidden");
}

#[test]
fn bibliography_titles_stay_owned_and_printed_labels_not_ordinals_drive_links() {
    let output = emit_native_candidate(
        None,
        Some(&references()),
        "See [12] here. See [1] here. See [99] here.",
        &hash(),
        "config",
    )
    .unwrap();
    let xml = std::str::from_utf8(output.envelope.raw_tei()).unwrap();
    let doc = roxmltree::Document::parse(xml).unwrap();
    let entries = doc
        .descendants()
        .filter(|n| n.has_tag_name("biblStruct"))
        .collect::<Vec<_>>();
    for (index, expected) in [
        ["First article", "First journal"],
        ["Second article", "Second journal"],
    ]
    .iter()
    .enumerate()
    {
        let titles = entries[index]
            .descendants()
            .filter(|n| n.has_tag_name("title"))
            .map(|n| n.text().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(titles, expected.to_vec());
        assert_eq!(
            entries[index]
                .descendants()
                .filter(|n| n.has_tag_name("note") && n.attribute("subtype") == Some("author"))
                .count(),
            1
        );
        assert_eq!(
            entries[index]
                .descendants()
                .filter(|n| n.has_tag_name("author"))
                .count(),
            0
        );
    }
    assert_eq!(
        inline_bindings(xml),
        vec![("[12]".into(), "#b0".into()), ("[1]".into(), "#b1".into())]
    );
    // A wrong but existing target passes mere XML/link-existence checks. The
    // independent printed-label expectation must reject it semantically.
    let wrong = xml.replacen("target=\"#b0\"", "target=\"#b1\"", 1);
    assert!(roxmltree::Document::parse(&wrong).is_ok());
    assert_ne!(inline_bindings(&wrong), inline_bindings(xml));
    let lifted = TeiLifter::new().lift(&output.envelope, "paper", hash());
    assert_eq!(lifted.citations.len(), 2);
}

#[test]
fn exact_utf8_body_escaping_and_unresolved_markers_are_preserved() {
    let body = "α < β & γ\nSee [12] and unknown [99]. <ref target=\"#b1\">not XML</ref>";
    let output = emit_native_candidate(None, Some(&references()), body, &hash(), "config").unwrap();
    let xml = std::str::from_utf8(output.envelope.raw_tei()).unwrap();
    let doc = roxmltree::Document::parse(xml).unwrap();
    let body_node = doc.descendants().find(|n| n.has_tag_name("body")).unwrap();
    assert_eq!(text_of(body_node), body);
    assert_eq!(
        body_node
            .descendants()
            .filter(|n| n.has_tag_name("p"))
            .count(),
        1
    );
    assert_eq!(inline_bindings(xml), vec![("[12]".into(), "#b0".into())]);
}

#[test]
fn xml_character_refusals_and_duplicate_keys_remain_fail_closed() {
    for text in [
        "bad\0text",
        "bad\u{1f}text",
        "bad\u{fffe}text",
        "bad\u{ffff}text",
        "bad\rtext",
    ] {
        assert_eq!(
            emit_native_candidate(None, None, text, &hash(), "config").unwrap_err(),
            NativeCandidateEmissionError::UnrepresentableText
        );
        let mut header = header();
        header.fields.title = vec![text.into()];
        assert_eq!(
            emit_native_candidate(Some(&header), None, "body", &hash(), "config").unwrap_err(),
            NativeCandidateEmissionError::UnrepresentableText
        );
    }
    for config in [
        "",
        "bad\tconfig",
        "bad\nconfig",
        "bad\rconfig",
        "bad\0config",
    ] {
        assert_eq!(
            emit_native_candidate(None, None, "body", &hash(), config).unwrap_err(),
            NativeCandidateEmissionError::Identity
        );
    }
    let mut references = references();
    references.records[1].ordinal = references.records[0].ordinal;
    assert_eq!(
        emit_native_candidate(None, Some(&references), "body", &hash(), "config").unwrap_err(),
        NativeCandidateEmissionError::DuplicateReferenceKey
    );
}

proptest! {
    #[test]
    fn closed_markup_roundtrips_adversarial_source_values(
        chars in proptest::collection::vec(any::<char>(), 0..64)
    ) {
        let mut text = String::from("α&<>\"']]><ref target=\"#b9\">&amp;");
        text.extend(chars.into_iter().filter(|c| {
            *c != '\r' && (*c == '\t' || *c == '\n' || (*c >= ' ' && *c != '\u{fffe}' && *c != '\u{ffff}'))
        }));
        let mut header = header();
        header.fields.title = vec![text.clone()];
        header.fields.email = vec![text.clone()];
        header.fields.ids = vec![text.clone()];
        let mut references = references();
        references.records[0].source_text = text.clone();
        references.records[0].citation.roles = [
            CitationRole::Title,
            CitationRole::Journal,
            CitationRole::Booktitle,
            CitationRole::Date,
            CitationRole::Publisher,
            CitationRole::Location,
            CitationRole::Volume,
            CitationRole::Issue,
            CitationRole::Pages,
            CitationRole::Author,
        ].into_iter().map(|kind| role(kind, &text)).collect();
        let url = String::from("https://example.org/?a=\"&b=<tag>&c=']]>é");
        references.records[0].citation.roles.push(role(CitationRole::Web, &url));
        let output = emit_native_candidate(Some(&header), Some(&references), &text, &hash(), "config").unwrap();
        let xml = std::str::from_utf8(output.envelope.raw_tei()).unwrap();
        let doc = roxmltree::Document::parse(xml).unwrap();
        let body = doc.descendants().find(|node| node.has_tag_name("body")).unwrap();
        prop_assert_eq!(text_of(body), text.clone());
        let title = doc.descendants().find(|node| node.has_tag_name("title")).unwrap();
        prop_assert_eq!(text_of(title), text.clone());
        let ptrs = doc.descendants().filter(|node| node.has_tag_name("ptr")).collect::<Vec<_>>();
        prop_assert_eq!(ptrs.len(), 1);
        prop_assert_eq!(ptrs[0].attribute("target"), Some(url.as_str()));
        for name in ["email", "idno", "raw_reference"] {
            let node = doc.descendants().find(|node| {
                node.has_tag_name(name) || (node.has_tag_name("note") && node.attribute("type") == Some(name))
            }).unwrap();
            prop_assert_eq!(text_of(node), text.clone());
        }
        let roles = doc.descendants().filter(|node| {
            node.has_tag_name("note") && node.attribute("type") == Some("native_citation_role")
        }).take(10).map(text_of).collect::<Vec<_>>();
        prop_assert_eq!(roles, vec![text; 10]);
    }
}

#[test]
fn bad_identity_limits_and_non_roundtrippable_text_refuse() {
    assert!(emit_native_candidate(None, None, "body", "", "config").is_err());
    assert!(
        emit_native_candidate(Some(&header()), None, "body", &"b".repeat(64), "config").is_err()
    );
    assert!(emit_native_candidate(None, None, "body\rline", &hash(), "config").is_err());
    let limits = NativeTeiEmissionLimits {
        max_fragment_bytes: 3,
        ..Default::default()
    };
    assert!(
        emit_native_candidate_with_limits(None, None, "body", &hash(), "config", limits).is_err()
    );
}

#[test]
fn ambiguous_printed_labels_keep_body_unlinked_without_discarding_records() {
    let mut references = references();
    references.records[1].printed_label = references.records[0].printed_label.clone();
    let output =
        emit_native_candidate(None, Some(&references), "See [12].", &hash(), "config").unwrap();
    let xml = std::str::from_utf8(output.envelope.raw_tei()).unwrap();
    assert!(inline_bindings(xml).is_empty());
    assert_eq!(output.manifest.reference_records, 2);
    let doc = roxmltree::Document::parse(xml).unwrap();
    assert_eq!(
        text_of(doc.descendants().find(|n| n.has_tag_name("body")).unwrap()),
        "See [12]."
    );
}

#[test]
fn absent_header_title_never_lifts_a_bibliography_title_as_paper_title() {
    let references = references();
    let mut empty_header = header();
    empty_header.fields.title.clear();
    for header in [None, Some(&empty_header)] {
        let output =
            emit_native_candidate(header, Some(&references), "See [12].", &hash(), "config")
                .unwrap();
        assert_eq!(output.manifest.header_present, header.is_some());
        let lifted = TeiLifter::new().lift(&output.envelope, "paper", hash());
        assert_eq!(lifted.title, "");
        assert_eq!(lifted.citations.len(), 2);
        let xml = std::str::from_utf8(output.envelope.raw_tei()).unwrap();
        let doc = roxmltree::Document::parse(xml).unwrap();
        let titles = doc
            .descendants()
            .filter(|n| n.has_tag_name("biblStruct"))
            .map(|entry| {
                entry
                    .descendants()
                    .filter(|n| n.has_tag_name("title"))
                    .map(|n| n.text().unwrap().to_owned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            titles,
            vec![
                vec!["First article", "First journal"],
                vec!["Second article", "Second journal"]
            ]
        );
    }
}

#[test]
fn punctuation_attached_and_partly_unknown_group_markers_stay_unlinked() {
    let references = references();
    let punctuated = "See [12], [1], and [99].";
    // The frozen scanner splits only whitespace, so none of these tokens ends
    // with `]`. This is a scanner limitation, not a target-resolution failure.
    assert!(classify_callout_markers(punctuated).unwrap().is_empty());
    let group = "See [1-12] here.";
    let markers = classify_callout_markers(group).unwrap();
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].labels.len(), 12);
    for body in [punctuated, group] {
        let output =
            emit_native_candidate(None, Some(&references), body, &hash(), "config").unwrap();
        let xml = std::str::from_utf8(output.envelope.raw_tei()).unwrap();
        assert!(inline_bindings(xml).is_empty());
        let doc = roxmltree::Document::parse(xml).unwrap();
        assert_eq!(
            text_of(doc.descendants().find(|n| n.has_tag_name("body")).unwrap()),
            body
        );
    }
}

#[test]
fn repeated_source_bound_unsupported_lines_count_categories_not_occurrences() {
    use sha2::{Digest, Sha256};
    let mut references = references();
    let source_lines = (0..65)
        .map(|line| format!("Unlabelled-source-line-{line}"))
        .collect::<Vec<_>>();
    references.lines = source_lines
        .iter()
        .enumerate()
        .map(|(line, text)| NativeReferenceLine {
            source_rows: vec![NativeCandidateSourceRow {
                row: line,
                location: FullLocation {
                    page_ordinal: 0,
                    block_ordinal: 0,
                    line_ordinal: line as u32,
                    token_ordinal: line as u32,
                },
                source_ordinal: line as u32,
                text_digest: format!("{:x}", Sha256::digest(text.as_bytes())),
            }],
            decoded: Some(ReferenceSegmentSequence {
                labels: vec!["<other>".into()],
                line_text: text.clone(),
                segments: vec![],
            }),
            disposition: NativeReferenceLineDisposition::Unsupported {
                reason: NativeReferenceLineUnsupported::NoPrintedLabel,
            },
        })
        .collect();
    assert!(!references.has_complete_labelled_line_inventory());
    let body = "See [12] here.";
    let output = emit_native_candidate(None, Some(&references), body, &hash(), "config").unwrap();
    assert_eq!(
        output.manifest.limitation_count_semantics,
        "bounded_typed_summary_entries_not_line_occurrences"
    );
    // Seven capability disclaimers + one typed line category + one linker refusal.
    assert_eq!(output.manifest.limitations.len(), 9);
    assert!(
        output
            .manifest
            .limitations
            .iter()
            .any(|value| value == "unsupported_reference_line:no_printed_label:count=65")
    );
    let xml = std::str::from_utf8(output.envelope.raw_tei()).unwrap();
    let doc = roxmltree::Document::parse(xml).unwrap();
    let raw = doc
        .descendants()
        .filter(|node| {
            node.has_tag_name("note")
                && node.attribute("type") == Some("unsupported_reference_line")
        })
        .map(text_of)
        .collect::<Vec<_>>();
    assert_eq!(raw, source_lines);
    let dispositions = doc
        .descendants()
        .filter(|node| {
            node.has_tag_name("note")
                && node.attribute("type") == Some("reference_line_disposition")
        })
        .map(text_of)
        .collect::<Vec<_>>();
    assert_eq!(
        dispositions,
        vec!["Unsupported { reason: NoPrintedLabel }"; 65]
    );
    assert!(inline_bindings(xml).is_empty());
    assert_eq!(
        text_of(
            doc.descendants()
                .find(|node| node.has_tag_name("body"))
                .unwrap()
        ),
        body
    );
    let low_cap = NativeTeiEmissionLimits {
        max_absences: 8,
        ..Default::default()
    };
    assert_eq!(
        emit_native_candidate_with_limits(
            None,
            Some(&references),
            body,
            &hash(),
            "config",
            low_cap
        )
        .unwrap_err(),
        NativeCandidateEmissionError::LimitsExceeded("limitations")
    );
}
