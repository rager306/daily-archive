use super::shared::{ArmCapture, Artifact, CAPTURE_SCHEMA, Capture, CaptureStatus};
use anyhow::{Context, Result, bail, ensure};
use da_application::fsm_policy_fallback::NativeFallbackPolicy;
use da_application::parse_policy_pair::{PairMetric, ParseArm};
use da_application::parse_policy_table::{
    PolicyObservation, TableLearnConfig, TableLearnError, learn_decision_table,
};
use da_application::pdf_toc_stage::TocObservations;
use da_domain::fsm_policy::{
    ArticleFeatures, DecisionPoint, GrammarRouteNative, LearnedDecisionTable, PolicyRegistry,
    TableCell, feature_bucket,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

// Both arms retain their own article schema. Only their common raw text is scored.
#[derive(Debug, Deserialize)]
struct RawArticle {
    paper_id: String,
    pdf_hash: String,
    sections: Vec<RawSection>,
}

#[derive(Debug, Deserialize)]
struct RawSection {
    text: String,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: String,
    candidate_only: bool,
    capture_count: usize,
    train_sizes: Vec<usize>,
    documents: Vec<Document>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    paper_id: String,
    pdf_path: PathBuf,
    source_sha256: String,
    bytes: u64,
    previously_seen_id: bool,
    partition: String,
    capture_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Metric {
    Sections,
    BodyChars,
    CharsPerMs,
    SectionsPerMs,
    LegacyYield,
    TokenF1,
    #[serde(rename = "ordered_5gram_f1")]
    Ordered5gramF1,
    QualityThenLatency,
}

impl Metric {
    fn name(self) -> &'static str {
        match self {
            Self::Sections => "sections",
            Self::BodyChars => "body_chars",
            Self::CharsPerMs => "chars_per_ms",
            Self::SectionsPerMs => "sections_per_ms",
            Self::LegacyYield => "legacy_yield",
            Self::TokenF1 => "token_f1",
            Self::Ordered5gramF1 => "ordered_5gram_f1",
            Self::QualityThenLatency => "quality_then_latency",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema_version: String,
    adr_ref: String,
    min_support: usize,
    token_normalization: String,
    ordered_ngram_size: usize,
    quality_tie_margin: f64,
    metric_variants: Vec<Metric>,
    legacy_metric_path: PathBuf,
    native_execution_equivalence: BTreeMap<String, String>,
    validation_usage: String,
    locks: Locks,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Locks {
    deployed: bool,
    import_eligible: bool,
    auto_promote: String,
    graph_write: String,
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

fn artifact_bytes(base: &Path, artifact: &Artifact, absolute: bool) -> Result<Vec<u8>> {
    let path = Path::new(&artifact.path);
    ensure!(
        if absolute {
            path.is_absolute()
        } else {
            relative(path)
        },
        "invalid artifact path {}",
        artifact.path
    );
    let bytes =
        std::fs::read(base.join(path)).with_context(|| format!("read {}", artifact.path))?;
    ensure!(
        digest(&bytes) == artifact.sha256,
        "artifact hash mismatch: {}",
        artifact.path
    );
    Ok(bytes)
}

fn partitions(manifest: &Manifest, train_limit: usize) -> Result<(Vec<&Document>, Vec<&Document>)> {
    ensure!(
        manifest.schema_version == "policy-corpus-manifest.v1" && manifest.candidate_only,
        "invalid manifest schema/locks"
    );
    ensure!(
        manifest.capture_count == manifest.documents.len(),
        "manifest capture count mismatch"
    );
    ensure!(
        train_limit > 0 && manifest.train_sizes.contains(&train_limit),
        "train-limit is not frozen in manifest"
    );
    let mut hashes = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut dirs = BTreeSet::new();
    for doc in &manifest.documents {
        ensure!(
            !doc.paper_id.trim().is_empty() && ids.insert(&doc.paper_id),
            "blank/duplicate paper ID"
        );
        ensure!(
            doc.source_sha256.len() == 64
                && doc
                    .source_sha256
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "invalid source SHA"
        );
        ensure!(hashes.insert(&doc.source_sha256), "duplicate source SHA");
        ensure!(
            relative(&doc.capture_dir) && dirs.insert(&doc.capture_dir),
            "invalid/duplicate capture directory"
        );
        ensure!(
            matches!(doc.partition.as_str(), "train" | "validation"),
            "invalid partition"
        );
        ensure!(
            doc.pdf_path.is_absolute() && doc.bytes > 0 && !doc.previously_seen_id,
            "invalid new-cohort source"
        );
    }
    let train: Vec<_> = manifest
        .documents
        .iter()
        .filter(|d| d.partition == "train")
        .collect();
    let validation: Vec<_> = manifest
        .documents
        .iter()
        .filter(|d| d.partition == "validation")
        .collect();
    ensure!(
        train_limit <= train.len() && !validation.is_empty(),
        "insufficient frozen partition"
    );
    Ok((train.into_iter().take(train_limit).collect(), validation))
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct TextScore {
    candidate_units: usize,
    reference_units: usize,
    matches: usize,
    precision: Option<f64>,
    recall: Option<f64>,
    f1: Option<f64>,
}

fn ratio(sum: f64, count: usize) -> Option<f64> {
    (count > 0).then(|| sum / count as f64)
}

fn unit_counts<T: Ord>(items: &[T]) -> BTreeMap<&T, usize> {
    let mut counts = BTreeMap::new();
    for item in items {
        *counts.entry(item).or_insert(0usize) += 1;
    }
    counts
}

fn overlap<T: Ord>(candidate: &[T], reference: &[T]) -> TextScore {
    let candidate_counts = unit_counts(candidate);
    let reference_counts = unit_counts(reference);
    let matches: usize = candidate_counts
        .iter()
        .map(|(unit, n)| reference_counts.get(unit).map_or(0, |m| (*n).min(*m)))
        .sum();
    TextScore {
        candidate_units: candidate.len(),
        reference_units: reference.len(),
        matches,
        precision: ratio(matches as f64, candidate.len()),
        recall: ratio(matches as f64, reference.len()),
        f1: ratio(2.0 * matches as f64, candidate.len() + reference.len()),
    }
}

#[derive(Debug, Clone, Serialize)]
struct ArmMetrics {
    sections: usize,
    body_chars: usize,
    chars_per_ms: f64,
    sections_per_ms: f64,
    token_overlap: TextScore,
    ordered_5gram: TextScore,
}

fn article_metrics(article: &RawArticle, reference: &[String], latency_ms: u64) -> ArmMetrics {
    let text = article
        .sections
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let extracted = tokens(&text);
    let candidate_grams: Vec<_> = extracted.windows(5).collect();
    let reference_grams: Vec<_> = reference.windows(5).collect();
    let body_chars = article
        .sections
        .iter()
        .map(|s| s.text.chars().count())
        .sum::<usize>();
    ArmMetrics {
        sections: article.sections.len(),
        body_chars,
        chars_per_ms: body_chars as f64 / latency_ms.max(1) as f64,
        sections_per_ms: article.sections.len() as f64 / latency_ms.max(1) as f64,
        token_overlap: overlap(&extracted, reference),
        ordered_5gram: overlap(&candidate_grams, &reference_grams),
    }
}

struct MeasuredDocument {
    document: Document,
    capture: Capture,
    capture_sha256: String,
    metrics: [Option<ArmMetrics>; 2],
}

fn load_document(root: &Path, doc: &Document) -> Result<MeasuredDocument> {
    let dir = root.join(&doc.capture_dir);
    let bytes = std::fs::read(dir.join("capture.json"))
        .with_context(|| format!("capture {}", doc.paper_id))?;
    let capture: Capture = serde_json::from_slice(&bytes)?;
    ensure!(
        capture.schema_version == CAPTURE_SCHEMA
            && capture.candidate_only
            && !capture.deployed
            && !capture.import_eligible
            && !capture.graph_write,
        "capture schema/locks"
    );
    ensure!(
        capture.paper_id == doc.paper_id && capture.source.sha256 == doc.source_sha256,
        "capture identity mismatch"
    );
    ensure!(
        Path::new(&capture.source.path).canonicalize()? == doc.pdf_path.canonicalize()?,
        "capture source path mismatch"
    );
    let source = artifact_bytes(&dir, &capture.source, true)?;
    ensure!(
        source.len() as u64 == doc.bytes && capture.source_bytes == doc.bytes,
        "source byte-count mismatch"
    );
    let features = capture
        .features
        .as_ref()
        .context("missing measured features")?;
    ensure!(
        capture.features_error.is_none()
            && features.page_count > 0
            && features.text_layer_quality.is_finite()
            && (0.0..=1.0).contains(&features.text_layer_quality),
        "invalid measured features"
    );
    artifact_bytes(&dir, &capture.exporter.script, true)?;
    artifact_bytes(&dir, &capture.exporter.furniture_policy, true)?;
    let observations_bytes = artifact_bytes(
        &dir,
        capture
            .exporter
            .observations
            .as_ref()
            .context("missing measured TOC observations")?,
        false,
    )?;
    let observations: TocObservations = serde_json::from_slice(&observations_bytes)?;
    ensure!(
        observations.total_pages == features.page_count
            && observations.toc.len() == features.toc_bookmarks as usize
            && observations.pdf_sha256.as_deref() == Some(doc.source_sha256.as_str()),
        "measured features/source disagree with TOC observations"
    );
    ensure!(
        capture
            .exporter
            .python_version
            .as_ref()
            .is_some_and(|v| !v.is_empty())
            && capture
                .exporter
                .pymupdf_version
                .as_ref()
                .is_some_and(|v| !v.is_empty()),
        "missing exporter versions"
    );
    ensure!(
        capture.reference.tool == "pdftotext"
            && capture
                .reference
                .version
                .as_ref()
                .is_some_and(|v| !v.is_empty())
            && capture.reference.error.is_none(),
        "invalid independent reference identity"
    );
    let reference_bytes = artifact_bytes(
        &dir,
        capture
            .reference
            .artifact
            .as_ref()
            .context("missing independent reference")?,
        false,
    )?;
    let reference_text = std::str::from_utf8(&reference_bytes)?;
    ensure!(
        capture.reference.char_count == Some(reference_text.chars().count()),
        "reference character-count mismatch"
    );
    let reference = tokens(reference_text);
    ensure!(
        reference.len() >= 5,
        "unsuitable reference: fewer than five alphabetic tokens"
    );
    ensure!(
        capture.arms[0].arm == ParseArm::Toc && capture.arms[1].arm == ParseArm::Grobid,
        "duplicate/misordered arms"
    );
    let mut metrics = [None, None];
    for (index, arm) in capture.arms.iter().enumerate() {
        ensure!(
            arm.decision_code == arm.arm.decision_code(),
            "arm decision-code mismatch"
        );
        ensure!(
            arm.grammar_profile
                == if index == 0 {
                    "page_outline"
                } else {
                    "arxiv_pdf_dot"
                },
            "unexpected arm grammar"
        );
        if index == 1 {
            ensure!(
                arm.backend_profile.as_deref() == Some("structural"),
                "GROBID arm must be structural"
            );
        }
        match arm.status {
            CaptureStatus::Success => ensure!(
                arm.error.is_none()
                    && arm.article.is_some()
                    && arm.evidence.as_ref().is_some_and(|e| e.ok)
                    && arm.backend_version.as_ref().is_some_and(|v| !v.is_empty()),
                "incomplete successful arm"
            ),
            CaptureStatus::NotApplicable => ensure!(
                index == 0
                    && features.toc_bookmarks == 0
                    && arm.error.as_deref() == Some("empty_toc")
                    && arm.article.is_none()
                    && arm.evidence.is_none(),
                "NotApplicable is only measured empty TOC"
            ),
            CaptureStatus::Failed => ensure!(
                arm.error.as_ref().is_some_and(|e| !e.is_empty())
                    && arm.evidence.as_ref().is_none_or(|e| !e.ok),
                "failure must retain an error, never success"
            ),
        }
        if let Some(artifact) = &arm.article {
            let raw = artifact_bytes(&dir, artifact, false)?;
            let article_json: Value = serde_json::from_slice(&raw)?;
            if index == 0 {
                ensure!(
                    article_json["schema_version"] == "parsed-article.v2"
                        && article_json["grammar_profile"] == "page_outline",
                    "invalid TOC article schema"
                );
            }
            let article: RawArticle =
                serde_json::from_slice(&raw).context("invalid common article text schema")?;
            ensure!(
                article.paper_id == doc.paper_id,
                "article paper ID mismatch"
            );
            ensure!(
                article.pdf_hash == doc.source_sha256,
                "article source hash mismatch"
            );
            let measured = article_metrics(&article, &reference, arm.latency_ms);
            ensure!(
                arm.status != CaptureStatus::Success
                    || (measured.sections > 0 && measured.body_chars > 0),
                "empty article marked successful"
            );
            if let Some(evidence) = &arm.evidence {
                ensure!(
                    evidence.section_count == measured.sections
                        && evidence.body_chars == measured.body_chars
                        && evidence.latency_ms == arm.latency_ms,
                    "arm evidence/output mismatch"
                );
            }
            metrics[index] = Some(measured);
        } else {
            ensure!(arm.evidence.is_none(), "evidence without raw article");
        }
    }
    Ok(MeasuredDocument {
        document: doc.clone(),
        capture,
        capture_sha256: digest(&bytes),
        metrics,
    })
}

#[derive(Debug, Clone, Serialize)]
struct PairResult {
    kind: &'static str,
    winner: Option<ParseArm>,
    rewards: [f64; 2],
}

fn decided(kind: &'static str, index: usize) -> PairResult {
    PairResult {
        kind,
        winner: Some(if index == 0 {
            ParseArm::Toc
        } else {
            ParseArm::Grobid
        }),
        rewards: if index == 0 { [1.0, 0.0] } else { [0.0, 1.0] },
    }
}

fn score(
    metric: Metric,
    arms: &[ArmCapture; 2],
    measurements: &[Option<ArmMetrics>; 2],
    margin: f64,
    legacy: &PairMetric,
) -> Result<PairResult> {
    let success = arms.each_ref().map(|a| a.status == CaptureStatus::Success);
    if arms[0].status == CaptureStatus::NotApplicable {
        return Ok(if success[1] {
            decided("forced_success", 1)
        } else {
            PairResult {
                kind: "forced_failed",
                winner: None,
                rewards: [0.0, 0.0],
            }
        });
    }
    if !success[0] && !success[1] {
        return Ok(PairResult {
            kind: "both_failed",
            winner: None,
            rewards: [0.0, 0.0],
        });
    }
    if !success[0] {
        return Ok(decided("parse_failure", 1));
    }
    if !success[1] {
        return Ok(decided("parse_failure", 0));
    }
    let m = measurements
        .each_ref()
        .map(|m| m.as_ref().expect("success carries measured article"));
    let values = |axis: Metric| -> Result<[f64; 2]> {
        m.map(|v| match axis {
            Metric::Sections => Some(v.sections as f64),
            Metric::BodyChars => Some(v.body_chars as f64),
            Metric::CharsPerMs => Some(v.chars_per_ms),
            Metric::SectionsPerMs => Some(v.sections_per_ms),
            Metric::TokenF1 => v.token_overlap.f1,
            Metric::Ordered5gramF1 | Metric::QualityThenLatency => v.ordered_5gram.f1,
            Metric::LegacyYield => None,
        })
        .into_iter()
        .map(|v| v.context("unobserved quality axis"))
        .collect::<Result<Vec<_>>>()
        .map(|v| [v[0], v[1]])
    };
    let winner = if metric == Metric::LegacyYield {
        let yields = arms
            .each_ref()
            .map(|a| legacy.yield_of(a.evidence.as_ref().expect("success evidence")));
        let wins = |a: usize, b: usize| {
            yields[a].0 > yields[b].0 + legacy.yield_rule.margin
                || (yields[a].0 == yields[b].0
                    && yields[a].1 > yields[b].1 + legacy.yield_rule.margin)
        };
        if wins(0, 1) {
            Some(0)
        } else if wins(1, 0) {
            Some(1)
        } else {
            None
        }
    } else {
        let v = values(metric)?;
        if metric == Metric::QualityThenLatency && (v[0] - v[1]).abs() <= margin {
            if arms[0].latency_ms < arms[1].latency_ms {
                Some(0)
            } else if arms[1].latency_ms < arms[0].latency_ms {
                Some(1)
            } else {
                None
            }
        } else if v[0] > v[1] {
            Some(0)
        } else if v[1] > v[0] {
            Some(1)
        } else {
            None
        }
    };
    Ok(winner.map_or(
        PairResult {
            kind: "tie",
            winner: None,
            rewards: [0.5, 0.5],
        },
        |i| decided("decisive", i),
    ))
}

fn execution_index(code: &str, config: &Config) -> Result<usize> {
    match config
        .native_execution_equivalence
        .get(code)
        .map(String::as_str)
    {
        Some("page_outline") => Ok(0),
        Some("grobid_structural") => Ok(1),
        _ => bail!("unobserved route {code}"),
    }
}

#[derive(Debug, Serialize)]
struct Selection {
    route: String,
    execution_arm: ParseArm,
    status: CaptureStatus,
    outcome_ok: bool,
    learned: bool,
    cell: Option<TableCell>,
    cell_distinct_documents: Option<usize>,
    reward: f64,
    regret: f64,
    latency_ms: u64,
    text_metrics: Option<ArmMetrics>,
}

fn select(
    policy: &NativeFallbackPolicy,
    doc: &MeasuredDocument,
    pair: &PairResult,
    config: &Config,
    support: &BTreeMap<String, usize>,
) -> Result<Selection> {
    let detail = policy.consult_detail(
        DecisionPoint::GrammarRoute,
        doc.capture.features.as_ref().context("missing features")?,
    )?;
    let route = detail
        .decision
        .route
        .context("unobserved grammar-route decision")?
        .as_str();
    let index = execution_index(route, config)?;
    let arm = &doc.capture.arms[index];
    let reward = pair.rewards[index];
    Ok(Selection {
        route: route.into(),
        execution_arm: arm.arm,
        status: arm.status,
        outcome_ok: arm.status == CaptureStatus::Success
            && arm.evidence.as_ref().is_some_and(|e| e.ok),
        learned: detail.decision.id.source.is_learned(),
        cell_distinct_documents: detail
            .cell
            .as_ref()
            .and_then(|c| support.get(&c.bucket_key_used).copied()),
        cell: detail.cell,
        reward,
        regret: pair.rewards[0].max(pair.rewards[1]) - reward,
        latency_ms: arm.latency_ms,
        text_metrics: if arm.status == CaptureStatus::Success {
            doc.metrics[index].clone()
        } else {
            None
        },
    })
}

#[derive(Debug, Serialize)]
struct DocumentEvaluation {
    paper_id: String,
    source_sha256: String,
    capture_sha256: String,
    reference_sha256: String,
    provenance: Value,
    features: ArticleFeatures,
    arms: [Value; 2],
    pair: PairResult,
    native: Selection,
    candidate: Selection,
    changed_decision: bool,
    changed_execution: bool,
}

fn evaluate(
    docs: &[MeasuredDocument],
    metric: Metric,
    config: &Config,
    legacy: &PairMetric,
    native: &NativeFallbackPolicy,
    candidate: &NativeFallbackPolicy,
    support: &BTreeMap<String, usize>,
) -> Result<Vec<DocumentEvaluation>> {
    docs.iter().map(|doc| {
        let pair = score(metric, &doc.capture.arms, &doc.metrics, config.quality_tie_margin, legacy)?;
        let baseline = select(native, doc, &pair, config, support)?;
        let chosen = select(candidate, doc, &pair, config, support)?;
        Ok(DocumentEvaluation {
            paper_id: doc.document.paper_id.clone(), source_sha256: doc.document.source_sha256.clone(),
            capture_sha256: doc.capture_sha256.clone(), reference_sha256: doc.capture.reference.artifact.as_ref().context("missing reference")?.sha256.clone(),
            provenance: json!({"capture_dir": doc.document.capture_dir, "source": doc.capture.source, "source_bytes": doc.capture.source_bytes, "exporter": doc.capture.exporter, "reference": doc.capture.reference}),
            features: doc.capture.features.clone().context("missing features")?,
            arms: std::array::from_fn(|i| json!({"capture": doc.capture.arms[i], "metrics": doc.metrics[i]})),
            changed_decision: baseline.route != chosen.route, changed_execution: baseline.execution_arm != chosen.execution_arm,
            native: baseline, candidate: chosen, pair,
        })
    }).collect()
}

fn mean(values: impl Iterator<Item = f64>) -> Value {
    let (sum, n) = values.fold((0.0, 0usize), |(sum, n), v| (sum + v, n + 1));
    json!({"n": n, "mean": ratio(sum, n)})
}

fn selection(row: &DocumentEvaluation, candidate: bool) -> &Selection {
    if candidate {
        &row.candidate
    } else {
        &row.native
    }
}

fn summary(rows: &[DocumentEvaluation], candidate: bool) -> Value {
    let selected: Vec<_> = rows.iter().map(|r| selection(r, candidate)).collect();
    let decisive: Vec<_> = rows.iter().filter(|r| r.pair.kind == "decisive").collect();
    let correct = decisive
        .iter()
        .filter(|r| r.pair.winner == Some(selection(r, candidate).execution_arm))
        .count();
    let success_count = selected.iter().filter(|s| s.outcome_ok).count();
    let covered = selected.iter().filter(|s| s.learned).count();
    let quality: Vec<_> = selected
        .iter()
        .filter_map(|s| s.text_metrics.as_ref())
        .collect();
    json!({
        "n": rows.len(), "selected_reward": mean(selected.iter().map(|s| s.reward)),
        "regret": mean(selected.iter().map(|s| s.regret)),
        "operational_success": {"count": success_count, "rate": ratio(success_count as f64, rows.len())},
        "selected_not_applicable": selected.iter().filter(|s| s.status == CaptureStatus::NotApplicable).count(),
        "selected_failed": selected.iter().filter(|s| s.status == CaptureStatus::Failed).count(),
        "learned_coverage": {"count": covered, "rate": ratio(covered as f64, rows.len())},
        "cell_observation_support": mean(selected.iter().filter_map(|s| s.cell.as_ref()?.support.map(|n| n as f64))),
        "cell_distinct_document_support": mean(selected.iter().filter_map(|s| s.cell_distinct_documents.map(|n| n as f64))),
        "full_pair_decisive_accuracy": {"correct": correct, "n": decisive.len(), "accuracy": ratio(correct as f64, decisive.len())},
        "forced_selected_reward": mean(rows.iter().filter(|r| r.pair.kind.starts_with("forced_")).map(|r| selection(r, candidate).reward)),
        "mean_selected_latency_ms": mean(selected.iter().map(|s| s.latency_ms as f64)),
        "successful_selected_token_f1": mean(quality.iter().filter_map(|m| m.token_overlap.f1)),
        "successful_selected_ordered_5gram_f1": mean(quality.iter().filter_map(|m| m.ordered_5gram.f1)),
        "successful_selected_body_chars": mean(quality.iter().map(|m| m.body_chars as f64)),
        "successful_selected_chars_per_ms": mean(quality.iter().map(|m| m.chars_per_ms)),
    })
}

fn validate_config(config: &Config) -> Result<()> {
    ensure!(
        config.schema_version == "policy-corpus-experiment.v1" && !config.adr_ref.trim().is_empty(),
        "invalid experiment schema"
    );
    ensure!(
        config.min_support > 0
            && config.token_normalization == "ascii_alphabetic_lowercase"
            && config.ordered_ngram_size == 5,
        "invalid frozen scoring recipe"
    );
    ensure!(
        config.quality_tie_margin.is_finite() && (0.0..=1.0).contains(&config.quality_tie_margin),
        "invalid quality margin"
    );
    ensure!(
        !config.metric_variants.is_empty()
            && config.metric_variants.iter().collect::<BTreeSet<_>>().len()
                == config.metric_variants.len(),
        "empty/duplicate metrics"
    );
    let expected = BTreeMap::from([
        ("xy_cut".into(), "grobid_structural".into()),
        ("grobid_structural".into(), "grobid_structural".into()),
        ("page_outline".into(), "page_outline".into()),
    ]);
    ensure!(
        config.native_execution_equivalence == expected,
        "execution equivalence is not frozen ADR-080 mapping"
    );
    ensure!(
        config.validation_usage == "adaptive_diagnostic_not_final_test"
            && !config.locks.deployed
            && !config.locks.import_eligible
            && config.locks.auto_promote == "forbidden"
            && config.locks.graph_write == "forbidden",
        "experiment lock violation"
    );
    Ok(())
}

pub(super) fn run(
    manifest_path: &Path,
    config_path: &Path,
    registry_path: &Path,
    root: &Path,
    train_limit: usize,
    out: &Path,
) -> Result<()> {
    let manifest_bytes = std::fs::read(manifest_path)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
    let (training, validation) = partitions(&manifest, train_limit)?;
    let config_bytes = std::fs::read(config_path)?;
    let config: Config = serde_yaml::from_slice(&config_bytes)?;
    validate_config(&config)?;
    let legacy_bytes = std::fs::read(root.join(&config.legacy_metric_path))?;
    let legacy = PairMetric::from_yaml_str(std::str::from_utf8(&legacy_bytes)?)?;
    ensure!(
        legacy.conservation_first
            && legacy.yield_rule.margin >= 0
            && [
                legacy.yield_rule.metric_one.as_str(),
                legacy.yield_rule.metric_two.as_str()
            ]
            .iter()
            .all(|s| matches!(*s, "section_count" | "body_chars")),
        "invalid legacy metric"
    );
    let registry_bytes = std::fs::read(registry_path)?;
    let registry = PolicyRegistry::from_yaml_str(std::str::from_utf8(&registry_bytes)?)?;
    ensure!(
        registry.schema_version == "v2",
        "invalid native registry schema"
    );
    let thresholds: GrammarRouteNative = serde_json::from_value(
        registry
            .spec(DecisionPoint::GrammarRoute)
            .context("missing native grammar-route DATA")?
            .native
            .clone(),
    )?;
    ensure!(
        thresholds.grobid_max_text_quality.is_finite()
            && (0.0..=1.0).contains(&thresholds.grobid_max_text_quality),
        "invalid native threshold"
    );
    let native = NativeFallbackPolicy::new(registry.clone());
    let train_docs: Vec<_> = training
        .iter()
        .map(|d| load_document(root, d))
        .collect::<Result<_>>()?;
    let val_docs: Vec<_> = validation
        .iter()
        .map(|d| load_document(root, d))
        .collect::<Result<_>>()?;
    let mut metrics = BTreeMap::new();
    let mut table_files: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    for metric in &config.metric_variants {
        let mut observations = Vec::new();
        let mut distinct: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for doc in &train_docs {
            let pair = score(
                *metric,
                &doc.capture.arms,
                &doc.metrics,
                config.quality_tie_margin,
                &legacy,
            )?;
            for (i, arm) in doc
                .capture
                .arms
                .iter()
                .enumerate()
                .filter(|(_, arm)| arm.status != CaptureStatus::NotApplicable)
            {
                let features = doc.capture.features.clone().context("missing features")?;
                distinct
                    .entry(feature_bucket(DecisionPoint::GrammarRoute, &features))
                    .or_default()
                    .insert(doc.document.source_sha256.clone());
                observations.push(PolicyObservation {
                    document_identity: doc.document.source_sha256.clone(),
                    decision_point: DecisionPoint::GrammarRoute,
                    features,
                    decision_code: arm.decision_code.clone(),
                    reward: pair.rewards[i],
                    outcome: arm.evidence.clone(),
                });
            }
        }
        let (table, status) = match learn_decision_table(
            DecisionPoint::GrammarRoute,
            &observations,
            TableLearnConfig {
                min_support: config.min_support,
            },
        ) {
            Ok(t) => (t, "trainable"),
            Err(TableLearnError::NoLearnableCells) => (
                LearnedDecisionTable::seal(DecisionPoint::GrammarRoute, BTreeMap::new()),
                "not_trainable",
            ),
            Err(error) => return Err(error.into()),
        };
        let distinct: BTreeMap<_, _> = distinct
            .into_iter()
            .map(|(key, ids)| (key, ids.len()))
            .collect();
        let candidate =
            NativeFallbackPolicy::new(registry.clone()).with_learned_table(table.clone())?;
        let train_rows = evaluate(
            &train_docs,
            *metric,
            &config,
            &legacy,
            &native,
            &candidate,
            &distinct,
        )?;
        let val_rows = evaluate(
            &val_docs, *metric, &config, &legacy, &native, &candidate, &distinct,
        )?;
        let train_mean = ratio(
            train_rows.iter().map(|r| r.candidate.reward).sum(),
            train_rows.len(),
        );
        let val_mean = ratio(
            val_rows.iter().map(|r| r.candidate.reward).sum(),
            val_rows.len(),
        );
        let table_bytes = serde_json::to_vec_pretty(&table)?;
        let name = format!(
            "{}.{}.table.json",
            out.file_stem()
                .context("report filename missing")?
                .to_string_lossy(),
            metric.name()
        );
        let table_path = out.with_file_name(name);
        let mut kinds = BTreeMap::new();
        for row in &val_rows {
            *kinds.entry(row.pair.kind).or_insert(0usize) += 1;
        }
        let metric_digest = digest(&serde_json::to_vec(
            &json!({"config_sha256": digest(&config_bytes), "variant": metric.name(), "legacy_metric_sha256": digest(&legacy_bytes), "evaluator_source_sha256": digest(include_bytes!("evaluation.rs"))}),
        )?);
        metrics.insert(metric.name(), json!({
            "metric_digest": metric_digest, "status": status, "training_documents": train_rows.len(), "training_observations": observations.len(),
            "min_support_units": "arm_observations", "table": {"path": table_path, "output_sha256": digest(&table_bytes), "table_sha256": table.table_sha256, "cells": table.cells.len(), "distinct_document_support": distinct},
            "training": {"native": summary(&train_rows, false), "candidate": summary(&train_rows, true)},
            "validation": {"native": summary(&val_rows, false), "candidate": summary(&val_rows, true), "pair_kinds": kinds,
                "changed_decisions": val_rows.iter().filter(|r| r.changed_decision).count(), "changed_executions": val_rows.iter().filter(|r| r.changed_execution).count()},
            "val_gap_train_minus_validation_reward": train_mean.zip(val_mean).map(|(train, val)| train - val), "validation_documents": val_rows,
        }));
        table_files.push((table_path, table_bytes));
    }
    let report = json!({
        "schema_version": "policy-corpus-evaluation.v1", "candidate_only": true, "deployed": false, "import_eligible": false, "auto_promote": "forbidden", "graph_write": "forbidden",
        "manifest_sha256": digest(&manifest_bytes), "config_sha256": digest(&config_bytes), "native_registry_sha256": digest(&registry_bytes),
        "legacy_metric_sha256": digest(&legacy_bytes), "legacy_metric_digest": legacy.digest(),
        "evaluator_source_sha256": digest(include_bytes!("evaluation.rs")), "capture_schema_source_sha256": digest(include_bytes!("shared.rs")),
        "train_limit": train_limit, "train_source_sha256": training.iter().map(|d| &d.source_sha256).collect::<Vec<_>>(),
        "validation_source_sha256": validation.iter().map(|d| &d.source_sha256).collect::<Vec<_>>(),
        "validation_usage": config.validation_usage, "text_reference": "independent pdftotext PDF text-retention diagnostic, not publisher ground truth or KG readiness",
        "latency_denominator_rule": "max(latency_ms,1); legacy_yield uses integer scaling and secondary-axis tiebreak",
        "reward_rule": "winner=1, loser/failed/not_applicable=0, successful tie=0.5; no winner when no arm succeeds",
        "quality_means_denominator": "successful selected arms only; n is explicit; latency/reward/regret use all assigned documents",
        "native_baseline": "actual NativeFallbackPolicy grammar_route DATA decisions, mapped to captured arms; counterfactual policy evaluation, not current production dispatch latency",
        "unobserved_routes": [],
        "native_execution_equivalence": config.native_execution_equivalence, "metrics": metrics,
    });
    std::fs::create_dir_all(
        out.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    for (path, bytes) in table_files {
        std::fs::write(path, bytes)?;
    }
    std::fs::write(out, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_application::parse_policy_table::PolicyOutcomeEvidence;

    fn config() -> Config {
        serde_yaml::from_str(include_str!(
            "../../../../data/policies/parse_policy_experiment.yaml"
        ))
        .unwrap()
    }

    fn legacy() -> PairMetric {
        PairMetric::from_yaml_str(include_str!(
            "../../../../data/policies/parse_pair_metric.yaml"
        ))
        .unwrap()
    }

    fn features(pages: u32, bookmarks: u32) -> ArticleFeatures {
        ArticleFeatures {
            page_count: pages,
            toc_bookmarks: bookmarks,
            text_layer_quality: 0.9,
            domain: String::new(),
        }
    }

    fn article(text: &str, sections: usize) -> RawArticle {
        RawArticle {
            paper_id: "fixture".into(),
            pdf_hash: "f".repeat(64),
            sections: (0..sections)
                .map(|i| RawSection {
                    text: if i == 0 { text.into() } else { String::new() },
                })
                .collect(),
        }
    }

    fn arm(
        index: usize,
        status: CaptureStatus,
        sections: usize,
        chars: usize,
        latency_ms: u64,
    ) -> ArmCapture {
        let arm = if index == 0 {
            ParseArm::Toc
        } else {
            ParseArm::Grobid
        };
        ArmCapture {
            arm,
            decision_code: arm.decision_code().into(),
            grammar_profile: if index == 0 {
                "page_outline"
            } else {
                "arxiv_pdf_dot"
            }
            .into(),
            backend_profile: (index == 1).then(|| "structural".into()),
            status,
            error: match status {
                CaptureStatus::Success => None,
                CaptureStatus::NotApplicable => Some("empty_toc".into()),
                CaptureStatus::Failed => Some("parse failed".into()),
            },
            article: None,
            evidence: (status == CaptureStatus::Success).then(|| PolicyOutcomeEvidence {
                ok: true,
                section_count: sections,
                body_chars: chars,
                latency_ms,
                backend: Some("fixture".into()),
            }),
            backend_version: Some("fixture-v1".into()),
            latency_ms,
        }
    }

    fn measurement(sections: usize, chars: usize, latency_ms: u64, f1: f64) -> ArmMetrics {
        let text_score = TextScore {
            candidate_units: 10,
            reference_units: 10,
            matches: 8,
            precision: Some(f1),
            recall: Some(f1),
            f1: Some(f1),
        };
        ArmMetrics {
            sections,
            body_chars: chars,
            chars_per_ms: chars as f64 / latency_ms.max(1) as f64,
            sections_per_ms: sections as f64 / latency_ms.max(1) as f64,
            token_overlap: text_score.clone(),
            ordered_5gram: text_score,
        }
    }

    #[test]
    fn same_pair_has_metric_specific_winner_not_code_agreement() {
        let arms = [
            arm(0, CaptureStatus::Success, 2, 100, 10),
            arm(1, CaptureStatus::Success, 4, 80, 20),
        ];
        let m = [
            Some(measurement(2, 100, 10, 0.8)),
            Some(measurement(4, 80, 20, 0.9)),
        ];
        for (metric, winner) in [
            (Metric::Sections, ParseArm::Grobid),
            (Metric::BodyChars, ParseArm::Toc),
            (Metric::CharsPerMs, ParseArm::Toc),
            (Metric::TokenF1, ParseArm::Grobid),
            (Metric::Ordered5gramF1, ParseArm::Grobid),
        ] {
            let result = score(metric, &arms, &m, 0.02, &legacy()).unwrap();
            assert_eq!(result.winner, Some(winner));
            assert_eq!(
                result.rewards,
                if winner == ParseArm::Toc {
                    [1.0, 0.0]
                } else {
                    [0.0, 1.0]
                }
            );
        }
        assert_eq!(
            score(Metric::SectionsPerMs, &arms, &m, 0.02, &legacy())
                .unwrap()
                .rewards,
            [0.5, 0.5]
        );
        assert_eq!(
            score(Metric::LegacyYield, &arms, &m, 0.02, &legacy())
                .unwrap()
                .winner,
            Some(ParseArm::Toc)
        );
    }

    #[test]
    fn quality_margin_then_latency_and_integer_legacy_tiebreak_are_distinct() {
        let arms = [
            arm(0, CaptureStatus::Success, 1, 2000, 3000),
            arm(1, CaptureStatus::Success, 1, 100, 3001),
        ];
        let m = [
            Some(measurement(1, 2000, 3000, 0.8)),
            Some(measurement(1, 100, 3001, 0.81)),
        ];
        assert_eq!(
            score(Metric::QualityThenLatency, &arms, &m, 0.02, &legacy())
                .unwrap()
                .winner,
            Some(ParseArm::Toc)
        );
        assert_eq!(
            score(Metric::Ordered5gramF1, &arms, &m, 0.02, &legacy())
                .unwrap()
                .winner,
            Some(ParseArm::Grobid)
        );
        assert_eq!(
            score(Metric::LegacyYield, &arms, &m, 0.02, &legacy())
                .unwrap()
                .winner,
            Some(ParseArm::Toc)
        );
        let inverted = [
            arm(0, CaptureStatus::Success, 1, 100, 3000),
            arm(1, CaptureStatus::Success, 1, 2000, 3001),
        ];
        assert_eq!(
            score(Metric::SectionsPerMs, &inverted, &m, 0.02, &legacy())
                .unwrap()
                .winner,
            Some(ParseArm::Toc)
        );
        assert_eq!(
            score(Metric::LegacyYield, &inverted, &m, 0.02, &legacy())
                .unwrap()
                .winner,
            Some(ParseArm::Grobid)
        );
    }

    #[test]
    fn forced_choice_requires_success_and_failures_are_not_missing_toc() {
        let mut arms = [
            arm(0, CaptureStatus::NotApplicable, 0, 0, 12),
            arm(1, CaptureStatus::Success, 1, 100, 10),
        ];
        let m = [None, Some(measurement(1, 100, 10, 1.0))];
        let forced = score(Metric::Sections, &arms, &m, 0.02, &legacy()).unwrap();
        assert_eq!(
            (forced.kind, forced.rewards),
            ("forced_success", [0.0, 1.0])
        );
        arms[1] = arm(1, CaptureStatus::Failed, 0, 0, 10);
        let failed = score(Metric::Sections, &arms, &m, 0.02, &legacy()).unwrap();
        assert_eq!(
            (failed.kind, failed.winner, failed.rewards),
            ("forced_failed", None, [0.0, 0.0])
        );
        arms[0] = arm(0, CaptureStatus::Failed, 0, 0, 12);
        assert_eq!(
            score(Metric::Sections, &arms, &m, 0.02, &legacy())
                .unwrap()
                .kind,
            "both_failed"
        );
        arms[1] = arm(1, CaptureStatus::Success, 1, 100, 10);
        assert_eq!(
            score(Metric::Sections, &arms, &m, 0.02, &legacy())
                .unwrap()
                .kind,
            "parse_failure"
        );
        arms[0] = arm(0, CaptureStatus::Success, 1, 100, 12);
        let equal = [
            Some(measurement(1, 100, 12, 1.0)),
            Some(measurement(1, 100, 10, 1.0)),
        ];
        assert_eq!(
            score(Metric::Sections, &arms, &equal, 0.02, &legacy())
                .unwrap()
                .rewards,
            [0.5, 0.5]
        );
    }

    #[test]
    fn repeated_tokens_are_multisets_and_grams_preserve_order() {
        let score = overlap(&tokens("A a b"), &tokens("a b b"));
        assert_eq!(score.matches, 2);
        assert_eq!(score.precision, Some(2.0 / 3.0));
        assert_eq!(score.recall, Some(2.0 / 3.0));
        let repeated = article_metrics(&article("a b c d e a b c d e", 1), &tokens("a b c d e"), 1);
        assert_eq!(repeated.ordered_5gram.matches, 1);
        assert_eq!(repeated.ordered_5gram.candidate_units, 6);
        assert_eq!(repeated.ordered_5gram.f1, Some(2.0 / 7.0));
        let ordered = article_metrics(&article("e d c b a", 1), &tokens("a b c d e"), 1);
        assert_eq!(ordered.token_overlap.f1, Some(1.0));
        assert_eq!(ordered.ordered_5gram.f1, Some(0.0));
        assert_eq!(overlap::<String>(&[], &[]).f1, None);
        assert_eq!(tokens("Alpha-β_42 BETA"), vec!["alpha", "beta"]);
    }

    fn manifest_doc(i: u32, partition: &str) -> Document {
        Document {
            paper_id: format!("doc-{i}"),
            pdf_path: format!("/tmp/doc-{i}.pdf").into(),
            source_sha256: format!("{i:064x}"),
            bytes: 10,
            previously_seen_id: false,
            partition: partition.into(),
            capture_dir: format!("captures/doc-{i}").into(),
        }
    }

    #[test]
    fn frozen_partitions_never_move_when_training_prefix_grows() {
        let mut manifest = Manifest {
            schema_version: "policy-corpus-manifest.v1".into(),
            candidate_only: true,
            capture_count: 4,
            train_sizes: vec![1, 3],
            documents: vec![
                manifest_doc(1, "train"),
                manifest_doc(2, "validation"),
                manifest_doc(3, "train"),
                manifest_doc(4, "train"),
            ],
        };
        let (small, fixed) = partitions(&manifest, 1).unwrap();
        let (large, still_fixed) = partitions(&manifest, 3).unwrap();
        assert_eq!(small.len(), 1);
        assert_eq!(large.len(), 3);
        assert_eq!(fixed[0].source_sha256, still_fixed[0].source_sha256);
        assert_eq!(fixed[0].paper_id, "doc-2");
        manifest.documents[3].source_sha256 = manifest.documents[0].source_sha256.clone();
        assert!(partitions(&manifest, 1).is_err());
    }

    #[test]
    fn tampered_output_is_rejected_before_scoring() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("article.json");
        std::fs::write(&path, b"original").unwrap();
        let artifact = Artifact {
            path: "article.json".into(),
            sha256: digest(b"original"),
        };
        assert_eq!(
            artifact_bytes(temp.path(), &artifact, false).unwrap(),
            b"original"
        );
        std::fs::write(&path, b"substituted").unwrap();
        assert!(artifact_bytes(temp.path(), &artifact, false).is_err());
        let escape = Artifact {
            path: "../article.json".into(),
            sha256: artifact.sha256,
        };
        assert!(artifact_bytes(temp.path(), &escape, false).is_err());
    }

    fn measured_document(pages: u32, bookmarks: u32) -> MeasuredDocument {
        let artifact = || Artifact {
            path: "fixture".into(),
            sha256: "f".repeat(64),
        };
        let arms = [
            arm(0, CaptureStatus::Success, 2, 100, 10),
            arm(1, CaptureStatus::Success, 4, 80, 20),
        ];
        MeasuredDocument {
            document: manifest_doc(1, "validation"),
            capture_sha256: "c".repeat(64),
            capture: Capture {
                schema_version: CAPTURE_SCHEMA.into(),
                paper_id: "doc-1".into(),
                source: artifact(),
                source_bytes: 10,
                features: Some(features(pages, bookmarks)),
                features_error: None,
                exporter: super::super::shared::ExporterIdentity {
                    script: artifact(),
                    furniture_policy: artifact(),
                    python: "python3".into(),
                    python_version: Some("v1".into()),
                    pymupdf_version: Some("v1".into()),
                    observations: Some(artifact()),
                },
                reference: super::super::shared::ReferenceCapture {
                    tool: "pdftotext".into(),
                    version: Some("v1".into()),
                    artifact: Some(artifact()),
                    char_count: Some(5),
                    error: None,
                },
                arms,
                candidate_only: true,
                deployed: false,
                import_eligible: false,
                graph_write: false,
            },
            metrics: [
                Some(measurement(2, 100, 10, 0.8)),
                Some(measurement(4, 80, 20, 0.9)),
            ],
        }
    }

    #[test]
    fn same_document_baseline_and_candidate_use_actual_native_and_fallback() {
        let config = config();
        let registry = PolicyRegistry::from_yaml_str("schema_version: v2\ndecision_points:\n  grammar_route:\n    native:\n      page_outline_min_pages: 10\n      page_outline_min_bookmarks: 1\n      grobid_max_text_quality: 0.4\n").unwrap();
        let native = NativeFallbackPolicy::new(registry.clone());
        let features = features(18, 1);
        let table = LearnedDecisionTable::seal(
            DecisionPoint::GrammarRoute,
            BTreeMap::from([(
                feature_bucket(DecisionPoint::GrammarRoute, &features),
                "xy_cut".into(),
            )]),
        );
        let candidate = NativeFallbackPolicy::new(registry)
            .with_learned_table(table)
            .unwrap();
        let rows = evaluate(
            &[measured_document(18, 1), measured_document(700, 1)],
            Metric::BodyChars,
            &config,
            &legacy(),
            &native,
            &candidate,
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].native.route, "page_outline");
        assert_eq!(rows[0].candidate.route, "xy_cut");
        assert_eq!(
            (rows[0].native.reward, rows[0].candidate.reward),
            (1.0, 0.0)
        );
        assert_eq!(rows[1].candidate.route, "page_outline");
        assert!(!rows[1].candidate.learned);
        assert_eq!(summary(&rows, false)["selected_reward"]["n"], 2);
        assert_eq!(summary(&rows, true)["selected_reward"]["n"], 2);
        assert_eq!(summary(&rows, true)["selected_reward"]["mean"], 0.5);
        assert_eq!(
            summary(&[], true)["full_pair_decisive_accuracy"]["accuracy"],
            Value::Null
        );
    }

    #[test]
    fn load_genuine_toc_stage_and_grobid_shape_then_reject_output_tamper() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("captures/fixture");
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = temp.path().join("source.pdf");
        std::fs::write(&pdf, b"source PDF fixture").unwrap();
        let source_sha = digest(b"source PDF fixture");
        let mut doc = manifest_doc(1, "validation");
        doc.pdf_path = pdf.clone();
        doc.bytes = 18;
        doc.source_sha256 = source_sha.clone();
        doc.capture_dir = "captures/fixture".into();
        let write = |name: &str, bytes: &[u8]| {
            std::fs::write(dir.join(name), bytes).unwrap();
            Artifact {
                path: name.into(),
                sha256: digest(bytes),
            }
        };
        let source = Artifact {
            path: pdf.to_string_lossy().into(),
            sha256: source_sha.clone(),
        };
        let script = temp.path().join("exporter.py");
        std::fs::write(&script, b"offline exporter fixture").unwrap();
        let input = Artifact {
            path: script.to_string_lossy().into(),
            sha256: digest(b"offline exporter fixture"),
        };
        let body = "alpha beta gamma delta epsilon";
        let observations = json!({"toc": [{"level": 1, "title": "Introduction", "page": 1}], "pages_text": {"1": body}, "total_pages": 1, "pdf_sha256": source_sha}).to_string();
        let toc = da_application::pdf_toc_stage::build_pdf_toc_stage(&doc.paper_id, &observations)
            .unwrap();
        let toc_raw: RawArticle = serde_json::from_str(&toc.parsed_json).unwrap();
        let grobid = da_ports::parser::ParsedArticle {
            paper_id: doc.paper_id.clone(),
            pdf_hash: source_sha.clone(),
            sections: vec![da_ports::parser::Section {
                text: body.into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let raw_grobid = serde_json::to_vec(&grobid).unwrap();
        let mut arms = [
            arm(
                0,
                CaptureStatus::Success,
                toc_raw.sections.len(),
                toc_raw
                    .sections
                    .iter()
                    .map(|s| s.text.chars().count())
                    .sum(),
                10,
            ),
            arm(1, CaptureStatus::Success, 1, body.chars().count(), 20),
        ];
        arms[0].article = Some(write("toc.json", toc.parsed_json.as_bytes()));
        arms[1].article = Some(write("grobid.json", &raw_grobid));
        let capture = Capture {
            schema_version: CAPTURE_SCHEMA.into(),
            paper_id: doc.paper_id.clone(),
            source,
            source_bytes: doc.bytes,
            features: Some(features(1, 1)),
            features_error: None,
            exporter: super::super::shared::ExporterIdentity {
                script: input.clone(),
                furniture_policy: input,
                python: "python3".into(),
                python_version: Some("fixture".into()),
                pymupdf_version: Some("fixture".into()),
                observations: Some(write("observations.json", observations.as_bytes())),
            },
            reference: super::super::shared::ReferenceCapture {
                tool: "pdftotext".into(),
                version: Some("fixture".into()),
                artifact: Some(write("reference.txt", body.as_bytes())),
                char_count: Some(body.chars().count()),
                error: None,
            },
            arms,
            candidate_only: true,
            deployed: false,
            import_eligible: false,
            graph_write: false,
        };
        std::fs::write(
            dir.join("capture.json"),
            serde_json::to_vec(&capture).unwrap(),
        )
        .unwrap();
        let measured = load_document(temp.path(), &doc).unwrap();
        assert!(measured.metrics.iter().all(Option::is_some));
        assert_eq!(
            measured.metrics[1].as_ref().unwrap().token_overlap.f1,
            Some(1.0)
        );
        std::fs::write(dir.join("toc.json"), b"{}").unwrap();
        assert!(load_document(temp.path(), &doc).is_err());
        std::fs::write(dir.join("toc.json"), toc.parsed_json).unwrap();
        std::fs::write(dir.join("reference.txt"), b"substituted reference").unwrap();
        assert!(load_document(temp.path(), &doc).is_err());
    }

    #[test]
    fn not_trainable_candidate_is_native_and_missing_toc_is_not_rerouted() {
        let config = config();
        let native = NativeFallbackPolicy::empty();
        let candidate = NativeFallbackPolicy::empty()
            .with_learned_table(LearnedDecisionTable::seal(
                DecisionPoint::GrammarRoute,
                BTreeMap::new(),
            ))
            .unwrap();
        let mut doc = measured_document(700, 0);
        doc.capture.arms[0] = arm(0, CaptureStatus::NotApplicable, 0, 0, 3);
        doc.metrics[0] = None;
        let pair = score(
            Metric::BodyChars,
            &doc.capture.arms,
            &doc.metrics,
            0.02,
            &legacy(),
        )
        .unwrap();
        let chosen = select(&candidate, &doc, &pair, &config, &BTreeMap::new()).unwrap();
        assert_eq!(chosen.route, "xy_cut");
        assert_eq!(chosen.reward, 1.0);
        assert!(!chosen.learned);
        let force_toc = NativeFallbackPolicy::from_yaml("decision_points:\n  grammar_route:\n    native:\n      page_outline_min_pages: 1\n      page_outline_min_bookmarks: 0\n").unwrap();
        let unavailable = select(&force_toc, &doc, &pair, &config, &BTreeMap::new()).unwrap();
        assert_eq!(unavailable.status, CaptureStatus::NotApplicable);
        assert_eq!(unavailable.reward, 0.0);
        assert_eq!(unavailable.regret, 1.0);
        assert!(!unavailable.outcome_ok);
        assert_eq!(
            select(&native, &doc, &pair, &config, &BTreeMap::new())
                .unwrap()
                .route,
            "xy_cut"
        );
    }
}
