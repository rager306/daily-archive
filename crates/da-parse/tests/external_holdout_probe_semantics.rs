//! M511-j7wz2f / S08 / T03 — the independent per-document oracle of the FROZEN
//! external holdout probe (H20 / GAR-015).
//!
//! S08/T02 froze a fresh live-Java `processFulltextDocument` reference for the
//! sha256-pinned, non-gold arXiv documents declared by the source manifest and
//! recorded its byte pins in the
//! `external_holdout_contract.release_probe` lane of
//! `data/parse/acceptance_manifest.yaml`.  This file is the independent
//! measurement over that frozen reference.  S05/T01 is the pattern donor: the
//! admitted band is assembled FROM the document through the public typed parsers,
//! the only SUT this file reaches is `compose_native_fulltext`, and the emission
//! is re-read with an independent XML parser (`roxmltree`, a dev-dependency the
//! composer's import root set cannot reach).
//!
//! ## Measured scope (read this before quoting any number)
//!
//! A small, sha256-pinned, non-gold local holdout (five documents in the recorded
//! measurement).  This is NOT a statistical corpus: every number below is a COUNT
//! over the measured documents, never a corpus rate, and
//! the probe makes no pooled claim (`claim_scope = local_proxy_only`).  The
//! reported per-document quantities are the frozen probe result for that
//! document, not an estimate of any population.
//!
//! ## What is measured, per document
//!
//! | counter | meaning |
//! |---|---|
//! | `source_population` | independent `roxmltree` count of the carrier elements the frozen reference declares |
//! | `admitted` | occurrences the admitted band really carried (a parser rejection is a **named** `AdmissionGap`, never a silent narrowing) |
//! | `conserved` | admitted occurrences the emitted band carries, net of the occurrences the conserved source subtree already contributes |
//! | `lost` | `admitted - conserved` (a false negative) |
//! | `fabricated` | emitted band carriers matching neither an admitted nor an independently measured source value (a false positive) |
//! | `admission_coverage` | `admitted / source_population`, or `no_denominator` when the source declares no such carrier |
//!
//! and the two DECLARED margin metrics, computed here exactly as the lane's
//! definitions declare (the lane carries the definitions; `acceptance_margins`
//! carries the thresholds, by `margin_ref` — this file holds no second numeric
//! dictionary):
//!
//! | metric | per-document value |
//! |---|---|
//! | `document_agreement_ratio` | `conserved / admitted` over the document's admitted evaluated population; `no_denominator` when that population is empty (MEM1702: a class without a denominator is never a vacuum `1.0`) |
//! | `document_uncertainty_interval_width` | the width of the document's bounded uncertainty interval: the 95% Wilson score interval for `conserved / admitted` with `n` = the document's admitted (narrowed) population, bounded to `[0, 1]`, document-derived and never constant |
//!
//! Both are reported per document and compared with the declared margins
//! (`MARGIN <article_id> <metric> -> PASS|FAIL|no_denominator`).  A FAIL is
//! reported and frozen, never re-fitted.
//!
//! ## Preconditions are FATAL, not advisory
//!
//! Before any measurement the lane is read and structurally validated, every
//! declared metric definition is checked against its declared contract, the
//! identity digest is RECOMPUTED from the frozen `holdout_manifests.yaml` by the
//! declared canonicalization handles, and every frozen TEI is re-hashed on disk.
//! A missing lane, a drifted digest, a renamed metric, a duplicated threshold, a
//! path escape or a drifted fixture byte is a NAMED failure — never a silently
//! different number.
//!
//! ## Independence guard
//!
//! The admitted band is assembled from document-derived values; no upstream
//! composition helper is reachable from this file, and
//! [`expected_side_does_not_call_a_composition_entry_point`] enforces that by
//! scanning this file's own source.  The judge of the emission is the independent
//! parser alone: the composer's manifest ledger is cross-checked *against* the
//! oracle, never used as its input.

use da_parse::*;
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

/// Every failure message carries this prefix so an inspection never guesses.
const PROBE: &str = "[external-holdout-probe-semantics]";

// The probe population is NOT a literal here: it is the number of article
// identities declared by `data/parse/holdout_manifests.yaml#articles`, carried on
// `ProbeData::expected_documents`.  A lane whose `reference_tei` does not cover
// exactly those identities is refused by name
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

const MANIFEST_REL: &str = "data/parse/acceptance_manifest.yaml";
const FIXTURE_DIR: &str = "crates/da-parse/tests/fixtures/holdout";

/// The z of the 95% Wilson score interval.  Not a margin.
const WILSON_Z: f64 = 1.96;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn manifest_value() -> Value {
    let text = std::fs::read_to_string(repo_root().join(MANIFEST_REL))
        .unwrap_or_else(|error| panic!("FATAL {PROBE} cannot read `{MANIFEST_REL}`: {error}"));
    serde_yaml::from_str(&text)
        .unwrap_or_else(|error| panic!("FATAL {PROBE} `{MANIFEST_REL}` is not valid YAML: {error}"))
}

// ─────────────────────── the DATA lane (definitions live here) ───────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    AtLeast,
    AtMost,
}

impl Direction {
    fn token(self) -> &'static str {
        match self {
            Direction::AtLeast => "at_least",
            Direction::AtMost => "at_most",
        }
    }
}

/// One declared acceptance margin, read from `acceptance_margins` by `margin_ref`.
#[derive(Debug, Clone, PartialEq)]
struct Margin {
    metric: String,
    direction: Direction,
    value: f64,
}

/// One declared metric definition.  The definitions are DATA; this file never
/// re-states them as literals and never carries a second numeric dictionary.
#[derive(Debug, Clone, PartialEq)]
struct MetricDefinition {
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
struct ReferenceRow {
    article_id: String,
    declared_sha256: String,
    path: PathBuf,
    size_bytes: u64,
}

/// The validated lane plus the identity surfaces the measurement needs.
#[derive(Debug, Clone)]
struct ProbeData {
    reference: Vec<ReferenceRow>,
    metrics: Vec<MetricDefinition>,
    margins: Vec<Margin>,
    identity_digest: String,
    source_manifest: String,
    /// The declared probe population, derived from
    /// `data/parse/holdout_manifests.yaml#articles` (never a literal).  The frozen
    /// reference must cover exactly this many identities.
    expected_documents: usize,
    profile_fingerprint: String,
    config_fingerprint: String,
}

impl ProbeData {
    fn metric(&self, name: &str) -> Option<&MetricDefinition> {
        self.metrics.iter().find(|metric| metric.metric == name)
    }

    fn margin(&self, name: &str) -> Option<&Margin> {
        self.margins.iter().find(|margin| margin.metric == name)
    }
}

fn scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
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
fn recompute_identity_digest(lane: &Value) -> Result<String, String> {
    let source_manifest = lane
        .get("source_manifest")
        .and_then(Value::as_str)
        .ok_or("probe_identity_source_missing")?;
    let fields: Vec<String> = lane
        .get("digest_input_fields")
        .and_then(Value::as_sequence)
        .ok_or("probe_identity_source_missing")?
        .iter()
        .map(|field| {
            field
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "probe_identity_source_missing".to_string())
        })
        .collect::<Result<_, _>>()?;
    if lane.get("digest_input_order").and_then(Value::as_str) != Some("as_listed") {
        return Err("probe_identity_not_frozen".into());
    }
    let field_separator = lane
        .get("digest_field_separator")
        .and_then(Value::as_str)
        .ok_or("probe_identity_source_missing")?;
    let record_separator = lane
        .get("digest_record_separator")
        .and_then(Value::as_str)
        .ok_or("probe_identity_source_missing")?;
    let text = std::fs::read_to_string(repo_root().join(source_manifest))
        .map_err(|_| format!("probe_identity_source_missing:{source_manifest}"))?;
    let source: Value = serde_yaml::from_str(&text)
        .map_err(|_| format!("probe_identity_source_unparsable:{source_manifest}"))?;
    let articles = source
        .get("articles")
        .and_then(Value::as_sequence)
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

/// Read and validate the frozen lane.  Every refusal is a NAMED label; the
/// caller turns an `Err` into a FATAL failure.  Nothing here measures anything:
/// a mutated definition can only change the label, never silently change a
/// number.
fn read_probe_data(manifest: &Value) -> Result<ProbeData, String> {
    let contract = manifest
        .get("external_holdout_contract")
        .ok_or("probe_lane_missing")?;
    let lane = contract.get("release_probe").ok_or("probe_lane_missing")?;

    // The external `bytes_absent` band must be untouched: the probe lane is
    // additive and never lifts it.
    if contract.get("execution_status").and_then(Value::as_str) != Some("not_executed") {
        return Err("external_lane_state_flipped".into());
    }
    if contract
        .get("byte_policy")
        .and_then(|policy| policy.get("external_bytes"))
        .and_then(Value::as_str)
        != Some("bytes_absent")
    {
        return Err("external_lane_state_flipped".into());
    }

    // The thresholds, read from the referenced block.  No second dictionary.
    let margins_value = contract
        .get(MARGIN_REF_KEY)
        .and_then(|margins| margins.get("margins"))
        .and_then(Value::as_sequence)
        .ok_or("probe_margin_ref_missing")?;
    let mut margins = Vec::with_capacity(margins_value.len());
    for entry in margins_value {
        let metric = entry
            .get("metric")
            .and_then(Value::as_str)
            .ok_or("probe_margin_metric_missing")?
            .to_string();
        let direction = match entry.get("direction").and_then(Value::as_str) {
            Some("at_least") => Direction::AtLeast,
            Some("at_most") => Direction::AtMost,
            _ => return Err("probe_margin_direction_drift".into()),
        };
        let value = entry
            .get("margin")
            .and_then(Value::as_f64)
            .ok_or("probe_margin_value_missing")?;
        margins.push(Margin {
            metric,
            direction,
            value,
        });
    }
    let margin_names: BTreeSet<String> =
        margins.iter().map(|margin| margin.metric.clone()).collect();

    // The metric definitions, validated against the declared contract.  The
    // definitions are DATA; this file binds behaviour to the declared names and
    // refuses any definition that does not carry the declared structure.
    let metrics_value = lane
        .get("metrics")
        .and_then(Value::as_sequence)
        .ok_or("probe_metrics_missing")?;
    let mut metrics = Vec::with_capacity(metrics_value.len());
    let mut metric_names = BTreeSet::new();
    for entry in metrics_value {
        let metric = entry
            .get("metric")
            .and_then(Value::as_str)
            .ok_or("probe_metric_name_missing")?
            .to_string();
        if !margin_names.contains(&metric) {
            return Err("probe_metric_vocabulary_drift".into());
        }
        let non_empty = |key: &str| {
            entry
                .get(key)
                .and_then(Value::as_str)
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
        metrics.push(MetricDefinition {
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
        .and_then(Value::as_sequence)
        .ok_or("probe_reference_tei_missing")?;
    let mut reference = Vec::with_capacity(reference_value.len());
    for row in reference_value {
        let article_id = row
            .get("article_id")
            .and_then(Value::as_str)
            .ok_or("probe_reference_row_missing")?
            .to_string();
        let declared_sha256 = row
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or("probe_reference_row_missing")?
            .to_string();
        let path_str = row
            .get("path")
            .and_then(Value::as_str)
            .ok_or("probe_reference_row_missing")?;
        let size_bytes = row
            .get("size_bytes")
            .and_then(Value::as_u64)
            .ok_or("probe_reference_row_missing")?;
        reference.push(ReferenceRow {
            article_id,
            declared_sha256,
            path: check_reference_path(path_str)?,
            size_bytes,
        });
    }

    // The frozen identity must recompute from the source manifest.
    let identity_digest = recompute_identity_digest(lane)?;
    let declared_digest = lane
        .get("identity_digest")
        .and_then(Value::as_str)
        .ok_or("probe_identity_source_missing")?;
    if identity_digest != declared_digest {
        return Err("probe_identity_digest_drift".into());
    }

    // The lane's byte pins must agree with the manifest they claim to derive from.
    let source_manifest = lane
        .get("source_manifest")
        .and_then(Value::as_str)
        .ok_or("probe_identity_source_missing")?
        .to_string();
    let source_text = std::fs::read_to_string(repo_root().join(&source_manifest))
        .map_err(|_| format!("probe_identity_source_missing:{source_manifest}"))?;
    let source: Value = serde_yaml::from_str(&source_text)
        .map_err(|_| format!("probe_identity_source_unparsable:{source_manifest}"))?;
    let articles = source
        .get("articles")
        .and_then(Value::as_sequence)
        .ok_or("probe_identity_source_missing")?;
    // The lane covers exactly the frozen source manifest's article identities.
    // The lane's `reference_tei[].sha256` is the byte pin of the committed TEI,
    // while `holdout_manifests.yaml` pins the source PDF: the two hashes are
    // deliberately different objects, so the cross-check is on identity, and the
    // byte pins are re-verified against the files themselves before measurement.
    // The population size is DATA, not a literal: the lane's frozen rows must
    // cover exactly the article identities the source manifest declares, and the
    // expected document count carried forward is that declared count.  A lane
    // covering a different number of documents is refused by name here, before
    // any measurement, rather than audited against a stale table.
    if reference.len() != articles.len() {
        return Err("probe_reference_population_size_drift".into());
    }
    let expected_documents = articles.len();
    let manifest_ids: BTreeSet<String> = articles
        .iter()
        .filter_map(|article| article.get("article_id").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    let reference_ids: BTreeSet<String> =
        reference.iter().map(|row| row.article_id.clone()).collect();
    if reference_ids != manifest_ids {
        return Err("probe_reference_source_mismatch".into());
    }

    // The reference identity (profile + consolidation flags) is part of the
    // composer identity, so a drift here must be a refusal, not a silent change.
    let reference_profile = lane
        .get("reference_profile")
        .and_then(Value::as_str)
        .ok_or("probe_reference_identity_missing")?;
    let flags = lane
        .get("reference_consolidation_flags")
        .and_then(Value::as_mapping)
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

    Ok(ProbeData {
        reference,
        metrics,
        margins,
        identity_digest,
        source_manifest,
        expected_documents,
        profile_fingerprint: format!("profile={reference_profile}"),
        config_fingerprint: format!("consolidation={}", flag_pairs.join(",")),
    })
}

fn load_probe_data() -> ProbeData {
    let manifest = manifest_value();
    read_probe_data(&manifest).unwrap_or_else(|label| {
        panic!("FATAL {PROBE} the frozen release_probe lane is refused: {label}")
    })
}

// ───────────────────────── the class vocabulary ─────────────────────────────

// The class tokens are derived from the production enum, from the composer's own
// declared reference-link derivation and from the composer's citation role
// mapping: none of these is a second literal copy, so the oracle and the
// composer gate cannot drift into near-miss spellings.
const CLASS_AUTHOR_LINE: &str = "AdmittedAuthorLine";
const CLASS_HEADER_TITLE: &str = "SemanticFieldLoss::HeaderTitle";
const CLASS_HEADER_EMAIL: &str = "SemanticFieldLoss::HeaderEmail";
const CLASS_HEADER_IDS: &str = "SemanticFieldLoss::HeaderIds";
const CLASS_AUTHOR_AFFILIATION: &str = "SemanticFieldLoss::AuthorAffiliation";
const CLASS_REFERENCE_DOMAIN: &str = NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION;
const CLASS_CITATION_DOI: &str = "CitationFieldRole::doi";
const CLASS_CITATION_ARXIV: &str = "CitationFieldRole::arxiv_id";
const CLASS_CITATION_URL: &str = "CitationFieldRole::url";
const CLASS_CITATION_ARTICLE_TITLE: &str = "CitationFieldRole::article_title";

const CLASS_COUNT: usize = 10;

const PROBE_CITATION_ROLES: [(CitationFieldRole, &str); 4] = [
    (CitationFieldRole::Doi, CLASS_CITATION_DOI),
    (CitationFieldRole::ArxivId, CLASS_CITATION_ARXIV),
    (CitationFieldRole::Url, CLASS_CITATION_URL),
    (
        CitationFieldRole::ArticleTitle,
        CLASS_CITATION_ARTICLE_TITLE,
    ),
];

fn citation_class_token(role: CitationFieldRole) -> &'static str {
    PROBE_CITATION_ROLES
        .iter()
        .find(|(candidate, _)| *candidate == role)
        .map(|(_, class)| *class)
        .expect("every measured citation role has a class token")
}

const CHECKSUM_IDNO_TYPES: [&str; 3] = ["MD5", "SHA1", "SHA256"];

fn is_checksum_idno(node: &roxmltree::Node<'_, '_>) -> bool {
    node.attribute("type")
        .is_some_and(|kind| CHECKSUM_IDNO_TYPES.contains(&kind))
}

// ─────────────────────────── independent XML readers ────────────────────────

const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

fn is_element(node: &roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name
}

fn normalized_text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn unique_element<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    name: &str,
) -> roxmltree::Node<'a, 'input> {
    let mut found = document.descendants().filter(|node| is_element(node, name));
    let first = found
        .next()
        .unwrap_or_else(|| panic!("{PROBE} the document declares no <{name}> element"));
    assert!(
        found.next().is_none(),
        "{PROBE} the independent scanner requires a unique <{name}> element"
    );
    first
}

fn unique_list_bibl<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> roxmltree::Node<'a, 'input> {
    unique_element(document, "listBibl")
}

fn xml_id_of<'a>(node: roxmltree::Node<'a, '_>) -> Option<&'a str> {
    node.attribute((XML_NAMESPACE, "id"))
}

fn direct_child<'a, 'input>(
    parent: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    parent.children().find(|child| is_element(child, name))
}

fn descendants_named<'a, 'input>(
    root: roxmltree::Node<'a, 'input>,
    name: &'static str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'input>> {
    root.descendants()
        .filter(move |node| is_element(node, name))
}

fn parse_document<'a>(fixture: &str, xml: &'a str) -> roxmltree::Document<'a> {
    roxmltree::Document::parse(xml)
        .unwrap_or_else(|error| panic!("{PROBE} {fixture}: the document does not parse: {error}"))
}

// ─────────────────── the independent source-side measurement ─────────────────

/// One real `<author>`/`<affiliation key>` binding, in document order.
struct AffiliationOccurrence {
    author_index: usize,
    key: String,
    text: String,
}

/// Everything the independent scanner reads from one frozen reference document.
/// These values are the *source population* side of the profile; nothing here is
/// produced by the composer.
struct SourceDocument {
    article_id: String,
    sha256: String,
    header_titles: Vec<String>,
    title_stmt_titles: Vec<String>,
    header_emails: Vec<String>,
    header_idnos: Vec<String>,
    header_pubnum_ids: Vec<String>,
    author_name_texts: Vec<String>,
    affiliation_occurrences: Vec<AffiliationOccurrence>,
    date_text: String,
    reference_texts: Vec<String>,
    citation_carriers: Vec<(CitationFieldRole, String)>,
}

fn measure_source(article_id: &str, declared_sha256: &str, xml: &str) -> SourceDocument {
    let document = parse_document(article_id, xml);
    let header = unique_element(&document, "teiHeader");
    let list = unique_list_bibl(&document);

    let title_stmt =
        direct_child(header, "fileDesc").and_then(|desc| direct_child(desc, "titleStmt"));
    let title_stmt_titles: Vec<String> = title_stmt
        .map(|stmt| {
            descendants_named(stmt, "title")
                .map(normalized_text)
                .collect()
        })
        .unwrap_or_default();
    let header_titles: Vec<String> = descendants_named(header, "title")
        .map(normalized_text)
        .collect();
    let header_emails: Vec<String> = descendants_named(header, "email")
        .map(normalized_text)
        .collect();
    let header_idnos: Vec<String> = descendants_named(header, "idno")
        .map(normalized_text)
        .collect();
    let header_pubnum_ids: Vec<String> = descendants_named(header, "idno")
        .filter(|node| !is_checksum_idno(node))
        .map(normalized_text)
        .collect();

    let author_elements: Vec<roxmltree::Node<'_, '_>> =
        descendants_named(header, "author").collect();
    let author_name_texts: Vec<String> = author_elements
        .iter()
        .map(|author| {
            author
                .children()
                .find(|child| is_element(child, "persName"))
                .map(normalized_text)
                .unwrap_or_default()
        })
        .collect();

    let mut affiliation_occurrences = Vec::new();
    for (author_index, author) in author_elements.iter().enumerate() {
        for affiliation in author
            .children()
            .filter(|child| is_element(child, "affiliation"))
        {
            let Some(key) = affiliation.attribute("key") else {
                continue;
            };
            affiliation_occurrences.push(AffiliationOccurrence {
                author_index,
                key: key.to_string(),
                text: normalized_text(affiliation),
            });
        }
    }

    let date_text = descendants_named(header, "date")
        .map(normalized_text)
        .find(|text| !text.is_empty())
        .unwrap_or_else(|| {
            panic!("{PROBE} {article_id}: the header declares no dated element with text")
        });

    let reference_texts: Vec<String> = list
        .children()
        .filter(|child| is_element(child, "biblStruct"))
        .map(normalized_text)
        .collect();

    let mut citation_carriers = Vec::new();
    for node in list.descendants() {
        if !node.is_element() {
            continue;
        }
        match node.tag_name().name() {
            "idno" => match node.attribute("type") {
                Some("DOI") => {
                    citation_carriers.push((CitationFieldRole::Doi, normalized_text(node)))
                }
                Some("arXiv") => {
                    citation_carriers.push((CitationFieldRole::ArxivId, normalized_text(node)))
                }
                _ => {}
            },
            "ptr" => {
                if let Some(target) = node.attribute("target")
                    && target.starts_with("http")
                {
                    citation_carriers.push((CitationFieldRole::Url, target.to_string()));
                }
            }
            "title" if node.attribute("level") == Some("a") => {
                citation_carriers.push((CitationFieldRole::ArticleTitle, normalized_text(node)));
            }
            _ => {}
        }
    }

    SourceDocument {
        article_id: article_id.to_string(),
        sha256: declared_sha256.to_string(),
        header_titles,
        title_stmt_titles,
        header_emails,
        header_idnos,
        header_pubnum_ids,
        author_name_texts,
        affiliation_occurrences,
        date_text,
        reference_texts,
        citation_carriers,
    }
}

// ────────────────────────── the admitted band (input) ───────────────────────

/// One named, typed admission narrowing: a class whose real carrier population
/// could not be admitted in full, with the reason.  `admitted < source_population`
/// is only allowed when a row of this table explains it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AdmissionGap {
    class: &'static str,
    source_population: usize,
    admitted: usize,
    reason: String,
}

/// The admitted band: the typed composition input plus the admitted-side values
/// the independent oracle measures the emission against.  All of it is derived
/// from the real document; none of it is a sentinel.
struct AdmittedBand {
    header: HeaderCompositionOutcome,
    references: ReferenceCompositionOutcome,
    titles: Vec<String>,
    emails: Vec<String>,
    ids: Vec<String>,
    author_lines: Vec<(String, String)>,
    affiliation_bindings: Vec<(String, String)>,
    reference_texts: Vec<String>,
    citation_fields: Vec<(CitationFieldRole, String)>,
    gaps: Vec<AdmissionGap>,
}

/// The serde wire tag and reason of a rejected parser outcome, e.g.
/// `ambiguous:single_token`.
fn outcome_token<T: serde::Serialize>(outcome: &T) -> String {
    let value = serde_json::to_value(outcome).expect("a parser outcome serializes");
    let tag = value
        .get("outcome")
        .and_then(|tag| tag.as_str())
        .unwrap_or("unknown")
        .to_string();
    match value.get("reason").and_then(|reason| reason.as_str()) {
        Some(reason) => format!("{tag}:{reason}"),
        None => tag,
    }
}

/// Assemble the admitted band from the real frozen document through the public
/// typed parsers.  Every parser rejection becomes a named [`AdmissionGap`];
/// nothing is silently dropped and no value is invented.
fn build_admitted_band(source: &SourceDocument) -> AdmittedBand {
    let mut gaps = Vec::new();

    let date = match NativeDateParser::new().parse(&source.date_text) {
        NativeDateOutcome::Recognized { date } => date,
        other => panic!(
            "FATAL {PROBE} {}: the real publication date form cannot be admitted by \
             NativeDateParser ({}); the header band cannot be composed from the frozen document",
            source.article_id,
            outcome_token(&other)
        ),
    };

    let titles = source.title_stmt_titles.clone();
    if titles.len() < source.header_titles.len() {
        gaps.push(AdmissionGap {
            class: CLASS_HEADER_TITLE,
            source_population: source.header_titles.len(),
            admitted: titles.len(),
            reason: format!(
                "band definition: the admitted title field is the direct fileDesc/titleStmt/title \
                 children; the remaining {} of {} header <title> carriers are conserved by the \
                 source subtree and are not produced by the admitted band",
                source.header_titles.len() - titles.len(),
                source.header_titles.len()
            ),
        });
    }

    let emails = source.header_emails.clone();
    let ids = source.header_pubnum_ids.clone();

    let affiliation_parser = NativeAffiliationParser::new();
    let mut components: Vec<NativeAffiliationOutcome> = Vec::new();
    let mut occurrence_component: Vec<Option<usize>> = Vec::new();
    let mut rejected_affiliations = Vec::new();
    for occurrence in &source.affiliation_occurrences {
        match affiliation_parser.parse(&occurrence.text) {
            NativeAffiliationOutcome::Recognized {
                components: recognized,
            } => {
                assert!(
                    !recognized.is_empty(),
                    "{}: a recognized affiliation must carry at least one component",
                    source.article_id
                );
                occurrence_component.push(Some(components.len()));
                components.push(NativeAffiliationOutcome::Recognized {
                    components: recognized,
                });
            }
            other => {
                occurrence_component.push(None);
                rejected_affiliations.push(outcome_token(&other));
            }
        }
    }

    let name_parser = NativePersonalNameParser::new();
    let mut authors: Vec<HeaderAuthor> = Vec::new();
    let mut author_lines: Vec<(String, String)> = Vec::new();
    let mut affiliation_bindings: Vec<(String, String)> = Vec::new();
    let mut rejected_names = Vec::new();

    for (author_index, name_text) in source.author_name_texts.iter().enumerate() {
        let name = match name_parser.parse(PersonalNameContext::Header, name_text) {
            NativePersonalNameOutcome::Recognized { name } => name,
            other => {
                rejected_names.push(outcome_token(&other));
                continue;
            }
        };
        let ordinal = authors.len();
        let owner = format!("author-{ordinal}");
        let mut links = Vec::new();
        for (occurrence_index, occurrence) in source.affiliation_occurrences.iter().enumerate() {
            if occurrence.author_index != author_index {
                continue;
            }
            let Some(component) = occurrence_component[occurrence_index] else {
                continue;
            };
            let NativeAffiliationOutcome::Recognized {
                components: recognized,
            } = &components[component]
            else {
                continue;
            };
            let span = recognized
                .first()
                .expect("a recognized affiliation carries a component")
                .span;
            links.push(NativeAffiliationLink {
                key: occurrence.key.clone(),
                affiliation_ordinal: component,
                affiliation_span: span,
            });
        }
        author_lines.push((owner.clone(), name.surname.clone()));
        affiliation_bindings.extend(links.iter().map(|link| (owner.clone(), link.key.clone())));
        authors.push(HeaderAuthor {
            ordinal,
            span: CompositionSpan {
                start: ordinal,
                end: ordinal + 1,
            },
            name,
            affiliations: links,
        });
    }

    assert!(
        !authors.is_empty(),
        "FATAL {PROBE} {}: the frozen reference admitted no author line",
        source.article_id
    );
    for reason in &rejected_names {
        gaps.push(AdmissionGap {
            class: CLASS_AUTHOR_LINE,
            source_population: source.author_name_texts.len(),
            admitted: authors.len(),
            reason: format!(
                "NativePersonalNameParser rejected the real <persName> text ({reason})"
            ),
        });
    }
    for reason in &rejected_affiliations {
        gaps.push(AdmissionGap {
            class: CLASS_AUTHOR_AFFILIATION,
            source_population: source.affiliation_occurrences.len(),
            admitted: affiliation_bindings.len(),
            reason: format!(
                "NativeAffiliationParser rejected the real <affiliation key> carrier text ({reason})"
            ),
        });
    }

    let references: Vec<NativeReference> = source
        .reference_texts
        .iter()
        .enumerate()
        .map(|(ordinal, text)| NativeReference {
            ordinal,
            source: text.clone(),
            span: CitationSpan {
                start: 0,
                end: text.len(),
            },
            provenance: "frozen-external-holdout-reference".into(),
        })
        .collect();
    assert!(
        references
            .iter()
            .all(|reference| reference.span.start < reference.span.end),
        "FATAL {PROBE} {}: a source biblStruct carries empty text, so no honest reference span exists",
        source.article_id
    );

    let citation_fields: Vec<CitationField> = source
        .citation_carriers
        .iter()
        .map(|(role, value)| CitationField {
            role: *role,
            value: value.clone(),
            span: CitationSpan {
                start: 0,
                end: value.len(),
            },
        })
        .collect();
    assert!(
        citation_fields
            .iter()
            .all(|field| field.span.start < field.span.end),
        "FATAL {PROBE} {}: a citation carrier carries an empty value",
        source.article_id
    );

    let header = HeaderCompositionOutcome::Accepted {
        composition: HeaderCompositionCandidate {
            candidate: HeaderCandidate {
                date,
                authors,
                fields: NativeHeaderFields {
                    title: titles.clone(),
                    email: emails.clone(),
                    ids: ids.clone(),
                },
            },
            receipts: vec![
                ComponentReceipt {
                    component: CompositionComponent::Date,
                    disposition: ComponentDisposition::Accepted,
                },
                ComponentReceipt {
                    component: CompositionComponent::Names,
                    disposition: ComponentDisposition::Accepted,
                },
                ComponentReceipt {
                    component: CompositionComponent::Affiliations,
                    disposition: ComponentDisposition::Accepted,
                },
            ],
        },
    };
    let references_outcome = ReferenceCompositionOutcome::Accepted {
        composition: ReferenceCompositionCandidate {
            candidate: ReferenceCandidate {
                references,
                fields: citation_fields.clone(),
                anchors: Vec::new(),
                adjacent_pairs: Vec::new(),
            },
            receipts: vec![
                ComponentReceipt {
                    component: CompositionComponent::References,
                    disposition: ComponentDisposition::Accepted,
                },
                ComponentReceipt {
                    component: CompositionComponent::CitationFields,
                    disposition: if citation_fields.is_empty() {
                        ComponentDisposition::Empty
                    } else {
                        ComponentDisposition::Accepted
                    },
                },
                ComponentReceipt {
                    component: CompositionComponent::Anchors,
                    disposition: ComponentDisposition::Empty,
                },
            ],
        },
    };

    assert!(
        affiliation_bindings.len() <= source.affiliation_occurrences.len(),
        "{}: the admitted band cannot carry more bindings than the source declares",
        source.article_id
    );

    AdmittedBand {
        header,
        references: references_outcome,
        titles,
        emails,
        ids,
        author_lines,
        affiliation_bindings,
        reference_texts: source.reference_texts.clone(),
        citation_fields: source.citation_carriers.clone(),
        gaps,
    }
}

// ─────────────────────────── the independent oracle ────────────────────────

/// One class's measured profile: `TP = conserved`, `FN = lost`, `FP = fabricated`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassProfile {
    article_id: String,
    class: &'static str,
    source_population: usize,
    admitted: usize,
    conserved: usize,
    lost: usize,
    fabricated: usize,
}

/// `numerator / denominator`, or `None` when there is no denominator.  A null
/// denominator is `no_denominator` (MEM1702): it is never read as a vacuum `1.0`.
fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

impl ClassProfile {
    /// Honest coverage of the real carrier population by the admitted band.
    /// `None` when the document declares no such carrier (there is no denominator).
    fn admission_coverage(&self) -> Option<f64> {
        ratio(self.admitted, self.source_population)
    }
}

/// Occurrence-counted conservation of text-valued carriers.
///
/// * `conserved` is `admitted` net of the occurrences the conserved source
///   subtrees already contribute: when the source's own carriers match the
///   admitted values, subtracting them is what makes a band that contributed
///   nothing read as `0` instead of a vacuous `1.0`.
/// * `fabricated` counts emitted band carriers matching neither an admitted value
///   nor a source value.
fn text_class(
    article_id: &str,
    class: &'static str,
    source_population: usize,
    admitted_values: &[String],
    emitted_carriers: &[String],
    source_carriers: &[String],
    source_is_conserved: bool,
) -> ClassProfile {
    let emitted_match = emitted_carriers
        .iter()
        .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
        .count();
    let source_match = if source_is_conserved {
        source_carriers
            .iter()
            .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
            .count()
    } else {
        0
    };
    let conserved = admitted_values
        .len()
        .min(emitted_match.saturating_sub(source_match));
    let fabricated = emitted_carriers
        .iter()
        .filter(|carrier| {
            !admitted_values.iter().any(|value| value == *carrier)
                && !source_carriers.iter().any(|value| value == *carrier)
        })
        .count();
    ClassProfile {
        article_id: article_id.to_string(),
        class,
        source_population,
        admitted: admitted_values.len(),
        conserved,
        lost: admitted_values.len() - conserved,
        fabricated,
    }
}

/// Occurrence-counted conservation of binding-valued carriers (an owner element
/// plus a value: the author line's surname, or an affiliation key).
fn binding_class(
    article_id: &str,
    class: &'static str,
    source_population: usize,
    admitted_values: &[(String, String)],
    emitted_carriers: &[(String, String)],
    source_carriers: &[(String, String)],
    source_is_conserved: bool,
) -> ClassProfile {
    let emitted_match = emitted_carriers
        .iter()
        .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
        .count();
    let source_match = if source_is_conserved {
        source_carriers
            .iter()
            .filter(|carrier| admitted_values.iter().any(|value| value == *carrier))
            .count()
    } else {
        0
    };
    let conserved = admitted_values
        .len()
        .min(emitted_match.saturating_sub(source_match));
    let fabricated = emitted_carriers
        .iter()
        .filter(|carrier| {
            !admitted_values.iter().any(|value| value == *carrier)
                && !source_carriers.iter().any(|value| value == *carrier)
        })
        .count();
    ClassProfile {
        article_id: article_id.to_string(),
        class,
        source_population,
        admitted: admitted_values.len(),
        conserved,
        lost: admitted_values.len() - conserved,
        fabricated,
    }
}

/// The emitted header's author-line carriers: `author-{ordinal}` -> surname.
fn emitted_author_lines(document: &roxmltree::Document<'_>) -> Vec<(String, String)> {
    let header = unique_element(document, "teiHeader");
    descendants_named(header, "author")
        .filter_map(|author| {
            let id = xml_id_of(author)?;
            let surname = author
                .children()
                .find(|child| is_element(child, "surname"))
                .map(normalized_text)
                .unwrap_or_default();
            Some((id.to_string(), surname))
        })
        .collect()
}

/// The emitted header's affiliation bindings: `author-{ordinal}` -> key.
fn emitted_affiliation_bindings(document: &roxmltree::Document<'_>) -> Vec<(String, String)> {
    let header = unique_element(document, "teiHeader");
    let mut bindings = Vec::new();
    for author in descendants_named(header, "author") {
        let Some(owner) = xml_id_of(author) else {
            continue;
        };
        for affiliation in descendants_named(author, "affiliation") {
            if let Some(key) = affiliation.attribute("key") {
                bindings.push((owner.to_string(), key.to_string()));
            }
        }
    }
    bindings
}

/// The emitted `listBibl` carriers of the measured citation roles.
fn emitted_citation_carriers(document: &roxmltree::Document<'_>) -> Vec<(String, String)> {
    let list = unique_list_bibl(document);
    list.descendants()
        .filter(|node| is_element(node, "note"))
        .filter_map(|node| {
            let role = node.attribute("role")?;
            let known = PROBE_CITATION_ROLES
                .iter()
                .any(|(candidate, _)| citation_field_role_token(*candidate) == role);
            known.then(|| (role.to_string(), normalized_text(node)))
        })
        .collect()
}

/// The source document's own author-line carriers, keyed by the empty owner (a
/// source `<author>` carries no `xml:id`).
fn source_author_lines(xml: &str) -> Vec<(String, String)> {
    let document = parse_document("source-author-lines", xml);
    let header = unique_element(&document, "teiHeader");
    descendants_named(header, "author")
        .map(|author| {
            let surname = author
                .children()
                .find(|child| is_element(child, "surname"))
                .map(normalized_text)
                .unwrap_or_default();
            (String::new(), surname)
        })
        .collect()
}

/// The source document's own affiliation bindings, keyed by the empty owner.
fn source_affiliation_bindings(xml: &str) -> Vec<(String, String)> {
    let document = parse_document("source-affiliation-bindings", xml);
    let header = unique_element(&document, "teiHeader");
    descendants_named(header, "author")
        .flat_map(|author| {
            descendants_named(author, "affiliation")
                .filter_map(|node| node.attribute("key").map(str::to_string))
                .map(|key| (String::new(), key))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Every class's measured profile for one document, from the emission bytes only.
fn oracle_class_profiles(
    source: &SourceDocument,
    source_xml: &str,
    admitted: &AdmittedBand,
    emission_xml: &str,
) -> Vec<ClassProfile> {
    let document = parse_document(&source.article_id, emission_xml);
    let header = unique_element(&document, "teiHeader");
    let list = unique_list_bibl(&document);
    let article_id = source.article_id.as_str();

    let emitted_titles: Vec<String> = descendants_named(header, "title")
        .map(normalized_text)
        .collect();
    let emitted_emails: Vec<String> = descendants_named(header, "email")
        .map(normalized_text)
        .collect();
    let emitted_ids: Vec<String> = descendants_named(header, "idno")
        .map(normalized_text)
        .collect();
    let emitted_reference_texts: Vec<String> = list
        .children()
        .filter(|child| is_element(child, "biblStruct"))
        .map(normalized_text)
        .collect();

    let emitted_author_lines = emitted_author_lines(&document);
    let emitted_affiliations = emitted_affiliation_bindings(&document);
    let source_author_line_carriers = source_author_lines(source_xml);
    let source_affiliation_carriers = source_affiliation_bindings(source_xml);

    let emitted_citations = emitted_citation_carriers(&document);
    let source_citations: Vec<(String, String)> = source
        .citation_carriers
        .iter()
        .map(|(role, value)| (citation_field_role_token(*role).to_string(), value.clone()))
        .collect();
    let admitted_citations: Vec<(String, String)> = admitted
        .citation_fields
        .iter()
        .map(|(role, value)| (citation_field_role_token(*role).to_string(), value.clone()))
        .collect();

    let mut profiles = Vec::with_capacity(CLASS_COUNT);

    profiles.push(binding_class(
        article_id,
        CLASS_AUTHOR_LINE,
        source.author_name_texts.len(),
        &admitted.author_lines,
        &emitted_author_lines,
        &source_author_line_carriers,
        false,
    ));
    profiles.push(text_class(
        article_id,
        CLASS_HEADER_TITLE,
        source.header_titles.len(),
        &admitted.titles,
        &emitted_titles,
        &source.header_titles,
        true,
    ));
    profiles.push(text_class(
        article_id,
        CLASS_HEADER_EMAIL,
        source.header_emails.len(),
        &admitted.emails,
        &emitted_emails,
        &source.header_emails,
        true,
    ));
    profiles.push(text_class(
        article_id,
        CLASS_HEADER_IDS,
        source.header_pubnum_ids.len(),
        &admitted.ids,
        &emitted_ids,
        &source.header_idnos,
        true,
    ));
    profiles.push(binding_class(
        article_id,
        CLASS_AUTHOR_AFFILIATION,
        source.affiliation_occurrences.len(),
        &admitted.affiliation_bindings,
        &emitted_affiliations,
        &source_affiliation_carriers,
        false,
    ));
    profiles.push(text_class(
        article_id,
        CLASS_REFERENCE_DOMAIN,
        source.reference_texts.len(),
        &admitted.reference_texts,
        &emitted_reference_texts,
        &source.reference_texts,
        false,
    ));

    for (role, class) in PROBE_CITATION_ROLES {
        let token = citation_field_role_token(role);
        let admitted_values: Vec<String> = admitted_citations
            .iter()
            .filter(|(candidate, _)| candidate == token)
            .map(|(_, value)| value.clone())
            .collect();
        let source_values: Vec<String> = source_citations
            .iter()
            .filter(|(candidate, _)| candidate == token)
            .map(|(_, value)| value.clone())
            .collect();
        let emitted_values: Vec<String> = emitted_citations
            .iter()
            .filter(|(candidate, _)| candidate == token)
            .map(|(_, value)| value.clone())
            .collect();
        profiles.push(text_class(
            article_id,
            class,
            source_values.len(),
            &admitted_values,
            &emitted_values,
            &source_values,
            false,
        ));
    }

    assert_eq!(
        profiles.len(),
        CLASS_COUNT,
        "every declared class must be measured"
    );
    profiles
}

// ─────────────────────────── composition harness ───────────────────────────

/// Compose one document's admitted band with the single SUT this file reaches.
/// A typed refusal is FATAL: the frozen probe measures a document only when the
/// SUT composes it, and a refusal names the document and the typed error.
fn compose_document(
    source: &SourceDocument,
    admitted: &AdmittedBand,
    xml: &str,
    data: &ProbeData,
) -> NativeFulltextCandidate {
    let input = NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            xml.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, data.profile_fingerprint.clone()),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: source.sha256.clone(),
            profile_fingerprint: data.profile_fingerprint.clone(),
            config_fingerprint: data.config_fingerprint.clone(),
        },
        header: admitted.header.clone(),
        references: admitted.references.clone(),
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: probe_admissions(&source.sha256),
        s17_lineage: None,
    };
    match compose_native_fulltext(input) {
        NativeFulltextCompositionOutcome::Accepted(candidate) => candidate,
        NativeFulltextCompositionOutcome::Rejected { error, .. } => panic!(
            "FATAL {PROBE} {}: the frozen reference's admitted band must compose; typed refusal: \
             {error:?}",
            source.article_id
        ),
    }
}

/// Header and references admitted, every other component typed
/// Unsupported/NotObserved.
fn probe_admissions(source_sha256: &str) -> Vec<FulltextAdmissionReceipt> {
    let receipt = |component: FulltextComponent, disposition: AdmissionDisposition| {
        FulltextAdmissionReceipt {
            component,
            disposition,
            producer: "frozen-external-holdout-reference".into(),
            evidence_state: if disposition == AdmissionDisposition::Passed {
                "passing"
            } else {
                "unsupported"
            }
            .into(),
            schema_version: "native-composition.outcome/v1".into(),
            source_sha256: source_sha256.to_string(),
            artifact_sha256: String::new(),
            fallback_target: NATIVE_FULLTEXT_FALLBACK_TARGET.into(),
        }
    };
    vec![
        receipt(FulltextComponent::Header, AdmissionDisposition::Passed),
        receipt(FulltextComponent::References, AdmissionDisposition::Passed),
        receipt(
            FulltextComponent::DocumentSegmentation,
            AdmissionDisposition::InvalidEvidence,
        ),
        receipt(FulltextComponent::Figure, AdmissionDisposition::Unsupported),
        receipt(FulltextComponent::Table, AdmissionDisposition::NotObserved),
        receipt(
            FulltextComponent::Formula,
            AdmissionDisposition::Unsupported,
        ),
        receipt(
            FulltextComponent::LearningPolicy,
            AdmissionDisposition::NotObserved,
        ),
    ]
}

// ─────────────────────────── the margin metrics ────────────────────────────

/// The two metrics the lane declares.  A declared metric that maps to no
/// computation is a refusal, so a third declared metric cannot be ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricKind {
    Agreement,
    UncertaintyWidth,
}

fn metric_kind(definition: &MetricDefinition) -> MetricKind {
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
    let source_population: usize = profiles
        .iter()
        .map(|profile| profile.source_population)
        .sum();
    let admitted: usize = profiles.iter().map(|profile| profile.admitted).sum();
    let conserved: usize = profiles.iter().map(|profile| profile.conserved).sum();
    let lost: usize = profiles.iter().map(|profile| profile.lost).sum();
    let fabricated: usize = profiles.iter().map(|profile| profile.fabricated).sum();
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
fn margin_verdict(margin: &Margin, value: Option<f64>) -> Verdict {
    let Some(value) = value else {
        return Verdict::NoDenominator;
    };
    let satisfied = match margin.direction {
        Direction::AtLeast => value >= margin.value,
        Direction::AtMost => value <= margin.value,
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
    direction: Direction,
    threshold: f64,
    value: Option<f64>,
    verdict: Verdict,
}

/// Evaluate one verdict per DECLARED metric (not per hard-coded name), so the
/// number of verdicts tracks the lane.
fn evaluate_margins(data: &ProbeData, metrics: &DocumentMetrics) -> Vec<MarginVerdict> {
    data.metrics
        .iter()
        .map(|definition| {
            let margin = data
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

// ───────────────────────────── measurement ─────────────────────────────────

/// One frozen document's measured result.
struct MeasuredDocument {
    article_id: String,
    identity_sha256: String,
    admitted_band: AdmittedBand,
    profiles: Vec<ClassProfile>,
    metrics: DocumentMetrics,
    verdicts: Vec<MarginVerdict>,
    ledger_admitted: [usize; 5],
    ledger_conserved: [usize; 5],
}

/// Re-verify one frozen TEI against its declared byte pin.  A missing file, a
/// drifted digest or a drifted size is a NAMED refusal; the caller turns it into
/// a FATAL failure so the probe never profiles bytes other than the pinned ones.
fn check_reference_bytes(row: &ReferenceRow) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(&row.path)
        .map_err(|_| format!("probe_reference_missing:{}", row.article_id))?;
    let actual = format!("sha256:{:x}", Sha256::digest(&bytes));
    if actual != row.declared_sha256 {
        return Err(format!("probe_reference_bytes_drift:{}", row.article_id));
    }
    if bytes.len() as u64 != row.size_bytes {
        return Err(format!("probe_reference_size_drift:{}", row.article_id));
    }
    Ok(bytes)
}

fn measure_document(row: &ReferenceRow, data: &ProbeData) -> MeasuredDocument {
    let bytes = check_reference_bytes(row).unwrap_or_else(|label| panic!("FATAL {PROBE} {label}"));
    let xml = String::from_utf8(bytes).unwrap_or_else(|error| {
        panic!(
            "FATAL {PROBE} probe_reference_not_utf8:{} ({error})",
            row.article_id
        )
    });

    let identity_sha256 = row
        .declared_sha256
        .strip_prefix("sha256:")
        .unwrap_or(&row.declared_sha256)
        .to_string();
    let source = measure_source(&row.article_id, &identity_sha256, &xml);
    let admitted = build_admitted_band(&source);
    let candidate = compose_document(&source, &admitted, &xml, data);
    let emission =
        String::from_utf8(candidate.envelope.raw_tei().to_vec()).unwrap_or_else(|error| {
            panic!(
                "{PROBE} {}: emitted document is not UTF-8 ({error})",
                row.article_id
            )
        });
    let profiles = oracle_class_profiles(&source, &xml, &admitted, &emission);
    let metrics = document_metrics(&profiles);
    let verdicts = evaluate_margins(data, &metrics);

    let ledger = &candidate.manifest.semantic_fields;
    let class_conserved = |class: &str| {
        profiles
            .iter()
            .find(|profile| profile.class == class)
            .map(|profile| profile.conserved)
            .unwrap_or(0)
    };
    let ledger_conserved = [
        class_conserved(CLASS_HEADER_TITLE),
        class_conserved(CLASS_HEADER_EMAIL),
        class_conserved(CLASS_HEADER_IDS),
        class_conserved(CLASS_AUTHOR_AFFILIATION),
        PROBE_CITATION_ROLES
            .iter()
            .map(|(_, class)| class_conserved(class))
            .sum(),
    ];
    let ledger_admitted = [
        ledger.admitted_header_titles,
        ledger.admitted_header_emails,
        ledger.admitted_header_ids,
        ledger.admitted_author_affiliations,
        ledger.admitted_citation_fields,
    ];

    MeasuredDocument {
        article_id: row.article_id.clone(),
        identity_sha256: row.declared_sha256.clone(),
        admitted_band: admitted,
        profiles,
        metrics,
        verdicts,
        ledger_admitted,
        ledger_conserved,
    }
}

fn measure_all(data: &ProbeData) -> Vec<MeasuredDocument> {
    data.reference
        .iter()
        .map(|row| measure_document(row, data))
        .collect()
}

/// Counts over the measured documents.  No rate, no pooled corpus claim: every field
/// is a count and the document count is printed next to them.
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

fn micros(value: f64) -> u64 {
    (value * 1_000_000.0).round() as u64
}

// Navigation helpers for the mutation matrix.  Free functions so a boxed
// `'static` closure can call them without capturing a borrow.
fn contract_mut(manifest: &mut Value) -> &mut Value {
    manifest
        .get_mut("external_holdout_contract")
        .expect("the contract")
}

fn lane_mut(manifest: &mut Value) -> &mut Value {
    contract_mut(manifest)
        .get_mut("release_probe")
        .expect("the lane")
}

fn metrics_mut(manifest: &mut Value) -> &mut Value {
    lane_mut(manifest).get_mut("metrics").expect("the metrics")
}

// ───────────────────────────── frozen measurement ──────────────────────────

/// One frozen per-class row: `(class, source_population, admitted, conserved,
/// fabricated)`.  Every number is asserted, so a deviation in either direction
/// fails.  `admitted == conserved` and `fabricated == 0` everywhere is the frozen
/// claim of this probe.
struct FrozenDocument {
    article_id: &'static str,
    classes: [(&'static str, usize, usize, usize, usize); CLASS_COUNT],
}

/// The measured profile of 2026-09-24 on the sha256-pinned frozen holdout TEI.
/// The table is written out rather than derived so that a change in any counter is
/// visible as a diff in the assertion, not as a silently recomputed baseline.
/// Its length is deliberately a slice length, not a type-level literal: the audit
/// compares it to the measured population by name, so a manifest that declares a
/// different number of documents cannot be silently truncated by `zip`.
static FROZEN_PROFILE: &[FrozenDocument] = &[
    FrozenDocument {
        article_id: "1203.2295",
        classes: [
            (CLASS_AUTHOR_LINE, 2, 2, 2, 0),
            (CLASS_HEADER_TITLE, 2, 1, 1, 0),
            (CLASS_HEADER_EMAIL, 2, 2, 2, 0),
            (CLASS_HEADER_IDS, 1, 1, 1, 0),
            (CLASS_AUTHOR_AFFILIATION, 3, 3, 3, 0),
            (CLASS_REFERENCE_DOMAIN, 22, 22, 22, 0),
            (CLASS_CITATION_DOI, 7, 7, 7, 0),
            (CLASS_CITATION_ARXIV, 1, 1, 1, 0),
            (CLASS_CITATION_URL, 4, 4, 4, 0),
            (CLASS_CITATION_ARTICLE_TITLE, 13, 13, 13, 0),
        ],
    },
    FrozenDocument {
        article_id: "2401.17167",
        classes: [
            (CLASS_AUTHOR_LINE, 239, 235, 235, 0),
            (CLASS_HEADER_TITLE, 2, 1, 1, 0),
            (CLASS_HEADER_EMAIL, 2, 2, 2, 0),
            (CLASS_HEADER_IDS, 1, 1, 1, 0),
            (CLASS_AUTHOR_AFFILIATION, 16, 6, 6, 0),
            (CLASS_REFERENCE_DOMAIN, 16, 16, 16, 0),
            (CLASS_CITATION_DOI, 2, 2, 2, 0),
            (CLASS_CITATION_ARXIV, 0, 0, 0, 0),
            (CLASS_CITATION_URL, 0, 0, 0, 0),
            (CLASS_CITATION_ARTICLE_TITLE, 5, 5, 5, 0),
        ],
    },
    FrozenDocument {
        article_id: "2411.07228",
        classes: [
            (CLASS_AUTHOR_LINE, 11, 8, 8, 0),
            (CLASS_HEADER_TITLE, 2, 1, 1, 0),
            (CLASS_HEADER_EMAIL, 0, 0, 0, 0),
            (CLASS_HEADER_IDS, 1, 1, 1, 0),
            (CLASS_AUTHOR_AFFILIATION, 11, 8, 8, 0),
            (CLASS_REFERENCE_DOMAIN, 53, 53, 53, 0),
            (CLASS_CITATION_DOI, 1, 1, 1, 0),
            (CLASS_CITATION_ARXIV, 20, 20, 20, 0),
            (CLASS_CITATION_URL, 2, 2, 2, 0),
            (CLASS_CITATION_ARTICLE_TITLE, 27, 27, 27, 0),
        ],
    },
    FrozenDocument {
        article_id: "2505.16270",
        classes: [
            (CLASS_AUTHOR_LINE, 10, 9, 9, 0),
            (CLASS_HEADER_TITLE, 2, 1, 1, 0),
            (CLASS_HEADER_EMAIL, 3, 3, 3, 0),
            (CLASS_HEADER_IDS, 1, 1, 1, 0),
            (CLASS_AUTHOR_AFFILIATION, 7, 7, 7, 0),
            (CLASS_REFERENCE_DOMAIN, 100, 100, 100, 0),
            (CLASS_CITATION_DOI, 0, 0, 0, 0),
            (CLASS_CITATION_ARXIV, 57, 57, 57, 0),
            (CLASS_CITATION_URL, 1, 1, 1, 0),
            (CLASS_CITATION_ARTICLE_TITLE, 40, 40, 40, 0),
        ],
    },
    FrozenDocument {
        article_id: "2608.22118",
        classes: [
            (CLASS_AUTHOR_LINE, 1, 1, 1, 0),
            (CLASS_HEADER_TITLE, 2, 1, 1, 0),
            (CLASS_HEADER_EMAIL, 0, 0, 0, 0),
            (CLASS_HEADER_IDS, 1, 1, 1, 0),
            (CLASS_AUTHOR_AFFILIATION, 0, 0, 0, 0),
            (CLASS_REFERENCE_DOMAIN, 14, 14, 14, 0),
            (CLASS_CITATION_DOI, 1, 1, 1, 0),
            (CLASS_CITATION_ARXIV, 2, 2, 2, 0),
            (CLASS_CITATION_URL, 14, 14, 14, 0),
            (CLASS_CITATION_ARTICLE_TITLE, 7, 7, 7, 0),
        ],
    },
];

/// The frozen per-document metrics.  Fractions are stored in micros so the
/// assertion is exact rather than a floating-point comparison.
struct FrozenMetrics {
    article_id: &'static str,
    source_population: usize,
    admitted: usize,
    conserved: usize,
    agreement_micros: u64,
    low_micros: u64,
    high_micros: u64,
    width_micros: u64,
}

static FROZEN_METRICS: &[FrozenMetrics] = &[
    FrozenMetrics {
        article_id: "1203.2295",
        source_population: 57,
        admitted: 56,
        conserved: 56,
        agreement_micros: 1_000_000,
        low_micros: 935_804,
        high_micros: 1_000_000,
        width_micros: 64_196,
    },
    FrozenMetrics {
        article_id: "2401.17167",
        source_population: 283,
        admitted: 268,
        conserved: 268,
        agreement_micros: 1_000_000,
        low_micros: 985_868,
        high_micros: 1_000_000,
        width_micros: 14_132,
    },
    FrozenMetrics {
        article_id: "2411.07228",
        source_population: 128,
        admitted: 121,
        conserved: 121,
        agreement_micros: 1_000_000,
        low_micros: 969_228,
        high_micros: 1_000_000,
        width_micros: 30_772,
    },
    FrozenMetrics {
        article_id: "2505.16270",
        source_population: 221,
        admitted: 219,
        conserved: 219,
        agreement_micros: 1_000_000,
        low_micros: 982_761,
        high_micros: 1_000_000,
        width_micros: 17_239,
    },
    FrozenMetrics {
        article_id: "2608.22118",
        source_population: 42,
        admitted: 41,
        conserved: 41,
        agreement_micros: 1_000_000,
        low_micros: 914_330,
        high_micros: 1_000_000,
        width_micros: 85_670,
    },
];

/// The frozen corpus-wide aggregate: counts over the documents, never a rate and
/// never a pooled corpus claim.  `documents` and `agreeing_documents` are derived
/// from the frozen per-document tables rather than restated as literals, so a
/// table/population drift cannot be hidden by a matching hardcoded count.
fn frozen_aggregate() -> Aggregate {
    Aggregate {
        documents: FROZEN_PROFILE.len(),
        source_population: 731,
        admitted: 705,
        tp: 705,
        fn_: 0,
        fp: 0,
        agreeing_documents: FROZEN_METRICS
            .iter()
            .filter(|metrics| metrics.agreement_micros == 1_000_000)
            .count(),
        margin_pass: 10,
        margin_fail: 0,
        no_denominator_documents: 0,
    }
}

/// The frozen, named admission narrowings: `(article_id, class,
/// source_population, admitted)`.  Every class whose admitted band is smaller
/// than the real carrier population must appear here; a narrowing that is not
/// declared is a silent shrink and fails.
static FROZEN_NARROWINGS: [(&str, &str, usize, usize); 10] = [
    ("1203.2295", CLASS_HEADER_TITLE, 2, 1),
    ("2401.17167", CLASS_AUTHOR_LINE, 239, 235),
    ("2401.17167", CLASS_HEADER_TITLE, 2, 1),
    ("2401.17167", CLASS_AUTHOR_AFFILIATION, 16, 6),
    ("2411.07228", CLASS_AUTHOR_LINE, 11, 8),
    ("2411.07228", CLASS_HEADER_TITLE, 2, 1),
    ("2411.07228", CLASS_AUTHOR_AFFILIATION, 11, 8),
    ("2505.16270", CLASS_AUTHOR_LINE, 10, 9),
    ("2505.16270", CLASS_HEADER_TITLE, 2, 1),
    ("2608.22118", CLASS_HEADER_TITLE, 2, 1),
];

/// The frozen classes whose frozen reference declares no such carrier at all:
/// reported as `no_denominator`, never as a vacuum `1.0` (MEM1702).
static FROZEN_NO_DENOMINATOR_CLASSES: [(&str, &str); 6] = [
    ("2401.17167", CLASS_CITATION_ARXIV),
    ("2401.17167", CLASS_CITATION_URL),
    ("2411.07228", CLASS_HEADER_EMAIL),
    ("2505.16270", CLASS_CITATION_DOI),
    ("2608.22118", CLASS_HEADER_EMAIL),
    ("2608.22118", CLASS_AUTHOR_AFFILIATION),
];

// ──────────────────────────────── tests ────────────────────────────────────

/// The oracle's class vocabulary is the live production vocabulary, not a second
/// literal copy.
#[test]
fn class_vocabulary_matches_the_production_tokens() {
    assert_eq!(CLASS_HEADER_TITLE, SemanticFieldLoss::HeaderTitle.token());
    assert_eq!(CLASS_HEADER_EMAIL, SemanticFieldLoss::HeaderEmail.token());
    assert_eq!(CLASS_HEADER_IDS, SemanticFieldLoss::HeaderIds.token());
    assert_eq!(
        CLASS_AUTHOR_AFFILIATION,
        SemanticFieldLoss::AuthorAffiliation.token()
    );
    assert_eq!(
        SemanticFieldLoss::ALL
            .iter()
            .map(|class| class.token())
            .collect::<Vec<_>>(),
        vec![
            CLASS_HEADER_TITLE,
            CLASS_HEADER_EMAIL,
            CLASS_HEADER_IDS,
            CLASS_AUTHOR_AFFILIATION,
            "SemanticFieldLoss::CitationRole",
        ],
        "every production loss class stays in the oracle vocabulary"
    );
    assert_eq!(
        CLASS_REFERENCE_DOMAIN,
        NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION
    );
    for (role, class) in PROBE_CITATION_ROLES {
        assert_eq!(class, citation_class_token(role));
        assert_eq!(
            citation_field_role_token(role),
            serde_json::to_value(role)
                .expect("CitationFieldRole serializes")
                .as_str()
                .expect("CitationFieldRole serializes as a string"),
            "the emitted role token must equal the serde wire form for {role:?}"
        );
    }
}

/// The declared lane is read from DATA and structurally self-consistent: the
/// metric vocabulary equals the margin vocabulary, every metric carries the
/// declared structure, and the identity digest recomputes from the frozen
/// manifest by the declared handles.
#[test]
fn frozen_lane_is_read_from_data_and_its_identity_recomputes() {
    let manifest = manifest_value();
    let data = read_probe_data(&manifest)
        .unwrap_or_else(|label| panic!("FATAL {PROBE} the frozen lane is refused: {label}"));

    assert_eq!(
        data.reference.len(),
        data.expected_documents,
        "the frozen reference population must equal the declared article count \
         (data/parse/holdout_manifests.yaml#articles)"
    );
    let unique: BTreeSet<&str> = data
        .reference
        .iter()
        .map(|row| row.declared_sha256.as_str())
        .collect();
    assert_eq!(
        unique.len(),
        data.expected_documents,
        "one distinct fixture digest per declared document"
    );

    let metric_names: BTreeSet<&str> = data
        .metrics
        .iter()
        .map(|definition| definition.metric.as_str())
        .collect();
    let margin_names: BTreeSet<&str> = data
        .margins
        .iter()
        .map(|margin| margin.metric.as_str())
        .collect();
    assert_eq!(
        metric_names, margin_names,
        "one vocabulary: the declared metrics are exactly the declared margins"
    );
    for definition in &data.metrics {
        assert!(
            !definition.numerator_definition.is_empty()
                && !definition.denominator_definition.is_empty(),
            "{}: the declared definition must be non-empty",
            definition.metric
        );
        assert_eq!(definition.margin_ref, MARGIN_REF);
    }
    let agreement = data
        .metric(AGREEMENT_METRIC)
        .expect("the declared agreement metric");
    let uncertainty = data
        .metric(UNCERTAINTY_METRIC)
        .expect("the declared uncertainty metric");
    assert_ne!(
        agreement.numerator_definition, uncertainty.numerator_definition,
        "the two metrics must stay distinct definitions"
    );

    // The declared digest equals the one recomputed from the frozen manifest.
    let lane = manifest
        .get("external_holdout_contract")
        .and_then(|contract| contract.get("release_probe"))
        .expect("the lane exists");
    let recomputed = recompute_identity_digest(lane)
        .unwrap_or_else(|label| panic!("FATAL {PROBE} identity refused: {label}"));
    assert_eq!(recomputed, data.identity_digest);
    assert_eq!(data.source_manifest, "data/parse/holdout_manifests.yaml");
}

/// The measurement: per-class profile, per-document metrics and margin verdicts,
/// printed for `--nocapture` and asserted.
#[test]
fn real_holdout_probe_profile_is_measured_per_document() {
    let manifest = manifest_value();
    let data = read_probe_data(&manifest)
        .unwrap_or_else(|label| panic!("FATAL {PROBE} the frozen lane is refused: {label}"));
    let measured = measure_all(&data);

    // Print everything before asserting, so a failure shows the whole profile.
    for document in &measured {
        for profile in &document.profiles {
            let coverage = profile
                .admission_coverage()
                .map(|value| format!("{value:.6}"))
                .unwrap_or_else(|| "no_denominator".to_string());
            eprintln!(
                "PROBE CLASS {} {} source_population={} admitted={} conserved={} lost={} \
                 fabricated={} coverage={coverage}",
                profile.article_id,
                profile.class,
                profile.source_population,
                profile.admitted,
                profile.conserved,
                profile.lost,
                profile.fabricated,
            );
        }
        let agreement = document
            .metrics
            .agreement
            .map(|value| format!("{value:.6}"))
            .unwrap_or_else(|| "no_denominator".to_string());
        let (low, high, width) = match (
            document.metrics.uncertainty,
            document.metrics.uncertainty_width,
        ) {
            (Some((low, high)), Some(width)) => (
                format!("{low:.6}"),
                format!("{high:.6}"),
                format!("{width:.6}"),
            ),
            _ => (
                "no_denominator".to_string(),
                "no_denominator".to_string(),
                "no_denominator".to_string(),
            ),
        };
        eprintln!(
            "PROBE DOC {} identity_sha256={} source_population={} admitted={} conserved={} \
             lost={} fabricated={} agreement={agreement} uncertainty_low={low} \
             uncertainty_high={high} uncertainty_width={width}",
            document.article_id,
            document.identity_sha256,
            document.metrics.source_population,
            document.metrics.admitted,
            document.metrics.conserved,
            document.metrics.lost,
            document.metrics.fabricated,
        );
        for verdict in &document.verdicts {
            let value = verdict
                .value
                .map(|value| format!("{value:.6}"))
                .unwrap_or_else(|| "no_denominator".to_string());
            eprintln!(
                "MARGIN {} {} -> {} value={value} threshold={:.6} direction={}",
                document.article_id,
                verdict.metric,
                verdict.verdict.token(),
                verdict.threshold,
                verdict.direction.token(),
            );
        }
        for gap in &document.admitted_band.gaps {
            eprintln!(
                "PROBE GAP {} {}: {}/{} admitted — {}",
                document.article_id, gap.class, gap.admitted, gap.source_population, gap.reason
            );
        }
    }
    let aggregate = Aggregate::of(&measured);
    eprintln!("{}", aggregate.render());

    assert_eq!(aggregate.documents, data.expected_documents);
    for document in &measured {
        assert_eq!(
            document.profiles.len(),
            CLASS_COUNT,
            "{}: every class must be measured",
            document.article_id
        );
        assert_eq!(
            document.verdicts.len(),
            data.metrics.len(),
            "{}: one verdict per declared metric",
            document.article_id
        );
        for profile in &document.profiles {
            assert_eq!(
                profile.conserved,
                profile.admitted,
                "{}/{}: the frozen probe must conserve every admitted occurrence \
                 (admitted {}, conserved {}, lost {})",
                profile.article_id,
                profile.class,
                profile.admitted,
                profile.conserved,
                profile.lost
            );
            assert_eq!(
                profile.fabricated, 0,
                "{}/{}: the frozen probe must fabricate nothing",
                profile.article_id, profile.class
            );
            let coverage = profile.admission_coverage();
            assert_eq!(
                coverage.is_none(),
                profile.source_population == 0,
                "{}/{}: coverage has no denominator exactly when the source declares no carrier",
                profile.article_id,
                profile.class
            );
        }
        assert_eq!(
            document.metrics.lost, 0,
            "{}: no admitted occurrence may be lost",
            document.article_id
        );
        assert_eq!(
            document.metrics.fabricated, 0,
            "{}: no emitted carrier may be fabricated",
            document.article_id
        );
        assert_eq!(
            document.metrics.agreement,
            Some(1.0),
            "{}: the per-document agreement ratio",
            document.article_id
        );
        let (low, high) = document
            .metrics
            .uncertainty
            .unwrap_or_else(|| panic!("{}: a bounded interval is owed", document.article_id));
        assert!(
            low < high,
            "{}: the uncertainty interval must be non-degenerate ({low}..{high})",
            document.article_id
        );
        assert_eq!(
            document.metrics.uncertainty_width,
            Some(high - low),
            "{}: the width is upper minus lower",
            document.article_id
        );
    }

    // Non-vacuity: the metric is non-zero and the band really narrowed somewhere,
    // so the oracle distinguishes narrowing rather than passing vacuously.
    assert!(
        aggregate.tp > 0,
        "the measured aggregate must be non-vacuous"
    );
    let narrowings: Vec<(&str, &'static str, usize, usize)> = measured
        .iter()
        .flat_map(|document| {
            document
                .profiles
                .iter()
                .filter(|profile| profile.admitted < profile.source_population)
                .map(|profile| {
                    (
                        document.article_id.as_str(),
                        profile.class,
                        profile.source_population,
                        profile.admitted,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        !narrowings.is_empty(),
        "the admitted band must genuinely narrow somewhere, or the oracle cannot tell"
    );
    for (article_id, class, source_population, admitted) in &narrowings {
        let document = measured
            .iter()
            .find(|document| document.article_id == *article_id)
            .expect("the narrowing comes from a measured document");
        let profile = document
            .profiles
            .iter()
            .find(|profile| profile.class == *class)
            .expect("the narrowing class is measured");
        assert_eq!(
            (profile.source_population, profile.admitted),
            (*source_population, *admitted)
        );
        assert!(
            profile.admitted < profile.source_population,
            "a narrowing means admitted < source_population"
        );
        let admitted_total = document.metrics.admitted;
        let conserved_total = document.metrics.conserved;
        assert!(
            admitted_total < document.metrics.source_population,
            "{article_id}: the evaluated population is narrower than the source population"
        );
        let successes_source =
            conserved_total * document.metrics.source_population / admitted_total;
        let narrowed_width = wilson_interval(conserved_total, admitted_total)
            .expect("the admitted population is non-empty")
            .1
            - wilson_interval(conserved_total, admitted_total)
                .expect("the admitted population is non-empty")
                .0;
        let source_width = wilson_interval(successes_source, document.metrics.source_population)
            .expect("the source population is non-empty")
            .1
            - wilson_interval(successes_source, document.metrics.source_population)
                .expect("the source population is non-empty")
                .0;
        assert!(
            narrowed_width > source_width,
            "{article_id}: the interval must widen when the admitted population narrows \
             (narrowed {narrowed_width:.6} vs source {source_width:.6})"
        );
    }

    // The composer's ledger is cross-checked against the independent oracle —
    // never the other way round.
    for document in &measured {
        let expected_admitted = [
            document.admitted_band.titles.len(),
            document.admitted_band.emails.len(),
            document.admitted_band.ids.len(),
            document.admitted_band.affiliation_bindings.len(),
            document.admitted_band.citation_fields.len(),
        ];
        assert_eq!(
            document.ledger_admitted, expected_admitted,
            "{}: ledger admitted counts vs oracle",
            document.article_id
        );
        assert_eq!(
            document.ledger_conserved[0],
            document.admitted_band.titles.len(),
            "{}: ledger conserved titles",
            document.article_id
        );
        assert_eq!(
            document.ledger_conserved[1],
            document.admitted_band.emails.len(),
            "{}: ledger conserved emails",
            document.article_id
        );
        assert_eq!(
            document.ledger_conserved[2],
            document.admitted_band.ids.len(),
            "{}: ledger conserved ids",
            document.article_id
        );
        assert_eq!(
            document.ledger_conserved[3],
            document.admitted_band.affiliation_bindings.len(),
            "{}: ledger conserved affiliations",
            document.article_id
        );
        assert_eq!(
            document.ledger_conserved[4],
            document.admitted_band.citation_fields.len(),
            "{}: ledger conserved citation roles",
            document.article_id
        );
    }

    // Every named narrowing carries its reason and is genuinely a narrowing.
    for document in &measured {
        for gap in &document.admitted_band.gaps {
            assert!(!gap.reason.is_empty(), "a narrowing must carry its reason");
            assert!(gap.admitted < gap.source_population);
        }
    }

    // The identity shas are distinct digests, one per declared document.
    let shas: BTreeSet<&str> = measured
        .iter()
        .map(|document| document.identity_sha256.as_str())
        .collect();
    assert_eq!(shas.len(), data.expected_documents);
}

/// The frozen aggregate: the measured numbers are asserted, so any drift in
/// either direction fails.  `documents=5` is printed next to the counts, and the
/// rendered aggregate is counts-only — never a corpus rate.
#[test]
fn measured_probe_aggregate_is_frozen() {
    let data = load_probe_data();
    let measured = measure_all(&data);
    let aggregate = Aggregate::of(&measured);
    eprintln!("{}", aggregate.render());

    let frozen = frozen_aggregate();
    assert_eq!(
        aggregate, frozen,
        "the measured aggregate drifted from the frozen one (the frozen table covers {} \
         documents, the manifest declares {})",
        frozen.documents, data.expected_documents,
    );
    assert_eq!(
        aggregate.documents, data.expected_documents,
        "the measured population must equal the declared article count"
    );
    assert!(aggregate.tp > 0, "the frozen aggregate must be non-vacuous");
    let rendered = aggregate.render();
    assert!(
        rendered.contains(&format!("documents={}", data.expected_documents)),
        "the rendered aggregate must print the measured document count"
    );
    assert!(
        !rendered.contains("ratio") && !rendered.contains("rate") && !rendered.contains('%'),
        "the aggregate is counts over the measured documents, never a corpus rate"
    );
}

/// The oracle is not fooled by an occurrence the conserved source subtree
/// already carries, and it reports a loss when the band's occurrence is missing.
#[test]
fn oracle_is_not_fooled_by_source_conserved_occurrences() {
    let value = vec!["V".to_string()];
    let other = vec!["W".to_string()];
    let values = |items: &[&str]| {
        items
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<_>>()
    };

    let conserved = text_class(
        "synthetic",
        CLASS_HEADER_TITLE,
        2,
        &value,
        &values(&["V", "V"]),
        &value,
        true,
    );
    assert_eq!(
        (conserved.admitted, conserved.conserved, conserved.lost),
        (1, 1, 0)
    );
    assert_eq!(conserved.fabricated, 0);

    // The same emission without the band's contribution: a membership-only oracle
    // would still report a clean 1.0; the occurrence-net reading reports the loss.
    let lost = text_class(
        "synthetic",
        CLASS_HEADER_TITLE,
        2,
        &value,
        &values(&["V"]),
        &value,
        true,
    );
    assert_eq!((lost.conserved, lost.lost), (0, 1));

    let fabricated = text_class(
        "synthetic",
        CLASS_HEADER_TITLE,
        2,
        &value,
        &values(&["V", "V", "X"]),
        &value,
        true,
    );
    assert_eq!((fabricated.conserved, fabricated.fabricated), (1, 1));

    let source_only = text_class(
        "synthetic",
        CLASS_HEADER_TITLE,
        2,
        &value,
        &values(&["V", "V", "W"]),
        &[value.clone(), other.clone()].concat(),
        true,
    );
    assert_eq!(
        source_only.fabricated, 0,
        "a source-carried value is not fabricated"
    );

    let admitted = vec![("author-0".to_string(), "aff0".to_string())];
    let emitted = vec![
        ("author-0".to_string(), "aff0".to_string()),
        (String::new(), "aff0".to_string()),
    ];
    let bindings = binding_class(
        "synthetic",
        CLASS_AUTHOR_AFFILIATION,
        1,
        &admitted,
        &emitted,
        &[],
        false,
    );
    assert_eq!((bindings.conserved, bindings.fabricated), (1, 1));
}

/// MEM1702: a class or document with no denominator is reported as
/// `no_denominator`, never as a vacuum `1.0`; a genuine total loss is `0.0`.
#[test]
fn zero_admitted_population_is_reported_as_no_denominator_not_one() {
    // A source class nobody admits: coverage is 0.0, not `no_denominator` and not 1.0.
    let total_loss = text_class(
        "synthetic",
        CLASS_HEADER_TITLE,
        2,
        &[],
        &["V".to_string()],
        &["V".to_string()],
        true,
    );
    assert_eq!(total_loss.admitted, 0);
    assert_eq!(total_loss.admission_coverage(), Some(0.0));

    // A class the source never declares: no denominator at all, never 1.0.
    let absent = text_class("synthetic", CLASS_HEADER_TITLE, 0, &[], &[], &[], true);
    assert_eq!(absent.source_population, 0);
    assert_eq!(absent.admission_coverage(), None);
    assert_eq!(absent.admitted, 0);

    // A document whose admitted population is empty: agreement and interval are
    // `no_denominator`, and the margin is not a pass.
    let empty_document = document_metrics(&[]);
    assert_eq!(empty_document.admitted, 0);
    assert_eq!(empty_document.agreement, None);
    assert_eq!(empty_document.uncertainty, None);
    assert_eq!(empty_document.uncertainty_width, None);

    let data = load_probe_data();
    let margin = data
        .margin(AGREEMENT_METRIC)
        .expect("the declared agreement margin");
    assert_eq!(
        margin_verdict(margin, empty_document.agreement),
        Verdict::NoDenominator
    );
    assert_eq!(
        margin_verdict(margin, Some(0.0)),
        Verdict::Fail,
        "a real zero is a measured failure, not a missing denominator"
    );
}

/// The uncertainty interval is document-derived, bounded, and responds to the
/// admitted-band narrowing; it is never a constant.
#[test]
fn uncertainty_interval_is_document_derived_and_responds_to_admitted_narrowing() {
    let narrow = wilson_interval(10, 10).expect("10 of 10");
    let wide = wilson_interval(1000, 1000).expect("1000 of 1000");
    assert!(narrow.0 >= 0.0 && narrow.1 <= 1.0);
    assert!(wide.0 >= 0.0 && wide.1 <= 1.0);
    assert!(
        (narrow.1 - narrow.0) > (wide.1 - wide.0),
        "a smaller population must widen the interval"
    );
    assert!(wilson_interval(0, 0).is_none(), "no denominator");
    assert!(wilson_interval(1, 2).is_some());
}

/// The margins are read from DATA: moving only the declared threshold changes
/// the verdict of an unchanged measurement, so no threshold is baked in here.
#[test]
fn margin_verdicts_are_read_from_data_and_not_baked_into_the_oracle() {
    let mut manifest = manifest_value();
    let data = read_probe_data(&manifest).expect("the real lane");
    let declared = data
        .margin(AGREEMENT_METRIC)
        .expect("the declared agreement margin");
    assert_eq!(
        margin_verdict(declared, Some(declared.value)),
        Verdict::Pass,
        "a value exactly at the declared threshold passes"
    );

    {
        let entry = manifest
            .get_mut("external_holdout_contract")
            .and_then(|contract| contract.get_mut(MARGIN_REF_KEY))
            .and_then(|margins| margins.get_mut("margins"))
            .and_then(Value::as_sequence_mut)
            .expect("the declared margins");
        let moved = entry
            .iter_mut()
            .find(|margin| margin.get("metric").and_then(Value::as_str) == Some(AGREEMENT_METRIC))
            .expect("the declared agreement margin");
        let raised = declared.value + 1.0;
        *moved.get_mut("margin").expect("the margin value") = Value::from(raised);
    }
    let moved = read_probe_data(&manifest).expect("the mutated lane");
    let raised = moved
        .margin(AGREEMENT_METRIC)
        .expect("the declared agreement margin");
    assert_eq!(
        margin_verdict(raised, Some(declared.value)),
        Verdict::Fail,
        "the same measurement fails once only the DATA threshold moves"
    );

    // The same for an `at_most` margin: lowering it below the measured value fails.
    let mut manifest = manifest_value();
    let uncertainty = {
        let data = read_probe_data(&manifest).expect("the real lane");
        data.margin(UNCERTAINTY_METRIC)
            .expect("the declared uncertainty margin")
            .clone()
    };
    {
        let entry = manifest
            .get_mut("external_holdout_contract")
            .and_then(|contract| contract.get_mut(MARGIN_REF_KEY))
            .and_then(|margins| margins.get_mut("margins"))
            .and_then(Value::as_sequence_mut)
            .expect("the declared margins");
        let moved = entry
            .iter_mut()
            .find(|margin| margin.get("metric").and_then(Value::as_str) == Some(UNCERTAINTY_METRIC))
            .expect("the declared uncertainty margin");
        *moved.get_mut("margin").expect("the margin value") = Value::from(0.0);
    }
    let moved = read_probe_data(&manifest).expect("the mutated lane");
    let lowered = moved
        .margin(UNCERTAINTY_METRIC)
        .expect("the declared uncertainty margin");
    assert_eq!(lowered.direction, Direction::AtMost);
    assert_eq!(
        margin_verdict(lowered, Some(uncertainty.value + 0.5)),
        Verdict::Fail
    );
}

/// Anti-drift: every mutation of the declared lane is refused BY NAME.  A
/// mutated definition must never silently produce a different number.
#[test]
fn negative_matrix_catches_every_named_lane_mutation() {
    type Mutation = Box<dyn FnOnce(&mut Value)>;

    let matrix: Vec<(&str, Mutation, &str)> = vec![
        (
            "lane-removed",
            Box::new(|manifest: &mut Value| {
                manifest
                    .get_mut("external_holdout_contract")
                    .and_then(|contract| contract.as_mapping_mut())
                    .expect("the contract")
                    .remove("release_probe");
            }),
            "probe_lane_missing",
        ),
        (
            "metrics-removed",
            Box::new(|manifest: &mut Value| {
                lane_mut(manifest)
                    .as_mapping_mut()
                    .expect("the lane")
                    .remove("metrics");
            }),
            "probe_metrics_missing",
        ),
        (
            "metric-renamed",
            Box::new(|manifest: &mut Value| {
                *metrics_mut(manifest)
                    .as_sequence_mut()
                    .expect("the metrics")[0]
                    .get_mut("metric")
                    .expect("the metric name") = Value::from("renamed_metric");
            }),
            "probe_metric_vocabulary_drift",
        ),
        (
            "aggregation-pooled",
            Box::new(|manifest: &mut Value| {
                *metrics_mut(manifest)
                    .as_sequence_mut()
                    .expect("the metrics")[0]
                    .get_mut("aggregation")
                    .expect("the aggregation") = Value::from("pooled");
            }),
            "probe_metric_aggregation_not_per_document",
        ),
        (
            "zero-denominator-read-as-one",
            Box::new(|manifest: &mut Value| {
                *metrics_mut(manifest)
                    .as_sequence_mut()
                    .expect("the metrics")[0]
                    .get_mut("zero_admitted_population_behavior")
                    .expect("the zero behavior") = Value::from("one_point_zero");
            }),
            "probe_zero_admitted_population_read_as_one",
        ),
        (
            "numerator-removed",
            Box::new(|manifest: &mut Value| {
                metrics_mut(manifest)
                    .as_sequence_mut()
                    .expect("the metrics")[0]
                    .as_mapping_mut()
                    .expect("the metric")
                    .remove("numerator_definition");
            }),
            "probe_metric_definition_missing",
        ),
        (
            "threshold-duplicated",
            Box::new(|manifest: &mut Value| {
                metrics_mut(manifest)
                    .as_sequence_mut()
                    .expect("the metrics")[0]
                    .as_mapping_mut()
                    .expect("the metric")
                    .insert(Value::from("margin"), Value::from(0.5));
            }),
            "probe_metric_threshold_duplicated",
        ),
        (
            "claim-scope-corpus",
            Box::new(|manifest: &mut Value| {
                *metrics_mut(manifest)
                    .as_sequence_mut()
                    .expect("the metrics")[0]
                    .get_mut("claim_scope")
                    .expect("the claim scope") = Value::from("corpus");
            }),
            "probe_metric_claim_scope_not_local",
        ),
        (
            "margin-ref-moved",
            Box::new(|manifest: &mut Value| {
                *metrics_mut(manifest)
                    .as_sequence_mut()
                    .expect("the metrics")[0]
                    .get_mut("margin_ref")
                    .expect("the margin ref") = Value::from("external_holdout_contract.other");
            }),
            "probe_metric_margin_ref_missing",
        ),
        (
            "external-band-flipped",
            Box::new(|manifest: &mut Value| {
                *contract_mut(manifest)
                    .get_mut("execution_status")
                    .expect("the execution status") = Value::from("executed");
            }),
            "external_lane_state_flipped",
        ),
        (
            "reference-path-absolute",
            Box::new(|manifest: &mut Value| {
                *lane_mut(manifest)
                    .get_mut("reference_tei")
                    .and_then(Value::as_sequence_mut)
                    .expect("the reference rows")[0]
                    .get_mut("path")
                    .expect("the path") = Value::from("/etc/hosts");
            }),
            "probe_reference_path_absolute",
        ),
        (
            "reference-path-escape",
            Box::new(|manifest: &mut Value| {
                *lane_mut(manifest)
                    .get_mut("reference_tei")
                    .and_then(Value::as_sequence_mut)
                    .expect("the reference rows")[0]
                    .get_mut("path")
                    .expect("the path") =
                    Value::from(format!("{FIXTURE_DIR}/../../../../etc/hosts"));
            }),
            "probe_reference_path_escape",
        ),
        (
            "reference-population-shrunk",
            Box::new(|manifest: &mut Value| {
                lane_mut(manifest)
                    .get_mut("reference_tei")
                    .and_then(Value::as_sequence_mut)
                    .expect("the reference rows")
                    .pop();
            }),
            "probe_reference_population_size_drift",
        ),
        (
            "identity-handles-drifted",
            Box::new(|manifest: &mut Value| {
                *lane_mut(manifest)
                    .get_mut("digest_field_separator")
                    .expect("the field separator") = Value::from(":");
            }),
            "probe_identity_digest_drift",
        ),
        (
            "reference-source-mismatch",
            Box::new(|manifest: &mut Value| {
                *lane_mut(manifest)
                    .get_mut("reference_tei")
                    .and_then(Value::as_sequence_mut)
                    .expect("the reference rows")[0]
                    .get_mut("article_id")
                    .expect("the article id") = Value::from("9999.99999");
            }),
            "probe_reference_source_mismatch",
        ),
        (
            "reference-bytes-drift",
            Box::new(|manifest: &mut Value| {
                *lane_mut(manifest)
                    .get_mut("reference_tei")
                    .and_then(Value::as_sequence_mut)
                    .expect("the reference rows")[0]
                    .get_mut("sha256")
                    .expect("the sha") = Value::from(format!("sha256:{}", "0".repeat(64)));
            }),
            "probe_reference_bytes_drift:1203.2295",
        ),
        (
            "reference-size-drift",
            Box::new(|manifest: &mut Value| {
                *lane_mut(manifest)
                    .get_mut("reference_tei")
                    .and_then(Value::as_sequence_mut)
                    .expect("the reference rows")[0]
                    .get_mut("size_bytes")
                    .expect("the size") = Value::from(1_u64);
            }),
            "probe_reference_size_drift:1203.2295",
        ),
    ];

    let mut survivors = Vec::new();
    for (name, mutate, expected) in matrix {
        let mut manifest = manifest_value();
        mutate(&mut manifest);
        // Two stages, so a mutation that only drifts a byte pin is caught by the
        // byte check rather than silently accepted by the shape reader.
        let refused = read_probe_data(&manifest).and_then(|data| {
            data.reference
                .iter()
                .try_for_each(|row| check_reference_bytes(row).map(|_| ()))
        });
        match refused {
            Ok(()) => survivors.push(format!("{name}: no refusal (expected {expected})")),
            Err(label) if label == expected => {}
            Err(label) => {
                survivors.push(format!("{name}: refused with {label}, expected {expected}"))
            }
        }
    }
    assert!(
        survivors.is_empty(),
        "every mutated lane must be refused by its own name; survivors: {survivors:#?}"
    );
}

/// The guard: this file reaches exactly one composition entry point — the SUT —
/// and never the upstream composition helpers, so the admitted band cannot be a
/// co-simulation of the thing it measures.
#[test]
fn expected_side_does_not_call_a_composition_entry_point() {
    let src = include_str!("external_holdout_probe_semantics.rs");
    let use_block: String = src
        .lines()
        .filter(|line| line.trim_start().starts_with("use "))
        .collect();
    for name in [
        "compose_header",
        "compose_references",
        "run_header_sequence",
        "run_reference_sequence",
    ] {
        assert!(
            !use_block.contains(name),
            "{name} must not be imported into the probe expected builder"
        );
        assert!(
            !src.contains(&format!("{name}(")),
            "{name} must not be called from the probe expected builder"
        );
    }
    assert!(
        src.contains("compose_native_fulltext("),
        "the probe expected builder must reach the single SUT"
    );
    assert!(
        src.len() > 4096,
        "the guard must scan the whole expected-builder source"
    );
}

/// The SUT boundary is real: a corrupted envelope is a typed refusal, not a
/// silent accept.  This exercises the failure path the measurement relies on.
#[test]
fn the_sut_refuses_a_corrupted_envelope_instead_of_accepting_it() {
    let data = load_probe_data();
    let row = data.reference.first().expect("the first frozen reference");
    let bytes = std::fs::read(&row.path).expect("the frozen fixture");
    let xml = String::from_utf8(bytes).expect("the fixture is UTF-8");
    let identity_sha256 = row
        .declared_sha256
        .strip_prefix("sha256:")
        .unwrap_or(&row.declared_sha256)
        .to_string();
    let source = measure_source(&row.article_id, &identity_sha256, &xml);
    let admitted = build_admitted_band(&source);

    // Truncated bytes: the composer must refuse rather than emit something it
    // cannot parse.
    let corrupted = &xml[..xml.len() / 4];
    let input = NativeFulltextCompositionInput {
        java_fulltext: TeiEnvelope::new(
            corrupted.as_bytes(),
            TeiIdentity::new(GROBID_HTTP_BACKEND_ID, data.profile_fingerprint.clone()),
        ),
        identity: FulltextCompositionIdentity {
            source_sha256: source.sha256.clone(),
            profile_fingerprint: data.profile_fingerprint.clone(),
            config_fingerprint: data.config_fingerprint.clone(),
        },
        header: admitted.header.clone(),
        references: admitted.references.clone(),
        segmentation: None,
        figure: None,
        table: None,
        formula: None,
        admissions: probe_admissions(&source.sha256),
        s17_lineage: None,
    };
    match compose_native_fulltext(input) {
        NativeFulltextCompositionOutcome::Accepted(_) => {
            panic!("FATAL {PROBE} the SUT accepted a truncated envelope")
        }
        NativeFulltextCompositionOutcome::Rejected { error, .. } => {
            assert!(
                !error.to_string().is_empty(),
                "the typed refusal must name itself"
            );
        }
    }
}

/// The frozen per-class profile, per-document metrics and named narrowings: every
/// measured number is asserted, so a drift in either direction fails.
/// `documents=5` sits next to every number.
#[test]
fn frozen_per_document_profile_metrics_and_narrowings_are_asserted() {
    let data = load_probe_data();
    let measured = measure_all(&data);
    assert_eq!(
        measured.len(),
        FROZEN_PROFILE.len(),
        "{PROBE} the frozen per-document profile covers {} documents but the lane measured {}; \
         a population change requires a fresh measurement, not auditing a subset against a \
         stale table (data/parse/holdout_manifests.yaml#articles)",
        FROZEN_PROFILE.len(),
        measured.len(),
    );

    for (document, frozen) in measured.iter().zip(FROZEN_PROFILE.iter()) {
        assert_eq!(document.article_id, frozen.article_id);
        assert_eq!(document.profiles.len(), CLASS_COUNT);
        assert_eq!(document.profiles.len(), frozen.classes.len());
        for (profile, row) in document.profiles.iter().zip(frozen.classes.iter()) {
            let (class, source_population, admitted, conserved, fabricated) = *row;
            assert_eq!(profile.class, class, "{}: class order", document.article_id);
            assert_eq!(
                profile.source_population, source_population,
                "{}/{class}: source_population drifted (measured {}, frozen {source_population})",
                document.article_id, profile.source_population,
            );
            assert_eq!(
                profile.admitted, admitted,
                "{}/{class}: admitted drifted (measured {}, frozen {admitted})",
                document.article_id, profile.admitted,
            );
            assert_eq!(
                profile.conserved, conserved,
                "{}/{class}: conserved drifted (measured {}, frozen {conserved})",
                document.article_id, profile.conserved,
            );
            assert_eq!(
                profile.lost,
                admitted - conserved,
                "{}/{class}: lost must be admitted - conserved",
                document.article_id,
            );
            assert_eq!(
                profile.fabricated, fabricated,
                "{}/{class}: fabricated drifted (measured {}, frozen {fabricated})",
                document.article_id, profile.fabricated,
            );
        }
    }

    assert_eq!(
        measured.len(),
        FROZEN_METRICS.len(),
        "{PROBE} the frozen per-document metrics cover {} documents but the lane measured {}",
        FROZEN_METRICS.len(),
        measured.len(),
    );
    for (document, frozen) in measured.iter().zip(FROZEN_METRICS.iter()) {
        assert_eq!(document.article_id, frozen.article_id);
        assert_eq!(
            document.metrics.source_population, frozen.source_population,
            "{}: frozen source population",
            document.article_id
        );
        assert_eq!(
            document.metrics.admitted, frozen.admitted,
            "{}: frozen admitted population",
            document.article_id
        );
        assert_eq!(
            document.metrics.conserved, frozen.conserved,
            "{}: frozen conserved occurrences",
            document.article_id
        );
        assert_eq!(
            document.metrics.lost, 0,
            "{}: no admitted occurrence may be lost",
            document.article_id
        );
        assert_eq!(
            document.metrics.fabricated, 0,
            "{}: no emitted carrier may be fabricated",
            document.article_id
        );
        assert_eq!(
            micros(
                document
                    .metrics
                    .agreement
                    .expect("the agreement has a denominator")
            ),
            frozen.agreement_micros,
            "{}: frozen document_agreement_ratio",
            document.article_id
        );
        let (low, high) = document
            .metrics
            .uncertainty
            .expect("the interval has a denominator");
        assert_eq!(
            micros(low),
            frozen.low_micros,
            "{}: frozen low",
            document.article_id
        );
        assert_eq!(
            micros(high),
            frozen.high_micros,
            "{}: frozen high",
            document.article_id
        );
        assert_eq!(
            micros(
                document
                    .metrics
                    .uncertainty_width
                    .expect("the width has a denominator")
            ),
            frozen.width_micros,
            "{}: frozen document_uncertainty_interval_width",
            document.article_id
        );
        assert_eq!(
            frozen.width_micros,
            frozen.high_micros - frozen.low_micros,
            "{}: the width is upper minus lower",
            document.article_id
        );
        eprintln!(
            "PROBE FROZEN documents={} {} source_population={} admitted={} conserved={} \
             agreement_micros={} low_micros={} high_micros={} width_micros={}",
            measured.len(),
            frozen.article_id,
            frozen.source_population,
            frozen.admitted,
            frozen.conserved,
            frozen.agreement_micros,
            frozen.low_micros,
            frozen.high_micros,
            frozen.width_micros,
        );
    }

    // Every narrowing is named and frozen; nothing is silently shrunk.
    let mut observed_narrowings: Vec<(&str, &str, usize, usize)> = measured
        .iter()
        .flat_map(|document| {
            document
                .profiles
                .iter()
                .filter(|profile| profile.admitted < profile.source_population)
                .map(|profile| {
                    (
                        document.article_id.as_str(),
                        profile.class,
                        profile.source_population,
                        profile.admitted,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let mut expected_narrowings = FROZEN_NARROWINGS.to_vec();
    observed_narrowings.sort();
    expected_narrowings.sort();
    assert_eq!(
        observed_narrowings, expected_narrowings,
        "the admitted band's narrowings must be exactly the declared, named ones"
    );

    // The classes with no source population are reported as `no_denominator`,
    // never as a vacuum 1.0.
    let mut observed_void: Vec<(&str, &str)> = measured
        .iter()
        .flat_map(|document| {
            document
                .profiles
                .iter()
                .filter(|profile| profile.source_population == 0)
                .map(|profile| (document.article_id.as_str(), profile.class))
                .collect::<Vec<_>>()
        })
        .collect();
    let mut expected_void = FROZEN_NO_DENOMINATOR_CLASSES.to_vec();
    observed_void.sort();
    expected_void.sort();
    assert_eq!(
        observed_void, expected_void,
        "the classes without a denominator must be exactly the declared ones"
    );
    for (article_id, class) in &observed_void {
        let document = measured
            .iter()
            .find(|document| document.article_id == *article_id)
            .expect("the class comes from a measured document");
        let profile = document
            .profiles
            .iter()
            .find(|profile| profile.class == *class)
            .expect("the class is measured");
        assert_eq!(
            profile.admission_coverage(),
            None,
            "{article_id}/{class}: no denominator must be reported as no_denominator, not 1.0"
        );
    }
}
