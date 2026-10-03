//! M511-j7wz2f / S08 / T04 — the runtime artifact of the frozen external
//! holdout probe, in the GAR-006 shape, and its byte determinism (H20/GAR-015).
//!
//! S08/T02 froze a fresh live-Java `processFulltextDocument` reference for the
//! sha256-pinned non-gold arXiv documents the source manifest declares; S08/T03
//! measured it with an
//! independent, structurally validating oracle in a dev-only `da-parse` test
//! target.  This example is the *runtime* half: it re-measures the same frozen
//! reference, prints the per-document profile, and freezes the result as
//! `artifacts/parse_compat/external_holdout_release_probe.json` so the governor
//! and a human release reviewer can read the state of the probe without running
//! a build.
//!
//! ## The compact scan is not the oracle (declared, not implied)
//!
//! `da-cli` has no `roxmltree` dependency and adding one is out of scope, so this
//! file carries the same minimal, non-validating tag/substring scanner the S05
//! shadow runners use: it builds an element list with attributes and content
//! ranges and reconstructs an element's normalized, entity-decoded text.  It does
//! not validate well-formedness.  The AUTHORITATIVE structural oracle for this
//! probe stays dev-only in `crates/da-parse/tests/external_holdout_probe_semantics.rs`
//! (S08/T03, `cargo test -p da-parse --test external_holdout_probe_semantics`).
//! The runtime artifact therefore claims a *comparable re-measurement*, not a
//! second independent oracle: its value is that identity, population split,
//! per-document outcomes, latency/cost accounting, locks and redaction are
//! frozen in one readable file, and that the file is byte-reproducible.
//!
//! ## Measured scope (read before quoting any number)
//!
//! A non-gold local holdout census; its population is read at run time from the
//! frozen source manifest (`data/parse/holdout_manifests.yaml#articles`), never a
//! compiled-in size (610 rows in the recorded census).
//! This is NOT a statistical corpus: every number is a COUNT over the measured
//! documents, never a corpus rate, and the probe makes no pooled claim
//! (`claim_scope = local_proxy_only`).  The probe
//! does not claim independence from the production route: the reference was
//! produced by the declared live Java release and the admitted band is a local
//! proxy.
//!
//! ## What is frozen
//!
//! * `reference_population` — identity-only: the recomputed probe identity
//!   digest, the source-manifest sha256, the TEI byte pins, the declared
//!   release and profile.
//! * `executed_population` — `corpus_execution = true`, the named execution
//!   evidence, the live route the reference came from, and the document counts.
//! * `evaluated_population` — `status = evaluated` with the frozen counts-only
//!   aggregate and the margin verdicts.
//! * `outcomes` — per document: identity, admitted band, agreement ratio,
//!   bounded uncertainty interval, recorded latency (read from provenance, never
//!   from a clock), the per-metric margin verdicts and every typed admission gap.
//! * `production_catalog_overlap` — a counted census: where each holdout
//!   identity is (and is not) present, so the non-gold claim is measured.
//! * `cost_accounting` / `latency_accounting` — bounded, never measured; an
//!   unknown value is never read as zero and neither may be pooled.
//! * `policy_locks` / `redaction` — D127 and ADR-040 stay closed, no payload.
//!
//! ## Byte determinism
//!
//! No wall-clock value is read at run time: latency comes from the S08/T02
//! provenance record (`recorded_wall_clock_per_document`), and every fraction is
//! frozen as an integer in micros so no float formatting can vary the bytes.  A
//! canonical run requires `--report <path>`; `--self-check` proves each declared
//! guard rejects its own violating input and touches no repo file, no network and
//! no log surface.
//!
//! Shadow/offline only.  No production route, no graph mutation, no import, no
//! promotion, no external byte: `import_eligible=false`, D127 stays closed.
#![cfg(feature = "pdf-layout")]

use clap::Parser;
use da_cli::native_class_parity::{
    ClassProfile, compose_native_document, harmonic_mean, measure_document_class_profiles, ratio,
};
use da_cli::native_composition_measurement::{
    AdmittedBand, CLASS_HEADER_TITLE, CLASS_ORDER, build_admitted_band, measure_source,
};
use serde::Deserialize;
use serde_json::{Value as Json, json};
use serde_yaml::Value as Yaml;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

/// Every failure message carries this prefix so an inspection never guesses.
const PROBE: &str = "[external-holdout-release-probe]";

const REPORT_SCHEMA: &str = "external-holdout-release-probe.v1";
/// The recorded-tables schema the `--emit-frozen-tables` generator writes.
const FROZEN_TABLES_SCHEMA: &str = "external-holdout-frozen-tables.v1";
const MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
const PROVENANCE_REL: &str = "crates/da-parse/tests/fixtures/holdout/provenance.yaml";
const FIXTURE_DIR: &str = "crates/da-parse/tests/fixtures/holdout";
const CANONICAL_CATALOG_ROOT: &str = "data/article_catalog/article_catalog/arxiv";
const SOURCES_ARCHIVE_ROOT: &str = "data/sources/arxiv";
const GOLD_STANDARD_DIR: &str = "data/gold_standard";

/// The latency field is read from the S08/T02 provenance record; a run-time clock
/// reading would destroy byte determinism and is never a source.
const LATENCY_SOURCE_REF: &str =
    "crates/da-parse/tests/fixtures/holdout/provenance.yaml#documents[].latency_ms";
/// The named execution evidence kind.  A member of the governor's declared
/// `evidence_kinds` vocabulary (`tracked_offline_fixture`); the executed
/// population names it inside a structured receipt, because a bare token cannot
/// bind a corpus-scale number (GAR-006).
const EXECUTION_EVIDENCE_KIND: &str = "tracked_offline_fixture";
/// The closed `evidence_kinds` vocabulary declared by the governor DATA
/// (`scripts/governor/checks/executed_population_binding.yaml`).  Mirrored here
/// so the producer cannot name a kind the control plane does not declare.
const EXECUTION_EVIDENCE_KINDS: [&str; 7] = [
    "offline_loopback_stub_existing_typed_transport",
    "tracked_offline_fixture",
    "tracked_offline_join_fixture",
    "tracked_reviewed_oracle_receipts",
    "tracked_source_pdfs_with_sha256",
    "tracked_source_pdfs_with_sha256_and_donor_lanes",
    "tracked_synthetic_clone_fixture",
];
/// A document count at or above this is corpus-scale: a bare token cannot bind
/// it, only a structured receipt.  Mirrors `corpus_scale_documents` in the
/// governor DATA.
const CORPUS_SCALE_DOCUMENTS: u64 = 50;
/// The executed-population identity the reference, executed and receipt all
/// carry, so a receipt can never be spliced in from another report.
const POPULATION_IDENTITY: &str = "canonical_dataset_id_plus_identity";
/// The two release labels this artifact may spell.  Any other `@`-bearing string
/// is a leak, not an identity: the measured release and the declared rollback
/// target.  The DATA-plane authority label (`java-grobid-http@0.9.0-crf`) IS the
/// rollback label, and the divergence is the named open item H21.
const RELEASE_LABEL: &str = "java-grobid-http@0.9.1";
const ROLLBACK_TARGET: &str = "java-grobid-http@0.9.0-crf";

/// The census identifier.  Count-neutral on purpose: the population is DATA (50
/// rows in the recorded measurement), so a name carrying a fixed count would go
/// stale the moment the lane grew.  The producer and the guard share this one
/// constant, so they cannot drift apart.
const CENSUS_KIND: &str = "filesystem_presence_of_the_holdout_identities";

// The probe population is NOT a literal here: it is the number of article
// identities declared by `data/parse/holdout_manifests.yaml#articles`, read at
// run time into `LaneData::expected_documents`.  A lane whose `reference_tei`
// does not cover exactly those identities is refused by name
// (`probe_reference_population_size_drift` / `probe_reference_source_mismatch`)
// before any measurement, never audited against a stale fixed-size table.

/// The two metric names the lane declares, read from DATA at run time and bound
/// to a computation here.  A declared metric with no binding is a refusal, so a
/// third declared metric can never be silently ignored.
const AGREEMENT_METRIC: &str = "document_agreement_ratio";
const UNCERTAINTY_METRIC: &str = "document_uncertainty_interval_width";

/// The one place thresholds live.  The lane's `margin_ref` must name it.
const MARGIN_REF: &str = "external_holdout_contract.acceptance_margins";
const MARGIN_REF_KEY: &str = "acceptance_margins";

/// The z of the 95% Wilson score interval.  Not a margin.
const WILSON_Z: f64 = 1.96;

/// Every redaction flag the artifact must carry, all `false`.
const REDACTION_FLAGS: [&str; 16] = [
    "raw_tei",
    "raw_pdf",
    "raw_text",
    "field_values",
    "names",
    "email_addresses",
    "institution_names",
    "captions",
    "references",
    "physical_paths",
    "hostname",
    "executable",
    "secrets",
    "pdf_bytes",
    "graph_payload",
    "fulltext",
];

/// Payload keys that may only ever appear as redaction *flags*.
const FORBIDDEN_KEYS: [&str; 12] = [
    "raw_tei",
    "tei_xml",
    "pdf_bytes",
    "body_text",
    "text",
    "captions",
    "references",
    "physical_path",
    "path",
    "hostname",
    "executable",
    "secret",
];

// ──────────────────────────────── io helpers ────────────────────────────────

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn read_repo_file(rel: &str) -> Result<String, String> {
    std::fs::read_to_string(repo_root().join(rel))
        .map_err(|error| format!("{rel} unreadable ({error})"))
}

// ─────────────────────── the DATA lane (definitions live here) ───────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarginDirection {
    AtLeast,
    AtMost,
}

impl MarginDirection {
    fn token(self) -> &'static str {
        match self {
            MarginDirection::AtLeast => "at_least",
            MarginDirection::AtMost => "at_most",
        }
    }
}

/// One declared acceptance margin, read from `acceptance_margins` by `margin_ref`.
#[derive(Debug, Clone, PartialEq)]
struct LaneMargin {
    metric: String,
    direction: MarginDirection,
    value: f64,
}

/// One declared metric definition.  The definitions are DATA; this file never
/// re-states them as literals and never carries a second numeric dictionary.
#[derive(Debug, Clone, PartialEq)]
struct LaneMetricDefinition {
    metric: String,
    numerator_definition: String,
    denominator_definition: String,
    aggregation: String,
    claim_scope: String,
    zero_admitted_population_behavior: String,
    margin_ref: String,
}

/// One frozen reference row: the byte pin of a committed holdout TEI.
#[derive(Debug, Clone, PartialEq)]
struct LaneReferenceRow {
    article_id: String,
    declared_sha256: String,
    path: PathBuf,
    size_bytes: u64,
}

/// One frozen reference row the lane cannot measure, carried as DATA with its own
/// byte pins and a reason drawn from the manifest's closed reason vocabulary.
/// This is the opposite of a silent filter: the document keeps a name, a pin and
/// a cause, so an exclusion is auditable and never an implicit drop.
#[derive(Debug, Clone, PartialEq)]
struct LaneExceptionRow {
    article_id: String,
    declared_sha256: String,
    path: PathBuf,
    size_bytes: u64,
    reason: String,
}

/// The declared cost bounds, read from `external_holdout_contract.cost_bounds`.
#[derive(Debug, Clone)]
struct LaneCostBounds {
    currency: String,
    basis: String,
    lower_bound_includes: Vec<String>,
    upper_bound_includes: Vec<String>,
    unknown_cost_is_not_zero: bool,
    release_claim_allowed: bool,
}

/// The declared Java-relative parity semantics, read from
/// `external_holdout_contract.release_probe.class_parity`.  This is DATA: the
/// producer never re-states the denominator, the aggregation, the class
/// vocabulary or the source-consumed class set — it reads them here and refuses
/// by name when a declaration is absent or disagrees with what it computes.
#[derive(Debug, Clone)]
struct LaneClassParity {
    schema_version: String,
    claim_scope: String,
    denominator: String,
    numerator: String,
    missing: String,
    extra: String,
    aggregation: String,
    precision: String,
    recall: String,
    f1: String,
    zero_zero_denominator: String,
    source_consumed_classes: BTreeSet<String>,
    source_consumed_rule: String,
}

/// The validated lane plus the identity surfaces the measurement needs.
#[derive(Debug, Clone)]
struct LaneData {
    reference: Vec<LaneReferenceRow>,
    /// The frozen references this lane cannot measure, each with a named reason
    /// from the manifest's closed vocabulary. DATA, never a silent filter.
    exceptions: Vec<LaneExceptionRow>,
    metrics: Vec<LaneMetricDefinition>,
    margins: Vec<LaneMargin>,
    identity_digest: String,
    source_manifest: String,
    source_manifest_sha256: String,
    /// The declared probe population, derived from
    /// `data/parse/holdout_manifests.yaml#articles` (never a literal).
    expected_documents: usize,
    profile_fingerprint: String,
    config_fingerprint: String,
    reference_profile: String,
    reference_version: String,
    reference_endpoint: String,
    release_backend: String,
    release_image_digest: String,
    release_pin: String,
    rollback_version: String,
    label_divergence_item: String,
    external_execution_status: String,
    external_bytes: String,
    /// The declared Java-relative parity semantics (DATA, not a compiled-in
    /// definition).
    class_parity: LaneClassParity,
    cost: LaneCostBounds,
}

impl LaneData {
    fn margin(&self, name: &str) -> Option<&LaneMargin> {
        self.margins.iter().find(|margin| margin.metric == name)
    }

    /// The frozen population the lane declares: the measurable subset plus the
    /// named exceptions. It must equal `expected_documents` (the source
    /// manifest's article identities); a partition that drops or double-counts a
    /// document is refused in `read_lane`, never measured.
    fn frozen_documents(&self) -> usize {
        self.reference.len() + self.exceptions.len()
    }
}

fn scalar_to_string(value: &Yaml) -> Option<String> {
    match value {
        Yaml::String(text) => Some(text.clone()),
        Yaml::Number(number) => Some(number.to_string()),
        Yaml::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn yaml_str<'a>(node: &'a Yaml, key: &str) -> Option<&'a str> {
    node.get(key).and_then(Yaml::as_str)
}

fn yaml_strings(node: &Yaml, key: &str) -> Vec<String> {
    node.get(key)
        .and_then(Yaml::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(Yaml::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Containment for a declared fixture path, before any byte is read: relative,
/// no `..` component, and inside the committed fixture directory.  Lexical only,
/// so an escape is refused even when the target exists.
fn check_reference_path(path: &str) -> Result<PathBuf, String> {
    if path.starts_with('/') {
        return Err("probe_reference_path_absolute".into());
    }
    if Path::new(path)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("probe_reference_path_escape".into());
    }
    if !path.starts_with(&format!("{FIXTURE_DIR}/")) {
        return Err("probe_reference_path_outside_fixture_root".into());
    }
    Ok(repo_root().join(path))
}

/// Recompute the frozen probe identity digest from `holdout_manifests.yaml`
/// using ONLY the declared canonicalization handles (fields, order, separators).
/// The digest is recomputed from bytes, never trusted from the declared value.
fn recompute_identity_digest(lane: &Yaml) -> Result<String, String> {
    let source_manifest =
        yaml_str(lane, "source_manifest").ok_or("probe_identity_source_missing")?;
    let fields: Vec<String> = lane
        .get("digest_input_fields")
        .and_then(Yaml::as_sequence)
        .ok_or("probe_identity_source_missing")?
        .iter()
        .map(|field| {
            field
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "probe_identity_source_missing".to_string())
        })
        .collect::<Result<_, _>>()?;
    if yaml_str(lane, "digest_input_order") != Some("as_listed") {
        return Err("probe_identity_not_frozen".into());
    }
    let field_separator =
        yaml_str(lane, "digest_field_separator").ok_or("probe_identity_source_missing")?;
    let record_separator =
        yaml_str(lane, "digest_record_separator").ok_or("probe_identity_source_missing")?;
    let text = read_repo_file(source_manifest)
        .map_err(|_| format!("probe_identity_source_missing:{source_manifest}"))?;
    let source: Yaml = serde_yaml::from_str(&text)
        .map_err(|_| format!("probe_identity_source_unparsable:{source_manifest}"))?;
    let articles = source
        .get("articles")
        .and_then(Yaml::as_sequence)
        .ok_or("probe_identity_source_missing")?;
    let mut records = Vec::with_capacity(articles.len());
    for article in articles {
        let mut parts = Vec::with_capacity(fields.len());
        for field in &fields {
            parts.push(
                article
                    .get(field.as_str())
                    .and_then(scalar_to_string)
                    .ok_or("probe_identity_source_missing")?,
            );
        }
        records.push(parts.join(field_separator));
    }
    let canonical = records.join(record_separator);
    Ok(format!("sha256:{:x}", Sha256::digest(canonical.as_bytes())))
}

/// Read and validate the frozen lane.  Every refusal is a NAMED label; the caller
/// turns an `Err` into a FATAL failure.  Nothing here measures anything: a
/// mutated definition can only change the label, never silently change a number.
fn read_lane(manifest: &Yaml) -> Result<LaneData, String> {
    let contract = manifest
        .get("external_holdout_contract")
        .ok_or("probe_lane_missing")?;
    let lane = contract.get("release_probe").ok_or("probe_lane_missing")?;

    // The external `bytes_absent` band must be untouched: the probe lane is
    // additive and never lifts it.
    let external_execution_status = yaml_str(contract, "execution_status")
        .ok_or("external_lane_state_flipped")?
        .to_string();
    if external_execution_status != "not_executed" {
        return Err("external_lane_state_flipped".into());
    }
    let external_bytes = contract
        .get("byte_policy")
        .and_then(|policy| yaml_str(policy, "external_bytes"))
        .ok_or("external_lane_state_flipped")?
        .to_string();
    if external_bytes != "bytes_absent" {
        return Err("external_lane_state_flipped".into());
    }

    // The thresholds, read from the referenced block.  No second dictionary.
    let margins_value = contract
        .get(MARGIN_REF_KEY)
        .and_then(|margins| margins.get("margins"))
        .and_then(Yaml::as_sequence)
        .ok_or("probe_margin_ref_missing")?;
    let mut margins = Vec::with_capacity(margins_value.len());
    for entry in margins_value {
        let metric = yaml_str(entry, "metric")
            .ok_or("probe_margin_metric_missing")?
            .to_string();
        let direction = match yaml_str(entry, "direction") {
            Some("at_least") => MarginDirection::AtLeast,
            Some("at_most") => MarginDirection::AtMost,
            _ => return Err("probe_margin_direction_drift".into()),
        };
        let value = entry
            .get("margin")
            .and_then(Yaml::as_f64)
            .ok_or("probe_margin_value_missing")?;
        margins.push(LaneMargin {
            metric,
            direction,
            value,
        });
    }
    let margin_names: BTreeSet<String> =
        margins.iter().map(|margin| margin.metric.clone()).collect();

    // The metric definitions, validated against the declared contract.
    let metrics_value = lane
        .get("metrics")
        .and_then(Yaml::as_sequence)
        .ok_or("probe_metrics_missing")?;
    let mut metrics = Vec::with_capacity(metrics_value.len());
    let mut metric_names = BTreeSet::new();
    for entry in metrics_value {
        let metric = yaml_str(entry, "metric")
            .ok_or("probe_metric_name_missing")?
            .to_string();
        if !margin_names.contains(&metric) {
            return Err("probe_metric_vocabulary_drift".into());
        }
        let non_empty = |key: &str| {
            entry
                .get(key)
                .and_then(Yaml::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        };
        let numerator_definition =
            non_empty("numerator_definition").ok_or("probe_metric_definition_missing")?;
        let denominator_definition =
            non_empty("denominator_definition").ok_or("probe_metric_definition_missing")?;
        let aggregation = non_empty("aggregation").ok_or("probe_metric_definition_missing")?;
        if aggregation != "per_document" {
            return Err("probe_metric_aggregation_not_per_document".into());
        }
        let claim_scope = non_empty("claim_scope").ok_or("probe_metric_definition_missing")?;
        if claim_scope != "local_proxy_only" {
            return Err("probe_metric_claim_scope_not_local".into());
        }
        let zero_admitted_population_behavior = non_empty("zero_admitted_population_behavior")
            .ok_or("probe_metric_definition_missing")?;
        if zero_admitted_population_behavior != "no_denominator" {
            return Err("probe_zero_admitted_population_read_as_one".into());
        }
        let margin_ref = non_empty("margin_ref").ok_or("probe_metric_margin_ref_missing")?;
        if margin_ref != MARGIN_REF {
            return Err("probe_metric_margin_ref_missing".into());
        }
        for threshold_key in ["margin", "threshold", "direction", "tolerance"] {
            if entry.get(threshold_key).is_some() {
                return Err("probe_metric_threshold_duplicated".into());
            }
        }
        if !metric_names.insert(metric.clone()) {
            return Err("probe_metric_duplicate".into());
        }
        metrics.push(LaneMetricDefinition {
            metric,
            numerator_definition,
            denominator_definition,
            aggregation,
            claim_scope,
            zero_admitted_population_behavior,
            margin_ref,
        });
    }
    if metric_names != margin_names
        || !metric_names.contains(AGREEMENT_METRIC)
        || !metric_names.contains(UNCERTAINTY_METRIC)
    {
        return Err("probe_metric_vocabulary_drift".into());
    }

    // The frozen reference rows.
    let reference_value = lane
        .get("reference_tei")
        .and_then(Yaml::as_sequence)
        .ok_or("probe_reference_tei_missing")?;
    let mut reference = Vec::with_capacity(reference_value.len());
    for row in reference_value {
        let article_id = yaml_str(row, "article_id")
            .ok_or("probe_reference_row_missing")?
            .to_string();
        let declared_sha256 = yaml_str(row, "sha256")
            .ok_or("probe_reference_row_missing")?
            .to_string();
        let path_str = yaml_str(row, "path").ok_or("probe_reference_row_missing")?;
        let size_bytes = row
            .get("size_bytes")
            .and_then(Yaml::as_u64)
            .ok_or("probe_reference_row_missing")?;
        reference.push(LaneReferenceRow {
            article_id,
            declared_sha256,
            path: check_reference_path(path_str)?,
            size_bytes,
        });
    }

    // The typed exception list. The frozen population is a partition: the rows
    // above are what this lane can measure, and the rows below are what it
    // cannot, each with its own byte pins and a reason drawn from the closed
    // vocabulary declared in the manifest. Nothing here filters silently: an
    // unlisted reason or a nameless exclusion is refused by label.
    let reason_vocabulary: BTreeSet<String> = lane
        .get("reference_exception_reasons")
        .and_then(Yaml::as_sequence)
        .ok_or("probe_reference_exception_vocabulary_missing")?
        .iter()
        .map(|entry| {
            let reason =
                yaml_str(entry, "reason").ok_or("probe_reference_exception_vocabulary_missing")?;
            if yaml_str(entry, "detail")
                .map(str::trim)
                .filter(|detail| !detail.is_empty())
                .is_none()
            {
                return Err("probe_reference_exception_vocabulary_missing".to_string());
            }
            Ok(reason.to_string())
        })
        .collect::<Result<_, String>>()?;
    if reason_vocabulary.is_empty() {
        return Err("probe_reference_exception_vocabulary_missing".into());
    }

    let exception_value = lane
        .get("reference_exceptions")
        .and_then(Yaml::as_sequence)
        .ok_or("probe_reference_exceptions_missing")?;
    let mut exceptions = Vec::with_capacity(exception_value.len());
    for row in exception_value {
        let article_id = yaml_str(row, "article_id")
            .ok_or("probe_reference_exception_row_missing")?
            .to_string();
        let declared_sha256 = yaml_str(row, "sha256")
            .ok_or("probe_reference_exception_row_missing")?
            .to_string();
        let path_str = yaml_str(row, "path").ok_or("probe_reference_exception_row_missing")?;
        let size_bytes = row
            .get("size_bytes")
            .and_then(Yaml::as_u64)
            .ok_or("probe_reference_exception_row_missing")?;
        let reason = yaml_str(row, "reason")
            .ok_or("probe_reference_exception_row_missing")?
            .to_string();
        if !reason_vocabulary.contains(&reason) {
            return Err(format!(
                "probe_reference_exception_reason_unlisted:{article_id}"
            ));
        }
        let path = check_reference_path(path_str)?;
        // An exclusion still pins bytes: the row is verified against disk here,
        // so a stale exclusion cannot hide a drift behind the word "excluded".
        let bytes = std::fs::read(&path)
            .map_err(|_| format!("probe_reference_exception_missing:{article_id}"))?;
        if sha256_prefixed(&bytes) != declared_sha256 {
            return Err(format!(
                "probe_reference_exception_bytes_drift:{article_id}"
            ));
        }
        if bytes.len() as u64 != size_bytes {
            return Err(format!("probe_reference_exception_size_drift:{article_id}"));
        }
        exceptions.push(LaneExceptionRow {
            article_id,
            declared_sha256,
            path,
            size_bytes,
            reason,
        });
    }

    // The frozen identity must recompute from the source manifest.
    let identity_digest = recompute_identity_digest(lane)?;
    let declared_digest =
        yaml_str(lane, "identity_digest").ok_or("probe_identity_source_missing")?;
    if identity_digest != declared_digest {
        return Err("probe_identity_digest_drift".into());
    }

    // The lane's byte pins must agree with the manifest they claim to derive from.
    let source_manifest = yaml_str(lane, "source_manifest")
        .ok_or("probe_identity_source_missing")?
        .to_string();
    let source_text = read_repo_file(&source_manifest)
        .map_err(|_| format!("probe_identity_source_missing:{source_manifest}"))?;
    let source: Yaml = serde_yaml::from_str(&source_text)
        .map_err(|_| format!("probe_identity_source_unparsable:{source_manifest}"))?;
    let articles = source
        .get("articles")
        .and_then(Yaml::as_sequence)
        .ok_or("probe_identity_source_missing")?;
    // The lane covers exactly the frozen source manifest's article identities.
    // The lane's `reference_tei[].sha256` is the byte pin of the committed TEI,
    // while `holdout_manifests.yaml` pins the source PDF: the two hashes are
    // deliberately different objects, so the cross-check is on identity, and the
    // byte pins are re-verified against the files themselves before measurement.
    // The population size is DATA, not a literal: the measurable subset and the
    // named exceptions must partition exactly the article identities the source
    // manifest declares. A lane that drops a document, double-counts one, or
    // covers a different number is refused by name here, before any measurement,
    // rather than audited against a stale fixed-size table.
    if reference.len() + exceptions.len() != articles.len() {
        return Err("probe_reference_population_size_drift".into());
    }
    let expected_documents = articles.len();
    let manifest_ids: BTreeSet<String> = articles
        .iter()
        .filter_map(|article| yaml_str(article, "article_id"))
        .map(str::to_string)
        .collect();
    let reference_ids: BTreeSet<String> =
        reference.iter().map(|row| row.article_id.clone()).collect();
    let exception_ids: BTreeSet<String> = exceptions
        .iter()
        .map(|row| row.article_id.clone())
        .collect();
    if !reference_ids.is_disjoint(&exception_ids) {
        return Err("probe_reference_exception_overlap".into());
    }
    let covered: BTreeSet<String> = reference_ids.union(&exception_ids).cloned().collect();
    if covered != manifest_ids {
        return Err("probe_reference_source_mismatch".into());
    }

    // The declared source-manifest byte pin: the artifact must publish the same
    // value the lane declares, and the file on disk must still hash to it.
    let source_manifest_sha256 = yaml_str(lane, "source_manifest_sha256")
        .ok_or("probe_source_manifest_sha_missing")?
        .to_string();
    let observed_manifest_sha = sha256_prefixed(source_text.as_bytes());
    if observed_manifest_sha != source_manifest_sha256 {
        return Err("probe_source_manifest_sha_drift".into());
    }

    // The reference identity (profile + consolidation flags) is part of the
    // composer identity, so a drift here must be a refusal, not a silent change.
    let reference_profile = yaml_str(lane, "reference_profile")
        .ok_or("probe_reference_identity_missing")?
        .to_string();
    let flags = lane
        .get("reference_consolidation_flags")
        .and_then(Yaml::as_mapping)
        .ok_or("probe_reference_identity_missing")?;
    let mut flag_pairs = flags
        .iter()
        .map(|(key, value)| {
            let key = key
                .as_str()
                .ok_or_else(|| "probe_reference_identity_missing".to_string())?;
            let value = value
                .as_str()
                .ok_or_else(|| "probe_reference_identity_missing".to_string())?;
            Ok(format!("{key}={value}"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    flag_pairs.sort();

    // The declared release the reference must be attributable to.
    let declared = lane
        .get("declared_release")
        .ok_or("probe_reference_release_missing")?;
    let release_version = yaml_str(declared, "version")
        .ok_or("probe_reference_release_missing")?
        .to_string();
    let reference_version = yaml_str(lane, "reference_java_version")
        .ok_or("probe_reference_release_missing")?
        .to_string();
    if reference_version != release_version {
        return Err("probe_reference_release_disagreement".into());
    }

    // The cost bounds, read from DATA so the artifact cannot invent a currency or
    // a bound shape.
    let cost_node = contract
        .get("cost_bounds")
        .ok_or("probe_cost_bounds_missing")?;
    let cost = LaneCostBounds {
        currency: yaml_str(cost_node, "currency")
            .ok_or("probe_cost_bounds_missing")?
            .to_string(),
        basis: yaml_str(cost_node, "basis")
            .ok_or("probe_cost_bounds_missing")?
            .to_string(),
        lower_bound_includes: yaml_strings(cost_node, "lower_bound_includes"),
        upper_bound_includes: yaml_strings(cost_node, "upper_bound_includes"),
        unknown_cost_is_not_zero: cost_node
            .get("unknown_cost_is_not_zero")
            .and_then(Yaml::as_bool)
            .ok_or("probe_cost_bounds_missing")?,
        release_claim_allowed: cost_node
            .get("release_claim_allowed")
            .and_then(Yaml::as_bool)
            .ok_or("probe_cost_bounds_missing")?,
    };
    if !cost.unknown_cost_is_not_zero || cost.release_claim_allowed {
        return Err("probe_cost_bounds_eroded".into());
    }

    // The declared Java-relative parity semantics.  The producer never
    // re-states the denominator, the aggregation or the source-consumed class
    // set: they are read here, so a DATA edit is the only place the definition
    // can change.  A missing definition is refused by name before any
    // measurement; a definition that disagrees with what this producer computes
    // is refused by its own name.
    let parity = lane
        .get("class_parity")
        .ok_or("probe_class_parity_missing")?;
    let non_empty = |key: &str| {
        parity
            .get(key)
            .and_then(Yaml::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    let class_parity = LaneClassParity {
        schema_version: non_empty("schema_version")
            .ok_or("probe_class_parity_definition_missing")?,
        claim_scope: non_empty("claim_scope").ok_or("probe_class_parity_definition_missing")?,
        denominator: non_empty("denominator").ok_or("probe_class_parity_definition_missing")?,
        numerator: non_empty("numerator").ok_or("probe_class_parity_definition_missing")?,
        missing: non_empty("missing").ok_or("probe_class_parity_definition_missing")?,
        extra: non_empty("extra").ok_or("probe_class_parity_definition_missing")?,
        aggregation: non_empty("aggregation").ok_or("probe_class_parity_definition_missing")?,
        precision: non_empty("precision").ok_or("probe_class_parity_definition_missing")?,
        recall: non_empty("recall").ok_or("probe_class_parity_definition_missing")?,
        f1: non_empty("f1").ok_or("probe_class_parity_definition_missing")?,
        zero_zero_denominator: non_empty("zero_zero_denominator")
            .ok_or("probe_class_parity_definition_missing")?,
        source_consumed_classes: yaml_strings(parity, "source_consumed_classes")
            .into_iter()
            .collect(),
        source_consumed_rule: non_empty("source_consumed_rule")
            .ok_or("probe_class_parity_definition_missing")?,
    };
    if class_parity.schema_version != "external-holdout-class-parity.v1" {
        return Err("probe_class_parity_schema_drift".into());
    }
    if class_parity.claim_scope != "local_proxy_only" {
        return Err("probe_class_parity_scope_drift".into());
    }
    // The denominator is the Java-relative `source_population`, never the
    // narrowed `admitted` band: a collapsed denominator would manufacture a
    // recall of 1.0.
    if class_parity.denominator != "source_population" {
        return Err("probe_class_parity_denominator_drift".into());
    }
    if class_parity.aggregation != "micro_occurrence" {
        return Err("probe_class_parity_aggregation_drift".into());
    }
    if class_parity.source_consumed_classes.is_empty() {
        return Err("probe_class_parity_source_consumed_drift".into());
    }
    let known_classes: BTreeSet<&str> = CLASS_ORDER.iter().copied().collect();
    if class_parity
        .source_consumed_classes
        .iter()
        .any(|class| !known_classes.contains(class.as_str()))
    {
        return Err("probe_class_parity_source_consumed_drift".into());
    }

    Ok(LaneData {
        reference,
        exceptions,
        metrics,
        margins,
        identity_digest,
        source_manifest,
        source_manifest_sha256,
        expected_documents,
        profile_fingerprint: format!("profile={reference_profile}"),
        config_fingerprint: format!("consolidation={}", flag_pairs.join(",")),
        reference_profile,
        reference_version,
        reference_endpoint: yaml_str(lane, "reference_endpoint")
            .ok_or("probe_reference_identity_missing")?
            .to_string(),
        release_backend: yaml_str(declared, "backend")
            .ok_or("probe_reference_release_missing")?
            .to_string(),
        release_image_digest: yaml_str(declared, "image_digest")
            .ok_or("probe_reference_release_missing")?
            .to_string(),
        release_pin: yaml_str(declared, "release_pin")
            .ok_or("probe_reference_release_missing")?
            .to_string(),
        rollback_version: yaml_str(declared, "rollback_version")
            .ok_or("probe_reference_release_missing")?
            .to_string(),
        label_divergence_item: yaml_str(declared, "label_divergence_item")
            .ok_or("probe_reference_release_missing")?
            .to_string(),
        external_execution_status,
        external_bytes,
        class_parity,
        cost,
    })
}

// ──────────────────────── the frozen reference provenance ───────────────────

#[derive(Debug, Deserialize)]
struct Provenance {
    source_manifest: String,
    source_manifest_sha256: String,
    identity_digest_ref: String,
    reference_endpoint: String,
    reference_java_version: String,
    reference_profile: String,
    reference_consolidation_flags: ProvenanceFlags,
    declared_release: ProvenanceRelease,
    captured_at_revision: String,
    captured_at_unix_secs: u64,
    documents: Vec<ProvenanceDocument>,
}

#[derive(Debug, Deserialize)]
struct ProvenanceFlags {
    #[serde(rename = "consolidateHeader")]
    consolidate_header: String,
    #[serde(rename = "consolidateCitations")]
    consolidate_citations: String,
    #[serde(rename = "teiCoordinates")]
    tei_coordinates: String,
}

impl ProvenanceFlags {
    /// The same sorted `key=value` projection the lane's `config_fingerprint`
    /// uses, so the provenance flags are compared rather than merely parsed.
    fn fingerprint(&self) -> String {
        let mut pairs = [
            format!("consolidateCitations={}", self.consolidate_citations),
            format!("consolidateHeader={}", self.consolidate_header),
            format!("teiCoordinates={}", self.tei_coordinates),
        ];
        pairs.sort();
        format!("consolidation={}", pairs.join(","))
    }
}

#[derive(Debug, Deserialize)]
struct ProvenanceRelease {
    version: String,
    image_digest: String,
    release_pin: String,
    rollback_version: String,
    label_divergence_item: String,
}

#[derive(Debug, Deserialize)]
struct ProvenanceDocument {
    article_id: String,
    path: String,
    sha256: String,
    size_bytes: u64,
    source_pdf_sha256: String,
    latency_ms: u64,
}

fn load_provenance() -> Provenance {
    let text =
        read_repo_file(PROVENANCE_REL).unwrap_or_else(|reason| panic!("FATAL {PROBE} {reason}"));
    serde_yaml::from_str(&text)
        .unwrap_or_else(|error| panic!("FATAL {PROBE} {PROVENANCE_REL} is not valid YAML: {error}"))
}

/// The provenance record must agree with the lane and with the bytes on disk.
/// A disagreement is a NAMED refusal, never a silently different latency or
/// release attribution.
fn verify_provenance(lane: &LaneData, provenance: &Provenance) -> Result<(), String> {
    if provenance.source_manifest != lane.source_manifest {
        return Err("provenance_source_manifest_drift".into());
    }
    if provenance.source_manifest_sha256 != lane.source_manifest_sha256 {
        return Err("provenance_source_manifest_sha_drift".into());
    }
    if provenance.identity_digest_ref != "external_holdout_contract.release_probe.identity_digest" {
        return Err("provenance_identity_ref_drift".into());
    }
    if provenance.reference_profile != lane.reference_profile {
        return Err("provenance_profile_drift".into());
    }
    if provenance.reference_consolidation_flags.fingerprint() != lane.config_fingerprint {
        return Err("provenance_consolidation_drift".into());
    }
    if provenance.reference_endpoint != lane.reference_endpoint {
        return Err("provenance_endpoint_drift".into());
    }
    if provenance.reference_java_version != lane.reference_version {
        return Err("provenance_release_drift".into());
    }
    if provenance.declared_release.version != lane.reference_version
        || provenance.declared_release.image_digest != lane.release_image_digest
        || provenance.declared_release.release_pin != lane.release_pin
        || provenance.declared_release.rollback_version != lane.rollback_version
        || provenance.declared_release.label_divergence_item != lane.label_divergence_item
    {
        return Err("provenance_release_drift".into());
    }
    if provenance.documents.len() != lane.expected_documents {
        return Err("provenance_document_population_drift".into());
    }
    for row in &provenance.documents {
        // The frozen population is the measurable subset plus the named
        // exceptions: the provenance record covers every frozen document, so the
        // lookup must too. An id in neither list is a provenance document the
        // lane never declared, refused by name.
        let frozen = lane
            .reference
            .iter()
            .find(|candidate| candidate.article_id == row.article_id)
            .map(|candidate| {
                (
                    candidate.declared_sha256.as_str(),
                    candidate.size_bytes,
                    candidate.path.as_path(),
                )
            })
            .or_else(|| {
                lane.exceptions
                    .iter()
                    .find(|candidate| candidate.article_id == row.article_id)
                    .map(|candidate| {
                        (
                            candidate.declared_sha256.as_str(),
                            candidate.size_bytes,
                            candidate.path.as_path(),
                        )
                    })
            })
            .ok_or_else(|| format!("provenance_unknown_document:{}", row.article_id))?;
        let (declared_sha256, size_bytes, path) = frozen;
        if declared_sha256 != row.sha256 || size_bytes != row.size_bytes {
            return Err(format!("provenance_byte_pin_drift:{}", row.article_id));
        }
        if row.latency_ms == 0 {
            return Err(format!("provenance_latency_missing:{}", row.article_id));
        }
        if row.source_pdf_sha256.is_empty() {
            return Err(format!("provenance_source_pdf_missing:{}", row.article_id));
        }
        // The provenance path is the fixture the lane pins; nothing else.
        if Path::new(&row.path) != path.strip_prefix(repo_root()).unwrap_or(path) {
            return Err(format!("provenance_fixture_path_drift:{}", row.article_id));
        }
    }
    Ok(())
}

// ───────────────────── the expectations the guards compare against ──────────

/// One document's recomputed measurement, the ledger the report must agree with.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedDocument {
    document_id: String,
    identity_sha256: String,
    size_bytes: u64,
    latency_ms: u64,
    source_population: usize,
    admitted: usize,
    conserved: usize,
    lost: usize,
    fabricated: usize,
    agreement_micros: Option<u64>,
    low_micros: Option<u64>,
    high_micros: Option<u64>,
}

/// One named exclusion: a frozen document the lane cannot measure, with the reason
/// drawn from the manifest's closed vocabulary.  The artifact publishes it so the
/// exclusion is visible evidence, never an implicit drop.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedException {
    document_id: String,
    identity_sha256: String,
    size_bytes: u64,
    reason: String,
}

/// One margin verdict recomputed from the DATA thresholds.  A report whose
/// verdict disagrees with this is a re-fitted verdict, not a measured one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedVerdict {
    document_id: String,
    metric: String,
    verdict: String,
}

/// Everything the report-shape guards compare the artifact against.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedProbe {
    identity_digest: String,
    source_manifest_sha256: String,
    external_execution_status: String,
    external_bytes: String,
    latency_source: String,
    release_label: String,
    release_profile: String,
    release_consolidation: String,
    captured_at_revision: String,
    cost_currency: String,
    /// The frozen population the lane declares (measurable subset + exceptions),
    /// recomputed from the source manifest.  `documents` below is the measured
    /// subset only; the two differ by exactly the named exceptions.
    frozen_documents: usize,
    documents: Vec<ExpectedDocument>,
    exceptions: Vec<ExpectedException>,
    verdicts: Vec<ExpectedVerdict>,
    aggregate: Aggregate,
    canonical_catalog_present: usize,
    sources_archive_present: usize,
    gold_standard_present: usize,
}

// ─────────────────────────── the margin metrics ────────────────────────────

/// The two metrics the lane declares.  A declared metric that maps to no
/// computation is a refusal, so a third declared metric cannot be ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricKind {
    Agreement,
    UncertaintyWidth,
}

fn metric_kind(definition: &LaneMetricDefinition) -> MetricKind {
    match definition.metric.as_str() {
        AGREEMENT_METRIC => MetricKind::Agreement,
        UNCERTAINTY_METRIC => MetricKind::UncertaintyWidth,
        other => panic!("FATAL {PROBE} unbound declared metric definition `{other}`"),
    }
}

/// The 95% Wilson score interval for `successes / population`, bounded to
/// `[0, 1]`.  `None` when the population is empty (`no_denominator`).  The width
/// depends on the population, so narrowing the admitted band widens the interval.
fn wilson_interval(successes: usize, population: usize) -> Option<(f64, f64)> {
    if population == 0 {
        return None;
    }
    let n = population as f64;
    let p = successes as f64 / n;
    let z = WILSON_Z;
    let denominator = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / denominator;
    let half = (z / denominator) * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt();
    Some(((center - half).max(0.0), (center + half).min(1.0)))
}

#[derive(Debug, Clone, PartialEq)]
struct DocumentMetrics {
    source_population: usize,
    admitted: usize,
    conserved: usize,
    lost: usize,
    fabricated: usize,
    /// `document_agreement_ratio`, or `None` when there is no denominator.
    agreement: Option<f64>,
    /// `document_uncertainty_interval_width` bounds, or `None` without a denominator.
    uncertainty: Option<(f64, f64)>,
    uncertainty_width: Option<f64>,
}

fn document_metrics(profiles: &[ClassProfile]) -> DocumentMetrics {
    let source_population: usize = profiles.iter().map(|p| p.source_population).sum();
    let admitted: usize = profiles.iter().map(|p| p.admitted).sum();
    let conserved: usize = profiles.iter().map(|p| p.conserved).sum();
    let lost: usize = profiles.iter().map(|p| p.lost).sum();
    let fabricated: usize = profiles.iter().map(|p| p.fabricated).sum();
    let agreement = ratio(conserved, admitted);
    let uncertainty = wilson_interval(conserved, admitted);
    let uncertainty_width = uncertainty.map(|(low, high)| high - low);
    DocumentMetrics {
        source_population,
        admitted,
        conserved,
        lost,
        fabricated,
        agreement,
        uncertainty,
        uncertainty_width,
    }
}

impl DocumentMetrics {
    fn admission_coverage(&self) -> Option<f64> {
        ratio(self.admitted, self.source_population)
    }

    fn agreement_micros(&self) -> Option<u64> {
        self.agreement.map(micros)
    }

    fn low_micros(&self) -> Option<u64> {
        self.uncertainty.map(|(low, _)| micros(low))
    }

    fn high_micros(&self) -> Option<u64> {
        self.uncertainty.map(|(_, high)| micros(high))
    }
}

fn metric_value(kind: MetricKind, metrics: &DocumentMetrics) -> Option<f64> {
    match kind {
        MetricKind::Agreement => metrics.agreement,
        MetricKind::UncertaintyWidth => metrics.uncertainty_width,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Pass,
    Fail,
    NoDenominator,
}

impl Verdict {
    fn token(self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::NoDenominator => "no_denominator",
        }
    }
}

/// Compare a measured value with the margin read from DATA.  No denominator
/// short-circuits to `NoDenominator` before any comparison, so a missing
/// population is never read as a pass.
fn margin_verdict(margin: &LaneMargin, value: Option<f64>) -> Verdict {
    let Some(value) = value else {
        return Verdict::NoDenominator;
    };
    let satisfied = match margin.direction {
        MarginDirection::AtLeast => value >= margin.value,
        MarginDirection::AtMost => value <= margin.value,
    };
    if satisfied {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}

#[derive(Debug, Clone, PartialEq)]
struct MarginVerdict {
    metric: String,
    direction: MarginDirection,
    threshold: f64,
    value: Option<f64>,
    verdict: Verdict,
}

/// Evaluate one verdict per DECLARED metric (not per hard-coded name), so the
/// number of verdicts tracks the lane.
fn evaluate_margins(lane: &LaneData, metrics: &DocumentMetrics) -> Vec<MarginVerdict> {
    lane.metrics
        .iter()
        .map(|definition| {
            let margin = lane
                .margin(&definition.metric)
                .unwrap_or_else(|| panic!("FATAL {PROBE} no margin for `{}`", definition.metric));
            let value = metric_value(metric_kind(definition), metrics);
            MarginVerdict {
                metric: definition.metric.clone(),
                direction: margin.direction,
                threshold: margin.value,
                value,
                verdict: margin_verdict(margin, value),
            }
        })
        .collect()
}

fn micros(value: f64) -> u64 {
    (value * 1_000_000.0).round() as u64
}

// ───────────────────────────── measurement ─────────────────────────────────

/// One frozen document's measured result.
struct MeasuredDocument {
    document_id: String,
    identity_sha256: String,
    size_bytes: u64,
    latency_ms: u64,
    admitted_band: AdmittedBand,
    profiles: Vec<ClassProfile>,
    metrics: DocumentMetrics,
    verdicts: Vec<MarginVerdict>,
}

impl MeasuredDocument {
    fn expected(&self) -> ExpectedDocument {
        ExpectedDocument {
            document_id: self.document_id.clone(),
            identity_sha256: self.identity_sha256.clone(),
            size_bytes: self.size_bytes,
            latency_ms: self.latency_ms,
            source_population: self.metrics.source_population,
            admitted: self.metrics.admitted,
            conserved: self.metrics.conserved,
            lost: self.metrics.lost,
            fabricated: self.metrics.fabricated,
            agreement_micros: self.metrics.agreement_micros(),
            low_micros: self.metrics.low_micros(),
            high_micros: self.metrics.high_micros(),
        }
    }

    fn expected_verdicts(&self) -> Vec<ExpectedVerdict> {
        self.verdicts
            .iter()
            .map(|verdict| ExpectedVerdict {
                document_id: self.document_id.clone(),
                metric: verdict.metric.clone(),
                verdict: verdict.verdict.token().to_string(),
            })
            .collect()
    }
}

/// Re-verify one frozen TEI against its declared byte pin.  A missing file, a
/// drifted digest or a drifted size is a NAMED refusal; the caller turns it into
/// a FATAL failure so the probe never profiles bytes other than the pinned ones.
fn check_reference_bytes(row: &LaneReferenceRow) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(&row.path)
        .map_err(|_| format!("probe_reference_missing:{}", row.article_id))?;
    let actual = sha256_prefixed(&bytes);
    if actual != row.declared_sha256 {
        return Err(format!("probe_reference_bytes_drift:{}", row.article_id));
    }
    if bytes.len() as u64 != row.size_bytes {
        return Err(format!("probe_reference_size_drift:{}", row.article_id));
    }
    Ok(bytes)
}

fn measure_document(row: &LaneReferenceRow, lane: &LaneData, latency_ms: u64) -> MeasuredDocument {
    let bytes = check_reference_bytes(row).unwrap_or_else(|label| panic!("FATAL {PROBE} {label}"));
    let xml = String::from_utf8(bytes).unwrap_or_else(|error| {
        panic!(
            "FATAL {PROBE} probe_reference_not_utf8:{} ({error})",
            row.article_id
        )
    });

    // The composer's identity check takes a raw 64-hex source digest while the
    // lane pin carries the `sha256:` prefix: strip it here, once, for both the
    // identity surface and the frozen report.
    let identity_sha256 = row
        .declared_sha256
        .strip_prefix("sha256:")
        .unwrap_or(&row.declared_sha256)
        .to_string();
    let source = measure_source(&row.article_id, &identity_sha256, &xml)
        .unwrap_or_else(|error| panic!("FATAL {PROBE} {error}"));
    let admitted =
        build_admitted_band(&source).unwrap_or_else(|error| panic!("FATAL {PROBE} {error}"));
    let emission = compose_native_document(
        &lane.profile_fingerprint,
        &lane.config_fingerprint,
        &source,
        &admitted,
        &xml,
    )
    .unwrap_or_else(|error| panic!("FATAL {PROBE} {}: {error}", row.article_id));
    let profiles = measure_document_class_profiles(
        &source,
        &xml,
        &admitted,
        &emission,
        &lane.class_parity.source_consumed_classes,
    )
    .unwrap_or_else(|error| panic!("FATAL {PROBE} {}: {error}", row.article_id));
    let metrics = document_metrics(&profiles);
    let verdicts = evaluate_margins(lane, &metrics);

    MeasuredDocument {
        document_id: row.article_id.clone(),
        identity_sha256,
        size_bytes: row.size_bytes,
        latency_ms,
        admitted_band: admitted,
        profiles,
        metrics,
        verdicts,
    }
}

fn measure_all(lane: &LaneData, provenance: &Provenance) -> Vec<MeasuredDocument> {
    lane.reference
        .iter()
        .map(|row| {
            let latency_ms = provenance
                .documents
                .iter()
                .find(|document| document.article_id == row.article_id)
                .map(|document| document.latency_ms)
                .unwrap_or_else(|| {
                    panic!(
                        "FATAL {PROBE} provenance_latency_missing:{}",
                        row.article_id
                    )
                });
            measure_document(row, lane, latency_ms)
        })
        .collect()
}

/// The declared producer/authority wording for the parity block.  The producer
/// is the runtime compact scan; the authority is the independent dev-only
/// structural oracle, which is a separate crate root and shares no scan code.
const CLASS_PARITY_PRODUCER: &str = "da-cli compact non-validating scan over the composed emission";
const CLASS_PARITY_AUTHORITY: &str = "dev-only structural oracle crates/da-parse/tests/external_holdout_probe_semantics.rs (independent roxmltree judge)";

/// The declared Java-relative parity aggregate: micro counts over every
/// measured class, with the two honest aggregate recalls.  The denominator is
/// the Java `source_population`, never the narrowed `admitted` band, so the
/// aggregate recall cannot be 1.0 by construction.
///
/// `native_capability_recall` counts only the classes the native composer
/// actually produces (the source-consumed classes are excluded from it);
/// `covered_union_recall` counts a source-consumed class as covered by its full
/// source population, because the copied source subtree carries those carriers.
struct ClassParity {
    denominator: usize,
    tp: usize,
    fn_: usize,
    fp: usize,
    native_denominator: usize,
    native_tp: usize,
    covered_union_numerator: usize,
}

impl ClassParity {
    fn of(measured: &[MeasuredDocument]) -> ClassParity {
        let mut parity = ClassParity {
            denominator: 0,
            tp: 0,
            fn_: 0,
            fp: 0,
            native_denominator: 0,
            native_tp: 0,
            covered_union_numerator: 0,
        };
        for document in measured {
            for profile in &document.profiles {
                parity.denominator += profile.source_population;
                parity.tp += profile.conserved;
                parity.fn_ += profile.source_population.saturating_sub(profile.conserved);
                parity.fp += profile.fabricated;
                if profile.source_is_conserved {
                    parity.covered_union_numerator += profile.source_population;
                } else {
                    parity.native_denominator += profile.source_population;
                    parity.native_tp += profile.conserved;
                    parity.covered_union_numerator += profile.conserved;
                }
            }
        }
        parity
    }

    fn native_capability_recall(&self) -> Option<f64> {
        ratio(self.native_tp, self.native_denominator)
    }

    fn covered_union_recall(&self) -> Option<f64> {
        ratio(self.covered_union_numerator, self.denominator)
    }

    fn precision(&self) -> Option<f64> {
        ratio(self.tp, self.tp + self.fp)
    }

    fn f1(&self) -> Option<f64> {
        harmonic_mean(self.precision(), self.native_capability_recall())
    }

    fn json(&self) -> Json {
        json!({
            "aggregate": {
                "denominator": self.denominator,
                "tp": self.tp,
                "fn": self.fn_,
                "fp": self.fp,
                "native_capability_recall_micros": micros_of(self.native_capability_recall()),
                "covered_union_recall_micros": micros_of(self.covered_union_recall()),
                "precision_micros": micros_of(self.precision()),
                "f1_micros": micros_of(self.f1()),
            },
            "producer": CLASS_PARITY_PRODUCER,
            "authority": CLASS_PARITY_AUTHORITY,
            "claim_scope": "local_proxy_only",
        })
    }
}

/// The declared Java-relative parity semantics, emitted at the artifact top
/// level so the reviewer sees the denominator, the aggregation and the
/// source-consumed rule beside the numbers.  It is deliberately kept OUT of
/// `evaluated_population`: the population-binding oracle scans that block for
/// oversized string values, and the declared rule is a sentence, not a value.
fn class_parity_declaration(lane: &LaneData) -> Json {
    let declaration = &lane.class_parity;
    json!({
        "schema_version": declaration.schema_version,
        "claim_scope": declaration.claim_scope,
        "denominator": declaration.denominator,
        "numerator": declaration.numerator,
        "missing": declaration.missing,
        "extra": declaration.extra,
        "aggregation": declaration.aggregation,
        "precision": declaration.precision,
        "recall": declaration.recall,
        "f1": declaration.f1,
        "zero_zero_denominator": declaration.zero_zero_denominator,
        "source_consumed_classes": declaration
            .source_consumed_classes
            .iter()
            .cloned()
            .collect::<Vec<String>>(),
        "source_consumed_rule": declaration.source_consumed_rule,
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Aggregate {
    documents: usize,
    source_population: usize,
    admitted: usize,
    tp: usize,
    fn_: usize,
    fp: usize,
    agreeing_documents: usize,
    margin_pass: usize,
    margin_fail: usize,
    no_denominator_documents: usize,
}

impl Aggregate {
    fn of(measured: &[MeasuredDocument]) -> Aggregate {
        let mut aggregate = Aggregate {
            documents: measured.len(),
            source_population: 0,
            admitted: 0,
            tp: 0,
            fn_: 0,
            fp: 0,
            agreeing_documents: 0,
            margin_pass: 0,
            margin_fail: 0,
            no_denominator_documents: 0,
        };
        for document in measured {
            aggregate.source_population += document.metrics.source_population;
            aggregate.admitted += document.metrics.admitted;
            aggregate.tp += document.metrics.conserved;
            aggregate.fn_ += document.metrics.lost;
            aggregate.fp += document.metrics.fabricated;
            if document.metrics.agreement == Some(1.0) {
                aggregate.agreeing_documents += 1;
            }
            if document.metrics.agreement.is_none() {
                aggregate.no_denominator_documents += 1;
            }
            for verdict in &document.verdicts {
                match verdict.verdict {
                    Verdict::Pass => aggregate.margin_pass += 1,
                    Verdict::Fail => aggregate.margin_fail += 1,
                    Verdict::NoDenominator => {}
                }
            }
        }
        aggregate
    }

    fn render(&self) -> String {
        format!(
            "PROBE AGGREGATE: documents={} source_population={} admitted={} tp={} fn={} fp={} \
             agreeing_documents={} margin_pass={} margin_fail={} no_denominator_documents={} \
             claim_scope=local_proxy_only counts_only=true",
            self.documents,
            self.source_population,
            self.admitted,
            self.tp,
            self.fn_,
            self.fp,
            self.agreeing_documents,
            self.margin_pass,
            self.margin_fail,
            self.no_denominator_documents,
        )
    }
}

// ─────────────────── the production-catalog overlap census ─────────────────

struct CatalogCensus {
    rows: Vec<Json>,
    declared_pdf_present: usize,
    canonical_catalog_present: usize,
    sources_archive_present: usize,
    gold_standard_present: usize,
    absent_from_every_locator: usize,
}

fn dirs_under(rel: &str) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(repo_root().join(rel)) {
        for entry in entries.flatten() {
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    names.sort();
    names
}

fn pdf_under(root: &str, category: &str, id: &str) -> bool {
    repo_root()
        .join(root)
        .join(category)
        .join(id)
        .join("source")
        .join(format!("{id}.pdf"))
        .is_file()
}

/// A counted presence census over the holdout identities.  `found` is a
/// measurement: a directory listing plus a `stat`, never an assertion, and the
/// gold-standard column is the one that would invalidate the non-gold claim.
fn catalog_census(lane: &LaneData, source_manifest: &Yaml) -> CatalogCensus {
    let canonical_categories = dirs_under(CANONICAL_CATALOG_ROOT);
    let sources_categories = dirs_under(SOURCES_ARCHIVE_ROOT);
    let declared_paths: Vec<(String, String)> = source_manifest
        .get("articles")
        .and_then(Yaml::as_sequence)
        .map(|articles| {
            articles
                .iter()
                .filter_map(|article| {
                    Some((
                        yaml_str(article, "article_id")?.to_string(),
                        yaml_str(article, "pdf_path")?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut census = CatalogCensus {
        rows: Vec::new(),
        declared_pdf_present: 0,
        canonical_catalog_present: 0,
        sources_archive_present: 0,
        gold_standard_present: 0,
        absent_from_every_locator: 0,
    };

    let frozen_ids: Vec<&str> = lane
        .reference
        .iter()
        .map(|row| row.article_id.as_str())
        .chain(lane.exceptions.iter().map(|row| row.article_id.as_str()))
        .collect();

    for id in frozen_ids {
        let declared_path = declared_paths
            .iter()
            .find(|(article_id, _)| article_id == id)
            .map(|(_, path)| path.clone())
            .unwrap_or_default();
        let declared_present =
            !declared_path.is_empty() && repo_root().join(&declared_path).is_file();
        let declared_class = if declared_path.starts_with("data/article_catalog/") {
            "canonical_catalog"
        } else if declared_path.starts_with("data/sources/") {
            "sources_archive"
        } else {
            "other"
        };
        let canonical_present = canonical_categories
            .iter()
            .any(|category| pdf_under(CANONICAL_CATALOG_ROOT, category, id));
        let sources_present = sources_categories
            .iter()
            .any(|category| pdf_under(SOURCES_ARCHIVE_ROOT, category, id))
            || dirs_under(&format!("{SOURCES_ARCHIVE_ROOT}/article_catalog/arxiv"))
                .iter()
                .any(|category| {
                    repo_root()
                        .join(SOURCES_ARCHIVE_ROOT)
                        .join("article_catalog")
                        .join("arxiv")
                        .join(category)
                        .join(id)
                        .join("source")
                        .join(format!("{id}.pdf"))
                        .is_file()
                });
        let gold_present = repo_root()
            .join(GOLD_STANDARD_DIR)
            .join(format!("{id}.gold.json"))
            .is_file();

        census.declared_pdf_present += usize::from(declared_present);
        census.canonical_catalog_present += usize::from(canonical_present);
        census.sources_archive_present += usize::from(sources_present);
        census.gold_standard_present += usize::from(gold_present);
        if !declared_present && !canonical_present && !sources_present && !gold_present {
            census.absent_from_every_locator += 1;
        }

        census.rows.push(json!({
            "document_id": id,
            "declared_locator": declared_path,
            "declared_locator_class": declared_class,
            "declared_pdf_present": declared_present,
            "canonical_catalog_present": canonical_present,
            "sources_archive_present": sources_present,
            "gold_standard_present": gold_present,
        }));
    }
    census
}

// ───────────────────────────── the artifact ────────────────────────────────

/// The declared guard list.  Every label is proven to fire on its own violating
/// input by `--self-check`, and the committed artifact is audited with the same
/// predicate before it is written.
const GUARD_LABELS: [&str; 36] = [
    "schema_version_drift",
    "reference_population_role",
    "reference_documents",
    "reference_identity_missing",
    "reference_identity_digest_drift",
    "source_manifest_sha_drift",
    "reference_byte_pins_drift",
    "outcome_count",
    "identity_sha_collapse",
    "identity_pin_drift",
    "executed_population_not_executed",
    "execution_evidence_unbound",
    "accepted_documents",
    "evaluated_status",
    "aggregate_zero_admitted_population",
    "aggregate_conservation_mismatch",
    "margin_verdict_absent",
    "margin_verdict_refitted",
    "latency_source_substituted",
    "latency_unmeasured",
    "legacy_block_present",
    "external_lane_state_flipped",
    "policy_lock_open",
    "redaction_flag_raised",
    "redaction_scope_replaced",
    "raw_payload_leak",
    "cost_unknown_read_as_zero",
    "production_catalog_overlap_absent",
    "named_limitations_missing",
    "class_narrowings_incomplete",
    "reference_measurable_documents",
    "reference_exceptions_drift",
    "class_parity_missing",
    "class_parity_denominator_drift",
    "class_parity_precision_inconsistent",
    "class_parity_conservation_mismatch",
];

fn collect_strings<'a>(value: &'a Json, out: &mut Vec<&'a str>) {
    match value {
        Json::String(text) => out.push(text),
        Json::Array(items) => items.iter().for_each(|item| collect_strings(item, out)),
        Json::Object(map) => map.values().for_each(|child| collect_strings(child, out)),
        _ => {}
    }
}

/// Payload keys may only ever appear as redaction flags.
fn raw_key_errors(report: &Json) -> Vec<String> {
    fn visit(value: &Json, inside_redaction: bool, errors: &mut Vec<String>) {
        match value {
            Json::Object(map) => {
                for (key, child) in map {
                    let in_redaction = inside_redaction || key == "redaction";
                    if !in_redaction && FORBIDDEN_KEYS.contains(&key.as_str()) {
                        errors.push("raw_payload_leak".into());
                    }
                    visit(child, in_redaction, errors);
                }
            }
            Json::Array(items) => items
                .iter()
                .for_each(|item| visit(item, inside_redaction, errors)),
            _ => {}
        }
    }
    let mut errors = Vec::new();
    visit(report, false, &mut errors);
    errors
}

fn u64_of(value: &Json) -> Option<u64> {
    value.as_u64()
}

/// Recompute the Java-relative parity aggregate from the artifact's own
/// per-class rows.  The guard uses this so a reclassified source-consumed class
/// (which would fabricate false negatives), a drifted per-class counter or a
/// hand-edited aggregate cannot pass: the block must be exactly the micro
/// aggregation of the rows it claims to summarize.
fn class_parity_from_class_rows(profiles: &[Json]) -> ClassParity {
    let mut parity = ClassParity {
        denominator: 0,
        tp: 0,
        fn_: 0,
        fp: 0,
        native_denominator: 0,
        native_tp: 0,
        covered_union_numerator: 0,
    };
    for document in profiles {
        for class in document["classes"].as_array().into_iter().flatten() {
            let source_population = u64_of(&class["source_population"]).unwrap_or(0) as usize;
            let conserved = u64_of(&class["conserved"]).unwrap_or(0) as usize;
            let fabricated = u64_of(&class["fabricated"]).unwrap_or(0) as usize;
            let source_consumed = class["source_consumed"].as_bool().unwrap_or(false);
            parity.denominator += source_population;
            parity.tp += conserved;
            parity.fn_ += source_population.saturating_sub(conserved);
            parity.fp += fabricated;
            if source_consumed {
                parity.covered_union_numerator += source_population;
            } else {
                parity.native_denominator += source_population;
                parity.native_tp += conserved;
                parity.covered_union_numerator += conserved;
            }
        }
    }
    parity
}

/// Whether `evidence` BINDS the executed population, mirroring the governor's
/// independent oracle (`executed_population_binding.py`).  This is the
/// value/vocabulary half of GAR-006: the receipt's SHAPE (non-empty string
/// `kind`/`population_identity`, unsigned-integer `documents`) is the contract
/// in `crates/da-parse/tests/data_contracts.rs` (`population_evidence`) and is
/// re-checked here because a shape-valid receipt is not a bound one.
/// `Some(label)` is a violation: a bare token at corpus scale
/// (`execution_evidence_unbound`), an unknown kind, or a receipt whose count,
/// identity or kind disagrees with the population it claims to have produced.
/// `None` means the evidence binds.
fn execution_evidence_binding(
    evidence: &Json,
    executed: &Json,
    reference: &Json,
) -> Option<String> {
    let executed_documents = executed.get("documents").and_then(Json::as_u64);
    let identity = executed
        .get("identity")
        .and_then(Json::as_str)
        .or_else(|| reference.get("identity").and_then(Json::as_str))
        .filter(|text| !text.is_empty());
    match evidence {
        Json::String(token) => {
            let kind = token.split(':').next().unwrap_or("").trim();
            if !EXECUTION_EVIDENCE_KINDS.contains(&kind) {
                return Some("execution_evidence_unbound:unknown_kind".to_string());
            }
            if executed_documents.is_none_or(|count| count >= CORPUS_SCALE_DOCUMENTS) {
                return Some("execution_evidence_unbound:bare_token".to_string());
            }
            None
        }
        Json::Object(receipt) => {
            if executed_documents.is_none()
                || receipt.get("documents").and_then(Json::as_u64) != executed_documents
            {
                return Some("execution_evidence_unbound:receipt_documents".to_string());
            }
            let Some(target) = identity else {
                return Some("execution_evidence_unbound:receipt_identity".to_string());
            };
            if receipt.get("population_identity").and_then(Json::as_str) != Some(target) {
                return Some("execution_evidence_unbound:receipt_identity".to_string());
            }
            match receipt.get("kind").and_then(Json::as_str) {
                Some(kind) if !kind.is_empty() && EXECUTION_EVIDENCE_KINDS.contains(&kind) => None,
                _ => Some("execution_evidence_unbound:receipt_kind".to_string()),
            }
        }
        _ => Some("execution_evidence_unbound:absent".to_string()),
    }
}

/// The structured execution receipt the executed population publishes.  A bare
/// token cannot bind a corpus-scale number (GAR-006), so the receipt carries the
/// same identity the populations declare and the document count this run
/// actually executed; the governor oracle checks both.
fn execution_receipt(identity: &str, documents: u64) -> Json {
    json!({
        "kind": EXECUTION_EVIDENCE_KIND,
        "population_identity": identity,
        "documents": documents,
    })
}

/// Every reason the runtime probe artifact is unacceptable.  A pure predicate over
/// the parsed report plus the recomputed ledger, so `--self-check` can prove it
/// rejects one violation at a time without touching a fixture.
fn report_errors(report: &Json, expected: &ExpectedProbe) -> Vec<String> {
    let mut errors = BTreeSet::new();

    if report.get("schema_version").and_then(Json::as_str) != Some(REPORT_SCHEMA) {
        errors.insert("schema_version_drift".to_string());
    }

    // ── reference population: identity-only, distinct pinned bytes ───────────
    // The frozen population is the measurable subset plus the named exceptions;
    // the byte pins cover all of them, so an exclusion is visible evidence with
    // its own pin, never a silent drop from the denominator.
    let reference = &report["reference_population"];
    if reference.get("role").and_then(Json::as_str) != Some("identity_only") {
        errors.insert("reference_population_role".to_string());
    }
    if reference
        .get("identity")
        .and_then(Json::as_str)
        .map(str::is_empty)
        .unwrap_or(true)
    {
        errors.insert("reference_identity_missing".to_string());
    }
    if reference.get("identity_sha256").and_then(Json::as_str)
        != Some(expected.identity_digest.as_str())
    {
        errors.insert("reference_identity_digest_drift".to_string());
    }
    if reference
        .get("source_manifest_sha256")
        .and_then(Json::as_str)
        != Some(expected.source_manifest_sha256.as_str())
    {
        errors.insert("source_manifest_sha_drift".to_string());
    }
    if u64_of(&reference["documents"]) != Some(expected.frozen_documents as u64) {
        errors.insert("reference_documents".to_string());
    }
    if u64_of(&reference["measurable_documents"]) != Some(expected.documents.len() as u64) {
        errors.insert("reference_measurable_documents".to_string());
    }
    if u64_of(&reference["excluded_documents"]) != Some(expected.exceptions.len() as u64) {
        errors.insert("reference_exceptions_drift".to_string());
    }
    let declared_exceptions = reference["exceptions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if declared_exceptions.len() != expected.exceptions.len() {
        errors.insert("reference_exceptions_drift".to_string());
    } else {
        for expected_exception in &expected.exceptions {
            let declared = declared_exceptions.iter().find(|row| {
                row.get("document_id").and_then(Json::as_str)
                    == Some(expected_exception.document_id.as_str())
            });
            let matches = declared
                .map(|row| {
                    row.get("identity_sha256").and_then(Json::as_str)
                        == Some(expected_exception.identity_sha256.as_str())
                        && u64_of(&row["size_bytes"]) == Some(expected_exception.size_bytes)
                        && row.get("reason").and_then(Json::as_str)
                            == Some(expected_exception.reason.as_str())
                })
                .unwrap_or(false);
            if !matches {
                errors.insert("reference_exceptions_drift".to_string());
            }
        }
    }
    let pins = reference["byte_pins"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if pins.len() != expected.frozen_documents {
        errors.insert("reference_byte_pins_drift".to_string());
    } else {
        let expected_pins = expected.documents.iter().map(|document| {
            (
                document.document_id.as_str(),
                document.identity_sha256.as_str(),
                document.size_bytes,
            )
        });
        let expected_exception_pins = expected.exceptions.iter().map(|exception| {
            (
                exception.document_id.as_str(),
                exception.identity_sha256.as_str(),
                exception.size_bytes,
            )
        });
        for (document_id, identity_sha256, size_bytes) in
            expected_pins.chain(expected_exception_pins)
        {
            let pinned = pins
                .iter()
                .find(|pin| pin.get("document_id").and_then(Json::as_str) == Some(document_id));
            let matches = pinned
                .map(|pin| {
                    pin.get("identity_sha256").and_then(Json::as_str) == Some(identity_sha256)
                        && u64_of(&pin["size_bytes"]) == Some(size_bytes)
                })
                .unwrap_or(false);
            if !matches {
                errors.insert("reference_byte_pins_drift".to_string());
            }
        }
    }

    // ── executed population: really executed, evidence named ───────────────
    let executed = &report["executed_population"];
    if executed.get("corpus_execution").and_then(Json::as_bool) != Some(true) {
        errors.insert("executed_population_not_executed".to_string());
    }
    if let Some(label) = execution_evidence_binding(
        &executed["execution_evidence"],
        executed,
        &report["reference_population"],
    ) {
        errors.insert(label);
    }
    if u64_of(&executed["documents"]) != Some(expected.documents.len() as u64)
        || u64_of(&executed["accepted"]) != Some(expected.documents.len() as u64)
    {
        errors.insert("accepted_documents".to_string());
    }

    // ── evaluated population: a measured status with the frozen counts ─────
    let evaluated = &report["evaluated_population"];
    if evaluated.get("status").and_then(Json::as_str) != Some("evaluated") {
        errors.insert("evaluated_status".to_string());
    }
    if u64_of(&evaluated["documents"]) != Some(expected.documents.len() as u64) {
        errors.insert("evaluated_status".to_string());
    }
    let aggregate = &evaluated["aggregate"];
    let aggregate_matches = u64_of(&aggregate["documents"])
        == Some(expected.aggregate.documents as u64)
        && u64_of(&aggregate["source_population"])
            == Some(expected.aggregate.source_population as u64)
        && u64_of(&aggregate["admitted"]) == Some(expected.aggregate.admitted as u64)
        && u64_of(&aggregate["tp"]) == Some(expected.aggregate.tp as u64)
        && u64_of(&aggregate["fn"]) == Some(expected.aggregate.fn_ as u64)
        && u64_of(&aggregate["fp"]) == Some(expected.aggregate.fp as u64)
        && u64_of(&aggregate["agreeing_documents"])
            == Some(expected.aggregate.agreeing_documents as u64)
        && u64_of(&aggregate["margin_pass"]) == Some(expected.aggregate.margin_pass as u64)
        && u64_of(&aggregate["margin_fail"]) == Some(expected.aggregate.margin_fail as u64)
        && u64_of(&aggregate["no_denominator_documents"])
            == Some(expected.aggregate.no_denominator_documents as u64);
    if !aggregate_matches {
        errors.insert("aggregate_conservation_mismatch".to_string());
    }
    if aggregate
        .get("admitted")
        .and_then(Json::as_u64)
        .unwrap_or(0)
        == 0
    {
        errors.insert("aggregate_zero_admitted_population".to_string());
    }
    if aggregate.get("counts_only").and_then(Json::as_bool) != Some(true) {
        errors.insert("aggregate_conservation_mismatch".to_string());
    }

    // ── Java-relative class parity: declared, and arithmetically consistent ─
    // The parity block is the Java-relative reading of the same counts.  Its
    // denominator MUST be the Java `source_population` total, and its micro
    // aggregate MUST be exactly the aggregation of the per-class rows, so a
    // collapsed denominator, a reclassified source-consumed class or a
    // hand-edited block is refused rather than averaged into the artifact.
    match evaluated.get("class_parity").and_then(Json::as_object) {
        None => {
            errors.insert("class_parity_missing".to_string());
        }
        Some(block) => {
            let parity = &block["aggregate"];
            let declared_fp = u64_of(&parity["fp"]).unwrap_or(0);
            if u64_of(&parity["denominator"]) != Some(expected.aggregate.source_population as u64) {
                errors.insert("class_parity_denominator_drift".to_string());
            }
            let declared = json!({
                "denominator": parity["denominator"],
                "tp": parity["tp"],
                "fn": parity["fn"],
                "fp": parity["fp"],
                "native_capability_recall_micros": parity["native_capability_recall_micros"],
                "covered_union_recall_micros": parity["covered_union_recall_micros"],
                "precision_micros": parity["precision_micros"],
                "f1_micros": parity["f1_micros"],
            });
            let recomputed = class_parity_from_class_rows(
                report["class_profiles"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
            );
            let observed = json!({
                "denominator": recomputed.denominator,
                "tp": recomputed.tp,
                "fn": recomputed.fn_,
                "fp": recomputed.fp,
                "native_capability_recall_micros": micros_of(recomputed.native_capability_recall()),
                "covered_union_recall_micros": micros_of(recomputed.covered_union_recall()),
                "precision_micros": micros_of(recomputed.precision()),
                "f1_micros": micros_of(recomputed.f1()),
            });
            if declared != observed {
                errors.insert("class_parity_conservation_mismatch".to_string());
            }
            let precision = u64_of(&parity["precision_micros"]);
            if (declared_fp == 0 && precision != Some(1_000_000))
                || (declared_fp > 0 && precision == Some(1_000_000))
            {
                errors.insert("class_parity_precision_inconsistent".to_string());
            }
        }
    }

    // ── outcomes: distinct identities, per-document measurement ────────────
    let outcomes = report["outcomes"].as_array().cloned().unwrap_or_default();
    if outcomes.len() != expected.documents.len() {
        errors.insert("outcome_count".to_string());
    }
    let outcome_shas: BTreeSet<&str> = outcomes
        .iter()
        .filter_map(|outcome| outcome.get("identity_sha256").and_then(Json::as_str))
        .collect();
    if outcome_shas.len() != outcomes.len() || outcome_shas.len() != expected.documents.len() {
        errors.insert("identity_sha_collapse".to_string());
    }
    let expected_verdicts: BTreeSet<(String, String, String)> = expected
        .verdicts
        .iter()
        .map(|verdict| {
            (
                verdict.document_id.clone(),
                verdict.metric.clone(),
                verdict.verdict.clone(),
            )
        })
        .collect();
    let mut seen_verdicts: BTreeSet<(String, String, String)> = BTreeSet::new();
    for expected_document in &expected.documents {
        let outcome = outcomes
            .iter()
            .find(|outcome| {
                outcome.get("document_id").and_then(Json::as_str)
                    == Some(expected_document.document_id.as_str())
            })
            .cloned()
            .unwrap_or(Json::Null);
        if outcome.is_null() {
            errors.insert("outcome_count".to_string());
            continue;
        }
        if outcome.get("identity_sha256").and_then(Json::as_str)
            != Some(expected_document.identity_sha256.as_str())
        {
            errors.insert("identity_pin_drift".to_string());
        }
        let observed = [
            u64_of(&outcome["source_population"]),
            u64_of(&outcome["admitted"]),
            u64_of(&outcome["conserved"]),
            u64_of(&outcome["lost"]),
            u64_of(&outcome["fabricated"]),
        ];
        let wanted = [
            Some(expected_document.source_population as u64),
            Some(expected_document.admitted as u64),
            Some(expected_document.conserved as u64),
            Some(expected_document.lost as u64),
            Some(expected_document.fabricated as u64),
        ];
        if observed != wanted {
            errors.insert("aggregate_conservation_mismatch".to_string());
        }
        if u64_of(&outcome["agreement_ratio_micros"]) != expected_document.agreement_micros {
            errors.insert("aggregate_conservation_mismatch".to_string());
        }
        if u64_of(&outcome["uncertainty_low"]) != expected_document.low_micros
            || u64_of(&outcome["uncertainty_high"]) != expected_document.high_micros
        {
            errors.insert("aggregate_conservation_mismatch".to_string());
        }
        if outcome.get("latency_source").and_then(Json::as_str)
            != Some(expected.latency_source.as_str())
        {
            errors.insert("latency_source_substituted".to_string());
        }
        if u64_of(&outcome["latency_ms"]) != Some(expected_document.latency_ms)
            || expected_document.latency_ms == 0
        {
            errors.insert("latency_unmeasured".to_string());
        }
        let margins = outcome["margins"].as_array().cloned().unwrap_or_default();
        if margins.len()
            != expected
                .verdicts
                .iter()
                .filter(|verdict| verdict.document_id == expected_document.document_id)
                .count()
        {
            errors.insert("margin_verdict_absent".to_string());
        }
        for margin in &margins {
            let key = (
                expected_document.document_id.clone(),
                margin
                    .get("metric")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .to_string(),
                margin
                    .get("verdict")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .to_string(),
            );
            if !expected_verdicts.contains(&key) {
                errors.insert("margin_verdict_refitted".to_string());
            }
            seen_verdicts.insert(key);
        }
    }
    if !expected_verdicts.is_subset(&seen_verdicts) {
        errors.insert("margin_verdict_absent".to_string());
    }
    if outcome_shas
        != expected
            .documents
            .iter()
            .map(|document| document.identity_sha256.as_str())
            .collect()
    {
        errors.insert("identity_pin_drift".to_string());
    }

    // ── the external band is not lifted, and the legacy blocks stay gone ───
    let external = &report["external_holdout_band"];
    if external.get("execution_status").and_then(Json::as_str)
        != Some(expected.external_execution_status.as_str())
        || external.get("external_bytes").and_then(Json::as_str)
            != Some(expected.external_bytes.as_str())
    {
        errors.insert("external_lane_state_flipped".to_string());
    }
    for legacy in ["denominators", "corpus"] {
        if report.get(legacy).is_some() {
            errors.insert("legacy_block_present".to_string());
        }
    }

    // ── policy locks: D127 and ADR-040 stay closed ─────────────────────────
    let policy = &report["policy_locks"];
    let lock_ok = policy.get("import_eligible").and_then(Json::as_bool) == Some(false)
        && policy.get("deployed").and_then(Json::as_bool) == Some(false)
        && policy.get("auto_promote").and_then(Json::as_str) == Some("forbidden")
        && policy.get("production_route").and_then(Json::as_str) == Some("forbidden")
        && policy.get("graph_write").and_then(Json::as_str) == Some("forbidden")
        && policy.get("pooled_claim").and_then(Json::as_str) == Some("forbidden");
    if !lock_ok {
        errors.insert("policy_lock_open".to_string());
    }

    // ── redaction: declared flags only, all false, no payload anywhere ─────
    let redaction = &report["redaction"];
    let declared_flags: BTreeSet<&str> = redaction
        .as_object()
        .map(|flags| flags.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let expected_flags: BTreeSet<&str> = REDACTION_FLAGS.iter().copied().collect();
    if declared_flags != expected_flags {
        errors.insert("redaction_scope_replaced".to_string());
    }
    if let Some(flags) = redaction.as_object() {
        for (flag, value) in flags {
            if value.as_bool() != Some(false) {
                errors.insert(format!("redaction_flag_raised:{flag}"));
            }
        }
    }
    errors.extend(raw_key_errors(report));
    let serialized = serde_json::to_string(report).unwrap_or_default();
    if serialized.contains('<') {
        errors.insert("raw_payload_leak".to_string());
    }
    let mut strings = Vec::new();
    collect_strings(report, &mut strings);
    for text in strings.iter().filter(|text| text.contains('@')) {
        if *text != RELEASE_LABEL && *text != ROLLBACK_TARGET {
            errors.insert("raw_payload_leak".to_string());
        }
    }

    // ── cost: bounded, never measured, an unknown is never zero ───────────
    let cost = &report["cost_accounting"];
    let cost_ok = cost.get("status").and_then(Json::as_str) == Some("bounded_not_measured")
        && cost.get("unknown_is_not_zero").and_then(Json::as_bool) == Some(true)
        && cost.get("currency").and_then(Json::as_str) == Some(expected.cost_currency.as_str())
        && cost.get("cost_lower_bound").map(Json::is_null) == Some(true)
        && cost.get("cost_upper_bound").map(Json::is_null) == Some(true)
        && cost.get("pooled_cost_claim").and_then(Json::as_str) == Some("forbidden");
    if !cost_ok {
        errors.insert("cost_unknown_read_as_zero".to_string());
    }

    // ── the census is a measurement and the limitations are declared ───────
    let census = &report["production_catalog_overlap"];
    let census_ok = census.get("census_kind").and_then(Json::as_str) == Some(CENSUS_KIND)
        && u64_of(&census["documents"]) == Some(expected.frozen_documents as u64)
        && u64_of(&census["canonical_catalog_present_count"])
            == Some(expected.canonical_catalog_present as u64)
        && u64_of(&census["sources_archive_present_count"])
            == Some(expected.sources_archive_present as u64)
        && u64_of(&census["gold_standard_present_count"])
            == Some(expected.gold_standard_present as u64)
        && census
            .get("locators")
            .and_then(Json::as_array)
            .map(Vec::len)
            == Some(expected.frozen_documents);
    if !census_ok {
        errors.insert("production_catalog_overlap_absent".to_string());
    }
    if report["named_limitations"]
        .as_array()
        .map(Vec::is_empty)
        .unwrap_or(true)
    {
        errors.insert("named_limitations_missing".to_string());
    }

    // ── every real narrowing is declared, not silently shrunk ─────────────
    // A class whose admitted band is smaller than its real carrier population
    // must appear in `class_narrowings`; the table is what makes a shrink
    // explainable rather than invisible.
    let declared_narrowings: BTreeSet<(String, String)> = report["class_narrowings"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    Some((
                        row.get("document_id").and_then(Json::as_str)?.to_string(),
                        row.get("class").and_then(Json::as_str)?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut derived_narrowings: BTreeSet<(String, String)> = BTreeSet::new();
    for document in report["class_profiles"].as_array().into_iter().flatten() {
        let Some(document_id) = document.get("document_id").and_then(Json::as_str) else {
            continue;
        };
        for class_row in document["classes"].as_array().into_iter().flatten() {
            let admitted = u64_of(&class_row["admitted"]).unwrap_or(0);
            let source_population = u64_of(&class_row["source_population"]).unwrap_or(0);
            if admitted < source_population
                && let Some(class) = class_row.get("class").and_then(Json::as_str)
            {
                derived_narrowings.insert((document_id.to_string(), class.to_string()));
            }
        }
    }
    if declared_narrowings != derived_narrowings {
        errors.insert("class_narrowings_incomplete".to_string());
    }

    // ── the frozen release attribution is the measured one ────────────────
    let release = &reference["reference_release"];
    if release.get("label").and_then(Json::as_str) != Some(expected.release_label.as_str())
        || release.get("profile").and_then(Json::as_str) != Some(expected.release_profile.as_str())
        || release.get("consolidation").and_then(Json::as_str)
            != Some(expected.release_consolidation.as_str())
        || release.get("captured_at_revision").and_then(Json::as_str)
            != Some(expected.captured_at_revision.as_str())
    {
        errors.insert("reference_identity_digest_drift".to_string());
    }

    errors.into_iter().collect()
}

// ─────────────────────── the artifact assembly ─────────────────────────────

fn micros_of(value: Option<f64>) -> Json {
    match value {
        Some(value) => json!(micros(value)),
        None => Json::Null,
    }
}

/// Assemble the canonical artifact.  Every value is either recomputed from the
/// frozen reference or read from DATA/provenance; no wall clock and no float is
/// serialized, so two runs are byte-identical.
fn build_report(
    lane: &LaneData,
    provenance: &Provenance,
    measured: &[MeasuredDocument],
    census: &CatalogCensus,
) -> Json {
    let profiles: Vec<Json> = measured
        .iter()
        .map(|document| {
            json!({
                "document_id": document.document_id,
                "identity_sha256": document.identity_sha256,
                "classes": document.profiles.iter().map(|profile| json!({
                    "class": profile.class,
                    "source_population": profile.source_population,
                    "admitted": profile.admitted,
                    "conserved": profile.conserved,
                    "lost": profile.lost,
                    "fabricated": profile.fabricated,
                    "admission_coverage_micros": micros_of(profile.admission_coverage()),
                    "recall_java_micros": micros_of(profile.recall_java()),
                    "precision_micros": micros_of(profile.precision()),
                    "f1_micros": micros_of(profile.f1()),
                    "source_consumed": profile.source_is_conserved,
                })).collect::<Vec<Json>>(),
            })
        })
        .collect();

    let outcomes: Vec<Json> = measured
        .iter()
        .enumerate()
        .map(|(index, document)| {
            json!({
                "document_id": document.document_id,
                "state": "evaluated",
                "identity_sha256": document.identity_sha256,
                "source_population": document.metrics.source_population,
                "admitted": document.metrics.admitted,
                "conserved": document.metrics.conserved,
                "lost": document.metrics.lost,
                "fabricated": document.metrics.fabricated,
                "admission_coverage_micros": micros_of(document.metrics.admission_coverage()),
                "agreement_ratio_micros": micros_of(document.metrics.agreement),
                "uncertainty_low": micros_of(document.metrics.uncertainty.map(|(low, _)| low)),
                "uncertainty_high": micros_of(document.metrics.uncertainty.map(|(_, high)| high)),
                "uncertainty_width_micros": micros_of(document.metrics.uncertainty_width),
                "cost_lower_bound": Json::Null,
                "cost_upper_bound": Json::Null,
                "latency_ms": document.latency_ms,
                "latency_source": LATENCY_SOURCE_REF,
                "margins": document.verdicts.iter().map(|verdict| json!({
                    "metric": verdict.metric,
                    "direction": verdict.direction.token(),
                    "threshold_micros": micros(verdict.threshold),
                    "value_micros": micros_of(verdict.value),
                    "verdict": verdict.verdict.token(),
                })).collect::<Vec<Json>>(),
                "typed_gaps": document.admitted_band.gaps.iter().map(|gap| json!({
                    "class": gap.class,
                    "source_population": gap.source_population,
                    "admitted": gap.admitted,
                    "reason": gap.reason,
                })).collect::<Vec<Json>>(),
                "profile_ordinal": index,
            })
        })
        .collect();

    let aggregate = Aggregate::of(measured);
    let latencies: Vec<u64> = measured
        .iter()
        .map(|document| document.latency_ms)
        .collect();

    json!({
        "schema_version": REPORT_SCHEMA,
        "artifact_id": "external_holdout_release_probe",
        "measured_scope": format!(
            "{} of {} frozen non-gold local holdout documents are measurable; the other {} are named typed exceptions ({}); every number is a count, never a corpus rate; no pooled claim",
            measured.len(),
            lane.frozen_documents(),
            lane.exceptions.len(),
            lane.exceptions
                .iter()
                .map(|row| row.reason.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
        ),
        "units": {
            "agreement_ratio": "ratio_micros",
            "uncertainty": "ratio_micros",
            "latency": "milliseconds",
        },
        "declared_metric_definitions": lane.metrics.iter().map(|definition| json!({
            "metric": definition.metric,
            "numerator_definition": definition.numerator_definition,
            "denominator_definition": definition.denominator_definition,
            "aggregation": definition.aggregation,
            "claim_scope": definition.claim_scope,
            "zero_admitted_population_behavior": definition.zero_admitted_population_behavior,
            "margin_ref": definition.margin_ref,
        })).collect::<Vec<Json>>(),
        "class_parity_declaration": class_parity_declaration(lane),
        "reference_population": {
            "role": "identity_only",
            "identity": POPULATION_IDENTITY,
            "identity_sha256": lane.identity_digest,
            "identity_digest_is_byte_hash": false,
            "source_manifest": lane.source_manifest,
            "source_manifest_sha256": lane.source_manifest_sha256,
            "documents": lane.frozen_documents(),
            "measurable_documents": lane.reference.len(),
            "excluded_documents": lane.exceptions.len(),
            "exceptions": lane.exceptions.iter().map(|row| json!({
                "document_id": row.article_id,
                "identity_sha256": row.declared_sha256.strip_prefix("sha256:").unwrap_or(&row.declared_sha256),
                "size_bytes": row.size_bytes,
                "reason": row.reason,
            })).collect::<Vec<Json>>(),
            "reference_release": {
                "backend": lane.release_backend,
                "version": lane.reference_version,
                "label": RELEASE_LABEL,
                "image_digest": lane.release_image_digest,
                "release_pin": lane.release_pin,
                "rollback_version": lane.rollback_version,
                "rollback_target": ROLLBACK_TARGET,
                "label_divergence_item": lane.label_divergence_item,
                "profile": lane.reference_profile,
                "consolidation": lane.config_fingerprint,
                "endpoint": lane.reference_endpoint,
                "captured_at_revision": provenance.captured_at_revision,
                "captured_at_unix_secs": provenance.captured_at_unix_secs,
            },
            "byte_pins": lane.reference.iter().map(|row| json!({
                "document_id": row.article_id,
                "identity_sha256": row.declared_sha256.strip_prefix("sha256:").unwrap_or(&row.declared_sha256),
                "size_bytes": row.size_bytes,
            })).chain(lane.exceptions.iter().map(|row| json!({
                "document_id": row.article_id,
                "identity_sha256": row.declared_sha256.strip_prefix("sha256:").unwrap_or(&row.declared_sha256),
                "size_bytes": row.size_bytes,
            }))).collect::<Vec<Json>>(),
        },
        "executed_population": {
            "corpus_execution": true,
            "execution_evidence": execution_receipt(POPULATION_IDENTITY, measured.len() as u64),
            "execution_route": format!("{}/{}/processFulltextDocument", lane.release_backend, lane.reference_version),
            "execution_route_profile": lane.reference_profile,
            "identity": POPULATION_IDENTITY,
            "documents": measured.len(),
            "accepted": measured.len(),
            "synthetic_corpus_execution": false,
        },
        "evaluated_population": {
            "status": "evaluated",
            "oracle": "da-cli compact non-validating scan over the composed emission (the structural validating oracle is the dev-only da-parse test)",
            "documents": measured.len(),
            "aggregate": {
                "documents": aggregate.documents,
                "source_population": aggregate.source_population,
                "admitted": aggregate.admitted,
                "tp": aggregate.tp,
                "fn": aggregate.fn_,
                "fp": aggregate.fp,
                "agreeing_documents": aggregate.agreeing_documents,
                "margin_pass": aggregate.margin_pass,
                "margin_fail": aggregate.margin_fail,
                "no_denominator_documents": aggregate.no_denominator_documents,
                "counts_only": true,
                "claim_scope": "local_proxy_only",
            },
            "class_parity": ClassParity::of(measured).json(),
        },
        "outcomes": outcomes,
        "class_profiles": profiles,
        "class_narrowings": measured.iter().flat_map(|document| {
            document.profiles.iter().filter(|profile| profile.admitted < profile.source_population).map(move |profile| json!({
                "document_id": document.document_id,
                "class": profile.class,
                "source_population": profile.source_population,
                "admitted": profile.admitted,
            })).collect::<Vec<Json>>()
        }).collect::<Vec<Json>>(),
        "production_catalog_overlap": {
            "census_kind": CENSUS_KIND,
            "documents": lane.frozen_documents(),
            "declared_pdf_present_count": census.declared_pdf_present,
            "canonical_catalog_present_count": census.canonical_catalog_present,
            "sources_archive_present_count": census.sources_archive_present,
            "gold_standard_present_count": census.gold_standard_present,
            "absent_from_every_locator_count": census.absent_from_every_locator,
            "production_corpus_identity_overlap": if census.gold_standard_present == 0 { "none" } else { "present" },
            "locators": census.rows,
        },
        "cost_accounting": {
            "status": "bounded_not_measured",
            "currency": lane.cost.currency,
            "basis": lane.cost.basis,
            "cost_lower_bound": Json::Null,
            "cost_upper_bound": Json::Null,
            "unknown_is_not_zero": true,
            "release_claim_allowed": false,
            "pooled_cost_claim": "forbidden",
            "reference": "data/parse/acceptance_manifest.yaml#external_holdout_contract.cost_bounds",
            "lower_bound_includes": lane.cost.lower_bound_includes,
            "upper_bound_includes": lane.cost.upper_bound_includes,
        },
        "latency_accounting": {
            "unit": "milliseconds_per_document",
            "measurement_source": "recorded_wall_clock_per_document",
            "latency_source": LATENCY_SOURCE_REF,
            "documents": measured.len(),
            "min_latency_ms": latencies.iter().copied().min().unwrap_or(0),
            "max_latency_ms": latencies.iter().copied().max().unwrap_or(0),
            "pooled_latency_claim": "forbidden",
            "wall_clock_at_run_time": false,
        },
        "external_holdout_band": {
            "execution_status": lane.external_execution_status,
            "external_bytes": lane.external_bytes,
            "external_rows_are_evidence": false,
            "citation_without_release_decision": "forbidden",
        },
        "policy_locks": {
            "import_eligible": false,
            "auto_promote": "forbidden",
            "deployed": false,
            "production_route": "forbidden",
            "graph_write": "forbidden",
            "pooled_claim": "forbidden",
            "observation_mode": "shadow",
            "fallback_target": ROLLBACK_TARGET,
        },
        "redaction": REDACTION_FLAGS.iter().map(|flag| ((*flag).to_string(), json!(false))).collect::<serde_json::Map<String, Json>>(),
        "named_limitations": [
            format!(
                "{} of {} frozen documents are measured; the other {} are excluded by name, not silently dropped",
                measured.len(),
                lane.frozen_documents(),
                lane.exceptions.len()
            ),
            "excluded documents declare a typed reason (for example header_date_absent: a frozen TEI header with no dated element is fatal to both this probe and the native header composer); they are not measured and are not zero rows"
                .to_string(),
            format!(
                "{} documents is not a statistical corpus; every number is a count",
                measured.len()
            ),
            "the probe does not claim independence from the production route: the reference was produced by the declared live Java release and the admitted band is a local proxy",
            "latency and cost are bounded, not measured; an unknown value is never zero",
            "the probe does not lift the external bytes_absent band and does not authorize release; a named human release decision is still required",
            "the compact scan is not the structural validating oracle; the authoritative oracle is the dev-only da-parse test external_holdout_probe_semantics",
            "typed route boundary: the native PDF to TEI route is not measured by this probe — no runner connects a PDF input to compose_native_fulltext, and the PDF-route corridor is verified on 3 documents (corpus_n = 3) only; this parity is local_proxy_only over the admitted band, not a native-PDF claim",
        ],
    })
}

/// Project the canonical artifact into the recorded measurement tables.
///
/// The tables are a *projection of the artifact*, not a second measurement: the
/// same `--report` run that passes the artifact guards emits them, so a recorded
/// row can never be written beside an artifact it does not describe.  Every
/// number is read back from the artifact JSON (never recomputed), and each
/// narrowing reason is the artifact's own typed-gap reason.  A narrowing the
/// artifact leaves unexplained (an occurrence conserved by the source subtree
/// rather than rejected by the admitted band) records a null reason, never an
/// invented one.
fn build_frozen_tables(report: &Json, artifact_sha256: &str) -> Json {
    let empty: Vec<Json> = Vec::new();
    let outcomes = report["outcomes"].as_array().unwrap_or(&empty);
    let profiles = report["class_profiles"].as_array().unwrap_or(&empty);
    let narrowings = report["class_narrowings"].as_array().unwrap_or(&empty);
    let profile_by_document: BTreeMap<&str, &Json> = profiles
        .iter()
        .filter_map(|profile| Some((profile["document_id"].as_str()?, profile)))
        .collect();

    let documents: Vec<Json> = outcomes
        .iter()
        .map(|outcome| {
            let document_id = outcome["document_id"].as_str().unwrap_or_default();
            let classes = profile_by_document
                .get(document_id)
                .and_then(|profile| profile["classes"].as_array())
                .cloned()
                .unwrap_or_default();
            let gaps: BTreeMap<&str, &str> = outcome["typed_gaps"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .filter_map(|gap| Some((gap["class"].as_str()?, gap["reason"].as_str()?)))
                .collect();
            let margins = outcome["margins"].as_array().cloned().unwrap_or_default();
            let margin_pass = margins
                .iter()
                .filter(|margin| margin["verdict"].as_str() == Some("PASS"))
                .count();
            let margin_fail = margins
                .iter()
                .filter(|margin| margin["verdict"].as_str() == Some("FAIL"))
                .count();
            let document_narrowings: Vec<Json> = narrowings
                .iter()
                .filter(|narrowing| narrowing["document_id"].as_str() == Some(document_id))
                .map(|narrowing| {
                    let class = narrowing["class"].as_str().unwrap_or_default();
                    json!({
                        "class": class,
                        "reason": gaps.get(class).copied().map(Json::from).unwrap_or(Json::Null),
                        "source_population": narrowing["source_population"],
                        "admitted": narrowing["admitted"],
                    })
                })
                .collect();
            json!({
                "article_id": document_id,
                "sha256": outcome["identity_sha256"],
                "source_population": outcome["source_population"],
                "admitted": outcome["admitted"],
                "conserved": outcome["conserved"],
                "rejected": outcome["fabricated"],
                "agreement_micros": outcome["agreement_ratio_micros"],
                "uncertainty_low_micros": outcome["uncertainty_low"],
                "uncertainty_high_micros": outcome["uncertainty_high"],
                "uncertainty_width_micros": outcome["uncertainty_width_micros"],
                "latency_ms": outcome["latency_ms"],
                "margin_pass": margin_pass,
                "margin_fail": margin_fail,
                "no_denominator": outcome["agreement_ratio_micros"].is_null(),
                "profile": classes.iter().map(|class| json!({
                    "class": class["class"],
                    "source_population": class["source_population"],
                    "admitted": class["admitted"],
                    "conserved": class["conserved"],
                    "rejected": class["fabricated"],
                    "recall_java_micros": class["recall_java_micros"],
                    "precision_micros": class["precision_micros"],
                    "f1_micros": class["f1_micros"],
                    "source_consumed": class["source_consumed"],
                })).collect::<Vec<Json>>(),
                "narrowings": document_narrowings,
            })
        })
        .collect();

    let aggregate = &report["evaluated_population"]["aggregate"];
    let recorded_at = report["reference_population"]["reference_release"]["captured_at_unix_secs"]
        .as_i64()
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
        .map(|timestamp| timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default();
    json!({
        "schema_version": FROZEN_TABLES_SCHEMA,
        "recorded_at": recorded_at,
        "artifact_sha256": artifact_sha256,
        "aggregate": {
            "documents": aggregate["documents"],
            "source_population": aggregate["source_population"],
            "admitted": aggregate["admitted"],
            "tp": aggregate["tp"],
            "fn": aggregate["fn"],
            "fp": aggregate["fp"],
            "agreeing_documents": aggregate["agreeing_documents"],
            "margin_pass": aggregate["margin_pass"],
            "margin_fail": aggregate["margin_fail"],
            "no_denominator_documents": aggregate["no_denominator_documents"],
            // The Java-relative parity aggregate is a projection of the
            // artifact's `evaluated_population.class_parity` block, read back
            // here (never recomputed): a recorded parity can never be written
            // beside an artifact it does not describe.
            "class_parity": report["evaluated_population"]["class_parity"].clone(),
        },
        "documents": documents,
    })
}

fn expected_probe(
    lane: &LaneData,
    provenance: &Provenance,
    measured: &[MeasuredDocument],
    census: &CatalogCensus,
) -> ExpectedProbe {
    ExpectedProbe {
        identity_digest: lane.identity_digest.clone(),
        source_manifest_sha256: lane.source_manifest_sha256.clone(),
        external_execution_status: lane.external_execution_status.clone(),
        external_bytes: lane.external_bytes.clone(),
        latency_source: LATENCY_SOURCE_REF.to_string(),
        release_label: RELEASE_LABEL.to_string(),
        release_profile: lane.reference_profile.clone(),
        release_consolidation: lane.config_fingerprint.clone(),
        captured_at_revision: provenance.captured_at_revision.clone(),
        cost_currency: lane.cost.currency.clone(),
        frozen_documents: lane.frozen_documents(),
        documents: measured.iter().map(MeasuredDocument::expected).collect(),
        exceptions: lane
            .exceptions
            .iter()
            .map(|row| ExpectedException {
                document_id: row.article_id.clone(),
                identity_sha256: row
                    .declared_sha256
                    .strip_prefix("sha256:")
                    .unwrap_or(&row.declared_sha256)
                    .to_string(),
                size_bytes: row.size_bytes,
                reason: row.reason.clone(),
            })
            .collect(),
        verdicts: measured
            .iter()
            .flat_map(MeasuredDocument::expected_verdicts)
            .collect(),
        aggregate: Aggregate::of(measured),
        canonical_catalog_present: census.canonical_catalog_present,
        sources_archive_present: census.sources_archive_present,
        gold_standard_present: census.gold_standard_present,
    }
}

// ─────────────────────────────── self-check ────────────────────────────────

/// A synthetic valid report plus its ledger.  Built in code on purpose: the
/// self-check must prove the guard predicate, not the current repo state, so it
/// reads no file, touches no network and writes nothing.
fn synthetic_pair(document_count: usize, exception_count: usize) -> (Json, ExpectedProbe) {
    let frozen_count = document_count + exception_count;
    let exceptions: Vec<ExpectedException> = (0..exception_count)
        .map(|index| ExpectedException {
            document_id: format!("x{}", index + 1),
            identity_sha256: format!("{:064x}", 9000 + index + 1),
            size_bytes: 500 + index as u64,
            reason: SYNTHETIC_EXCEPTION_REASON.to_string(),
        })
        .collect();
    let documents: Vec<(String, String, u64, usize, usize)> = (0..document_count)
        .map(|index| {
            let source_population = 10 + index;
            // The first document's admitted band is narrower than its Java
            // population, so the synthetic report exercises a real narrowing
            // while the micro aggregate stays internally consistent.
            let admitted = if index == 0 {
                source_population - 1
            } else {
                source_population
            };
            (
                format!("d{}", index + 1),
                format!("{:064x}", index + 1),
                1000 + index as u64,
                source_population,
                admitted,
            )
        })
        .collect();

    let expected_documents: Vec<ExpectedDocument> = documents
        .iter()
        .map(
            |(id, sha, size, source_population, admitted)| ExpectedDocument {
                document_id: id.clone(),
                identity_sha256: sha.clone(),
                size_bytes: *size,
                latency_ms: 100 + *source_population as u64,
                source_population: *source_population,
                admitted: *admitted,
                conserved: *admitted,
                lost: 0,
                fabricated: 0,
                agreement_micros: Some(1_000_000),
                low_micros: Some(900_000),
                high_micros: Some(1_000_000),
            },
        )
        .collect();

    let metrics = [AGREEMENT_METRIC, UNCERTAINTY_METRIC];
    let verdicts: Vec<ExpectedVerdict> = expected_documents
        .iter()
        .flat_map(|document| {
            metrics.iter().map(|metric| ExpectedVerdict {
                document_id: document.document_id.clone(),
                metric: (*metric).to_string(),
                verdict: "PASS".to_string(),
            })
        })
        .collect();

    let expected = ExpectedProbe {
        identity_digest: "sha256:synthetic".to_string(),
        source_manifest_sha256: "sha256:synthetic".to_string(),
        external_execution_status: "not_executed".to_string(),
        external_bytes: "bytes_absent".to_string(),
        latency_source: LATENCY_SOURCE_REF.to_string(),
        release_label: RELEASE_LABEL.to_string(),
        release_profile: "structural".to_string(),
        release_consolidation: "consolidation=synthetic".to_string(),
        captured_at_revision: "git:synthetic".to_string(),
        cost_currency: "USD".to_string(),
        frozen_documents: frozen_count,
        documents: expected_documents.clone(),
        exceptions: exceptions.clone(),
        verdicts,
        aggregate: Aggregate {
            documents: document_count,
            source_population: expected_documents
                .iter()
                .map(|document| document.source_population)
                .sum(),
            admitted: expected_documents
                .iter()
                .map(|document| document.admitted)
                .sum(),
            tp: expected_documents
                .iter()
                .map(|document| document.conserved)
                .sum(),
            fn_: 0,
            fp: 0,
            agreeing_documents: document_count,
            margin_pass: document_count * metrics.len(),
            margin_fail: 0,
            no_denominator_documents: 0,
        },
        canonical_catalog_present: 0,
        sources_archive_present: frozen_count,
        gold_standard_present: 0,
    };

    let outcomes: Vec<Json> = expected_documents
        .iter()
        .map(|document| {
            json!({
                "document_id": document.document_id,
                "state": "evaluated",
                "identity_sha256": document.identity_sha256,
                "source_population": document.source_population,
                "admitted": document.admitted,
                "conserved": document.conserved,
                "lost": document.lost,
                "fabricated": document.fabricated,
                "agreement_ratio_micros": document.agreement_micros,
                "uncertainty_low": document.low_micros,
                "uncertainty_high": document.high_micros,
                "cost_lower_bound": Json::Null,
                "cost_upper_bound": Json::Null,
                "latency_ms": document.latency_ms,
                "latency_source": LATENCY_SOURCE_REF,
                "margins": metrics.iter().map(|metric| json!({
                    "metric": metric,
                    "direction": if *metric == AGREEMENT_METRIC { "at_least" } else { "at_most" },
                    "threshold_micros": 900_000,
                    "value_micros": 1_000_000,
                    "verdict": "PASS",
                })).collect::<Vec<Json>>(),
                "typed_gaps": [],
            })
        })
        .collect();

    let report = json!({
        "schema_version": REPORT_SCHEMA,
        "reference_population": {
            "role": "identity_only",
            "identity": POPULATION_IDENTITY,
            "identity_sha256": expected.identity_digest,
            "source_manifest_sha256": expected.source_manifest_sha256,
            "documents": frozen_count,
            "measurable_documents": document_count,
            "excluded_documents": exception_count,
            "exceptions": exceptions.iter().map(|exception| json!({
                "document_id": exception.document_id,
                "identity_sha256": exception.identity_sha256,
                "size_bytes": exception.size_bytes,
                "reason": exception.reason,
            })).collect::<Vec<Json>>(),
            "reference_release": {
                "label": expected.release_label,
                "profile": expected.release_profile,
                "consolidation": expected.release_consolidation,
                "captured_at_revision": expected.captured_at_revision,
            },
            "byte_pins": expected_documents.iter().map(|document| json!({
                "document_id": document.document_id,
                "identity_sha256": document.identity_sha256,
                "size_bytes": document.size_bytes,
            })).chain(exceptions.iter().map(|exception| json!({
                "document_id": exception.document_id,
                "identity_sha256": exception.identity_sha256,
                "size_bytes": exception.size_bytes,
            }))).collect::<Vec<Json>>(),
        },
        "executed_population": {
            "corpus_execution": true,
            "execution_evidence": execution_receipt(POPULATION_IDENTITY, document_count as u64),
            "documents": document_count,
            "accepted": document_count,
        },
        "evaluated_population": {
            "status": "evaluated",
            "documents": document_count,
            "aggregate": {
                "documents": expected.aggregate.documents,
                "source_population": expected.aggregate.source_population,
                "admitted": expected.aggregate.admitted,
                "tp": expected.aggregate.tp,
                "fn": expected.aggregate.fn_,
                "fp": expected.aggregate.fp,
                "agreeing_documents": expected.aggregate.agreeing_documents,
                "margin_pass": expected.aggregate.margin_pass,
                "margin_fail": expected.aggregate.margin_fail,
                "no_denominator_documents": expected.aggregate.no_denominator_documents,
                "counts_only": true,
            },
            "class_parity": {
                "aggregate": {
                    "denominator": expected.aggregate.source_population,
                    "tp": expected.aggregate.tp,
                    "fn": expected.aggregate.source_population - expected.aggregate.tp,
                    "fp": expected.aggregate.fp,
                    "native_capability_recall_micros": Json::Null,
                    "covered_union_recall_micros": 1_000_000,
                    "precision_micros": 1_000_000,
                    "f1_micros": Json::Null,
                },
                "producer": CLASS_PARITY_PRODUCER,
                "authority": CLASS_PARITY_AUTHORITY,
                "claim_scope": "local_proxy_only",
            },
        },
        "outcomes": outcomes,
        "class_profiles": expected_documents.iter().map(|document| {
            let recall = ratio(document.conserved, document.source_population);
            let precision = ratio(document.conserved, document.conserved + document.fabricated);
            json!({
                "document_id": document.document_id,
                "identity_sha256": document.identity_sha256,
                "classes": [{
                    "class": CLASS_HEADER_TITLE,
                    "source_population": document.source_population,
                    "admitted": document.admitted,
                    "conserved": document.conserved,
                    "lost": document.lost,
                    "fabricated": document.fabricated,
                    "recall_java_micros": micros_of(recall),
                    "precision_micros": micros_of(precision),
                    "f1_micros": micros_of(harmonic_mean(precision, recall)),
                    // The synthetic class is declared source-consumed, so the
                    // synthetic parity aggregate exercises the covered-union
                    // path exactly as the real DATA does.
                    "source_consumed": true,
                }],
            })
        }).collect::<Vec<Json>>(),
        "class_narrowings": expected_documents.iter()
            .filter(|document| document.admitted < document.source_population)
            .map(|document| json!({
                "document_id": document.document_id,
                "class": CLASS_HEADER_TITLE,
                "source_population": document.source_population,
                "admitted": document.admitted,
            })).collect::<Vec<Json>>(),
        "production_catalog_overlap": {
            "census_kind": CENSUS_KIND,
            "documents": frozen_count,
            "canonical_catalog_present_count": expected.canonical_catalog_present,
            "sources_archive_present_count": expected.sources_archive_present,
            "gold_standard_present_count": expected.gold_standard_present,
            "locators": (0..document_count).map(|index| json!({"document_id": format!("d{}", index + 1)}))
                .chain((0..exception_count).map(|index| json!({"document_id": format!("x{}", index + 1)})))
                .collect::<Vec<Json>>(),
        },
        "cost_accounting": {
            "status": "bounded_not_measured",
            "currency": expected.cost_currency,
            "cost_lower_bound": Json::Null,
            "cost_upper_bound": Json::Null,
            "unknown_is_not_zero": true,
            "pooled_cost_claim": "forbidden",
        },
        "external_holdout_band": {
            "execution_status": expected.external_execution_status,
            "external_bytes": expected.external_bytes,
        },
        "policy_locks": {
            "import_eligible": false,
            "auto_promote": "forbidden",
            "deployed": false,
            "production_route": "forbidden",
            "graph_write": "forbidden",
            "pooled_claim": "forbidden",
        },
        "redaction": REDACTION_FLAGS.iter().map(|flag| ((*flag).to_string(), json!(false))).collect::<serde_json::Map<String, Json>>(),
        "named_limitations": ["synthetic self-check report"],
    });

    (report, expected)
}

/// One mutant per declared guard, each violating exactly one declared rule.
fn mutants(report: &Json, document_count: usize) -> Vec<(&'static str, Json)> {
    let mut out: Vec<(&'static str, Json)> = Vec::new();

    let mut mutate = |label: &'static str, change: &dyn Fn(&mut Json)| {
        let mut clone = report.clone();
        change(&mut clone);
        out.push((label, clone));
    };

    mutate("schema_version_drift", &|clone| {
        clone["schema_version"] = json!("external-holdout-release-probe.v0");
    });
    mutate("reference_population_role", &|clone| {
        clone["reference_population"]["role"] = json!("identity_and_execution");
    });
    mutate("reference_documents", &|clone| {
        clone["reference_population"]["documents"] = json!(1);
    });
    mutate("reference_identity_missing", &|clone| {
        clone["reference_population"]["identity"] = json!("");
    });
    mutate("reference_identity_digest_drift", &|clone| {
        clone["reference_population"]["identity_sha256"] = json!("sha256:drift");
    });
    mutate("source_manifest_sha_drift", &|clone| {
        clone["reference_population"]["source_manifest_sha256"] = json!("sha256:drift");
    });
    mutate("reference_byte_pins_drift", &|clone| {
        clone["reference_population"]["byte_pins"][0]["size_bytes"] = json!(1);
    });
    mutate("outcome_count", &|clone| {
        clone["outcomes"].as_array_mut().expect("outcomes").pop();
    });
    mutate("identity_sha_collapse", &|clone| {
        let first = clone["outcomes"][0]["identity_sha256"].clone();
        clone["outcomes"][1]["identity_sha256"] = first;
    });
    mutate("identity_pin_drift", &|clone| {
        clone["outcomes"][0]["identity_sha256"] = json!("0".repeat(64));
    });
    mutate("executed_population_not_executed", &|clone| {
        clone["executed_population"]["corpus_execution"] = json!(false);
    });
    mutate("execution_evidence_unbound", &|clone| {
        clone["executed_population"]["execution_evidence"] = json!("n/a");
    });
    // A receipt spliced in from another report (identity drift) must redden the
    // same guard: the dict branch is exercised, not only the string branch.
    mutate("execution_evidence_unbound", &|clone| {
        clone["executed_population"]["execution_evidence"]["population_identity"] =
            json!("some_other_report");
    });
    // The receipt's own fields are independently required: an undeclared kind, a
    // missing kind and a document count that disagrees with the executed
    // population each redden the same guard, so removing one check is not hidden
    // by the identity mutant above.
    mutate("execution_evidence_unbound", &|clone| {
        clone["executed_population"]["execution_evidence"]["kind"] = json!("undeclared_kind");
    });
    mutate("execution_evidence_unbound", &|clone| {
        clone["executed_population"]["execution_evidence"]
            .as_object_mut()
            .expect("the baseline receipt is an object")
            .remove("kind");
    });
    mutate("execution_evidence_unbound", &|clone| {
        clone["executed_population"]["execution_evidence"]["documents"] =
            json!(document_count as u64 + 1);
    });
    mutate("accepted_documents", &|clone| {
        clone["executed_population"]["accepted"] = json!(1);
    });
    mutate("evaluated_status", &|clone| {
        clone["evaluated_population"]["status"] = json!("not_evaluated");
    });
    mutate("aggregate_zero_admitted_population", &|clone| {
        clone["evaluated_population"]["aggregate"]["admitted"] = json!(0);
    });
    mutate("aggregate_conservation_mismatch", &|clone| {
        clone["evaluated_population"]["aggregate"]["tp"] = json!(1);
    });
    mutate("margin_verdict_absent", &|clone| {
        clone["outcomes"][0]["margins"]
            .as_array_mut()
            .expect("margins")
            .clear();
    });
    mutate("margin_verdict_refitted", &|clone| {
        clone["outcomes"][0]["margins"][0]["verdict"] = json!("FAIL");
    });
    mutate("latency_source_substituted", &|clone| {
        clone["outcomes"][0]["latency_source"] = json!("std::time::Instant::now()");
    });
    mutate("latency_unmeasured", &|clone| {
        clone["outcomes"][0]["latency_ms"] = json!(0);
    });
    mutate("legacy_block_present", &|clone| {
        clone["denominators"] = json!({"documents": document_count});
    });
    mutate("external_lane_state_flipped", &|clone| {
        clone["external_holdout_band"]["execution_status"] = json!("executed");
    });
    mutate("policy_lock_open", &|clone| {
        clone["policy_locks"]["import_eligible"] = json!(true);
    });
    mutate("redaction_flag_raised", &|clone| {
        clone["redaction"]["raw_tei"] = json!(true);
    });
    mutate("redaction_scope_replaced", &|clone| {
        clone["redaction"]["tei_xml"] = json!(false);
    });
    mutate("raw_payload_leak", &|clone| {
        clone["outcomes"][0]["contact"] = json!("someone@example.org");
    });
    mutate("cost_unknown_read_as_zero", &|clone| {
        clone["cost_accounting"]["cost_lower_bound"] = json!(0);
    });
    mutate("production_catalog_overlap_absent", &|clone| {
        clone["production_catalog_overlap"]["locators"] = json!([]);
    });
    mutate("named_limitations_missing", &|clone| {
        clone["named_limitations"] = json!([]);
    });
    mutate("class_narrowings_incomplete", &|clone| {
        clone["class_narrowings"] = json!([]);
    });
    mutate("reference_measurable_documents", &|clone| {
        clone["reference_population"]["measurable_documents"] = json!(0);
    });
    mutate("reference_exceptions_drift", &|clone| {
        clone["reference_population"]["exceptions"][0]["reason"] = json!("unlisted_reason");
    });
    mutate("class_parity_missing", &|clone| {
        clone["evaluated_population"]
            .as_object_mut()
            .expect("evaluated_population")
            .remove("class_parity");
    });
    mutate("class_parity_denominator_drift", &|clone| {
        // Collapsing the Java denominator to the admitted band would
        // manufacture a recall of 1.0; the guard refuses the drift by name
        // instead of reading the collapsed value as agreement.
        clone["evaluated_population"]["class_parity"]["aggregate"]["denominator"] =
            clone["evaluated_population"]["aggregate"]["admitted"].clone();
    });
    mutate("class_parity_precision_inconsistent", &|clone| {
        clone["evaluated_population"]["class_parity"]["aggregate"]["fp"] = json!(1);
    });
    mutate("class_parity_conservation_mismatch", &|clone| {
        // Reclassify a source-consumed class as native: the admitted band no
        // longer covers it, so the native and union recalls move (false
        // negatives) while the declared block does not.
        clone["class_profiles"][0]["classes"][0]["source_consumed"] = json!(false);
    });

    out
}

/// The synthetic exception population.  The frozen population is always
/// `measured + exceptions`, so the guard matrix proves the partition at a fixed
/// exception count while `--documents` supplies the measured side (the N/N+2
/// scalability proof).  There is no compiled-in document count: the population is
/// DATA, supplied per invocation, so no literal can drift from the manifest.
const SYNTHETIC_EXCEPTION_COUNT: usize = 2;
const SYNTHETIC_EXCEPTION_REASON: &str = "synthetic_exclusion";

/// The hermetic guard matrix: the unmutated synthetic report must be clean, and
/// each declared guard must fire on its own mutant.  No repo read, no network,
/// no write, no log surface.
fn self_check(document_count: usize) -> bool {
    if document_count == 0 {
        // A zero-document population cannot exercise a population guard: refuse
        // rather than report a vacuous matrix.
        println!("guard self-check: a zero-document population is not a probe");
        return false;
    }
    // Record the exercised population first: a failing run must still say which
    // population failed.
    println!("guard self-check population: {document_count} documents");
    let (valid, expected) = synthetic_pair(document_count, SYNTHETIC_EXCEPTION_COUNT);
    let baseline = report_errors(&valid, &expected);
    if !baseline.is_empty() {
        println!("guard baseline: rejected {baseline:?}");
        return false;
    }
    let mut ok = true;
    let mut fired: BTreeSet<&str> = BTreeSet::new();
    for (label, mutant) in mutants(&valid, document_count) {
        let errors = report_errors(&mutant, &expected);
        // A guard may suffix its label with the offending key (`label:key`), so a
        // mutant is caught when the exact label or a `label:`-prefixed refusal
        // appears.
        let prefix = format!("{label}:");
        let caught = errors
            .iter()
            .any(|error| error == label || error.starts_with(&prefix));
        println!(
            "guard {label}: {}",
            if caught { "rejected" } else { "ACCEPTED" }
        );
        if !caught {
            ok = false;
        }
        fired.insert(label);
    }
    // A declared guard that no mutant exercises is not proven: a missing case is a
    // gap in the matrix, not a pass.
    for label in GUARD_LABELS {
        if !fired.contains(label) {
            println!("guard {label}: NOT EXERCISED");
            ok = false;
        }
    }
    if ok {
        println!(
            "guard self-check: {} guards rejected their violating input",
            GUARD_LABELS.len()
        );
    }
    ok
}

// ────────────────────────────────── main ───────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    about = "Frozen external-holdout release probe runtime artifact (S08/T04)",
    long_about = None
)]
struct Args {
    /// Write the canonical artifact to this path (required for a canonical run).
    #[arg(long, value_name = "PATH")]
    report: Option<PathBuf>,
    /// Run the hermetic guard self-check (no repo read, no network, no write).
    #[arg(long)]
    self_check: bool,
    /// The population the hermetic guard self-check exercises.  DATA, never a
    /// compiled-in constant: the same binary proves the guard matrix at any N.
    /// Required together with `--self-check`; refused without it.
    #[arg(long, value_name = "N")]
    documents: Option<usize>,
    /// Print the per-document profile and the aggregate to stdout.
    #[arg(long)]
    print: bool,
    /// Write the recorded measurement tables to this path.  The tables are a
    /// projection of the canonical run, so `--report` is required: a recorded
    /// measurement is never written without the artifact it describes.
    #[arg(long, value_name = "PATH")]
    emit_frozen_tables: Option<PathBuf>,
}

fn run(args: &Args) -> anyhow::Result<()> {
    let manifest_text =
        read_repo_file(MANIFEST_REL).map_err(|reason| anyhow::anyhow!("FATAL {PROBE} {reason}"))?;
    let manifest: Yaml = serde_yaml::from_str(&manifest_text).map_err(|error| {
        anyhow::anyhow!("FATAL {PROBE} {MANIFEST_REL} is not valid YAML: {error}")
    })?;
    let lane = read_lane(&manifest).map_err(|label| {
        anyhow::anyhow!("FATAL {PROBE} the frozen release_probe lane is refused: {label}")
    })?;
    let provenance = load_provenance();
    verify_provenance(&lane, &provenance)
        .map_err(|label| anyhow::anyhow!("FATAL {PROBE} {label}"))?;

    let source_manifest_text = read_repo_file(&lane.source_manifest)
        .map_err(|reason| anyhow::anyhow!("FATAL {PROBE} {reason}"))?;
    let source_manifest: Yaml = serde_yaml::from_str(&source_manifest_text).map_err(|error| {
        anyhow::anyhow!(
            "FATAL {PROBE} {} is not valid YAML: {error}",
            lane.source_manifest
        )
    })?;

    let measured = measure_all(&lane, &provenance);
    let census = catalog_census(&lane, &source_manifest);
    let expected = expected_probe(&lane, &provenance, &measured, &census);
    let report = build_report(&lane, &provenance, &measured, &census);

    if args.print {
        for document in &measured {
            println!(
                "PROBE DOC {} identity_sha256={} source_population={} admitted={} conserved={} \
                 lost={} fabricated={} agreement_micros={} uncertainty_low={} uncertainty_high={} \
                 uncertainty_width_micros={} latency_ms={} latency_source={}",
                document.document_id,
                document.identity_sha256,
                document.metrics.source_population,
                document.metrics.admitted,
                document.metrics.conserved,
                document.metrics.lost,
                document.metrics.fabricated,
                document
                    .metrics
                    .agreement_micros()
                    .map_or("null".to_string(), |value| value.to_string()),
                document
                    .metrics
                    .low_micros()
                    .map_or("null".to_string(), |value| value.to_string()),
                document
                    .metrics
                    .high_micros()
                    .map_or("null".to_string(), |value| value.to_string()),
                document
                    .metrics
                    .uncertainty_width
                    .map_or("null".to_string(), |value| micros(value).to_string()),
                document.latency_ms,
                LATENCY_SOURCE_REF,
            );
            for verdict in &document.verdicts {
                println!(
                    "MARGIN {} {} -> {} value={} threshold={} direction={}",
                    document.document_id,
                    verdict.metric,
                    verdict.verdict.token(),
                    verdict
                        .value
                        .map_or("no_denominator".to_string(), |value| format!("{value:.6}")),
                    verdict.threshold,
                    verdict.direction.token(),
                );
            }
            for gap in &document.admitted_band.gaps {
                println!(
                    "PROBE GAP {} {}: {}/{} admitted — {}",
                    document.document_id,
                    gap.class,
                    gap.admitted,
                    gap.source_population,
                    gap.reason
                );
            }
        }
        println!(
            "PROBE FROZEN frozen_documents={} measurable_documents={} excluded_documents={} probe_identity={} source_manifest_sha256={}",
            lane.frozen_documents(),
            measured.len(),
            lane.exceptions.len(),
            lane.identity_digest,
            lane.source_manifest_sha256
        );
        for row in &lane.exceptions {
            println!("PROBE EXCLUDED {} reason={}", row.article_id, row.reason);
        }
        println!(
            "PROBE CENSUS canonical_catalog={} sources_archive={} gold_standard={} absent={} declared={}",
            census.canonical_catalog_present,
            census.sources_archive_present,
            census.gold_standard_present,
            census.absent_from_every_locator,
            census.declared_pdf_present
        );
        println!("{}", expected.aggregate.render());
    }

    // Fail closed before writing: the artifact is only ever written when it
    // passes its own declared guard list.
    let errors = report_errors(&report, &expected);
    if !errors.is_empty() {
        anyhow::bail!("FATAL {PROBE} the assembled artifact failed its guards: {errors:?}");
    }

    let Some(path) = args.report.as_ref() else {
        anyhow::bail!(
            "--report <path> is required for the canonical run (the artifact is never written implicitly)"
        );
    };
    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');
    std::fs::write(path, &bytes)?;
    println!(
        "PROBE ARTIFACT wrote {} ({} bytes, documents={}, admitted={})",
        path.display(),
        bytes.len(),
        expected.aggregate.documents,
        expected.aggregate.admitted
    );

    // The recorded tables are emitted from the same validated run, and always
    // after the artifact they project: `--emit-frozen-tables` cannot be used to
    // record a measurement the canonical artifact does not carry (`main` refuses
    // the run without `--report`, so this branch always has a written artifact).
    if let Some(tables_path) = args.emit_frozen_tables.as_ref() {
        let artifact_sha256 = format!("{:x}", Sha256::digest(&bytes));
        let tables = build_frozen_tables(&report, &artifact_sha256);
        let mut tables_bytes = serde_json::to_vec_pretty(&tables)?;
        tables_bytes.push(b'\n');
        std::fs::write(tables_path, &tables_bytes)?;
        println!(
            "PROBE FROZEN TABLES wrote {} ({} bytes, documents={}, artifact_sha256={})",
            tables_path.display(),
            tables_bytes.len(),
            report["outcomes"].as_array().map(Vec::len).unwrap_or(0),
            artifact_sha256
        );
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // Composition root: `.env`-backed rolling JSON logging (ADR-052), held for the
    // whole process.  `--self-check` is a pure predicate matrix with no repo,
    // network or log surface, so it deliberately runs before the root is opened.
    if args.self_check {
        // The population is DATA, never a compiled-in constant: a bare
        // `--self-check` has no population to exercise and is refused by name.
        let Some(document_count) = args.documents else {
            eprintln!(
                "--self-check requires --documents <N>: the population is DATA, never a compiled-in constant"
            );
            std::process::exit(2);
        };
        return if self_check(document_count) {
            Ok(())
        } else {
            std::process::exit(1)
        };
    }
    if args.documents.is_some() {
        eprintln!("--documents is only meaningful together with --self-check");
        std::process::exit(2);
    }
    if args.report.is_none() {
        eprintln!(
            "--report <path> is required for the canonical run (the artifact is never written implicitly)"
        );
        std::process::exit(2);
    }
    let _log_guard = da_cli::logging::init_pipeline_logging("external_holdout_release_probe");
    run(&args)
}
