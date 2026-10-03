#![cfg(all(feature = "pdf-layout", feature = "pdf-classifier"))]
//! S08/T01 (M514-1lss2s) — corpus-bound native acceptance measurement runner.
//!
//! This is the executable half of the S08 acceptance measurement contract. It
//! turns the frozen 610-document holdout, the frozen prospective regression floor
//! (`absorption_progress_reference_pins.json`, resolved **by pointer**) and the
//! closed absorption-progress schema into a corpus-bound
//! [`da_cli::native_acceptance::NativeAcceptanceReceipt`] at
//! `artifacts/parse_compat/native_corpus_acceptance.json`.
//!
//! ## What it measures, and what it names as a gap
//!
//! Where a frozen metric is natively observable *through this runner's own
//! producing path*, it is measured with a non-empty denominator. Where the
//! producing mechanism is absent, the runner emits a **typed gap cell**
//! (`not_observable` with a machine-readable reason) instead of manufacturing a
//! value: the class-parity axis behind the two extraction thresholds is not a
//! native route product. The preservation threshold
//! (`preservation.occurrence_conservation`) is measured over the frozen
//! reference TEI through the shared composition scanner
//! (`da_cli::native_composition_measurement`), whose admitted/source aggregate is
//! emission-independent; its former `producing_mechanism_absent` gap cell is
//! dropped once the metric is measured. A family the closed schema still carries
//! as `threshold_pending` is emitted as a `threshold_pending` gap.
//!
//! Every one of the 610 assigned documents is re-verified by PDF sha256 before
//! any work; the 531 measurable documents are folded through the S03 native
//! plane and their observation digest is recorded. An uncovered *plane* is a
//! typed `refused` cell, never a silent zero.
//!
//! ## Modes
//!
//! * `--self-check` — hermetic. No corpus, no artifact write. Rejects each named
//!   mutation (sha drift, unknown document, guard-less zero, observed Java > 0,
//!   tuned threshold, shrunken denominator, the two gap-ledger mutations, a
//!   zero-denominator metric and a threshold both measured and gapped).
//! * `--limit N` — bounded local run, non-canonical limited artifact only.
//! * default — full-corpus canonical run, written atomically (tmp + rename)
//!   with an incremental per-document resume cache so a resumed run converges.
//!
//! Diagnostic only: `claim_scope = diagnostic_only`, `product_coverage_effect
//! = none`, `thresholds = not_accepted`, all five locks closed, no graph/import/
//! promotion, no production route (D127 / ADR-040 / ADR-076 §14).

use da_adapters::PdfInspectorLayoutDonor;
use da_adapters::layout_donors::LayoutDonorLimits;
use da_cli::native_acceptance::{
    ABSORPTION_SCHEMA_REL, AcceptanceDocumentRow, AcceptanceError, AcceptanceExpectation,
    AcceptanceGapCell, AcceptanceGuard, AcceptanceMetricRow, AcceptancePopulations,
    DEFAULT_ACCEPTANCE_ARTIFACT_REL, HOLDOUT_MANIFEST_REL, NativeAcceptanceReceipt,
    REFERENCE_PINS_REL, REFERENCE_PROBE_REL, assert_acceptance_redacted, build_native_acceptance,
    derive_expectation, validate_native_acceptance,
};
use da_cli::native_composition_measurement::measure_document_admitted_coverage;
use da_cli::native_family_receipt::sha256_prefixed;
use da_parse::{
    GrobidHttpBackend, GrobidProfile, JavaInvocationCounter, ParseBackend, ParseBackendRequest,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Provisional loopback endpoint; the real route never consults it.
const GUARD_ENDPOINT: &str = "http://127.0.0.1:9";
/// An unroutable seam for the guard's positive control.
const CONTROL_ENDPOINT: &str = "http://";
/// The observed seam token.
const OBSERVER: &str = "java_transport_fetch_tei";
/// The frozen acceptance carrier pinning each measurable document's reference TEI
/// bytes. Read directly, same precedent as the holdout manifest in
/// `load_expectation`: every row is byte-pinned by its own sha256.
const ACCEPTANCE_MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
/// Pointer inside the acceptance manifest to the measurable reference TEI rows.
const REFERENCE_TEI_POINTER: [&str; 3] = [
    "external_holdout_contract",
    "release_probe",
    "reference_tei",
];
/// The frozen preservation metric id, resolved by pointer from the pins.
const PRESERVATION_METRIC_ID: &str = "preservation.occurrence_conservation";

// ─────────────────────────────── arguments ───────────────────────────────

struct Args {
    root: PathBuf,
    report: PathBuf,
    self_check: bool,
    limit: Option<usize>,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_ACCEPTANCE_ARTIFACT_REL);
    let mut self_check = false;
    let mut limit = None;
    let mut explicit_report = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => {
                report = args.next().unwrap_or_default().into();
                explicit_report = true;
            }
            "--self-check" => self_check = true,
            "--limit" => limit = Some(args.next().unwrap_or_default().parse().unwrap_or(0)),
            "--help" => {
                println!(
                    "native_corpus_acceptance [--root DIR] [--report FILE] [--limit N] [--self-check]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    if limit.is_some() && !explicit_report {
        report = PathBuf::from(format!("{DEFAULT_ACCEPTANCE_ARTIFACT_REL}.limited.json"));
    }
    Args {
        root,
        report,
        self_check,
        limit,
    }
}

fn main() -> ExitCode {
    let args = parse_args();
    let _log_guard = da_cli::logging::init_pipeline_logging("native_corpus_acceptance");
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_corpus_acceptance: {error}");
            ExitCode::FAILURE
        }
    }
}

// ───────────────────────── guard provenance ─────────────────────────

/// Install the Java-invocation observer on the *real* transport seam and prove
/// it with a positive control on a separate unreachable seam. A zero reading is
/// only a measurement when the observer was installed, identity-isolated and the
/// control raised the counter.
fn install_guard() -> Result<AcceptanceGuard, Box<dyn Error>> {
    let counter = JavaInvocationCounter::new();
    let plain =
        GrobidHttpBackend::new(Some(GUARD_ENDPOINT)).with_profile(GrobidProfile::structural());
    let watched = plain.clone().with_invocation_observer(counter.clone());
    let identity_isolated = plain.identity() == watched.identity();
    let installed = watched.java_invocation_observation().is_observed();
    let positive_control = guard_positive_control()?;
    let observed_java_calls = counter
        .observation()
        .attempts()
        .ok_or("an installed observer must read as observed")?;
    Ok(AcceptanceGuard {
        observer: OBSERVER.to_owned(),
        installed,
        identity_isolated,
        observed_java_calls,
        positive_control,
    })
}

fn guard_positive_control() -> Result<bool, Box<dyn Error>> {
    let control = JavaInvocationCounter::new();
    let backend = GrobidHttpBackend::new(Some(CONTROL_ENDPOINT))
        .with_profile(GrobidProfile::structural())
        .with_invocation_observer(control.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let request = ParseBackendRequest::new("native-acceptance-guard-control", b"%PDF-1.4".to_vec());
    let _ = runtime.block_on(async { backend.fetch_tei(request).await });
    if control.observation().attempts() != Some(1) {
        return Err("the Java guard positive control did not raise the counter".into());
    }
    Ok(true)
}

// ───────────────────────── carrier loading ─────────────────────────

/// Read a repository-relative carrier and return (text, prefixed sha256).
fn read_carrier(root: &Path, rel: &str) -> Result<(String, String), Box<dyn Error>> {
    let bytes = fs::read(root.join(rel))?;
    let text = String::from_utf8(bytes.clone())?;
    Ok((text, sha256_prefixed(&bytes)))
}

fn load_expectation(root: &Path) -> Result<AcceptanceExpectation, Box<dyn Error>> {
    let (pins_text, pins_sha) = read_carrier(root, REFERENCE_PINS_REL)?;
    let (probe_text, probe_sha) = read_carrier(root, REFERENCE_PROBE_REL)?;
    let (schema_text, schema_sha) = read_carrier(root, ABSORPTION_SCHEMA_REL)?;
    let (manifest_text, _) = read_carrier(root, HOLDOUT_MANIFEST_REL)?;
    Ok(derive_expectation(
        &pins_text,
        &pins_sha,
        &probe_text,
        &probe_sha,
        &schema_text,
        &schema_sha,
        &manifest_text,
    )?)
}

// ───────────────────── frozen reference-TEI pins ─────────────────────

/// One frozen reference TEI row: the committed bytes the preservation metric is
/// measured over.
#[derive(Debug, Clone)]
struct ReferenceTei {
    path: String,
    sha256: String,
    size_bytes: u64,
}

/// Load the measurable reference-TEI pins from the acceptance manifest. Every
/// measurable document is pinned here by its own sha256 and size, so the join
/// into the run can never silently shrink the preservation denominator.
fn load_reference_tei(root: &Path) -> Result<BTreeMap<String, ReferenceTei>, Box<dyn Error>> {
    let text = fs::read_to_string(root.join(ACCEPTANCE_MANIFEST_REL))?;
    let manifest: serde_yaml::Value = serde_yaml::from_str(&text)?;
    let rows = manifest
        .get(REFERENCE_TEI_POINTER[0])
        .and_then(|contract| contract.get(REFERENCE_TEI_POINTER[1]))
        .and_then(|lane| lane.get(REFERENCE_TEI_POINTER[2]))
        .and_then(serde_yaml::Value::as_sequence)
        .ok_or_else(|| {
            format!(
                "{ACCEPTANCE_MANIFEST_REL} is missing {}",
                REFERENCE_TEI_POINTER.join(".")
            )
        })?;
    let mut reference = BTreeMap::new();
    for row in rows {
        let article_id = row
            .get("article_id")
            .and_then(serde_yaml::Value::as_str)
            .ok_or("reference_tei row is missing `article_id`")?
            .to_owned();
        let path = row
            .get("path")
            .and_then(serde_yaml::Value::as_str)
            .ok_or_else(|| format!("reference_tei {article_id} is missing `path`"))?
            .to_owned();
        let sha256 = row
            .get("sha256")
            .and_then(serde_yaml::Value::as_str)
            .ok_or_else(|| format!("reference_tei {article_id} is missing `sha256`"))?
            .to_owned();
        let size_bytes = row
            .get("size_bytes")
            .and_then(serde_yaml::Value::as_u64)
            .ok_or_else(|| format!("reference_tei {article_id} is missing `size_bytes`"))?;
        let previous = reference.insert(
            article_id.clone(),
            ReferenceTei {
                path,
                sha256,
                size_bytes,
            },
        );
        if previous.is_some() {
            return Err(format!("reference_tei names {article_id} more than once").into());
        }
    }
    Ok(reference)
}

/// The typed gap ledger: one cell per frozen threshold that is not measured, and
/// one per pending required family. No threshold literal is restated; only
/// ids/reasons. A threshold whose `metric_id` is measured by the run drops its
/// `not_observable` cell rather than being carried both measured and gapped.
fn build_gap_ledger(
    expectation: &AcceptanceExpectation,
    measured_metric_ids: &BTreeSet<String>,
) -> Vec<AcceptanceGapCell> {
    let mut gaps = Vec::new();
    for threshold in &expectation.thresholds {
        if measured_metric_ids.contains(&threshold.metric_id) {
            continue;
        }
        let reason = match threshold.family_id.as_str() {
            "parse.component.native_fulltext_composition_shadow" => {
                "producing_mechanism_absent:s01_composition_scanner_consumes_composed_emission"
            }
            _ => "class_parity_axis_absent:not_a_native_route_product",
        };
        gaps.push(AcceptanceGapCell {
            threshold_id: threshold.threshold_id.clone(),
            metric_id: threshold.metric_id.clone(),
            family_id: threshold.family_id.clone(),
            kind: "not_observable".to_owned(),
            reason: reason.to_owned(),
        });
    }
    for family in &expectation.families {
        if family.state == "threshold_pending" {
            let reason = family
                .reason
                .clone()
                .unwrap_or_else(|| "threshold_pending_family".to_owned());
            gaps.push(AcceptanceGapCell {
                threshold_id: String::new(),
                metric_id: String::new(),
                family_id: family.family_id.clone(),
                kind: "threshold_pending".to_owned(),
                reason,
            });
        }
    }
    gaps
}

// ───────────────────────── resume cache ─────────────────────────

/// A per-document plane observation, cached so a resumed run converges.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CachedPlane {
    pages: Option<u64>,
    layout_units: Option<u64>,
    layout_digest: Option<String>,
    plane_state: String,
    plane_reason: Option<String>,
}

struct Cache {
    path: PathBuf,
    rows: BTreeMap<String, CachedPlane>,
}

impl Cache {
    fn load(root: &Path) -> Self {
        let path = root.join("target/native_corpus_acceptance_cache.json");
        let rows = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<BTreeMap<String, CachedPlane>>(&bytes).ok())
            .unwrap_or_default();
        Cache { path, rows }
    }

    fn flush(&self) -> Result<(), Box<dyn Error>> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_vec_pretty(&self.rows)?)?;
        Ok(())
    }
}

// ───────────────────────── full run ─────────────────────────

fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    if args.self_check {
        return run_self_check();
    }

    let expectation = load_expectation(&args.root)?;
    let guard = install_guard()?;
    let reference_tei = load_reference_tei(&args.root)?;

    let limit = args
        .limit
        .unwrap_or(expectation.documents.len())
        .min(expectation.documents.len());
    let bounded = args.limit.is_some();

    let mut cache = Cache::load(&args.root);
    let mut documents = Vec::with_capacity(limit);
    let mut verified = 0u64;
    let mut plane_observed = 0u64;
    let mut plane_units_total = 0u64;
    let mut plane_rows = 0u64;
    let limits_donor = LayoutDonorLimits::default();
    let mut preservation_admitted = 0u64;
    let mut preservation_source = 0u64;
    let mut preservation_rows = 0u64;
    let mut unpinned_measurable: Vec<String> = Vec::new();

    for (index, expected) in expectation.documents.iter().take(limit).enumerate() {
        let pdf_bytes = match fs::read(args.root.join(&expected.source_pdf)) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Err(format!(
                    "document {} source PDF could not be read: {error}",
                    expected.id
                )
                .into());
            }
        };
        let observed_sha = sha256_prefixed(&pdf_bytes);
        if observed_sha != expected.sha256 {
            return Err(format!(
                "checksum drift for {}: observed {observed_sha}, pinned {}",
                expected.id, expected.sha256
            )
            .into());
        }
        verified += 1;

        // The preservation aggregate is measured over the frozen reference TEI,
        // not over the composed emission: `admitted` and `source_population`
        // come from the same shared scanner the release probe uses, so this
        // runner and that probe cannot drift.
        if expected.state == "measurable" {
            if let Some(reference) = reference_tei.get(&expected.id) {
                let tei_bytes = fs::read(args.root.join(&reference.path)).map_err(|error| {
                    format!(
                        "reference TEI for {} could not be read ({}): {error}",
                        expected.id, reference.path
                    )
                })?;
                if tei_bytes.len() as u64 != reference.size_bytes {
                    return Err(format!(
                        "reference TEI size drift for {}: observed {}, pinned {}",
                        expected.id,
                        tei_bytes.len(),
                        reference.size_bytes
                    )
                    .into());
                }
                let observed_tei_sha = sha256_prefixed(&tei_bytes);
                if observed_tei_sha != reference.sha256 {
                    return Err(format!(
                        "reference TEI checksum drift for {}: observed {observed_tei_sha}, \
                         pinned {}",
                        expected.id, reference.sha256
                    )
                    .into());
                }
                let tei_xml = std::str::from_utf8(&tei_bytes).map_err(|error| {
                    format!("reference TEI for {} is not UTF-8: {error}", expected.id)
                })?;
                let counts = measure_document_admitted_coverage(tei_xml).map_err(|error| {
                    format!(
                        "reference TEI for {} could not be measured: {error}",
                        expected.id
                    )
                })?;
                preservation_admitted += counts.admitted as u64;
                preservation_source += counts.source_population as u64;
                preservation_rows += 1;
            } else {
                unpinned_measurable.push(expected.id.clone());
            }
        }

        let plane = if expected.state == "excluded" {
            CachedPlane {
                pages: None,
                layout_units: None,
                layout_digest: None,
                plane_state: "not_run".to_owned(),
                plane_reason: None,
            }
        } else if let Some(cached) = cache.rows.get(&expected.id) {
            cached.clone()
        } else {
            match PdfInspectorLayoutDonor.extract_shadow(&pdf_bytes, &expected.id, limits_donor) {
                Ok(observation) => {
                    let mut ordered: Vec<&da_parse::LexemeRow> =
                        observation.lexemes.iter().collect();
                    ordered.sort_by_key(|lexeme| (lexeme.page_ordinal, lexeme.source_ordinal));
                    let mut material = String::new();
                    for lexeme in &ordered {
                        material.push_str(lexeme.text());
                        material.push('\n');
                    }
                    let row = CachedPlane {
                        pages: Some(observation.document.pages.len() as u64),
                        layout_units: Some(ordered.len() as u64),
                        layout_digest: Some(sha256_prefixed(material.as_bytes())),
                        plane_state: "observed".to_owned(),
                        plane_reason: None,
                    };
                    cache.rows.insert(expected.id.clone(), row.clone());
                    row
                }
                Err(_) => {
                    let row = CachedPlane {
                        pages: None,
                        layout_units: None,
                        layout_digest: None,
                        plane_state: "refused".to_owned(),
                        plane_reason: Some("native_plane_refused".to_owned()),
                    };
                    cache.rows.insert(expected.id.clone(), row.clone());
                    row
                }
            }
        };

        if plane.plane_state == "observed" {
            plane_observed += 1;
            plane_rows += 1;
            plane_units_total += plane.layout_units.unwrap_or(0);
        } else if plane.plane_state == "refused" {
            plane_rows += 1;
        }

        documents.push(AcceptanceDocumentRow {
            id: expected.id.clone(),
            source_pdf: expected.source_pdf.clone(),
            sha256: expected.sha256.clone(),
            state: expected.state.clone(),
            exclusion_reason: expected.exclusion_reason.clone(),
            pages: plane.pages,
            layout_units: plane.layout_units,
            layout_digest: plane.layout_digest,
            plane_state: plane.plane_state,
            plane_reason: plane.plane_reason,
        });

        if !bounded && index % 25 == 24 {
            cache.flush()?;
        }
    }
    if !bounded {
        cache.flush()?;
    }

    let populations = if bounded {
        AcceptancePopulations {
            assigned: documents.len() as u64,
            measurable: documents
                .iter()
                .filter(|row| row.state == "measurable")
                .count() as u64,
            refused: 0,
            excluded: documents
                .iter()
                .filter(|row| row.state == "excluded")
                .count() as u64,
        }
    } else {
        expectation.populations.clone()
    };

    // Every measurable document must be pinned in the reference-TEI carrier and
    // measured exactly once, so a broken join fails loudly here instead of
    // quietly shrinking the preservation denominator.
    if preservation_rows != populations.measurable {
        return Err(format!(
            "the reference-TEI join measured {preservation_rows} rows but the run has {} \
             measurable documents (unpinned: {unpinned_measurable:?})",
            populations.measurable
        )
        .into());
    }

    let mut bounded_expectation = expectation.clone();
    bounded_expectation.populations = populations.clone();
    bounded_expectation.documents = expectation.documents.iter().take(limit).cloned().collect();

    let mut metrics = vec![
        AcceptanceMetricRow {
            metric_id: "native_acceptance.pdf_verification_coverage".to_owned(),
            numerator: verified,
            denominator: populations.assigned,
            stratum: None,
        },
        AcceptanceMetricRow {
            metric_id: "native_acceptance.native_plane_coverage".to_owned(),
            numerator: plane_observed,
            denominator: populations.measurable,
            stratum: None,
        },
        AcceptanceMetricRow {
            metric_id: "native_acceptance.native_plane_units".to_owned(),
            numerator: plane_units_total,
            denominator: plane_rows.max(1),
            stratum: None,
        },
    ];

    // The preservation metric is pushed only when it has a non-empty
    // denominator: a zero-denominator row is a silent zero, refused by name.
    if preservation_source > 0 {
        metrics.push(AcceptanceMetricRow {
            metric_id: PRESERVATION_METRIC_ID.to_owned(),
            numerator: preservation_admitted,
            denominator: preservation_source,
            stratum: None,
        });
    }

    // A threshold is gapped only while it is not measured. The preservation
    // threshold drops its `producing_mechanism_absent` cell here; the two
    // extraction cells and the `threshold_pending` family cells are unchanged.
    let measured_metric_ids: BTreeSet<String> = metrics
        .iter()
        .map(|metric| metric.metric_id.clone())
        .collect();
    let gaps = build_gap_ledger(&expectation, &measured_metric_ids);

    let receipt = build_native_acceptance(&bounded_expectation, guard, documents, metrics, gaps)?;
    let bytes = receipt.artifact_json_bytes()?;
    assert_acceptance_redacted(std::str::from_utf8(&bytes)?)?;
    write_artifact_atomically(&args.report, &bytes)?;

    Ok(format!(
        "native acceptance `{}`: {} documents ({} verified, {} measurable, {} excluded), \
         {} plane rows ({} observed, {} units), {} metrics, {} typed gaps ({} not_observable, \
         {} threshold_pending); guard installed={} isolated={} java_calls={} control={}; {}",
        receipt.schema_version,
        receipt.documents.len(),
        verified,
        receipt.populations.measurable,
        receipt.populations.excluded,
        plane_rows,
        plane_observed,
        plane_units_total,
        receipt.metrics.len(),
        receipt.gaps.len(),
        receipt
            .gaps
            .iter()
            .filter(|gap| gap.kind == "not_observable")
            .count(),
        receipt
            .gaps
            .iter()
            .filter(|gap| gap.kind == "threshold_pending")
            .count(),
        receipt.guard.installed,
        receipt.guard.identity_isolated,
        receipt.guard.observed_java_calls,
        receipt.guard.positive_control,
        if bounded {
            "bounded run (non-canonical limited artifact)"
        } else {
            "canonical run"
        },
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

/// Deterministic negative controls, all hermetic (no PDF, no corpus, no write).
/// Each mutation must be refused **by name**; any mutation that slips through
/// fails the self-check.
fn run_self_check() -> Result<String, Box<dyn Error>> {
    let (expectation, base) = synthetic_fixtures();
    let mut checked = 0usize;

    // The base receipt must pass, or the mutations prove nothing.
    validate_native_acceptance(&base, &expectation)?;

    // (1) sha256 drift
    let mut m = base.clone();
    m.documents[0].sha256 = "sha256:0000".to_owned();
    expect_refusal("sha256_drift", &m, &expectation, "DocumentMismatch")?;
    checked += 1;

    // (2) unknown document
    let mut m = base.clone();
    m.documents[0].id = "unknown-9999.00000".to_owned();
    expect_refusal("unknown_document", &m, &expectation, "DocumentMismatch")?;
    checked += 1;

    // (3) guard-less zero
    let mut m = base.clone();
    m.guard.installed = false;
    expect_refusal("guardless_zero", &m, &expectation, "GuardlessZero")?;
    checked += 1;

    // (4) observed Java calls > 0
    let mut m = base.clone();
    m.guard.observed_java_calls = 1;
    expect_refusal("java_consulted", &m, &expectation, "JavaConsulted")?;
    checked += 1;

    // (5) tuned / non-frozen threshold
    let mut m = base.clone();
    m.metrics.push(AcceptanceMetricRow {
        metric_id: "tuned_threshold_literal".to_owned(),
        numerator: 1,
        denominator: 2,
        stratum: None,
    });
    expect_refusal("tuned_threshold", &m, &expectation, "UnknownThreshold")?;
    checked += 1;

    // (6) shrunken denominator
    let mut m = base.clone();
    m.populations = AcceptancePopulations {
        assigned: 1,
        measurable: 1,
        refused: 0,
        excluded: 0,
    };
    expect_refusal("shrunken_denominator", &m, &expectation, "Denominator")?;
    checked += 1;

    // (7) gap kind outside the vocabulary
    let mut m = base.clone();
    m.gaps[0].kind = "mystery".to_owned();
    expect_refusal("unknown_gap_kind", &m, &expectation, "UnknownGapKind")?;
    checked += 1;

    // (8) gap cell with an empty reason
    let mut m = base.clone();
    m.gaps[0].reason = String::new();
    expect_refusal("empty_gap_reason", &m, &expectation, "EmptyGapReason")?;
    checked += 1;

    // (9) a measured metric row with a zero (silent) denominator
    let mut m = base.clone();
    m.metrics[0].denominator = 0;
    expect_refusal(
        "zero_denominator",
        &m,
        &expectation,
        "MetricZeroDenominator",
    )?;
    checked += 1;

    // (10) a frozen threshold measured while its gap cell is still carried
    let mut m = base.clone();
    m.metrics.push(AcceptanceMetricRow {
        metric_id: "metric.frozen".to_owned(),
        numerator: 1,
        denominator: 2,
        stratum: None,
    });
    expect_refusal(
        "both_measured_and_gapped",
        &m,
        &expectation,
        "ThresholdBothMeasuredAndGapped",
    )?;
    checked += 1;

    Ok(format!(
        "native acceptance self-check: {checked} named refusals executed, all refused"
    ))
}

fn expect_refusal(
    label: &str,
    receipt: &NativeAcceptanceReceipt,
    expectation: &AcceptanceExpectation,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    match validate_native_acceptance(receipt, expectation) {
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

/// Hermetic fixtures: a two-document expectation and a matching base receipt.
/// No corpus is read and nothing is written.
fn synthetic_fixtures() -> (AcceptanceExpectation, NativeAcceptanceReceipt) {
    use da_cli::native_acceptance::{
        AcceptanceCarrierBinding, AcceptanceDocumentExpectation, AcceptanceFamilyExpectation,
        AcceptanceThresholdExpectation,
    };
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
    .expect("the hermetic base receipt must validate");
    (expectation, receipt)
}

/// Keep the import used even in the self-check-only path.
#[allow(dead_code)]
fn _assert_error_type(_: &AcceptanceError) {}
