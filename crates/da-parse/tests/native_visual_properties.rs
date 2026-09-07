use da_parse::*;

const SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
fn p(kind: NativeVisualKind, page: u32, source: u32) -> NativeVisualPrimitive {
    NativeVisualPrimitive {
        document_id: "property-doc".into(),
        source_sha256: SHA.into(),
        provider: NativeVisualProvider {
            id: "p".into(),
            version: "1".into(),
        },
        page_ordinal: page,
        source_ordinal: source,
        kind,
        bbox: Some(LayoutBBox::new(source as f64, 1.0, source as f64 + 1.0, 2.0).unwrap()),
        content_digest: Some(SHA.into()),
        absence: None,
    }
}
fn input(mut primitives: Vec<NativeVisualPrimitive>) -> NativeVisualObservation {
    primitives.reverse();
    NativeVisualObservation {
        schema_version: NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: "property-doc".into(),
        source_sha256: SHA.into(),
        provider: NativeVisualProvider {
            id: "p".into(),
            version: "1".into(),
        },
        primitives,
        figure_captions: vec![],
        figure_callouts: vec![],
        table_captions: vec![],
        table_callouts: vec![],
        table_cells: vec![],
        formula_labels: vec![],
        formula_callouts: vec![],
        lane_absence: None,
    }
}

#[test]
fn source_order_and_replay_are_stable_under_input_permutation() {
    let a = input(vec![
        p(NativeVisualKind::Image, 3, 8),
        p(NativeVisualKind::Image, 1, 2),
        p(NativeVisualKind::Image, 2, 4),
    ]);
    let mut b = a.clone();
    b.primitives.reverse();
    let left = detect_figure_heads(&a, Default::default()).unwrap();
    let right = detect_figure_heads(&b, Default::default()).unwrap();
    assert_eq!(
        serde_json::to_vec(&left).unwrap(),
        serde_json::to_vec(&right).unwrap()
    );
    let replay: NativeVisualHeadOutcome =
        serde_json::from_slice(&serde_json::to_vec(&left).unwrap()).unwrap();
    assert_eq!(left, replay);
    let NativeVisualHeadOutcome::Figure { receipts, .. } = left else {
        panic!()
    };
    assert_eq!(
        receipts
            .iter()
            .map(|r| (r.page_ordinal, r.source_ordinal))
            .collect::<Vec<_>>(),
        vec![(1, 2), (2, 4), (3, 8)]
    );
}

#[test]
fn independent_heads_do_not_change_when_unrelated_kind_is_added() {
    let base = input(vec![
        p(NativeVisualKind::Image, 1, 1),
        p(NativeVisualKind::Formula, 1, 2),
    ]);
    let mut extended = base.clone();
    extended.primitives.push(p(NativeVisualKind::Table, 1, 3));
    assert_eq!(
        detect_figure_heads(&base, Default::default()).unwrap(),
        detect_figure_heads(&extended, Default::default()).unwrap()
    );
    assert_eq!(
        detect_formula_heads(&base, Default::default()).unwrap(),
        detect_formula_heads(&extended, Default::default()).unwrap()
    );
}

#[test]
fn per_page_head_cap_is_enforced_without_pooled_scoring() {
    let many = input((0..3).map(|n| p(NativeVisualKind::Formula, 7, n)).collect());
    let error = detect_formula_heads(
        &many,
        NativeVisualConfig::new(MAX_PRIMITIVES, MAX_RECEIPTS, 2),
    )
    .unwrap_err();
    assert_eq!(error, NativeVisualError::HeadLimit);
}

#[test]
fn missing_content_is_exact_terminal_not_fake_empty_figure() {
    let mut item = p(NativeVisualKind::Image, 1, 1);
    item.bbox = None;
    item.content_digest = None;
    item.absence = Some(NativeVisualAbsence::NotObserved);
    let mut observation = input(vec![item]);
    assert!(observation.validate(Default::default()).is_ok());
    let outcome = detect_figure_heads(&observation, Default::default()).unwrap();
    let NativeVisualHeadOutcome::Terminal { reason, metrics } = &outcome else {
        panic!("missing figure content must not become Figure(empty): {outcome:?}");
    };
    assert_eq!(*reason, NativeVisualAbsence::NotObserved);
    assert_eq!((metrics.admitted, metrics.terminal), (0, 1));
    assert_eq!(
        (
            metrics.caption_conservation,
            metrics.content_conservation,
            metrics.callout_conservation,
            metrics.geometry_conservation,
        ),
        (0, 0, 0, 0)
    );
    let replay: NativeVisualHeadOutcome =
        serde_json::from_slice(&serde_json::to_vec(&outcome).unwrap()).unwrap();
    assert_eq!(replay, outcome);

    observation.lane_absence = Some(NativeVisualAbsence::Unsupported);
    observation.primitives.clear();
    assert!(matches!(
        detect_formula_heads(&observation, Default::default()).unwrap(),
        NativeVisualHeadOutcome::Terminal { .. }
    ));
}
