use da_parse::*;
use std::collections::BTreeMap;

fn digest() -> String {
    "a".repeat(64)
}
fn join() -> P5ShadowJoinReport {
    let id = P5ShadowJoinIdentity {
        document_id: "doc-1".into(),
        source_sha256: digest(),
        grobid_profile: "structural".into(),
        provider_id: "native-test".into(),
        segmentation_schema_version: NATIVE_SEGMENTATION_SCHEMA_VERSION.into(),
        accounting_schema_version: NATIVE_BODY_SCHEMA_VERSION.into(),
        boundary_digest: digest(),
    };
    P5ShadowJoinReport {
        schema_version: P5_SHADOW_JOIN_SCHEMA_VERSION.into(),
        identity_match: true,
        identity: id.clone(),
        java_p5: JavaP5Observation {
            paper_id: "doc-1".into(),
            grobid_profile: "structural".into(),
            production_authority: "java_http".into(),
            observe_mode: "production_stamp".into(),
            sections_len: 0,
            body_empty: true,
            empty_count_not_success: true,
            native_surface: "none".into(),
        },
        java_p2: JavaP2Observation {
            paper_id: "doc-1".into(),
            grobid_profile: "structural".into(),
            java_section_n: 0,
        },
        native_sequence: NativeBodyShadowSummary {
            identity: NativeBodySequenceIdentity {
                document_id: id.document_id.clone(),
                source_sha256: id.source_sha256.clone(),
                provider_id: id.provider_id.clone(),
                segmentation_schema_version: id.segmentation_schema_version.clone(),
                accounting_schema_version: id.accounting_schema_version.clone(),
                boundary_digest: id.boundary_digest.clone(),
            },
            status: "pass".into(),
            counts: NativeBodySequenceCounts {
                admitted: 2,
                conserved: 2,
                unsupported: 0,
                loss: 0,
            },
        },
        native_status: "pass".into(),
        native_counts: NativeBodySequenceCounts {
            admitted: 2,
            conserved: 2,
            unsupported: 0,
            loss: 0,
        },
        java_fallback: JavaFallbackEvidence {
            disposition: "preserved_evidence_only".into(),
            target: "java_grobid".into(),
            source: "production_fallback".into(),
        },
        empty_count_not_success: true,
        redaction: BTreeMap::new(),
    }
}
fn p4() -> JavaP4Observation {
    JavaP4Observation {
        paper_id: "doc-1".into(),
        grobid_profile: "structural".into(),
        java_citation_n: 0,
        production_authority: "java_http".into(),
        observe_mode: "shadow_receipt".into(),
        empty_count_not_success: true,
    }
}
fn p6() -> JavaP6Observation {
    JavaP6Observation {
        paper_id: "doc-1".into(),
        grobid_profile: "structural".into(),
        java_figure_n: 0,
        java_table_n: 0,
        java_formula_n: 0,
        figure_typed_absence: true,
        table_typed_absence: true,
        formula_typed_absence: true,
        production_authority: "java_http".into(),
        observe_mode: "shadow_receipt".into(),
        empty_count_not_success: true,
    }
}
fn p7() -> JavaP7Observation {
    JavaP7Observation {
        paper_id: "doc-1".into(),
        grobid_profile: "structural".into(),
        java_callout_n: 0,
        unresolved_n: 0,
        production_authority: "java_http".into(),
        observe_mode: "production_stamp".into(),
        native_surface: "none".into(),
        empty_count_not_success: true,
    }
}
fn input<'a>(
    join: &'a P5ShadowJoinReport,
    accounting: Option<&'a NativeBodyAccounting>,
) -> CrossStageSemanticInvariantsInput<'a> {
    CrossStageSemanticInvariantsInput {
        join,
        java_p4: p4(),
        java_p6: p6(),
        java_p7: p7(),
        native_accounting: accounting,
        native_visual: None,
    }
}
fn accounting(j: &P5ShadowJoinReport) -> NativeBodyAccounting {
    let units = (0..2)
        .map(|n| AccountedUnit {
            page_ordinal: 0,
            source_ordinal: n,
            token_ordinal: n,
            text_digest: digest(),
            region: DocumentRegion::Body,
            bucket: AccountingBucket::BodyBlocks,
            kind: AccountedKind::Lexical,
        })
        .collect::<Vec<_>>();
    let mut buckets = BTreeMap::new();
    buckets.insert(
        AccountingBucket::BodyBlocks,
        BucketCount {
            admitted: 2,
            conserved: 2,
            unsupported: 0,
            loss: 0,
        },
    );
    NativeBodyAccounting {
        schema_version: NATIVE_BODY_SCHEMA_VERSION.into(),
        document_id: j.identity.document_id.clone(),
        source_sha256: j.identity.source_sha256.clone(),
        provider_id: j.identity.provider_id.clone(),
        source_boundary_digest: j.identity.boundary_digest.clone(),
        admitted: 2,
        conserved: 2,
        unsupported: 0,
        loss: 0,
        buckets,
        units,
        assignments: vec![],
    }
}

#[test]
fn matching_receipts_are_unpooled_and_missing_body_markers_are_zero() {
    let j = join();
    let a = accounting(&j);
    let report = check_cross_stage_semantic_invariants(input(&j, Some(&a))).unwrap();
    assert_eq!(report.body_markers_admitted, 0);
    assert_eq!(
        report.native_buckets[&AccountingBucket::BodyBlocks].admitted,
        2
    );
    assert_eq!(report.java_p5.sections_len, 0);
    assert_eq!(report.java_p2.java_section_n, 0);
    let json = serde_json::to_value(report).unwrap();
    let serialized = json.to_string();
    for forbidden in [
        "pooled_n",
        "p5_success",
        "java_section_n_on_p5",
        "body_text",
        "payload",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "forbidden field {forbidden}"
        );
    }
    assert_eq!(json["java_p6"]["java_figure_n"], 0);
    assert!(json["java_p5"].get("java_section_n").is_none());
}

#[test]
fn identity_and_empty_semantics_fail_closed() {
    let j = join();
    let mut bad = p6();
    bad.paper_id = "other".into();
    let err = check_cross_stage_semantic_invariants(CrossStageSemanticInvariantsInput {
        join: &j,
        java_p4: p4(),
        java_p6: bad,
        java_p7: p7(),
        native_accounting: None,
        native_visual: None,
    })
    .unwrap_err();
    assert_eq!(err, CrossStageSemanticInvariantsError::DocumentMismatch);
    let mut bad = p7();
    bad.empty_count_not_success = false;
    let err = check_cross_stage_semantic_invariants(CrossStageSemanticInvariantsInput {
        join: &j,
        java_p4: p4(),
        java_p6: p6(),
        java_p7: bad,
        native_accounting: None,
        native_visual: None,
    })
    .unwrap_err();
    assert_eq!(
        err,
        CrossStageSemanticInvariantsError::EmptyCountTreatedAsSuccess
    );
}

#[test]
fn count_equal_reorder_and_bucket_corruption_fail_closed() {
    let j = join();
    let mut a = accounting(&j);
    a.units.swap(0, 1);
    assert_eq!(
        check_cross_stage_semantic_invariants(input(&j, Some(&a))).unwrap_err(),
        CrossStageSemanticInvariantsError::CountEqualReorder
    );
    let mut a = accounting(&j);
    a.buckets
        .get_mut(&AccountingBucket::BodyBlocks)
        .unwrap()
        .admitted = 1;
    assert!(matches!(
        check_cross_stage_semantic_invariants(input(&j, Some(&a))),
        Err(CrossStageSemanticInvariantsError::BucketSumMismatch { .. })
    ));
}

#[test]
fn data_report_has_no_cross_family_success_claim() {
    let report = serde_yaml::from_str::<serde_yaml::Value>(include_str!(
        "../../../data/parse/p5_cross_stage.yaml"
    ))
    .unwrap();
    assert_eq!(
        report["report_schema_version"],
        CROSS_STAGE_SEMANTIC_SCHEMA_VERSION
    );
    assert_eq!(
        report["independently_selectable"],
        serde_yaml::from_str::<serde_yaml::Value>("[header, references]").unwrap()
    );
    assert_eq!(
        report["conservation"]["cross_oracle_equality_is_not_success"],
        true
    );
    assert_eq!(
        report["body_markers_policy"],
        "missing_bucket_is_zero_not_p7"
    );
    let forbidden = report["forbidden_report_fields"].as_sequence().unwrap();
    for field in [
        "pooled_n",
        "p5_success",
        "p6_success",
        "p7_success",
        "java_section_n_on_p5",
    ] {
        assert!(forbidden.iter().any(|v| v.as_str() == Some(field)));
    }
    let fields = report["unpooled_fields"].as_sequence().unwrap();
    for field in [
        "java_p4.java_citation_n",
        "java_p6.java_figure_n",
        "java_p7.java_callout_n",
        "native_buckets.*",
    ] {
        assert!(
            fields.iter().any(|v| v.as_str() == Some(field)),
            "missing {field}"
        );
    }
}
