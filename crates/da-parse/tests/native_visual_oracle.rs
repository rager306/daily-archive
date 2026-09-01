use da_parse::*;

const DIGEST: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
fn primitive(
    kind: NativeVisualKind,
    page: u32,
    source: u32,
    bounds: (f64, f64, f64, f64),
) -> NativeVisualPrimitive {
    NativeVisualPrimitive {
        document_id: "oracle-doc".into(),
        source_sha256: DIGEST.into(),
        provider: NativeVisualProvider {
            id: "reviewed-local-oracle".into(),
            version: "v1".into(),
        },
        page_ordinal: page,
        source_ordinal: source,
        kind,
        bbox: Some(LayoutBBox::new(bounds.0, bounds.1, bounds.2, bounds.3).unwrap()),
        content_digest: Some(DIGEST.into()),
        absence: None,
    }
}
fn receipt(page: u32, source: u32) -> NativeVisualDigestReceipt {
    NativeVisualDigestReceipt {
        page_ordinal: page,
        source_ordinal: source,
        digest: Some(DIGEST.into()),
        absence: None,
    }
}
fn fixture() -> NativeVisualObservation {
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: "oracle-doc".into(),
        source_sha256: DIGEST.into(),
        provider: NativeVisualProvider {
            id: "reviewed-local-oracle".into(),
            version: "v1".into(),
        },
        primitives: vec![
            primitive(NativeVisualKind::Graphic, 4, 9, (1.0, 2.0, 11.0, 12.0)),
            primitive(NativeVisualKind::Table, 5, 3, (2.0, 4.0, 22.0, 24.0)),
            primitive(NativeVisualKind::Formula, 6, 7, (3.0, 5.0, 13.0, 15.0)),
        ],
        figure_captions: vec![receipt(4, 9)],
        figure_callouts: vec![receipt(4, 9)],
        table_captions: vec![receipt(5, 3)],
        table_callouts: vec![receipt(5, 3)],
        table_cells: vec![NativeTableCellReceipt {
            row: 0,
            column: 0,
            digest: Some(DIGEST.into()),
            absence: None,
        }],
        formula_labels: vec![receipt(6, 7)],
        formula_callouts: vec![],
        lane_absence: None,
    }
}

#[test]
fn local_oracle_conserves_each_head_without_fulltext_or_java_truth() {
    let input = fixture();
    let NativeVisualHeadOutcome::Figure { receipts, metrics } =
        detect_figure_heads(&input, Default::default()).unwrap()
    else {
        panic!()
    };
    let figure = &receipts[0];
    assert_eq!((figure.page_ordinal, figure.source_ordinal), (4, 9));
    assert_eq!(figure.bbox, LayoutBBox::new(1.0, 2.0, 11.0, 12.0).unwrap());
    assert_eq!(figure.content_digest, DIGEST);
    assert_eq!(
        (
            figure.caption_digest.as_deref(),
            figure.callout_digest.as_deref()
        ),
        (Some(DIGEST), Some(DIGEST))
    );
    assert_eq!(
        (
            metrics.admitted,
            metrics.terminal,
            metrics.geometry_conservation
        ),
        (1, 0, 1)
    );

    let NativeVisualHeadOutcome::Table { receipts, metrics } =
        detect_table_heads(&input, Default::default()).unwrap()
    else {
        panic!()
    };
    let table = &receipts[0];
    assert_eq!((table.page_ordinal, table.source_ordinal), (5, 3));
    assert_eq!(table.bbox, LayoutBBox::new(2.0, 4.0, 22.0, 24.0).unwrap());
    assert_eq!(table.cells[0].digest.as_deref(), Some(DIGEST));
    assert_eq!(
        (
            metrics.admitted,
            metrics.caption_conservation,
            metrics.callout_conservation
        ),
        (1, 1, 1)
    );

    let NativeVisualHeadOutcome::Formula { receipts, metrics } =
        detect_formula_heads(&input, Default::default()).unwrap()
    else {
        panic!()
    };
    let formula = &receipts[0];
    assert_eq!((formula.page_ordinal, formula.source_ordinal), (6, 7));
    assert_eq!(formula.bbox, LayoutBBox::new(3.0, 5.0, 13.0, 15.0).unwrap());
    assert_eq!(formula.equation_label_digest.as_deref(), Some(DIGEST));
    assert_eq!(formula.callout_digest, None);
    assert_eq!(
        (
            metrics.admitted,
            metrics.content_conservation,
            metrics.callout_conservation
        ),
        (1, 1, 0)
    );
}

#[test]
fn source_checksum_and_provider_lineage_are_required_for_oracle_join() {
    let mut input = fixture();
    input.primitives[0].source_sha256 = "d".repeat(64);
    assert_eq!(
        input.validate(Default::default()),
        Err(NativeVisualError::InvalidIdentity("primitive lineage"))
    );
    let mut input = fixture();
    input.source_sha256 = "e".repeat(64);
    assert_eq!(
        input.validate(Default::default()),
        Err(NativeVisualError::InvalidIdentity("primitive lineage"))
    );
}
