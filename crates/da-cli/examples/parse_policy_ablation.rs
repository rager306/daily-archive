//! Offline policy-variant ablation emitter (M507-4fk6m6/S07/T04).
//!
//! Emits a deterministic, redacted artifact that fixes one closed
//! grammar-route policy variant against its baseline over one frozen ordered
//! same-n case set. Two independent producers are measured:
//!
//! * the **baseline arm** is recomputed by the SUT-free reference producer
//!   ([`da_cli::policy_ablation_reference`]) straight from the registry DATA —
//!   it never loads a policy module (ADR-051: the oracle must not be built by
//!   the code under test);
//! * the **candidate arm** is measured through the real production selector
//!   (`da_cli::source_cli::resolve_parsed_dispatch`), so the recorded action is
//!   the action production would really take.
//!
//! The artifact carries only digests, actions and counters: no paths, no
//! article text, no PII. Both arms keep their own producer code digest, and the
//! emitted report is candidate-only (`deployed=false`,
//! `import_eligible=false`, `auto_promote="forbidden"`).

use clap::Parser;
use da_application::{
    CriticObservation, FrozenEvalCase, PolicyAblationReport, PolicyVariantManifest,
    ProducerProvenance, PromotionQualityPolicy, aggregate_policy_variant_ablation,
};
use da_cli::policy_ablation_reference::{
    CaseFeatures, CaseSplit, DeclaredGrammarPolicy, PRODUCTION_PRODUCER_ID, PolicyAblationCase,
    REFERENCE_PRODUCER_ID, case_identity_digest, content_digest, corpus_digest, is_closed_action,
    observation_row, parse_cases, producer_identity_digest,
};
use da_ports::{PromotionState, Sha256Digest, SplitIdentity};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Schema identity of the emitted redacted artifact.
const ARTIFACT_SCHEMA_VERSION: &str = "parse-policy-ablation-artifact.v1";
/// Documented wire code declared by the candidate fixture (`always page_outline`).
const CANDIDATE_ROUTE_CODE: i32 = 0;
const VARIANT_ID: &str = "grammar-route-always-page-outline";
const VARIANT_LABEL: &str = "always page_outline (closed legal variant)";
/// Numeric qualification policy of the emitted report. It gates nothing: a
/// qualified ablation is still candidate-only.
const QUALITY_POLICY: PromotionQualityPolicy = PromotionQualityPolicy {
    minimum_validation_quality: 0.9,
    maximum_validation_gap: 0.05,
};
/// The DATA line the candidate registry is derived from.
const COMMITTED_BASELINE_WAT_LINE: &str = "wat: data/policies/fixtures/grammar_route.wat";

#[derive(Debug, Parser)]
#[command(about = "Emit a deterministic redacted policy-variant ablation report")]
struct Args {
    /// Frozen ordered case DATA (default: the committed S07 fixture).
    #[arg(long)]
    cases: Option<PathBuf>,
    /// Redacted report destination (default: artifacts/parse_learning/policy_variant_ablation.json).
    #[arg(long)]
    report: Option<PathBuf>,
    /// Baseline policy fixture (default: data/policies/fixtures/grammar_route.wat).
    #[arg(long)]
    baseline_wat: Option<PathBuf>,
    /// Candidate policy fixture (default: grammar_route_always_page_outline.wat).
    #[arg(long)]
    candidate_wat: Option<PathBuf>,
    /// Policy registry DATA the declared baseline is derived from.
    #[arg(long)]
    registry: Option<PathBuf>,
}

/// The redacted wrapper around the aggregated report: digests, actions,
/// counters. Nothing else may enter this struct.
#[derive(Debug, Serialize)]
struct PolicyAblationArtifact {
    schema_version: String,
    variant_id: String,
    registry_digest: Sha256Digest,
    baseline_n: u64,
    candidate_n: u64,
    /// Cases where the declared variant action and the production action agreed.
    declared_matches: u64,
    observed_routes: Vec<String>,
    observed_actions: Vec<String>,
    observed_policy_hashes: Vec<String>,
    artifact_digest: Sha256Digest,
    report: PolicyAblationReport,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let repo_root = repo_root();

    // Every path is made absolute BEFORE any chdir: the process later runs from
    // a temporary directory, where relative DATA paths would not resolve.
    let cases_path = resolve_path(args.cases.as_ref(), || {
        repo_root.join("crates/da-cli/tests/fixtures/policy_ablation_cases.json")
    })?;
    let registry_path = resolve_path(args.registry.as_ref(), || {
        repo_root.join("data/policies/registry.yaml")
    })?;
    let baseline_wat = resolve_path(args.baseline_wat.as_ref(), || {
        repo_root.join("data/policies/fixtures/grammar_route.wat")
    })?;
    let candidate_wat = resolve_path(args.candidate_wat.as_ref(), || {
        repo_root.join("data/policies/fixtures/grammar_route_always_page_outline.wat")
    })?;
    let report_path = resolve_path(args.report.as_ref(), || {
        PathBuf::from("artifacts/parse_learning/policy_variant_ablation.json")
    })?;

    let cases = parse_cases(&std::fs::read_to_string(&cases_path)?)
        .map_err(|e| anyhow::anyhow!("frozen case DATA: {e}"))?;
    let registry_yaml = std::fs::read_to_string(&registry_path)?;
    let declared = DeclaredGrammarPolicy::from_registry_yaml(&registry_yaml)
        .map_err(|e| anyhow::anyhow!("declared policy DATA: {e}"))?;
    let registry_digest = content_digest(registry_yaml.as_bytes());
    let baseline_policy_hash = compiled_module_digest(&baseline_wat)?;
    let candidate_policy_hash = compiled_module_digest(&candidate_wat)?;

    // The candidate registry is DERIVED from the committed DATA (only the
    // grammar_route artifact path is substituted); the committed registry file
    // is never rewritten.
    let temp = tempfile::tempdir()?;
    let temp_registry = write_candidate_registry(temp.path(), &registry_yaml, &candidate_wat)?;

    std::env::set_current_dir(temp.path())?;
    let _logging_guard = da_cli::logging::init_pipeline_logging("parse_policy_ablation");
    // SAFETY: the example is single-threaded and owns its process environment.
    unsafe {
        std::env::set_var("DA_POLICY_REGISTRY", temp_registry.as_os_str());
    }

    let corpus = corpus_digest(&cases);
    let produced = measure_arms(&cases, &declared, &corpus, &candidate_policy_hash)?;

    let split = SplitIdentity {
        train: produced
            .cases
            .iter()
            .filter(|case| case.split == CaseSplit::Train)
            .map(case_identity_digest)
            .collect(),
        validation: produced
            .cases
            .iter()
            .filter(|case| case.split == CaseSplit::Validation)
            .map(case_identity_digest)
            .collect(),
    };

    let manifest = PolicyVariantManifest {
        variant_id: VARIANT_ID.into(),
        policy_hash: candidate_policy_hash,
        baseline_policy_hash,
        registry_digest: registry_digest.clone(),
        label: VARIANT_LABEL.into(),
        promotion_state: PromotionState::Candidate,
        deployed: false,
        import_eligible: false,
    };
    let baseline_producer = ProducerProvenance {
        code_digest: producer_identity_digest(REFERENCE_PRODUCER_ID),
        input_digest: corpus.clone(),
        method: "declared-data-recompute".into(),
        authority: "data-registry".into(),
    };
    let candidate_producer = ProducerProvenance {
        code_digest: producer_identity_digest(PRODUCTION_PRODUCER_ID),
        input_digest: corpus,
        method: "production-dispatch-observation".into(),
        authority: "da-cli-production-path".into(),
    };

    let report = aggregate_policy_variant_ablation(
        &manifest,
        &produced.frozen_cases,
        &produced.baseline_rows,
        &produced.candidate_rows,
        true,
        &split,
        &baseline_producer,
        &candidate_producer,
        QUALITY_POLICY,
    )
    .map_err(|e| anyhow::anyhow!("ablation refused: {e}"))?;

    let mut artifact = PolicyAblationArtifact {
        schema_version: ARTIFACT_SCHEMA_VERSION.into(),
        variant_id: VARIANT_ID.into(),
        registry_digest,
        baseline_n: produced.baseline_rows.len() as u64,
        candidate_n: produced.candidate_rows.len() as u64,
        declared_matches: produced.declared_matches,
        observed_routes: produced.observed_routes.into_iter().collect(),
        observed_actions: produced.observed_actions.into_iter().collect(),
        observed_policy_hashes: produced.observed_policy_hashes.into_iter().collect(),
        artifact_digest: Sha256Digest::new("0".repeat(64)).expect("zero digest"),
        report,
    };
    artifact.artifact_digest = content_digest(&serde_json::to_vec(&artifact)?);

    let bytes = serde_json::to_vec_pretty(&artifact)?;
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = report_path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &report_path)?;

    println!(
        "policy-variant ablation: variant={} n={} declared_matches={} \
         routes={:?} actions={:?} artifact_digest={} report_digest={} report={}",
        artifact.variant_id,
        artifact.candidate_n,
        artifact.declared_matches,
        artifact.observed_routes,
        artifact.observed_actions,
        artifact.artifact_digest.as_str(),
        artifact.report.report_digest.as_str(),
        report_path.display()
    );
    Ok(())
}

/// Everything the two arms produced, ready for aggregation.
struct ArmObservations {
    cases: Vec<PolicyAblationCase>,
    frozen_cases: Vec<FrozenEvalCase>,
    baseline_rows: Vec<CriticObservation>,
    candidate_rows: Vec<CriticObservation>,
    declared_matches: u64,
    observed_routes: BTreeSet<String>,
    observed_actions: BTreeSet<String>,
    observed_policy_hashes: BTreeSet<String>,
}

/// Measure both arms over the frozen ordered case set.
///
/// The candidate arm is the production selector; the baseline arm is the
/// SUT-free declaration. A production result outside the closed action set, a
/// policy hash that is not the declared candidate artifact, or a declared
/// action that production contradicts all abort the run: those are trust
/// failures, never data points.
fn measure_arms(
    cases: &[PolicyAblationCase],
    declared: &DeclaredGrammarPolicy,
    corpus: &Sha256Digest,
    candidate_policy_hash: &Sha256Digest,
) -> anyhow::Result<ArmObservations> {
    let mut baseline_rows = Vec::with_capacity(cases.len());
    let mut candidate_rows = Vec::with_capacity(cases.len());
    let mut frozen_cases = Vec::with_capacity(cases.len());
    let mut observed_routes = BTreeSet::new();
    let mut observed_actions = BTreeSet::new();
    let mut observed_policy_hashes = BTreeSet::new();
    let mut declared_matches = 0_u64;

    for (index, case) in cases.iter().enumerate() {
        let order = index as u32;
        materialize_toc_cache(case)?;
        pin_features(&case.features);

        let dispatch = da_cli::source_cli::resolve_parsed_dispatch(&case.paper_id);
        let measured = match &dispatch {
            Some((decision, action)) => {
                if !is_closed_action(action) {
                    anyhow::bail!("production dispatched `{action}` outside the closed action set");
                }
                if decision.policy_hash != candidate_policy_hash.as_str() {
                    anyhow::bail!(
                        "case `{}` dispatched from policy bytes `{}` instead of the \
                         declared candidate artifact",
                        case.case_id,
                        decision.policy_hash
                    );
                }
                observed_routes.insert(decision.route.as_str().to_string());
                observed_actions.insert((*action).to_string());
                observed_policy_hashes.insert(decision.policy_hash.clone());
                Some(*action)
            }
            // Fail-closed production dispatch stays in the denominator with a
            // zero metric; it is recorded, never hidden.
            None => None,
        };

        // The declared variant action is re-derived from DATA alone; a
        // contradiction with the measured production action is a trust failure.
        let declared_candidate = declared
            .declared_action_for_code(CANDIDATE_ROUTE_CODE, case.toc_cache)
            .map_err(|e| anyhow::anyhow!("declared candidate action: {e}"))?;
        if let Some(action) = measured {
            if action != declared_candidate {
                anyhow::bail!(
                    "case `{}`: declared variant action `{declared_candidate}` but production \
                     dispatched `{action}`",
                    case.case_id
                );
            }
            declared_matches += 1;
        }

        let baseline_route = declared.declared_route_for_features(&case.features);
        let baseline_action = declared
            .declared_action(baseline_route, case.toc_cache)
            .map_err(|e| anyhow::anyhow!("declared baseline action: {e}"))?;

        baseline_rows.push(
            observation_row(
                case,
                corpus,
                order,
                REFERENCE_PRODUCER_ID,
                Some(baseline_action),
                &case.expect.baseline,
            )
            .map_err(|e| anyhow::anyhow!("baseline row: {e}"))?,
        );
        candidate_rows.push(
            observation_row(
                case,
                corpus,
                order,
                PRODUCTION_PRODUCER_ID,
                measured,
                &case.expect.candidate,
            )
            .map_err(|e| anyhow::anyhow!("candidate row: {e}"))?,
        );
        frozen_cases.push(FrozenEvalCase {
            case_id: case.case_id.clone(),
            corpus: corpus.clone(),
            order,
            expected_digest: case_identity_digest(case),
        });
    }

    Ok(ArmObservations {
        cases: cases.to_vec(),
        frozen_cases,
        baseline_rows,
        candidate_rows,
        declared_matches,
        observed_routes,
        observed_actions,
        observed_policy_hashes,
    })
}

/// Materialize the parsed-stage TOC observation cache for one case, relative to
/// the temporary process CWD. The paper identity was already validated as a
/// single safe path component when the case DATA was parsed.
fn materialize_toc_cache(case: &PolicyAblationCase) -> anyhow::Result<()> {
    if !case.toc_cache {
        return Ok(());
    }
    let dir = Path::new("artifacts/pipeline_cache/pdf_toc_observations");
    std::fs::create_dir_all(dir)?;
    let toc: Vec<serde_json::Value> = (0..case.features.bookmarks.max(1))
        .map(|index| serde_json::json!({ "title": format!("Section {}", index + 1) }))
        .collect();
    let body = serde_json::json!({ "total_pages": case.features.pages, "toc": toc });
    std::fs::write(
        dir.join(format!("{}.json", case.paper_id)),
        serde_json::to_vec(&body)?,
    )?;
    Ok(())
}

/// Pin the case features for the production selector (it reads them from the
/// environment / TOC cache, never from a parameter).
fn pin_features(features: &CaseFeatures) {
    // SAFETY: the example is single-threaded and owns its process environment.
    unsafe {
        std::env::set_var("DA_PARSE_PAGES", features.pages.to_string());
        std::env::set_var("DA_PARSE_BOOKMARKS", features.bookmarks.to_string());
        std::env::set_var("DA_PARSE_TEXT_QUALITY", features.quality.to_string());
    }
}

/// Derive the candidate registry from the committed DATA registry by pointing
/// `grammar_route` at the candidate artifact (absolute path).
fn write_candidate_registry(
    dir: &Path,
    committed_registry_yaml: &str,
    candidate_wat: &Path,
) -> anyhow::Result<PathBuf> {
    if !committed_registry_yaml.contains(COMMITTED_BASELINE_WAT_LINE) {
        anyhow::bail!("committed registry layout changed; cannot derive the candidate registry");
    }
    let patched = committed_registry_yaml.replace(
        COMMITTED_BASELINE_WAT_LINE,
        &format!("wat: {}", candidate_wat.display()),
    );
    let out = dir.join("registry.candidate.yaml");
    std::fs::write(&out, patched)?;
    Ok(out)
}

/// Content digest of the COMPILED policy module — the bytes the policy runtime
/// executes, which is exactly what a `Wasm` policy source stamps.
fn compiled_module_digest(path: &Path) -> anyhow::Result<Sha256Digest> {
    let raw = std::fs::read(path)?;
    let compiled =
        wat::parse_bytes(&raw).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    Ok(content_digest(&compiled))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

/// Resolve an explicit path, or the packaged default, to an absolute path that
/// survives the later chdir into the temporary directory.
fn resolve_path(
    explicit: Option<&PathBuf>,
    fallback: impl FnOnce() -> PathBuf,
) -> anyhow::Result<PathBuf> {
    let path = explicit.cloned().unwrap_or_else(fallback);
    Ok(std::path::absolute(&path)?)
}
