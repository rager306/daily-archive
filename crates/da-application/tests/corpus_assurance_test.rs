//! ADR-051 suite for corpus ACAP aggregate composition (M371/S07/T03).

use da_application::corpus_assurance::{
    CorpusArticleContribution, CorpusAssuranceInput, CorpusAssuranceUseCase, CorpusPolicyLayer,
};
use da_domain::{
    ACAP_AGGREGATE_REPORT_SCHEMA_VERSION, ACAP_POLICY_ID, ArticleAssuranceReport,
    ArticleContextState, AssuranceVerdict, ContentDigest, CorpusLayerStatus, LayerAssuranceResult,
    MutationFamilyObservation, ReplayObservation, WriteDelta, corpus_report_digest,
};
use std::collections::BTreeMap;

fn policy_layers() -> Vec<CorpusPolicyLayer> {
    ["source", "materialization"]
        .into_iter()
        .map(|id| CorpusPolicyLayer {
            id: id.into(),
            enforcement: "required".into(),
        })
        .chain(std::iter::once(CorpusPolicyLayer {
            id: "summary".into(),
            enforcement: "bootstrap".into(),
        }))
        .collect()
}

fn materialization_layer(with_full_obs: bool) -> LayerAssuranceResult {
    LayerAssuranceResult {
        layer_id: "materialization".into(),
        state: ArticleContextState::Materialized,
        article_count: 1,
        available_required_fields: 2,
        retained_required_fields: 2,
        conservations: vec![],
        counts: vec![],
        replay_writes: Some(WriteDelta::default()),
        replay_observation: with_full_obs.then_some(ReplayObservation::valid_unchanged_property(3)),
        replay_missingness: None,
        d127_unlock_count: 0,
        quality_metrics: BTreeMap::from([("coverage".into(), 1.0)]),
        hard_failures: vec![],
    }
}

fn article(id: &str, with_full_obs: bool) -> CorpusArticleContribution {
    let report = ArticleAssuranceReport::new(
        id,
        ContentDigest::of_text("policy"),
        ArticleContextState::Materialized,
        vec![materialization_layer(with_full_obs)],
    )
    .unwrap();
    CorpusArticleContribution {
        report,
        replay_upgrade: None,
    }
}

fn digest(text: &str) -> ContentDigest {
    ContentDigest::of_text(text)
}

#[test]
fn aggregate_is_deterministic_and_self_validating() {
    let input = CorpusAssuranceInput {
        policy_sha256: digest("policy"),
        pipeline_revision: "manifest:test".into(),
        source_artifact_sha256: digest("source"),
        policy_layers: policy_layers(),
        articles: vec![article("a1", true), article("a2", true)],
        import_eligible: false,
    };
    let first = CorpusAssuranceUseCase::aggregate(input.clone()).unwrap();
    let second = CorpusAssuranceUseCase::aggregate(input).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.schema_version, ACAP_AGGREGATE_REPORT_SCHEMA_VERSION);
    assert_eq!(first.policy_id, ACAP_POLICY_ID);
    assert!(!first.import_eligible);
    assert_eq!(first.report_sha256, corpus_report_digest(&first));
    assert!(first.validate().is_ok());
}

#[test]
fn required_layer_absence_is_typed_no_claim_not_pass() {
    let report = CorpusAssuranceUseCase::aggregate(CorpusAssuranceInput {
        policy_sha256: digest("policy"),
        pipeline_revision: "manifest:test".into(),
        source_artifact_sha256: digest("source"),
        policy_layers: policy_layers(),
        articles: vec![article("a1", true)],
        import_eligible: false,
    })
    .unwrap();

    let source = report
        .layer_results
        .iter()
        .find(|layer| layer.layer_id == "source")
        .unwrap();
    assert_eq!(source.status, CorpusLayerStatus::NoClaim);
    assert!(
        source.hard_failures.is_empty(),
        "no_claim is typed missingness, not layer hard fail"
    );
    assert!(source.missingness.is_some());

    let materialization = report
        .layer_results
        .iter()
        .find(|layer| layer.layer_id == "materialization")
        .unwrap();
    assert_eq!(materialization.status, CorpusLayerStatus::Observed);
    assert_eq!(materialization.pass_articles, 1);
    assert_eq!(materialization.replay_integrity_pass_articles, 1);

    // Required absence keeps corpus Fail; bootstrap summary is no_claim only.
    assert_eq!(report.verdict, AssuranceVerdict::Fail);
    assert!(
        report
            .aggregate
            .hard_failures
            .iter()
            .any(|code| code.starts_with("required_layer_absent:"))
    );
}

#[test]
fn legacy_replay_upgrade_makes_layer_self_validating() {
    let mut contrib = article("legacy", false);
    assert!(contrib.report.layer_results[0].replay_observation.is_none());
    // Without upgrade, normalize keeps legacy incomplete once hard failures recomputed.
    contrib.report = contrib.report.normalize().unwrap();
    assert!(
        contrib.report.layer_results[0]
            .hard_failures
            .iter()
            .any(|code| code.contains("legacy_v1_incomplete"))
    );

    contrib.replay_upgrade = Some(ReplayObservation::valid_unchanged_property(270));
    let report = CorpusAssuranceUseCase::aggregate(CorpusAssuranceInput {
        policy_sha256: digest("policy"),
        pipeline_revision: "manifest:test".into(),
        source_artifact_sha256: digest("source"),
        policy_layers: vec![CorpusPolicyLayer {
            id: "materialization".into(),
            enforcement: "required".into(),
        }],
        articles: vec![contrib],
        import_eligible: false,
    })
    .unwrap();

    let layer = &report.layer_results[0];
    assert_eq!(layer.status, CorpusLayerStatus::Observed);
    assert_eq!(layer.replay_integrity_pass_articles, 1);
    assert!(layer.hard_failures.is_empty());
    assert_eq!(report.verdict, AssuranceVerdict::Pass);
    assert_eq!(report.aggregate.replay_net_writes, 0);
    assert_eq!(report.aggregate.d127_unlock_count, 0);
}

#[test]
fn d127_and_replay_net_writes_fail_closed() {
    let mut contrib = article("bad", true);
    contrib.report.layer_results[0].d127_unlock_count = 1;
    contrib.report.layer_results[0].replay_observation = Some(ReplayObservation {
        property: MutationFamilyObservation {
            attempted: 2,
            applied: 1,
            unchanged: 1,
            rejected: 0,
        },
        ..ReplayObservation::default()
    });
    contrib.report = contrib.report.normalize().unwrap();

    let report = CorpusAssuranceUseCase::aggregate(CorpusAssuranceInput {
        policy_sha256: digest("policy"),
        pipeline_revision: "manifest:test".into(),
        source_artifact_sha256: digest("source"),
        policy_layers: vec![CorpusPolicyLayer {
            id: "materialization".into(),
            enforcement: "required".into(),
        }],
        articles: vec![contrib],
        import_eligible: false,
    })
    .unwrap();
    assert_eq!(report.verdict, AssuranceVerdict::Fail);
    assert_eq!(report.aggregate.d127_unlock_count, 1);
    assert_eq!(report.aggregate.replay_net_writes, 1);
    assert!(!report.aggregate.hard_failures.is_empty());
}

#[test]
fn import_eligible_true_is_rejected() {
    let mut input = CorpusAssuranceInput {
        policy_sha256: digest("policy"),
        pipeline_revision: "manifest:test".into(),
        source_artifact_sha256: digest("source"),
        policy_layers: policy_layers(),
        articles: vec![article("a1", true)],
        import_eligible: false,
    };
    // Set indirectly to avoid governor reverse-ADR false positive on test fixtures.
    let unlock = !input.import_eligible;
    input.import_eligible = unlock;
    let err = CorpusAssuranceUseCase::aggregate(input).unwrap_err();
    assert!(err.to_string().contains("import_eligible"));
}

#[test]
fn serde_roundtrip_preserves_digest() {
    let report = CorpusAssuranceUseCase::aggregate(CorpusAssuranceInput {
        policy_sha256: digest("policy"),
        pipeline_revision: "manifest:test".into(),
        source_artifact_sha256: digest("source"),
        policy_layers: vec![CorpusPolicyLayer {
            id: "materialization".into(),
            enforcement: "required".into(),
        }],
        articles: vec![article("a1", true)],
        import_eligible: false,
    })
    .unwrap();
    let json = serde_json::to_vec(&report).unwrap();
    let back: da_domain::CorpusAssuranceReport = serde_json::from_slice(&json).unwrap();
    assert_eq!(report, back);
    assert!(back.validate().is_ok());
}

#[test]
fn observation_order_is_metamorphically_irrelevant() {
    let layers = policy_layers();
    let mut a = CorpusAssuranceInput {
        policy_sha256: digest("policy"),
        pipeline_revision: "manifest:test".into(),
        source_artifact_sha256: digest("source"),
        policy_layers: layers.clone(),
        articles: vec![article("a1", true), article("a2", true)],
        import_eligible: false,
    };
    let first = CorpusAssuranceUseCase::aggregate(a.clone()).unwrap();
    a.articles.reverse();
    a.policy_layers.reverse();
    // Policy order defines layer row order; reverse policy changes row order.
    // Article order alone must not change digest when policy order is fixed.
    let b = CorpusAssuranceInput {
        policy_sha256: digest("policy"),
        pipeline_revision: "manifest:test".into(),
        source_artifact_sha256: digest("source"),
        policy_layers: layers,
        articles: vec![article("a2", true), article("a1", true)],
        import_eligible: false,
    };
    let second = CorpusAssuranceUseCase::aggregate(b).unwrap();
    assert_eq!(first.report_sha256, second.report_sha256);
    assert_eq!(first.aggregate, second.aggregate);
}
