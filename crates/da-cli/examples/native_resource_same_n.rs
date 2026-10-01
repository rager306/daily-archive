#![cfg(all(feature = "pdf-layout", feature = "pdf-classifier"))]
//! S06/T06 (M514-1lss2s) — same-n resource comparison runner.
//!
//! This is the executable half of the same-n resource contract. It measures the
//! native S03 plane extraction cost on exactly the three pinned documents and
//! pairs each native metric with a reference row **only** where a pinned same-n
//! source exists (`artifacts/parse_compat/native_java_work.json`), recording a
//! typed `not_observed{reason}` everywhere else (D510). It writes
//! [`da_cli::native_resource_same_n::DEFAULT_SAME_N_ARTIFACT_REL`].
//!
//! What it does, and deliberately does not do:
//!
//! - It reads the declaration, the corpus pin, the freeze and the frozen metric
//!   dictionary **by pointer/from the committed T03 receipt**. It never restates
//!   the document identities or the ceiling dictionary.
//! - It recomputes the composite freeze revision from the committed carrier
//!   bytes and refuses the run when it drifted (`fail-closed`).
//! - It refuses when the reference document set differs from the native one:
//!   `data/parse/comparison_contract.yaml#same_n.mismatch = hard_failure`.
//! - It refuses to claim a reference row without a pinned measured carrier, and
//!   it never reports a `not_observed` value as a number.
//! - It installs a Java-consult observer on the real transport seam and records
//!   a positive control, so the observed Java zero is a measurement rather than
//!   an absence. The native route never consults Java.
//!
//! Exit status is non-zero — and **no** receipt is written — on PDF sha256
//! drift, a drifted freeze revision, a same-n document mismatch, an unpinned
//! reference source, a pooled/saving claim, or an untyped metric row.
//!
//! Diagnostic only: `claim_scope = diagnostic_only`, `product_coverage_effect
//! = none`, `thresholds = not_accepted`, no promotion, no production splice, no
//! graph write (D127 / D499 / D510).

use da_adapters::PdfInspectorLayoutDonor;
use da_adapters::layout_donors::LayoutDonorLimits;
use da_cli::java_free_slice::{
    COMPATIBILITY_DATA_REL, SLICE_DECLARATION_DATA_REL, load_declaration, sha256_hex,
};
use da_cli::native_family_receipt::{
    CarrierBinding, GuardObserver, GuardProvenance, GuardProvenanceReceipt,
    PRODUCT_COVERAGE_EFFECT_NONE, normalize_document_identity, sha256_prefixed,
};
use da_cli::native_resource_same_n::{
    DEFAULT_SAME_N_ARTIFACT_REL, NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION, SAME_N_ABSENCE_POLICY,
    SAME_N_MISMATCH_HARD_FAILURE, SAME_N_POOLED_FORBIDDEN, SAME_N_REFERENCE_BUDGET_REL,
    SAME_N_REFERENCE_METRIC_ORDER, SAME_N_REFERENCE_WORK_REL, SAME_N_SAVING_CLAIM_FORBIDDEN,
    SameNCorpusBinding, SameNDocumentIdentity, SameNExpectation, SameNInput, SameNMeasuredDocument,
    SameNMetricObservation, SameNReceipt, SameNSource, build_same_n_receipt,
    validate_same_n_receipt,
};
use da_cli::native_route_receipt::{
    DEFAULT_ROUTE_ARTIFACT_REL, RouteFreezeDeclaration, RouteReceipt,
};
use da_parse::{
    GrobidHttpBackend, GrobidProfile, JavaInvocationCounter, ParseBackend, ParseBackendRequest,
};
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

/// The governor sibling DATA block that carries the prospective limit freeze.
const GOVERNOR_REL: &str = "scripts/governor/checks/grobid_absorption_progress.yaml";
/// Pointer inside the governor block.
const GOVERNOR_POINTER: &str = "native_route_e2e";
/// Provisional loopback endpoint; the real route never consults it.
const GUARD_ENDPOINT: &str = "http://127.0.0.1:9";
/// An unroutable seam for the guard's positive control.
const CONTROL_ENDPOINT: &str = "http://";

// ─────────────────────────────── arguments ───────────────────────────────

struct Args {
    root: PathBuf,
    report: PathBuf,
    self_check: bool,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_SAME_N_ARTIFACT_REL);
    let mut self_check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--self-check" => self_check = true,
            "--help" => {
                println!("native_resource_same_n [--root DIR] [--report FILE] [--self-check]");
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
    let args = parse_args();
    let _log_guard = da_cli::logging::init_pipeline_logging("native_resource_same_n");
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_resource_same_n: {error}");
            ExitCode::FAILURE
        }
    }
}

// ─────────────────────────────── helpers ───────────────────────────────

fn yaml(bytes: &[u8]) -> Result<Value, Box<dyn Error>> {
    Ok(serde_yaml::from_slice(bytes)?)
}

/// Recompute `sha256(join("<path>:<sha256-hex>", "\n"))` over the freeze
/// carriers the T02 freeze declares, from their committed bytes.
fn recompute_freeze_revision(root: &std::path::Path) -> Result<String, Box<dyn Error>> {
    let governor = yaml(&fs::read(root.join(GOVERNOR_REL))?)?;
    let carriers = governor[GOVERNOR_POINTER]["limit_policy"]["freeze_revision"]["carriers"]
        .as_sequence()
        .ok_or("the governor block declares no freeze carriers")?;
    let mut lines = Vec::with_capacity(carriers.len());
    for carrier in carriers {
        let path = carrier["path"]
            .as_str()
            .ok_or("a freeze carrier has no path")?;
        let bytes = fs::read(root.join(path))?;
        lines.push(format!("{path}:{}", sha256_hex(&bytes)));
    }
    Ok(sha256_prefixed(lines.join("\n").as_bytes()))
}

/// The process peak resident set size, read from `/proc/self/status#VmHWM`.
fn peak_rss_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb.saturating_mul(1024))
}

/// Nearest-rank percentile over an ascending sample.
fn percentile(sorted: &[u64], numerator: u64, denominator: u64) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let n = sorted.len() as u64;
    let rank = (numerator * n).div_ceil(denominator).max(1);
    let index = rank.saturating_sub(1).min(n - 1) as usize;
    Some(sorted[index])
}

fn native_metric(
    id: &str,
    direction: Option<&str>,
    observed: Option<u64>,
) -> SameNMetricObservation {
    match observed {
        Some(value) => SameNMetricObservation {
            id: id.to_owned(),
            direction: direction.map(str::to_owned),
            state: "observed".to_owned(),
            value: Some(value),
            reason: None,
            source: Some(SameNSource {
                kind: "native_resource_run".to_owned(),
                manifest: None,
                pointer: Some("documents[].native_latency_ms".to_owned()),
                sha256: None,
                measurement_state: Some("measured".to_owned()),
                metric_id: Some(id.to_owned()),
            }),
        },
        None => SameNMetricObservation {
            id: id.to_owned(),
            direction: direction.map(str::to_owned),
            state: "not_observed".to_owned(),
            value: None,
            reason: Some("native_metric_not_measured".to_owned()),
            source: None,
        },
    }
}

fn reference_metric(
    id: &str,
    observed: Option<u64>,
    carrier: &str,
    digest: &str,
    pointer: &str,
    metric_id: &str,
    absence_reason: &str,
) -> SameNMetricObservation {
    match observed {
        Some(value) => SameNMetricObservation {
            id: id.to_owned(),
            direction: None,
            state: "observed".to_owned(),
            value: Some(value),
            reason: None,
            source: Some(SameNSource {
                kind: "measured_reference".to_owned(),
                manifest: Some(carrier.to_owned()),
                pointer: Some(pointer.to_owned()),
                sha256: Some(digest.to_owned()),
                measurement_state: Some("measured".to_owned()),
                metric_id: Some(metric_id.to_owned()),
            }),
        },
        None => SameNMetricObservation {
            id: id.to_owned(),
            direction: None,
            state: "not_observed".to_owned(),
            value: None,
            reason: Some(absence_reason.to_owned()),
            source: None,
        },
    }
}

/// The pinned reference work rows: `(document identity, total_invocations)`.
fn parse_java_work(
    bytes: &[u8],
) -> Result<(Vec<SameNDocumentIdentity>, BTreeMap<String, u64>), Box<dyn Error>> {
    let document: Value = yaml(bytes)?;
    let corpus = document["corpus"]["documents"]
        .as_sequence()
        .ok_or("native_java_work has no corpus documents")?;
    let mut identities = Vec::with_capacity(corpus.len());
    for entry in corpus {
        identities.push(SameNDocumentIdentity {
            id: entry["id"]
                .as_str()
                .ok_or("corpus document has no id")?
                .to_owned(),
            source_pdf: entry["source_pdf"]
                .as_str()
                .ok_or("corpus document has no source_pdf")?
                .to_owned(),
            sha256: entry["sha256"]
                .as_str()
                .ok_or("corpus document has no sha256")?
                .to_owned(),
            pages: entry["pages"]
                .as_u64()
                .ok_or("corpus document has no pages")?,
        });
    }
    let mut work = BTreeMap::new();
    for entry in document["documents"]
        .as_sequence()
        .ok_or("native_java_work has no documents")?
    {
        let id = entry["document_id"]
            .as_str()
            .ok_or("work row has no document_id")?
            .to_owned();
        let receipt = &entry["receipt"];
        if receipt["measurement_state"].as_str() != Some("measured") {
            return Err(format!("work row `{id}` is not measured").into());
        }
        let invocations = receipt["total_invocations"]
            .as_u64()
            .ok_or_else(|| format!("work row `{id}` has no total_invocations"))?;
        work.insert(normalize_document_identity(&id), invocations);
    }
    Ok((identities, work))
}

/// Refuse when the reference document set is not the native one
/// (`same_n.mismatch = hard_failure`).
fn require_same_n(
    native: &[SameNDocumentIdentity],
    reference: &[SameNDocumentIdentity],
) -> Result<(), Box<dyn Error>> {
    let native: BTreeMap<&str, &str> = native
        .iter()
        .map(|document| (document.id.as_str(), document.sha256.as_str()))
        .collect();
    let reference: BTreeMap<&str, &str> = reference
        .iter()
        .map(|document| (document.id.as_str(), document.sha256.as_str()))
        .collect();
    if native != reference {
        return Err(format!(
            "same_n mismatch (hard failure): native {:?}, reference {:?}",
            native.keys().collect::<Vec<_>>(),
            reference.keys().collect::<Vec<_>>()
        )
        .into());
    }
    Ok(())
}

/// Confirm the batch-budget live observation is container-wide rather than a
/// per-document same-n row, which is why the Java CPU row is a typed absence.
fn require_budget_row_is_not_same_n(bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let document: Value = yaml(bytes)?;
    for batch in document["batches"].as_sequence().ok_or("no batches")? {
        if batch["documents"]
            .as_sequence()
            .is_some_and(|documents| !documents.is_empty())
        {
            return Err(
                "the Java batch-budget observation now carries per-document rows; \
                        the Java CPU row must be re-examined"
                    .into(),
            );
        }
    }
    Ok(())
}

// ─────────────────────────────── runner ───────────────────────────────

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    if args.self_check {
        return run_self_check(&args.root);
    }

    // ── declaration ───────────────────────────────────────────────────────
    let layout_bytes = fs::read(args.root.join(SLICE_DECLARATION_DATA_REL))?;
    let compatibility_bytes = fs::read(args.root.join(COMPATIBILITY_DATA_REL))?;
    let declaration = load_declaration(
        std::str::from_utf8(&layout_bytes)?,
        std::str::from_utf8(&compatibility_bytes)?,
    )?;

    // ── committed T03 receipt: freeze binding and corpus pins ─────────────
    let route_bytes = fs::read(args.root.join(DEFAULT_ROUTE_ARTIFACT_REL))?;
    let route = RouteReceipt::parse_json_bytes(&route_bytes)?;
    let recomputed = recompute_freeze_revision(&args.root)?;
    if route.limits.freeze_revision != recomputed {
        return Err(format!(
            "stale freeze revision: recomputed {recomputed}, route receipt {}",
            route.limits.freeze_revision
        )
        .into());
    }
    if route.declaration.revision_sha256 != sha256_prefixed(&layout_bytes) {
        return Err("the route declaration revision drifted from the committed carrier".into());
    }
    let freeze = RouteFreezeDeclaration {
        freeze_revision: route.limits.freeze_revision.clone(),
        metrics: route.limits.metrics.clone(),
        direction_vocabulary: route.limits.direction_vocabulary.clone(),
    };
    let declaration_binding: CarrierBinding = route.declaration.clone();

    let documents: Vec<SameNDocumentIdentity> = declaration
        .documents
        .iter()
        .map(|document| SameNDocumentIdentity {
            id: document.id.clone(),
            source_pdf: document.source_pdf.clone(),
            sha256: document.sha256.clone(),
            pages: document.pages,
        })
        .collect();
    let corpus = SameNCorpusBinding {
        document_count: documents.len() as u64,
        page_denominator: documents.iter().map(|document| document.pages).sum(),
        documents: documents.clone(),
    };

    // ── pinned same-n reference rows ──────────────────────────────────────
    let work_bytes = fs::read(args.root.join(SAME_N_REFERENCE_WORK_REL))?;
    let work_digest = sha256_hex(&work_bytes);
    let (reference_documents, work) = parse_java_work(&work_bytes)?;
    require_same_n(&documents, &reference_documents)?;

    let budget_bytes = fs::read(args.root.join(SAME_N_REFERENCE_BUDGET_REL))?;
    require_budget_row_is_not_same_n(&budget_bytes)?;

    // ── guard on the real Java seam, with a positive control ──────────────
    let counter = JavaInvocationCounter::new();
    let plain =
        GrobidHttpBackend::new(Some(GUARD_ENDPOINT)).with_profile(GrobidProfile::structural());
    let watched = plain.clone().with_invocation_observer(counter.clone());
    let identity_isolated = plain.identity() == watched.identity();
    let installed = watched.java_invocation_observation().is_observed();
    guard_positive_control()?;
    let observed_java_calls = counter
        .observation()
        .attempts()
        .ok_or("an installed observer must read as observed")?;
    let guard = GuardProvenance {
        installed,
        identity_isolated,
        observed_java_calls,
    };

    // ── per-document native resource measurement ──────────────────────────
    let limits_donor = LayoutDonorLimits::default();
    let mut samples = Vec::with_capacity(documents.len());
    let mut measured = Vec::with_capacity(documents.len());
    for document in &documents {
        let bytes = fs::read(args.root.join(&document.source_pdf))?;
        let digest = sha256_hex(&bytes);
        if digest != document.sha256 {
            return Err(format!(
                "checksum drift for {}: observed {digest}, pinned {}",
                document.id, document.sha256
            )
            .into());
        }
        let started = Instant::now();
        let observation =
            PdfInspectorLayoutDonor.extract_shadow(&bytes, &document.id, limits_donor)?;
        let elapsed_ms = started.elapsed().as_millis().max(1) as u64;
        let document_key = normalize_document_identity(&document.id);
        let invocations = work.get(&document_key).copied();
        if invocations.is_none() {
            return Err(format!("no measured Java work row for {}", document.id).into());
        }
        let reference = vec![
            reference_metric(
                "java_model_invocations",
                invocations,
                SAME_N_REFERENCE_WORK_REL,
                &work_digest,
                "documents[].receipt.total_invocations",
                "total_invocations",
                "reference_row_absent",
            ),
            reference_metric(
                "java_cpu_wall_clock_ns",
                None,
                SAME_N_REFERENCE_BUDGET_REL,
                &sha256_hex(&budget_bytes),
                "batches[].budget_observation",
                "java_cpu_wall_clock_ns",
                "reference_row_not_same_n",
            ),
        ];
        // The native plane must be non-empty for the measurement to mean anything.
        if observation.lexemes.is_empty() {
            return Err(format!(
                "the native plane for {} is empty; the extraction cost is vacuous",
                document.id
            )
            .into());
        }
        samples.push(elapsed_ms);
        measured.push(SameNMeasuredDocument {
            id: document.id.clone(),
            source_pdf: document.source_pdf.clone(),
            sha256: digest,
            pages: document.pages,
            native_latency_ms: Some(elapsed_ms),
            reference,
        });
    }

    // ── population-level native metrics ───────────────────────────────────
    let mut sorted = samples.clone();
    sorted.sort_unstable();
    let total_pages: u64 = documents.iter().map(|document| document.pages).sum();
    let total_ms: u64 = samples.iter().sum();
    let throughput = total_pages.saturating_mul(1000).checked_div(total_ms);
    let native_metrics = vec![
        native_metric(
            "latency_p50_ms",
            Some("ceiling"),
            percentile(&sorted, 50, 100),
        ),
        native_metric(
            "latency_p95_ms",
            Some("ceiling"),
            percentile(&sorted, 95, 100),
        ),
        native_metric(
            "latency_p99_ms",
            Some("ceiling"),
            percentile(&sorted, 99, 100),
        ),
        native_metric("peak_rss_bytes", Some("ceiling"), peak_rss_bytes()),
        native_metric("peak_retained_bytes", Some("ceiling"), None),
        native_metric("throughput", None, throughput),
    ];

    // ── population-level reference metrics ────────────────────────────────
    let total_invocations: u64 = work.values().sum();
    let reference_metrics = vec![
        reference_metric(
            "java_model_invocations",
            Some(total_invocations),
            SAME_N_REFERENCE_WORK_REL,
            &work_digest,
            "documents[].receipt.total_invocations",
            "total_invocations",
            "reference_row_absent",
        ),
        reference_metric(
            "java_cpu_wall_clock_ns",
            None,
            SAME_N_REFERENCE_BUDGET_REL,
            &sha256_hex(&budget_bytes),
            "batches[].budget_observation",
            "java_cpu_wall_clock_ns",
            "reference_row_not_same_n",
        ),
    ];

    let input = SameNInput {
        declaration: declaration_binding,
        guard,
        freeze,
        limits: route.limits.clone(),
        corpus: corpus.clone(),
        documents: measured,
        native_metrics,
        reference_metrics,
        claim_scope: route.claim_scope.clone(),
        product_coverage_effect: PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        thresholds: route.thresholds.clone(),
    };
    let receipt = build_same_n_receipt(&input)?;
    let expectation = SameNExpectation {
        freeze_revision: recomputed.clone(),
        corpus,
        reference_documents,
    };
    validate_same_n_receipt(&receipt, &expectation)?;

    let artifact = receipt.artifact_json_bytes()?;
    write_artifact(&args.report, &artifact)?;

    Ok(summary(&receipt))
}

fn write_artifact(path: &std::path::Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn summary(receipt: &SameNReceipt) -> String {
    format!(
        "same-n resource `{}`: {} documents / {} pages; native metrics {} observed / {} typed absence; \
         reference rows bound {}; pairing {} pooled={} saving_claim={} mismatch={}; freeze {}",
        receipt.schema_version,
        receipt.corpus.document_count,
        receipt.corpus.page_denominator,
        receipt.residuals.observed_metrics,
        receipt.residuals.absent_metrics,
        receipt.residuals.reference_rows_bound,
        receipt.pairing.rule,
        receipt.pairing.pooled,
        receipt.pairing.saving_claim,
        receipt.pairing.mismatch_disposition,
        receipt.limits.freeze_revision,
    )
}

/// A positive control on a separate unreachable seam: the observer must raise
/// its counter for an attempted consult, or the observed zero is vacuous.
fn guard_positive_control() -> Result<(), Box<dyn Error>> {
    let control = JavaInvocationCounter::new();
    let backend = GrobidHttpBackend::new(Some(CONTROL_ENDPOINT))
        .with_profile(GrobidProfile::structural())
        .with_invocation_observer(control.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let request = ParseBackendRequest::new("same-n-guard-control", b"%PDF-1.4".to_vec());
    let _ = runtime.block_on(async { backend.fetch_tei(request).await });
    if control.observation().attempts() != Some(1) {
        return Err("the Java guard positive control did not raise the counter".into());
    }
    Ok(())
}

// ──────────────────────────────── self-check ────────────────────────────────

/// Deterministic negative controls, all hermetic (no PDF, no corpus, no write).
///
/// Every mutation must be refused **by name**; any mutation that slips through
/// fails the self-check.
fn run_self_check(root: &std::path::Path) -> Result<String, Box<dyn Error>> {
    let mut checked = 0usize;

    // Build a synthetic, file-backed input from the live DATA and the committed
    // T03 receipt: two documents, one pinned reference row each.
    let layout_bytes = fs::read(root.join(SLICE_DECLARATION_DATA_REL))?;
    let compatibility_bytes = fs::read(root.join(COMPATIBILITY_DATA_REL))?;
    let declaration = load_declaration(
        std::str::from_utf8(&layout_bytes)?,
        std::str::from_utf8(&compatibility_bytes)?,
    )?;
    let route = RouteReceipt::parse_json_bytes(&fs::read(root.join(DEFAULT_ROUTE_ARTIFACT_REL))?)?;
    let freeze = RouteFreezeDeclaration {
        freeze_revision: route.limits.freeze_revision.clone(),
        metrics: route.limits.metrics.clone(),
        direction_vocabulary: route.limits.direction_vocabulary.clone(),
    };
    let declared: Vec<SameNDocumentIdentity> = declaration
        .documents
        .iter()
        .map(|document| SameNDocumentIdentity {
            id: document.id.clone(),
            source_pdf: document.source_pdf.clone(),
            sha256: document.sha256.clone(),
            pages: document.pages,
        })
        .collect();
    let corpus = SameNCorpusBinding {
        document_count: declared.len() as u64,
        page_denominator: declared.iter().map(|document| document.pages).sum(),
        documents: declared.clone(),
    };
    let synthetic =
        |document: &SameNDocumentIdentity, latency: Option<u64>| SameNMeasuredDocument {
            id: document.id.clone(),
            source_pdf: document.source_pdf.clone(),
            sha256: document.sha256.clone(),
            pages: document.pages,
            native_latency_ms: latency,
            reference: vec![SameNMetricObservation {
                id: "java_model_invocations".to_owned(),
                direction: None,
                state: "observed".to_owned(),
                value: Some(1),
                reason: None,
                source: Some(SameNSource {
                    kind: "measured_reference".to_owned(),
                    manifest: Some(SAME_N_REFERENCE_WORK_REL.to_owned()),
                    pointer: Some("documents[].receipt.total_invocations".to_owned()),
                    sha256: Some("0".repeat(64)),
                    measurement_state: Some("measured".to_owned()),
                    metric_id: Some("total_invocations".to_owned()),
                }),
            }],
        };
    let documents: Vec<SameNMeasuredDocument> = declared
        .iter()
        .enumerate()
        .map(|(index, document)| synthetic(document, Some((index + 1) as u64)))
        .collect();
    let native_metrics = vec![
        native_metric("latency_p50_ms", Some("ceiling"), Some(1)),
        native_metric("latency_p95_ms", Some("ceiling"), Some(2)),
        native_metric("latency_p99_ms", Some("ceiling"), Some(2)),
        native_metric("peak_rss_bytes", Some("ceiling"), Some(1024)),
        native_metric("peak_retained_bytes", Some("ceiling"), None),
        native_metric("throughput", None, Some(10)),
    ];
    let reference_metrics = vec![
        reference_metric(
            "java_model_invocations",
            Some(2),
            SAME_N_REFERENCE_WORK_REL,
            &"1".repeat(64),
            "documents[].receipt.total_invocations",
            "total_invocations",
            "reference_row_absent",
        ),
        reference_metric(
            "java_cpu_wall_clock_ns",
            None,
            SAME_N_REFERENCE_BUDGET_REL,
            &"2".repeat(64),
            "batches[].budget_observation",
            "java_cpu_wall_clock_ns",
            "reference_row_not_same_n",
        ),
    ];
    let base = SameNInput {
        declaration: route.declaration.clone(),
        guard: GuardProvenance {
            installed: true,
            identity_isolated: true,
            observed_java_calls: 0,
        },
        freeze: freeze.clone(),
        limits: route.limits.clone(),
        corpus: corpus.clone(),
        documents: documents.clone(),
        native_metrics: native_metrics.clone(),
        reference_metrics: reference_metrics.clone(),
        claim_scope: route.claim_scope.clone(),
        product_coverage_effect: PRODUCT_COVERAGE_EFFECT_NONE.to_owned(),
        thresholds: route.thresholds.clone(),
    };

    // (a) the happy path builds, so each mutation below is the only difference.
    let good = build_same_n_receipt(&base)?;
    checked += 1;

    // (b) a same-n document mismatch is a hard failure.
    let mut mismatched = base.clone();
    mismatched.documents[0].sha256 = "f".repeat(64);
    if build_same_n_receipt(&mismatched).is_ok() {
        return Err("a same-n sha256 mismatch was admitted".into());
    }
    checked += 1;

    // (c) a not_observed row carrying a number is a zero masquerade.
    let mut masquerade = base.clone();
    masquerade.native_metrics[4].value = Some(0);
    if build_same_n_receipt(&masquerade)
        .err()
        .map(|error| format!("{error:?}").contains("ZeroMasquerade"))
        != Some(true)
    {
        return Err("a not_observed row carrying a number was admitted".into());
    }
    checked += 1;

    // (d) an invented absence reason is refused by name.
    let mut invented = base.clone();
    invented.native_metrics[4].reason = Some("invented_reason".to_owned());
    if build_same_n_receipt(&invented)
        .err()
        .map(|error| format!("{error:?}").contains("AbsenceReasonUnknown"))
        != Some(true)
    {
        return Err("an invented absence reason was admitted".into());
    }
    checked += 1;

    // (e) an observed reference row with an absolute path is unpinned.
    let mut unpinned = base.clone();
    unpinned.documents[0].reference[0].source = Some(SameNSource {
        kind: "measured_reference".to_owned(),
        manifest: Some("/var/tmp/leak.json".to_owned()),
        pointer: Some("p".to_owned()),
        sha256: Some("0".repeat(64)),
        measurement_state: Some("measured".to_owned()),
        metric_id: Some("total_invocations".to_owned()),
    });
    if build_same_n_receipt(&unpinned)
        .err()
        .map(|error| format!("{error:?}").contains("ReferenceSourceUnpinned"))
        != Some(true)
    {
        return Err("an unpinned reference source was admitted".into());
    }
    checked += 1;

    // (f) a non-measured reference carrier is refused.
    let mut unmeasured = base.clone();
    unmeasured.documents[0].reference[0]
        .source
        .as_mut()
        .expect("source")
        .measurement_state = Some("not_observed".to_owned());
    if build_same_n_receipt(&unmeasured)
        .err()
        .map(|error| format!("{error:?}").contains("ReferenceSourceUnpinned"))
        != Some(true)
    {
        return Err("a not-measured reference source was admitted".into());
    }
    checked += 1;

    // (g) a stale freeze revision is refused by the validator.
    let expectation = SameNExpectation {
        freeze_revision: sha256_prefixed(b"drifted"),
        corpus: corpus.clone(),
        reference_documents: declared.clone(),
    };
    if validate_same_n_receipt(&good, &expectation)
        .err()
        .map(|error| format!("{error:?}").contains("StaleFreezeRevision"))
        != Some(true)
    {
        return Err("a stale freeze revision was admitted".into());
    }
    checked += 1;

    // (h) a drifted pairing is refused by the validator.
    let mut pooled = good.clone();
    pooled.pairing.pooled = "allowed".to_owned();
    let fresh = SameNExpectation {
        freeze_revision: route.limits.freeze_revision.clone(),
        corpus: corpus.clone(),
        reference_documents: declared.clone(),
    };
    if validate_same_n_receipt(&pooled, &fresh)
        .err()
        .map(|error| format!("{error:?}").contains("PooledClaimed"))
        != Some(true)
    {
        return Err("a pooled claim was admitted".into());
    }
    checked += 1;

    // (i) a claimed saving is refused by the validator.
    let mut saving = good.clone();
    saving.pairing.saving_claim = "claimed".to_owned();
    if validate_same_n_receipt(&saving, &fresh)
        .err()
        .map(|error| format!("{error:?}").contains("SavingClaimed"))
        != Some(true)
    {
        return Err("a saving claim was admitted".into());
    }
    checked += 1;

    // (j) an observed zero without a guard is refused by the builder.
    let mut unguarded = base.clone();
    unguarded.guard.installed = false;
    if build_same_n_receipt(&unguarded)
        .err()
        .map(|error| format!("{error:?}").contains("GuardMissingForObservedZero"))
        != Some(true)
    {
        return Err("an unguarded observed zero was admitted".into());
    }
    checked += 1;

    // (k) the declared constants are the ones the receipt must carry.
    if good.pairing.mismatch_disposition != SAME_N_MISMATCH_HARD_FAILURE
        || good.pairing.pooled != SAME_N_POOLED_FORBIDDEN
        || good.pairing.saving_claim != SAME_N_SAVING_CLAIM_FORBIDDEN
        || good.pairing.absence_policy != SAME_N_ABSENCE_POLICY
        || good.schema_version != NATIVE_RESOURCE_SAME_N_SCHEMA_VERSION
        || SAME_N_REFERENCE_METRIC_ORDER.len() != 2
        || good.guard.observer != GuardObserver::JavaTransportFetchTei
    {
        return Err("the receipt drifted from the declared constants".into());
    }
    checked += 1;

    Ok(format!(
        "same-n resource self-check: {checked} named refusals executed, all refused"
    ))
}

// Keep the receipt type and its guard row in scope for the summary typos lint.
const _: fn(&GuardProvenanceReceipt) = |_| {};
