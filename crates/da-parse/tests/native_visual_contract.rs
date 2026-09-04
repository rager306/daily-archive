use da_parse::*;

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn bbox(x: f64) -> LayoutBBox {
    LayoutBBox::new(x, 10.0, x + 20.0, 30.0).unwrap()
}
fn provider() -> NativeVisualProvider {
    NativeVisualProvider {
        id: "fixture-provider".into(),
        version: "1.0".into(),
    }
}
fn primitive(kind: NativeVisualKind, page: u32, source: u32, x: f64) -> NativeVisualPrimitive {
    NativeVisualPrimitive {
        document_id: "doc-1".into(),
        source_sha256: SHA.into(),
        provider: provider(),
        page_ordinal: page,
        source_ordinal: source,
        kind,
        bbox: Some(bbox(x)),
        content_digest: Some(SHA.into()),
        absence: None,
    }
}
fn link(page: u32, source: u32, digest: Option<&str>) -> NativeVisualDigestReceipt {
    NativeVisualDigestReceipt {
        page_ordinal: page,
        source_ordinal: source,
        digest: digest.map(str::to_owned),
        absence: digest
            .is_none()
            .then_some(NativeVisualAbsence::MissingReceipt),
    }
}
fn observation() -> NativeVisualObservation {
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: "doc-1".into(),
        source_sha256: SHA.into(),
        provider: provider(),
        primitives: vec![
            primitive(NativeVisualKind::Image, 2, 4, 10.0),
            primitive(NativeVisualKind::Table, 2, 4, 40.0),
            primitive(NativeVisualKind::Formula, 1, 2, 70.0),
        ],
        figure_captions: vec![link(2, 4, Some(SHA))],
        figure_callouts: vec![link(2, 4, Some(SHA))],
        table_captions: vec![link(2, 4, Some(SHA))],
        table_callouts: vec![link(2, 4, Some(SHA))],
        table_cells: vec![NativeTableCellReceipt {
            row: 0,
            column: 0,
            digest: Some(SHA.into()),
            absence: None,
        }],
        formula_labels: vec![link(1, 2, Some(SHA))],
        formula_callouts: vec![link(1, 2, None)],
        lane_absence: None,
    }
}

#[test]
fn three_heads_are_independent_and_preserve_source_bound_receipts() {
    let input = observation();
    let NativeVisualHeadOutcome::Figure { receipts, metrics } =
        detect_figure_heads(&input, Default::default()).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (
            receipts.len(),
            metrics.admitted,
            metrics.caption_conservation,
            metrics.callout_conservation
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(receipts[0].source_ordinal, 4);
    let NativeVisualHeadOutcome::Table { receipts, metrics } =
        detect_table_heads(&input, Default::default()).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (receipts.len(), metrics.admitted, receipts[0].cells.len()),
        (1, 1, 1)
    );
    let NativeVisualHeadOutcome::Formula { receipts, metrics } =
        detect_formula_heads(&input, Default::default()).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (
            receipts.len(),
            metrics.admitted,
            metrics.caption_conservation,
            metrics.callout_conservation
        ),
        (1, 1, 1, 0)
    );
}

#[test]
fn overlapping_figure_and_table_regions_do_not_pool_or_hide_each_other() {
    let input = observation();
    let figure = detect_figure_heads(&input, Default::default()).unwrap();
    let table = detect_table_heads(&input, Default::default()).unwrap();
    assert!(matches!(figure, NativeVisualHeadOutcome::Figure { .. }));
    assert!(matches!(table, NativeVisualHeadOutcome::Table { .. }));
    assert_ne!(
        serde_json::to_vec(&figure).unwrap(),
        serde_json::to_vec(&table).unwrap()
    );
}

#[test]
fn missing_image_is_typed_absence_not_zero_figures() {
    let mut input = observation();
    input
        .primitives
        .retain(|p| p.kind != NativeVisualKind::Image);
    input.figure_captions.clear();
    input.figure_callouts.clear();
    let figure = detect_figure_heads(&input, Default::default()).unwrap();
    assert!(
        matches!(
            figure,
            NativeVisualHeadOutcome::Terminal {
                reason: NativeVisualAbsence::NotObserved,
                ..
            }
        ),
        "missing Image must be Terminal, not Figure with zero receipts: {figure:?}"
    );
    assert!(matches!(
        detect_table_heads(&input, Default::default()).unwrap(),
        NativeVisualHeadOutcome::Table { .. }
    ));
    assert!(matches!(
        detect_formula_heads(&input, Default::default()).unwrap(),
        NativeVisualHeadOutcome::Formula { .. }
    ));

    let mut empty = observation();
    empty.primitives.clear();
    empty.figure_captions.clear();
    empty.figure_callouts.clear();
    empty.table_captions.clear();
    empty.table_callouts.clear();
    empty.table_cells.clear();
    empty.formula_labels.clear();
    empty.formula_callouts.clear();
    empty.lane_absence = None;
    assert!(matches!(
        detect_figure_heads(&empty, Default::default()).unwrap(),
        NativeVisualHeadOutcome::Terminal {
            reason: NativeVisualAbsence::NotObserved,
            ..
        }
    ));
    assert!(matches!(
        detect_table_heads(&empty, Default::default()).unwrap(),
        NativeVisualHeadOutcome::Terminal {
            reason: NativeVisualAbsence::NotObserved,
            ..
        }
    ));
    assert!(matches!(
        detect_formula_heads(&empty, Default::default()).unwrap(),
        NativeVisualHeadOutcome::Terminal {
            reason: NativeVisualAbsence::NotObserved,
            ..
        }
    ));
}

#[test]
fn unsupported_lane_is_terminal_for_each_head() {
    let mut input = observation();
    input.primitives.clear();
    input.lane_absence = Some(NativeVisualAbsence::Unsupported);
    for outcome in [
        detect_figure_heads(&input, Default::default()),
        detect_table_heads(&input, Default::default()),
        detect_formula_heads(&input, Default::default()),
    ] {
        assert!(matches!(
            outcome.unwrap(),
            NativeVisualHeadOutcome::Terminal {
                reason: NativeVisualAbsence::Unsupported,
                ..
            }
        ));
    }
}

#[test]
fn validation_rejects_duplicate_digest_identity_malformed_geometry_and_oversize() {
    let mut input = observation();
    input
        .primitives
        .push(primitive(NativeVisualKind::Image, 2, 4, 90.0));
    assert_eq!(
        input.validate(Default::default()),
        Err(NativeVisualError::DuplicatePrimitive)
    );
    let mut input = observation();
    input.figure_captions.push(link(2, 4, Some(SHA)));
    assert_eq!(
        input.validate(Default::default()),
        Err(NativeVisualError::DuplicateReceipt)
    );
    let mut input = observation();
    input.primitives[0].bbox = Some(LayoutBBox {
        x_min: f64::NAN,
        y_min: 0.0,
        x_max: 1.0,
        y_max: 1.0,
    });
    assert_eq!(
        input.validate(Default::default()),
        Err(NativeVisualError::InvalidGeometry)
    );
    let mut input = observation();
    input.primitives = (0..3)
        .map(|n| primitive(NativeVisualKind::Image, n, n, 1.0))
        .collect();
    assert_eq!(
        input.validate(NativeVisualConfig::new(2, MAX_RECEIPTS, 128)),
        Err(NativeVisualError::Oversize("primitives"))
    );
}

#[test]
fn serialized_receipts_are_redacted_by_construction() {
    let bytes =
        serde_json::to_vec(&detect_figure_heads(&observation(), Default::default()).unwrap())
            .unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("raw text"));
    assert!(!String::from_utf8_lossy(&bytes).contains("pdf_bytes"));
}
