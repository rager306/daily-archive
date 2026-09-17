//! Positive donor-visual fixture runner (GAR-003, M508/S02).
//!
//! GAR-003 asked for real positive fixtures proving that donor visual
//! primitives survive the donor->native conversion. The real corpus cannot
//! prove that: `pdfalto` is unavailable there (typed `Unsupported`), the
//! pdf-inspector lanes report `NotObserved`, and all nine lane receipts stay
//! terminal — which is *correct* behaviour, not adapter evidence. So the
//! positive claim is proven by committed fixtures instead of by corpus
//! receipts, and this runner is the executable form of that claim.
//!
//! The runner drives the real committed path
//! `da_adapters::parse_pdfalto_xml_shadow`
//! -> `da_cli::visual_adapter::native_visual_payload`
//! -> `detect_figure_heads` / `detect_table_heads` / `detect_formula_heads`,
//! asserts the preservation invariants per fixture, and refuses to write
//! anything when one of them is violated (non-zero exit, no partial
//! artifact). On success it emits a byte-reproducible artifact: no
//! timestamps, no absolute paths, no map iteration order.
//!
//! Evidence is shared with the Governor rule (`visual_adapter_preserves_donor_content`)
//! through the canonical record shape in
//! `artifacts/parse_compat/visual_positive_fixture_run.json`.
//!
//! Shadow-only: no route selection, cache/graph writes or promotion.

use da_adapters::{
    LayoutDonorError, LayoutDonorLimits, VisualPrimitive, VisualPrimitiveState,
    parse_pdfalto_xml_shadow,
};
use da_cli::visual_adapter::{
    NON_DELIVERABLE_VISUAL_FAMILIES, donor_error_absence, native_kind, native_visual_payload,
};
use da_parse::{
    NativeVisualAbsence, NativeVisualConfig, NativeVisualHeadOutcome, NativeVisualObservation,
    detect_figure_heads, detect_formula_heads, detect_table_heads,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MANIFEST_SCHEMA_VERSION: &str = "visual-positive-fixture-manifest.v1";
const ARTIFACT_SCHEMA_VERSION: &str = "visual-positive-fixture-run.v1";
const DEFAULT_MANIFEST: &str =
    "crates/da-cli/tests/fixtures/visual_positive_fixture_run/manifest.yaml";
const DEFAULT_REPORT: &str = "artifacts/parse_compat/visual_positive_fixture_run.json";

/// DATA contract of the fixture manifest (`manifest.yaml`).
#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: String,
    converter_source: String,
    fixture_families: Vec<String>,
    families_requiring_observed_head: Vec<String>,
    non_deliverable_families: Vec<String>,
    fixtures: Vec<FixtureSpec>,
}

#[derive(Debug, Deserialize)]
struct FixtureSpec {
    id: String,
    family: String,
    path: String,
    head: String,
    expected_head_outcome: String,
    min_primitive_count: usize,
}

/// Canonical evidence record shape. The Governor rule consumes these field
/// names verbatim, so rename them only with a matching contract change.
#[derive(Debug, Serialize)]
struct Artifact {
    schema_version: String,
    converter_source_sha256: String,
    families: Vec<FamilyOutcome>,
    records: Vec<FixtureRecord>,
    malformed_probe: Option<MalformedProbe>,
}

#[derive(Debug, Serialize)]
struct FamilyOutcome {
    family: String,
    head: String,
    head_outcome: String,
}

#[derive(Debug, Serialize)]
struct FixtureRecord {
    fixture_id: String,
    family: String,
    fixture_source_sha256: String,
    donor: DonorReceipt,
    native: NativeReceipt,
}

#[derive(Debug, Serialize)]
struct DonorReceipt {
    primitive_count: usize,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct NativeReceipt {
    primitive_count: usize,
    lane_absence: Option<String>,
    head_outcome: String,
    head_reason: Option<String>,
    primitives: Vec<PrimitiveReceipt>,
    declared_non_deliverable_families: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PrimitiveReceipt {
    source_ordinal: u32,
    page_ordinal: u32,
    kind: String,
    bbox: [f64; 4],
    content_digest: String,
    provider: ProviderReceipt,
    document_id: String,
    source_sha256: String,
}

#[derive(Debug, Serialize)]
struct ProviderReceipt {
    id: String,
    version: String,
}

/// Negative control kept beside the positive evidence: a mutated fixture must
/// stay a typed donor error and must never be collapsed into `not_observed`.
#[derive(Debug, Serialize)]
struct MalformedProbe {
    base_fixture_id: String,
    probe_source_sha256: String,
    error: String,
    mapped_lane_absence: String,
    is_not_observed: bool,
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Serde is the single source of truth for the closed enum spellings that the
/// Governor oracle compares (`not_observed`, `malformed_bbox`, `image`, ...).
fn snake<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(text)) => text,
        _ => "unknown".to_owned(),
    }
}

/// Typed donor failure code recorded in `donor.error`.
fn donor_error_code(error: &LayoutDonorError) -> String {
    match error {
        LayoutDonorError::Malformed { .. } | LayoutDonorError::InvalidLayout(_) => "Malformed",
        LayoutDonorError::Oversize { .. } => "Oversize",
        LayoutDonorError::Unavailable { .. } => "Unavailable",
        LayoutDonorError::Timeout { .. } => "Timeout",
        LayoutDonorError::NonZero { .. } => "NonZero",
        LayoutDonorError::Encrypted => "Encrypted",
        LayoutDonorError::Truncated => "Truncated",
        LayoutDonorError::Io(_) => "Io",
    }
    .to_owned()
}

fn donor_digest(primitive: &VisualPrimitive) -> Option<String> {
    match &primitive.state {
        VisualPrimitiveState::Observed { content_digest } => Some(content_digest.clone()),
        VisualPrimitiveState::Absent { .. } => None,
    }
}

fn head_outcome_for(
    head: &str,
    native: &NativeVisualObservation,
    config: NativeVisualConfig,
) -> Result<NativeVisualHeadOutcome, String> {
    match head {
        "figure" => detect_figure_heads(native, config),
        "table" => detect_table_heads(native, config),
        "formula" => detect_formula_heads(native, config),
        other => return Err(format!("unsupported detector head {other}")),
    }
    .map_err(|error| format!("{error:?}"))
}

/// `("observed"|"terminal", reason)`. Observed outcomes keep `reason: null`.
fn outcome_label(outcome: &NativeVisualHeadOutcome) -> (String, Option<String>) {
    match outcome {
        NativeVisualHeadOutcome::Terminal { reason, .. } => {
            ("terminal".to_owned(), Some(snake(reason)))
        }
        NativeVisualHeadOutcome::Figure { .. }
        | NativeVisualHeadOutcome::Table { .. }
        | NativeVisualHeadOutcome::Formula { .. } => ("observed".to_owned(), None),
    }
}

fn resolve(path: &str, manifest_path: &Path) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_file() {
        return candidate;
    }
    let mut base = manifest_path.parent();
    for _ in 0..4 {
        if let Some(parent) = base {
            let resolved = parent.join(&candidate);
            if resolved.is_file() {
                return resolved;
            }
            base = parent.parent();
        }
    }
    candidate
}

fn check_manifest_contract(manifest: &Manifest) -> Result<(), String> {
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "manifest schema_version {} != {MANIFEST_SCHEMA_VERSION}",
            manifest.schema_version
        ));
    }
    let declared = manifest
        .non_deliverable_families
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if declared != NON_DELIVERABLE_VISUAL_FAMILIES {
        return Err(
            "manifest non_deliverable_families diverges from the adapter declaration".into(),
        );
    }
    let mut families = manifest.fixture_families.clone();
    families.sort();
    families.dedup();
    if families.len() != manifest.fixture_families.len() {
        return Err("manifest fixture_families contains duplicates".into());
    }
    let mut fixture_families = manifest
        .fixtures
        .iter()
        .map(|spec| spec.family.clone())
        .collect::<Vec<_>>();
    fixture_families.sort();
    if fixture_families != families {
        return Err("manifest fixtures do not cover fixture_families exactly once".into());
    }
    for required in &manifest.families_requiring_observed_head {
        if !families.contains(required) {
            return Err(format!(
                "families_requiring_observed_head lists unknown family {required}"
            ));
        }
    }
    let mut ids = manifest
        .fixtures
        .iter()
        .map(|spec| spec.id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    if ids.len() != manifest.fixtures.len() {
        return Err("manifest fixture ids are not unique".into());
    }
    for spec in &manifest.fixtures {
        if !matches!(spec.head.as_str(), "figure" | "table" | "formula") {
            return Err(format!("fixture {} declares unsupported head", spec.id));
        }
        if !matches!(spec.expected_head_outcome.as_str(), "observed" | "terminal") {
            return Err(format!(
                "fixture {} declares unsupported head outcome",
                spec.id
            ));
        }
        if spec.min_primitive_count == 0 {
            return Err(format!("fixture {} declares an empty positive", spec.id));
        }
    }
    Ok(())
}

/// Mutate the first `x="` value of a fixture into a non-numeric value. The
/// probe is derived from a committed fixture so it cannot silently become a
/// no-op while still reporting success.
fn malformed_probe_source(source: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(source).ok()?;
    let marker = "x=\"";
    let index = text.find(marker)? + marker.len();
    let mut mutated = String::with_capacity(text.len() + 3);
    mutated.push_str(&text[..index]);
    mutated.push_str("nan");
    mutated.push_str(&text[index..]);
    Some(mutated.into_bytes())
}

fn parse_args() -> (PathBuf, PathBuf) {
    let mut manifest = PathBuf::from(DEFAULT_MANIFEST);
    let mut report = PathBuf::from(DEFAULT_REPORT);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" => manifest = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--help" => {
                println!(
                    "visual_positive_fixture_run [--manifest FILE] [--report FILE]\n\
                     Drives the committed positive visual fixtures through the donor->native\n\
                     adapter and writes {DEFAULT_REPORT}."
                );
                std::process::exit(0)
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2)
            }
        }
    }
    (manifest, report)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = da_cli::logging::init_pipeline_logging("visual_positive_fixture_run");
    let (manifest_path, report_path) = parse_args();
    let manifest_bytes = fs::read(&manifest_path)?;
    let manifest: Manifest = serde_yaml::from_slice(&manifest_bytes)?;
    check_manifest_contract(&manifest)?;
    let converter_sha = sha256_hex(&fs::read(&manifest.converter_source)?);

    let limits = LayoutDonorLimits::default();
    let config = NativeVisualConfig::default();
    let mut violations = Vec::new();
    let mut families = Vec::new();
    let mut records = Vec::new();
    let mut malformed_base: Option<(String, Vec<u8>)> = None;

    for spec in &manifest.fixtures {
        let path = resolve(&spec.path, &manifest_path);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                violations.push(format!("fixture_missing {}: {error}", spec.path));
                continue;
            }
        };
        let fixture_sha = sha256_hex(&bytes);
        if malformed_base.is_none() {
            malformed_base = Some((spec.id.clone(), bytes.clone()));
        }
        // No real PDF exists behind a synthetic fixture, so the synthetic
        // source identity is the fixture digest itself: deterministic,
        // traceable, and never an invented digest.
        let document_id = format!("visual-positive-fixture/{}", spec.id);
        let shadow = match parse_pdfalto_xml_shadow(&bytes, &bytes, &document_id, limits) {
            Ok(shadow) => shadow,
            Err(error) => {
                violations.push(format!(
                    "{} donor_parse_failed {}",
                    spec.id,
                    donor_error_code(&error)
                ));
                continue;
            }
        };
        let native = match native_visual_payload(&shadow) {
            Ok(native) => native,
            Err(absence) => {
                violations.push(format!("{} adapter_rejected {}", spec.id, snake(&absence)));
                continue;
            }
        };
        let donor_count = shadow.visual.primitives.len();
        if donor_count < spec.min_primitive_count {
            violations.push(format!(
                "{} donor_primitives {donor_count} < declared minimum {}",
                spec.id, spec.min_primitive_count
            ));
        }
        if donor_count == 0 {
            violations.push(format!("{} donor observation is empty", spec.id));
        }
        if native.primitives.len() != donor_count {
            violations.push(format!(
                "{} donor_primitives_discarded donor={donor_count} native={}",
                spec.id,
                native.primitives.len()
            ));
        }
        if native.lane_absence.is_some() {
            violations.push(format!("{} lane_absence_on_positive_fixture", spec.id));
        }
        let declared = native.figure_captions.len()
            + native.figure_callouts.len()
            + native.table_captions.len()
            + native.table_callouts.len()
            + native.table_cells.len()
            + native.formula_labels.len()
            + native.formula_callouts.len();
        if declared != 0 {
            violations.push(format!(
                "{} non_deliverable_receipt_supplied {declared}",
                spec.id
            ));
        }
        let mut primitives = Vec::new();
        for (index, donor_primitive) in shadow.visual.primitives.iter().enumerate() {
            let Some(native_primitive) = native.primitives.get(index) else {
                break;
            };
            let donor_digest = donor_digest(donor_primitive);
            let bbox = match native_primitive.bbox {
                Some(bbox) => [bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max],
                None => {
                    violations.push(format!("{} primitive {index} lost its bbox", spec.id));
                    [0.0, 0.0, 0.0, 0.0]
                }
            };
            let content_digest = native_primitive.content_digest.clone().unwrap_or_default();
            if native_primitive.document_id != shadow.document.document_id
                || native_primitive.source_sha256
                    != shadow
                        .document
                        .source_pdf_sha256
                        .trim_start_matches("sha256:")
                || native_primitive.provider.id != shadow.visual.provider.id
                || native_primitive.provider.version != shadow.visual.provider.version
                || native_primitive.page_ordinal != donor_primitive.page_ordinal
                || native_primitive.source_ordinal != donor_primitive.source_ordinal
                || native_primitive.kind != native_kind(donor_primitive.kind)
                || native_primitive.bbox != donor_primitive.bbox
                || native_primitive.content_digest != donor_digest
                || native_primitive.absence.is_some()
            {
                violations.push(format!(
                    "{} primitive {index} lost donor provenance",
                    spec.id
                ));
            }
            primitives.push(PrimitiveReceipt {
                source_ordinal: native_primitive.source_ordinal,
                page_ordinal: native_primitive.page_ordinal,
                kind: snake(&native_primitive.kind),
                bbox,
                content_digest,
                provider: ProviderReceipt {
                    id: native_primitive.provider.id.clone(),
                    version: native_primitive.provider.version.clone(),
                },
                document_id: native_primitive.document_id.clone(),
                source_sha256: native_primitive.source_sha256.clone(),
            });
        }
        let (head_outcome, head_reason) = match head_outcome_for(&spec.head, &native, config) {
            Ok(outcome) => outcome_label(&outcome),
            Err(detail) => {
                violations.push(format!("{} detector_failed {detail}", spec.id));
                ("unknown".to_owned(), None)
            }
        };
        if head_outcome != spec.expected_head_outcome {
            violations.push(format!(
                "{} expected head outcome {} but observed {}",
                spec.id, spec.expected_head_outcome, head_outcome
            ));
        }
        if manifest
            .families_requiring_observed_head
            .contains(&spec.family)
            && head_outcome != "observed"
        {
            violations.push(format!(
                "{} family {} requires an observed head",
                spec.id, spec.family
            ));
        }
        families.push(FamilyOutcome {
            family: spec.family.clone(),
            head: spec.head.clone(),
            head_outcome: head_outcome.clone(),
        });
        println!(
            "fixture {} family={} donor_primitives={} native_primitives={} head={} head_outcome={} head_reason={} lane_absence={} declared_non_deliverable_families={}",
            spec.id,
            spec.family,
            donor_count,
            native.primitives.len(),
            spec.head,
            head_outcome,
            head_reason.as_deref().unwrap_or("none"),
            native
                .lane_absence
                .map(|reason| snake(&reason))
                .unwrap_or_else(|| "none".to_owned()),
            NON_DELIVERABLE_VISUAL_FAMILIES.len(),
        );
        records.push(FixtureRecord {
            fixture_id: spec.id.clone(),
            family: spec.family.clone(),
            fixture_source_sha256: fixture_sha,
            donor: DonorReceipt {
                primitive_count: donor_count,
                error: None,
            },
            native: NativeReceipt {
                primitive_count: native.primitives.len(),
                lane_absence: native.lane_absence.map(|reason| snake(&reason)),
                head_outcome,
                head_reason,
                primitives,
                declared_non_deliverable_families: NON_DELIVERABLE_VISUAL_FAMILIES
                    .iter()
                    .map(|family| (*family).to_owned())
                    .collect(),
            },
        });
    }

    let mut malformed_probe = None;
    match malformed_base
        .as_ref()
        .and_then(|(id, bytes)| malformed_probe_source(bytes).map(|probe| (id.clone(), probe)))
    {
        None => violations.push("malformed_probe_not_constructible".into()),
        Some((base_fixture_id, probe_bytes)) => {
            match parse_pdfalto_xml_shadow(
                &probe_bytes,
                &probe_bytes,
                "visual-positive-fixture/malformed-probe",
                limits,
            ) {
                Ok(_) => violations.push("malformed_probe_parsed_as_success".into()),
                Err(error) => {
                    let code = donor_error_code(&error);
                    if !matches!(code.as_str(), "Malformed" | "InvalidLayout") {
                        violations.push(format!("malformed_probe_untyped_error {code}"));
                    }
                    let mapped = donor_error_absence(&error);
                    let is_not_observed = mapped == NativeVisualAbsence::NotObserved;
                    if is_not_observed {
                        violations.push("malformed_mapped_to_not_observed".into());
                    }
                    malformed_probe = Some(MalformedProbe {
                        base_fixture_id,
                        probe_source_sha256: sha256_hex(&probe_bytes),
                        error: code,
                        mapped_lane_absence: snake(&mapped),
                        is_not_observed,
                    });
                }
            }
        }
    }

    if !violations.is_empty() {
        for violation in &violations {
            eprintln!("visual_positive_fixture_run violation: {violation}");
        }
        return Err(format!(
            "{} fixture invariant violation(s); no artifact written",
            violations.len()
        )
        .into());
    }

    let artifact = Artifact {
        schema_version: ARTIFACT_SCHEMA_VERSION.into(),
        converter_source_sha256: converter_sha,
        families,
        records,
        malformed_probe,
    };
    let mut encoded = serde_json::to_vec_pretty(&artifact)?;
    encoded.push(b'\n');
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report_path, encoded)?;
    println!(
        "malformed_probe base={} error={} mapped_lane_absence={} is_not_observed=false",
        artifact
            .malformed_probe
            .as_ref()
            .map(|probe| probe.base_fixture_id.as_str())
            .unwrap_or("none"),
        artifact
            .malformed_probe
            .as_ref()
            .map(|probe| probe.error.as_str())
            .unwrap_or("none"),
        artifact
            .malformed_probe
            .as_ref()
            .map(|probe| probe.mapped_lane_absence.as_str())
            .unwrap_or("none"),
    );
    println!(
        "artifact {} records={} converter_source_sha256={}",
        report_path.display(),
        artifact.records.len(),
        artifact.converter_source_sha256,
    );
    Ok(())
}
