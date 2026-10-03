//! S08/T03 (M514-1lss2s) — corpus-bound same-n resource runner.
//!
//! This is the executable half of the corpus-bound same-n contract. It reads
//! committed carriers only and never runs the real 610-document native plane,
//! never consults Java and never requires network:
//!
//! * the frozen holdout manifest (`data/parse/holdout_manifests.yaml`, pointer
//!   `articles`) for the assigned document census;
//! * the T01 acceptance carrier
//!   (`artifacts/parse_compat/native_corpus_acceptance.json`) that measured the
//!   native resources over that census;
//! * the committed route receipt (`artifacts/parse_compat/native_route_e2e.json`)
//!   for the prospective limit freeze, whose revision is recomputed from the
//!   governor freeze carriers;
//! * the two pinned reference carriers (`native_java_work.json` and
//!   `parse_batch_budget_live.json`), neither of which is same-n to the 610
//!   assigned documents.
//!
//! Because no same-n reference exists for the 610-corpus, every reference metric
//! is emitted `not_observed` with a closed reason (D510) — never a restated Java
//! number, never a pooled value, never a saving claim. The native resources T01
//! actually measured are bound with their non-empty denominators.
//!
//! ## Modes
//!
//! * `--self-check` — hermetic. No carrier is read and nothing is written.
//!   Rejects each named mutation (mismatched document set, reference row without
//!   a pinned same-n source, pooled claim, saving claim, untyped reason, empty
//!   denominator, opened lock, freeze-revision drift).
//! * default — canonical run over the committed carriers, written atomically
//!   (tmp + rename).
//!
//! Diagnostic only: `claim_scope = diagnostic_only`, `product_coverage_effect
//! = none`, `thresholds = not_accepted`, all five locks closed, no graph/import/
//! promotion, no production route (D127 / ADR-040 / ADR-076 §14).

use da_cli::native_acceptance::{
    AcceptanceCarrierBinding, AcceptanceDocumentExpectation, AcceptanceDocumentRow,
    AcceptanceExpectation, AcceptanceFamilyExpectation, AcceptanceGapCell, AcceptanceGuard,
    AcceptanceMetricRow, AcceptancePopulations, AcceptanceThresholdExpectation,
    NativeAcceptanceReceipt, build_native_acceptance, load_holdout_documents,
};
use da_cli::native_corpus_resource_same_n::{
    AssignedIdentity, CORPUS_RESOURCE_GOVERNOR_POINTER, CORPUS_RESOURCE_GOVERNOR_REL,
    CORPUS_RESOURCE_HOLDOUT_MANIFEST_REL, CORPUS_RESOURCE_HOLDOUT_POINTER,
    CORPUS_RESOURCE_NATIVE_MEASUREMENT_REL, CORPUS_RESOURCE_REFERENCE_BUDGET_REL,
    CORPUS_RESOURCE_REFERENCE_NOT_SAME_N, CORPUS_RESOURCE_REFERENCE_WORK_REL,
    CORPUS_RESOURCE_ROUTE_RECEIPT_REL, CorpusResourceReferenceCarrier,
    CorpusResourceSameNExpectation, CorpusResourceSameNInput,
    DEFAULT_CORPUS_RESOURCE_SAME_N_ARTIFACT_REL, build_corpus_resource_same_n_receipt,
    carrier_bare_digest, freeze_revision, validate_corpus_resource_same_n_receipt,
};
use da_cli::native_family_receipt::{CarrierBinding, sha256_hex, sha256_prefixed};
use da_cli::native_resource_same_n::{
    SAME_N_REFERENCE_METRIC_ORDER, SameNMetricObservation, SameNSource,
};
use da_cli::native_route_receipt::{
    RouteFreezeDeclaration, RouteLimitsBinding, RouteMetricDeclaration,
};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The observed Java seam, inherited from the T01 measurement.
const OBSERVER: &str = "java_transport_fetch_tei";

/// Ordered freeze carriers as `(repository-relative path, bare sha256-hex)`.
type FrozenCarriers = Vec<(String, String)>;

// ─────────────────────────────── arguments ───────────────────────────────

struct Args {
    root: PathBuf,
    report: PathBuf,
    self_check: bool,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_CORPUS_RESOURCE_SAME_N_ARTIFACT_REL);
    let mut self_check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--self-check" => self_check = true,
            "--help" => {
                println!(
                    "native_corpus_resource_same_n [--root DIR] [--report FILE] [--self-check]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    Args {
        root,
        report,
        self_check,
    }
}

fn main() -> ExitCode {
    let _log_guard = da_cli::logging::init_pipeline_logging("native_corpus_resource_same_n");
    let args = parse_args();
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_corpus_resource_same_n: {error}");
            ExitCode::FAILURE
        }
    }
}

// ───────────────────────── carrier loading ─────────────────────────

fn read_carrier(root: &Path, rel: &str) -> Result<(Vec<u8>, String), Box<dyn Error>> {
    let bytes = fs::read(root.join(rel))?;
    let sha = sha256_prefixed(&bytes);
    Ok((bytes, sha))
}

/// Recompute the governor freeze revision from the committed carriers the
/// `native_route_e2e.limit_policy.freeze_revision` block declares.
fn recompute_freeze(
    root: &Path,
    governor_bytes: &[u8],
) -> Result<(String, FrozenCarriers), Box<dyn Error>> {
    let value: serde_yaml::Value = serde_yaml::from_slice(governor_bytes)?;
    let table =
        value[CORPUS_RESOURCE_GOVERNOR_POINTER]["limit_policy"]["freeze_revision"]["carriers"]
            .as_sequence()
            .ok_or("the governor block declares no freeze carriers")?;
    let mut carriers = Vec::with_capacity(table.len());
    for carrier in table {
        let path = carrier["path"]
            .as_str()
            .ok_or("a freeze carrier has no path")?;
        let bytes = fs::read(root.join(path))?;
        carriers.push((path.to_owned(), carrier_bare_digest(&bytes)));
    }
    let revision = freeze_revision(&carriers);
    Ok((revision, carriers))
}

fn reference_metrics() -> Vec<SameNMetricObservation> {
    SAME_N_REFERENCE_METRIC_ORDER
        .iter()
        .map(|id| SameNMetricObservation {
            id: (*id).to_owned(),
            direction: None,
            state: "not_observed".to_owned(),
            value: None,
            reason: Some(CORPUS_RESOURCE_REFERENCE_NOT_SAME_N.to_owned()),
            source: None,
        })
        .collect()
}

fn assigned_from_manifest(text: &str) -> Result<Vec<AssignedIdentity>, Box<dyn Error>> {
    let documents = load_holdout_documents(text)?;
    Ok(documents
        .into_iter()
        .map(|document| AssignedIdentity {
            id: document.id,
            source_pdf: document.pdf_path,
            sha256: document.sha256,
        })
        .collect())
}

// ───────────────────────── full run ─────────────────────────

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    if args.self_check {
        return run_self_check();
    }

    // ── committed carriers ────────────────────────────────────────────────
    let (manifest_bytes, manifest_sha) =
        read_carrier(args.root.as_path(), CORPUS_RESOURCE_HOLDOUT_MANIFEST_REL)?;
    let manifest_text = String::from_utf8(manifest_bytes)?;
    let assigned = assigned_from_manifest(&manifest_text)?;

    let (t01_bytes, t01_sha) =
        read_carrier(args.root.as_path(), CORPUS_RESOURCE_NATIVE_MEASUREMENT_REL)?;
    let native_receipt = NativeAcceptanceReceipt::parse_json_bytes(&t01_bytes)?;

    let (governor_bytes, governor_sha) =
        read_carrier(args.root.as_path(), CORPUS_RESOURCE_GOVERNOR_REL)?;
    let (recomputed, carriers) = recompute_freeze(args.root.as_path(), &governor_bytes)?;

    let (route_bytes, _) = read_carrier(args.root.as_path(), CORPUS_RESOURCE_ROUTE_RECEIPT_REL)?;
    let route_value: serde_json::Value = serde_json::from_slice(&route_bytes)?;
    let limits: RouteLimitsBinding = serde_json::from_value(
        route_value
            .get("limits")
            .cloned()
            .ok_or("the route receipt carries no `limits` block")?,
    )?;

    let (work_bytes, work_sha) =
        read_carrier(args.root.as_path(), CORPUS_RESOURCE_REFERENCE_WORK_REL)?;
    let _ = work_bytes;
    let (budget_bytes, budget_sha) =
        read_carrier(args.root.as_path(), CORPUS_RESOURCE_REFERENCE_BUDGET_REL)?;
    let _ = budget_bytes;

    let reference_carriers = vec![
        CorpusResourceReferenceCarrier {
            carrier: CarrierBinding {
                manifest: CORPUS_RESOURCE_REFERENCE_WORK_REL.to_owned(),
                pointer: "documents".to_owned(),
                revision_sha256: work_sha,
            },
            same_n: false,
            reason: CORPUS_RESOURCE_REFERENCE_NOT_SAME_N.to_owned(),
        },
        CorpusResourceReferenceCarrier {
            carrier: CarrierBinding {
                manifest: CORPUS_RESOURCE_REFERENCE_BUDGET_REL.to_owned(),
                pointer: "executed_population".to_owned(),
                revision_sha256: budget_sha,
            },
            same_n: false,
            reason: CORPUS_RESOURCE_REFERENCE_NOT_SAME_N.to_owned(),
        },
    ];

    let governance_pointer =
        format!("{CORPUS_RESOURCE_GOVERNOR_POINTER}.limit_policy.freeze_revision.carriers");
    let _ = carriers;

    let input = CorpusResourceSameNInput {
        assigned_source: CarrierBinding {
            manifest: CORPUS_RESOURCE_HOLDOUT_MANIFEST_REL.to_owned(),
            pointer: CORPUS_RESOURCE_HOLDOUT_POINTER.to_owned(),
            revision_sha256: manifest_sha,
        },
        native_measurement: CarrierBinding {
            manifest: CORPUS_RESOURCE_NATIVE_MEASUREMENT_REL.to_owned(),
            pointer: "documents".to_owned(),
            revision_sha256: t01_sha,
        },
        native_receipt,
        assigned: assigned.clone(),
        freeze_governor: CarrierBinding {
            manifest: CORPUS_RESOURCE_GOVERNOR_REL.to_owned(),
            pointer: governance_pointer,
            revision_sha256: governor_sha,
        },
        freeze: RouteFreezeDeclaration {
            freeze_revision: recomputed.clone(),
            metrics: limits.metrics.clone(),
            direction_vocabulary: limits.direction_vocabulary.clone(),
        },
        limits,
        reference_carriers,
        reference_metrics: reference_metrics(),
        claim_scope: "diagnostic_only".to_owned(),
        product_coverage_effect: "none".to_owned(),
        thresholds: "not_accepted".to_owned(),
    };

    let receipt = build_corpus_resource_same_n_receipt(&input)?;
    let expectation = CorpusResourceSameNExpectation {
        freeze_revision: recomputed,
        native_measurement: input.native_measurement.clone(),
        assigned,
    };
    validate_corpus_resource_same_n_receipt(&receipt, &expectation)?;

    let bytes = receipt.artifact_json_bytes()?;
    write_artifact_atomically(&args.report, &bytes)?;

    let observed_reference = receipt
        .reference_metrics
        .iter()
        .filter(|row| row.is_observed())
        .count();
    Ok(format!(
        "corpus resource same-n `{}`: {} assigned documents ({} measurable, {} excluded), \
         {} native metrics measured, {} reference metrics ({} observed, {} typed not_observed), \
         {} reference carriers all declared not same-n, {} documents rows; freeze {}; \
         pairing {} pooled={} saving_claim={}; canonical run",
        receipt.schema_version,
        receipt.populations.assigned,
        receipt.populations.measurable,
        receipt.populations.excluded,
        receipt.native_metrics.len(),
        receipt.reference_metrics.len(),
        observed_reference,
        receipt.reference_metrics.len() - observed_reference,
        receipt.reference_carriers.len(),
        receipt.documents.len(),
        receipt.limits.freeze_revision,
        receipt.pairing.rule,
        receipt.pairing.pooled,
        receipt.pairing.saving_claim,
    ))
}

fn write_artifact_atomically(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

// ──────────────────────────────── self-check ────────────────────────────────

/// Deterministic negative controls, all hermetic (no carrier, no write). Each
/// mutation must be refused **by name**; any mutation that slips through fails.
fn run_self_check() -> Result<String, Box<dyn Error>> {
    let (input, expectation) = synthetic_fixtures();
    let base = build_corpus_resource_same_n_receipt(&input)?;
    validate_corpus_resource_same_n_receipt(&base, &expectation)?;

    let mut checked = 0usize;

    // (1) document-set mismatch: a document sha256 drifts from the census.
    let mut m = base.clone();
    m.documents[0].sha256 = "sha256:0000".to_owned();
    expect_refusal("document_set_mismatch", &m, &expectation, "Mismatch")?;
    checked += 1;

    // (2) an observed reference row whose carrier is not declared same-n.
    let mut m = base.clone();
    m.reference_metrics[0] = SameNMetricObservation {
        id: "java_model_invocations".to_owned(),
        direction: None,
        state: "observed".to_owned(),
        value: Some(570),
        reason: None,
        source: Some(SameNSource {
            kind: "measured_reference".to_owned(),
            manifest: Some("reference/work.json".to_owned()),
            pointer: Some("documents".to_owned()),
            sha256: Some(sha256_hex(b"work")),
            measurement_state: Some("measured".to_owned()),
            metric_id: None,
        }),
    };
    expect_refusal(
        "reference_row_without_same_n_source",
        &m,
        &expectation,
        "ReferenceSourceUnpinned",
    )?;
    checked += 1;

    // (3) a pooled claim.
    let mut m = base.clone();
    m.pairing.pooled = "allowed".to_owned();
    expect_refusal("pooled_claim", &m, &expectation, "PooledClaimed")?;
    checked += 1;

    // (4) a saving claim.
    let mut m = base.clone();
    m.pairing.saving_claim = "allowed".to_owned();
    expect_refusal("saving_claim", &m, &expectation, "SavingClaimed")?;
    checked += 1;

    // (5) an absence reason outside the closed vocabulary.
    let mut m = base.clone();
    m.reference_metrics[0].reason = Some("mystery".to_owned());
    expect_refusal(
        "untyped_absence_reason",
        &m,
        &expectation,
        "AbsenceReasonUnknown",
    )?;
    checked += 1;

    // (6) an observed native row with an empty denominator.
    let mut m = base.clone();
    m.native_metrics[0].denominator = 0;
    expect_refusal("empty_denominator", &m, &expectation, "ZeroMasquerade")?;
    checked += 1;

    // (7) an opened lock.
    let mut m = base.clone();
    m.locks
        .insert("graph_write".to_owned(), "allowed".to_owned());
    expect_refusal("opened_lock", &m, &expectation, "LockOpened")?;
    checked += 1;

    // (8) a drifted freeze revision.
    let mut expectation = expectation.clone();
    expectation.freeze_revision = sha256_prefixed(b"drifted");
    expect_refusal(
        "freeze_revision_drift",
        &base,
        &expectation,
        "StaleFreezeRevision",
    )?;
    checked += 1;

    Ok(format!(
        "corpus resource same-n self-check: {checked} named refusals executed, all refused"
    ))
}

fn expect_refusal(
    label: &str,
    receipt: &da_cli::native_corpus_resource_same_n::CorpusResourceSameNReceipt,
    expectation: &CorpusResourceSameNExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_corpus_resource_same_n_receipt(receipt, expectation) {
        Ok(()) => Err(format!("{label} was admitted").into()),
        Err(error) => {
            let debug = format!("{error:?}");
            if debug.starts_with(variant) {
                Ok(())
            } else {
                Err(format!("{label} was refused as `{debug}`, expected `{variant}`").into())
            }
        }
    }
}

/// Hermetic fixtures: a synthetic T01 measurement, its assigned census, a
/// valid freeze and the matching receipt input/expectation. Nothing is read.
fn synthetic_fixtures() -> (CorpusResourceSameNInput, CorpusResourceSameNExpectation) {
    let (native_receipt, assigned) = synthetic_t01_receipt();
    let freeze_digest = sha256_prefixed(b"self-check-freeze");
    let limits = RouteLimitsBinding {
        freeze_revision: freeze_digest.clone(),
        freeze_state: "frozen".to_owned(),
        freeze_owner: "self-check".to_owned(),
        freeze_scope: "prospective_diagnostic_ceiling".to_owned(),
        tuned_after_scoring: "forbidden".to_owned(),
        zero_is_not_a_measurement: true,
        silent_clamp: "forbidden".to_owned(),
        pass_requires_frozen_threshold: true,
        claim_without_freeze: "forbidden".to_owned(),
        direction_vocabulary: vec!["floor".to_owned(), "ceiling".to_owned()],
        metrics: vec![RouteMetricDeclaration {
            id: "java_cpu_wall_clock_ns".to_owned(),
            direction: "ceiling".to_owned(),
        }],
    };
    let native_measurement = CarrierBinding {
        manifest: "t01.json".to_owned(),
        pointer: "documents".to_owned(),
        revision_sha256: sha256_prefixed(b"t01"),
    };
    let input = CorpusResourceSameNInput {
        assigned_source: CarrierBinding {
            manifest: "manifest.yaml".to_owned(),
            pointer: "articles".to_owned(),
            revision_sha256: sha256_prefixed(b"manifest"),
        },
        native_measurement: native_measurement.clone(),
        native_receipt,
        assigned: assigned.clone(),
        freeze_governor: CarrierBinding {
            manifest: "governor.yaml".to_owned(),
            pointer: "native_route_e2e.limit_policy.freeze_revision.carriers".to_owned(),
            revision_sha256: sha256_prefixed(b"governor"),
        },
        freeze: RouteFreezeDeclaration {
            freeze_revision: freeze_digest.clone(),
            metrics: limits.metrics.clone(),
            direction_vocabulary: limits.direction_vocabulary.clone(),
        },
        limits,
        reference_carriers: vec![CorpusResourceReferenceCarrier {
            carrier: CarrierBinding {
                manifest: "reference/work.json".to_owned(),
                pointer: "documents".to_owned(),
                revision_sha256: sha256_prefixed(b"work"),
            },
            same_n: false,
            reason: CORPUS_RESOURCE_REFERENCE_NOT_SAME_N.to_owned(),
        }],
        reference_metrics: reference_metrics(),
        claim_scope: "diagnostic_only".to_owned(),
        product_coverage_effect: "none".to_owned(),
        thresholds: "not_accepted".to_owned(),
    };
    let expectation = CorpusResourceSameNExpectation {
        freeze_revision: freeze_digest,
        native_measurement,
        assigned,
    };
    (input, expectation)
}

/// A two-document T01 receipt and its matching assigned census, built through
/// the T01 builder so the reused guard/partition checks are the real ones.
fn synthetic_t01_receipt() -> (NativeAcceptanceReceipt, Vec<AssignedIdentity>) {
    let expectation = AcceptanceExpectation {
        pins: AcceptanceCarrierBinding {
            path: "pins.json".to_owned(),
            pointer: "values".to_owned(),
            sha256: "sha256:aaaa".to_owned(),
        },
        probe: AcceptanceCarrierBinding {
            path: "probe.json".to_owned(),
            pointer: "reference_population".to_owned(),
            sha256: "sha256:bbbb".to_owned(),
        },
        schema: AcceptanceCarrierBinding {
            path: "schema.yaml".to_owned(),
            pointer: "required_scope.families".to_owned(),
            sha256: "sha256:cccc".to_owned(),
        },
        thresholds: vec![AcceptanceThresholdExpectation {
            threshold_id: "threshold.frozen".to_owned(),
            metric_id: "metric.frozen".to_owned(),
            family_id: "family.required".to_owned(),
            stratum: None,
        }],
        families: vec![AcceptanceFamilyExpectation {
            family_id: "family.required".to_owned(),
            state: "required".to_owned(),
            reason: None,
        }],
        populations: AcceptancePopulations {
            assigned: 2,
            measurable: 1,
            refused: 0,
            excluded: 1,
        },
        documents: vec![
            AcceptanceDocumentExpectation {
                id: "d1".to_owned(),
                source_pdf: "a.pdf".to_owned(),
                sha256: "sha256:1111".to_owned(),
                state: "measurable".to_owned(),
                exclusion_reason: None,
            },
            AcceptanceDocumentExpectation {
                id: "d2".to_owned(),
                source_pdf: "b.pdf".to_owned(),
                sha256: "sha256:2222".to_owned(),
                state: "excluded".to_owned(),
                exclusion_reason: Some("header_date_absent".to_owned()),
            },
        ],
    };
    let receipt = build_native_acceptance(
        &expectation,
        AcceptanceGuard {
            observer: OBSERVER.to_owned(),
            installed: true,
            identity_isolated: true,
            observed_java_calls: 0,
            positive_control: true,
        },
        vec![
            AcceptanceDocumentRow {
                id: "d1".to_owned(),
                source_pdf: "a.pdf".to_owned(),
                sha256: "sha256:1111".to_owned(),
                state: "measurable".to_owned(),
                exclusion_reason: None,
                pages: Some(1),
                layout_units: Some(2),
                layout_digest: Some("sha256:dddd".to_owned()),
                plane_state: "observed".to_owned(),
                plane_reason: None,
            },
            AcceptanceDocumentRow {
                id: "d2".to_owned(),
                source_pdf: "b.pdf".to_owned(),
                sha256: "sha256:2222".to_owned(),
                state: "excluded".to_owned(),
                exclusion_reason: Some("header_date_absent".to_owned()),
                pages: None,
                layout_units: None,
                layout_digest: None,
                plane_state: "not_run".to_owned(),
                plane_reason: None,
            },
        ],
        vec![AcceptanceMetricRow {
            metric_id: "native_acceptance.pdf_verification_coverage".to_owned(),
            numerator: 2,
            denominator: 2,
            stratum: None,
        }],
        vec![AcceptanceGapCell {
            threshold_id: "threshold.frozen".to_owned(),
            metric_id: "metric.frozen".to_owned(),
            family_id: "family.required".to_owned(),
            kind: "not_observable".to_owned(),
            reason: "producing_mechanism_absent".to_owned(),
        }],
    )
    .expect("the synthetic T01 receipt must validate");

    let assigned = vec![
        AssignedIdentity {
            id: "d1".to_owned(),
            source_pdf: "a.pdf".to_owned(),
            sha256: "sha256:1111".to_owned(),
        },
        AssignedIdentity {
            id: "d2".to_owned(),
            source_pdf: "b.pdf".to_owned(),
            sha256: "sha256:2222".to_owned(),
        },
    ];
    (receipt, assigned)
}
