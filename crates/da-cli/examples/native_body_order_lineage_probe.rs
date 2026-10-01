#![cfg(feature = "pdf-layout")]
//! M514-1lss2s / S05 / T01 — additive order-lineage diagnostic (read-only).
//!
//! The accepted S05/T01 character comparison builds its native token stream from
//! the positioned donor's lexical sidecar in delivered order
//! (`page_ordinal, source_ordinal`) and compares it against an independent
//! preserved-oracle token stream. A read-only diagnosis raised one hypothesis:
//! the low exact count (814 of 70 323 corpus-wide) is explained because that
//! stream is extraction order rather than the admitted token reading order
//! (`token_ordinal`), which a reading-order reorder can change while
//! `source_ordinal` is preserved.
//!
//! This example tests that hypothesis without touching the accepted measurement.
//! It is additive: it reads the same three pinned PDFs and the same independent
//! preserved-oracle bytes, writes a **separate** report, and changes no
//! primary-lane value, no frozen S03/S04 artifact, and no accepted API.
//!
//! Three native order axes are measured against the same oracle:
//!
//! * `raw_source_order` — the delivered lexical order, which is what the accepted
//!   probe measures;
//! * `positioned_admitted_order` — the admitted token order of the positioned
//!   lane, resolved through the accepted word projection;
//! * `xy_cut_admitted_order` — the admitted token order after the reading-order
//!   reorder, retained under its typed `segmenter_rejected_lattice` label.
//!
//! Before any comparison the three order maps are audited: every axis must name
//! exactly the same document token identities with the same text digest. Order is
//! therefore never taken from the oracle, no axis can silently drop or duplicate
//! a token, and a reorder cannot masquerade as identity.
//!
//! Exact/uncertain counts are a comparison, not a quality proof. A zero
//! uncertain count would still not be a clearance, and the P1 and P2 axes are
//! reported side by side with no prioritisation.

use da_adapters::{LayoutDonorLimits, PdfInspectorLayoutDonor, reorder_shadow_with_xy_cut};
use da_parse::{
    BodyAggregateInput, BodyCasePolicy, BodyCoverageAggregate, BodyCoverageGap,
    BodyDocumentOutcome, BodyJoinPolicy, BodyNormalization, BodyOracleCounts,
    BodyOracleIndependence, BodyOracleProvenance, BodyOrderEntry, BodyOrderLane,
    BodyOrderLaneDisposition, BodyOrderLineage, BodyOrderLineageAudit, BodyPlaneLocalization,
    BodyPlaneVerdict, BodyScope, BodyScopeClass, BodyScopeSide, BodyUnitInventory, BodyUnitRung,
    DocumentTokenId, FullSemanticRegion, HeadingRegionConfig, NativeBodyLengths, NativeBodyText,
    NativeBodyTextError, SegmentationConfig, aggregate_body_coverage, assign_heading_regions,
    audit_body_order_lineage, build_native_body_text, build_native_word_region_carrier,
    classify_body_coverage, classify_body_plane, localize_body_plane_checked, ordered_fragments,
    plane_verdict_for_population, project_native_words, segment_document_full,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

const REPORT_SCHEMA: &str = "native-body-order-lineage.v1";
const DEFAULT_REPORT: &str = "artifacts/parse_compat/native_body_order_lineage.json";
const CORPUS_ID: &str = "s13-same-input-pdf-corpus";
const LANE: &str = "pdf_inspector_positioned";
const ORACLE_GENERATOR: &str = "preserved-java-tei-lexical-scanner";
const PROVENANCE_PATH: &str = "crates/da-parse/tests/fixtures/preserved/provenance.yaml";
const NO_CLEARANCE: &str = "Comparison, not a quality proof: a zero uncertain count is not a clearance, and no axis is \
     prioritised over another.";

/// Artifacts that predate this diagnostic. Their bytes are recorded, never
/// rewritten, so a later reader can see that this run preserved them.
const PRIOR_ARTIFACTS: [&str; 4] = [
    "artifacts/parse_compat/native_body_character_shadow.json",
    "artifacts/parse_compat/native_body_character_scope_bound.prior_repair.json",
    "artifacts/parse_compat/native_body_character_scope_bound.repair2_artifact.json",
    "artifacts/parse_compat/native_body_character_scope_bound.json",
];

const RUNG: BodyUnitRung = BodyUnitRung::new(
    BodyNormalization::ZeroWidthAndLigatureNormalized,
    BodyCasePolicy::CaseFolded,
);

const EXCLUDED_SUBTREES: [&str; 5] = ["figure", "table", "formula", "note", "figDesc"];
const BLOCK_TAGS: [&str; 10] = [
    "p", "head", "div", "l", "item", "label", "list", "ab", "seg", "s",
];

#[derive(Deserialize)]
struct Manifest {
    corpus: Corpus,
}
#[derive(Deserialize)]
struct Corpus {
    id: String,
    #[serde(default)]
    page_denominator: Option<usize>,
    documents: Vec<ManifestDocument>,
}
#[derive(Deserialize)]
struct ManifestDocument {
    id: String,
    source_pdf: String,
    sha256: String,
}
#[derive(Deserialize)]
struct Provenance {
    fixtures: Vec<ProvenanceFixture>,
}
#[derive(Deserialize)]
struct ProvenanceFixture {
    path: String,
    sha256: String,
}

#[derive(Serialize)]
struct Report {
    schema_version: &'static str,
    corpus_id: String,
    page_denominator: Option<usize>,
    lane: &'static str,
    claim_scope: &'static str,
    product_coverage_effect: &'static str,
    thresholds: &'static str,
    superseded_artifact: &'static str,
    clearance_note: &'static str,
    oracle: OracleIdentity,
    order_axes: Vec<OrderAxis>,
    scopes: ScopeDeclaration,
    prior_artifacts: Vec<PriorArtifactBinding>,
    input_bindings: Vec<InputBinding>,
    documents: Vec<DocumentRow>,
    aggregate: Aggregate,
    locks: BTreeMap<&'static str, &'static str>,
}
#[derive(Serialize)]
struct OracleIdentity {
    generator_id: &'static str,
    provenance: &'static str,
    independence: &'static str,
    binding_check: &'static str,
}
#[derive(Serialize)]
struct OrderAxis {
    lane: BodyOrderLane,
    disposition: BodyOrderLaneDisposition,
    /// The literal label the slice uses; the rejected lattice keeps its typed
    /// name here rather than a serde-internal rendering.
    disposition_label: &'static str,
    promoted: bool,
    primary_lane_unchanged: bool,
}
#[derive(Serialize)]
struct ScopeDeclaration {
    native_plane: BodyScope,
    native_body_verbatim: BodyScope,
    native_body_word: BodyScope,
    oracle: BodyScope,
}
#[derive(Serialize)]
struct PriorArtifactBinding {
    path: &'static str,
    sha256: Option<String>,
}
#[derive(Serialize)]
struct InputBinding {
    document_id: String,
    source_pdf: String,
    source_pdf_sha256_pinned: String,
    source_pdf_sha256_observed: String,
    oracle_tei_path: String,
    oracle_tei_sha256_pinned: String,
    oracle_tei_sha256_observed: String,
}
#[derive(Serialize)]
struct DocumentRow {
    document_id: String,
    pages: usize,
    native_lexical_tokens: usize,
    pdf_inspector_spans: usize,
    order_audit: Option<BodyOrderLineageAudit>,
    order_audit_absence: Option<String>,
    orders: Vec<OrderRow>,
    p2_primary_lane: P2Row,
    oracle: NativeBodyLengths,
}
#[derive(Serialize)]
struct OrderRow {
    lane: BodyOrderLane,
    disposition: BodyOrderLaneDisposition,
    status: String,
    absence: Option<String>,
    token_count: usize,
    plane_units: usize,
    localization: Option<BodyPlaneLocalization>,
    verdict: Option<BodyPlaneVerdict>,
}
#[derive(Serialize)]
struct P2Row {
    s03_parent_span: BandRow,
    word_heading: BandRow,
}
#[derive(Serialize)]
struct BandRow {
    status: String,
    absence: Option<String>,
    token_count: usize,
    gap: Option<BodyCoverageGap>,
}
#[derive(Serialize)]
struct Aggregate {
    order_axes: Vec<AggregateOrderRow>,
    p2_primary_lane: P2AggregateRow,
}
#[derive(Serialize)]
struct AggregateOrderRow {
    lane: BodyOrderLane,
    disposition: BodyOrderLaneDisposition,
    documents_declared: usize,
    documents_measured: usize,
    population_complete: bool,
    localization: BodyPlaneLocalization,
    verdict: BodyPlaneVerdict,
}
#[derive(Serialize)]
struct P2AggregateRow {
    s03_parent_span: BodyCoverageAggregate,
    word_heading: BodyCoverageAggregate,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn report_path() -> PathBuf {
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next()
        && arg == "--report"
        && let Some(path) = args.next()
    {
        return PathBuf::from(path);
    }
    root().join(DEFAULT_REPORT)
}

fn fixture_file(document_id: &str) -> Option<&'static str> {
    match document_id {
        "hub-2507.19457" => Some("hub_2507.19457.tei.xml"),
        "rlm-2512.24601" => Some("rlm_2512.24601.tei.xml"),
        "reca-2605.26525v1" => Some("reca_2605.26525v1.tei.xml"),
        _ => None,
    }
}

fn token_id(page_ordinal: u32, source_ordinal: u32) -> DocumentTokenId {
    DocumentTokenId {
        page_ordinal,
        source_ordinal,
    }
}

// ───────────────────────── independent character oracle ─────────────────────────

fn tag_name(tag: &str) -> (&str, bool) {
    let is_close = tag.starts_with('/');
    let rest = tag.strip_prefix('/').unwrap_or(tag);
    let name = rest
        .split(|c: char| c.is_whitespace() || c == '/')
        .find(|token| !token.is_empty())
        .unwrap_or("");
    (name, is_close)
}

fn decode_entity(entity: &str) -> Option<String> {
    match entity {
        "amp" => Some("&".to_owned()),
        "lt" => Some("<".to_owned()),
        "gt" => Some(">".to_owned()),
        "quot" => Some("\"".to_owned()),
        "apos" => Some("'".to_owned()),
        _ => {
            let digits = entity.strip_prefix('#')?;
            let code = if let Some(hex) = digits.strip_prefix(['x', 'X']) {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                digits.parse::<u32>().ok()?
            };
            char::from_u32(code).map(String::from)
        }
    }
}

fn strip_tags_and_decode(fragment: &str) -> String {
    let mut out = String::new();
    let mut excluded: Vec<String> = Vec::new();
    let mut index = 0usize;
    while index < fragment.len() {
        let ch = fragment[index..].chars().next().expect("in-bounds char");
        if ch == '<' {
            let rest = &fragment[index..];
            let close = match rest.find('>') {
                Some(close) => close,
                None => break,
            };
            let (name, is_close) = tag_name(&rest[1..close]);
            if is_close {
                if excluded.last().map(String::as_str) == Some(name) {
                    excluded.pop();
                }
            } else if EXCLUDED_SUBTREES.contains(&name) {
                if !rest[1..close].ends_with('/') {
                    excluded.push(name.to_owned());
                }
            } else if excluded.is_empty() && BLOCK_TAGS.contains(&name) {
                out.push('\n');
            }
            index += close + 1;
        } else if ch == '&' && excluded.is_empty() {
            let rest = &fragment[index..];
            if let Some(semi) = rest.find(';')
                && let Some(decoded) = decode_entity(&rest[1..semi])
            {
                out.push_str(&decoded);
                index += semi + 1;
                continue;
            }
            out.push(ch);
            index += ch.len_utf8();
        } else {
            if excluded.is_empty() {
                out.push(ch);
            }
            index += ch.len_utf8();
        }
    }
    out
}

fn body_text(tei: &str) -> Option<String> {
    let start = tei.find("<body")?;
    let content_start = tei[start..].find('>')? + start + 1;
    let end = tei[content_start..].find("</body>")? + content_start;
    Some(strip_tags_and_decode(&tei[content_start..end]))
}

fn provenance() -> BodyOracleProvenance {
    BodyOracleProvenance {
        generator_id: ORACLE_GENERATOR.to_owned(),
        independence: BodyOracleIndependence::Independent,
    }
}

fn absence_code(error: &NativeBodyTextError) -> String {
    format!("{error:?}")
}

fn native_plane_scope() -> BodyScope {
    BodyScope::new(
        BodyScopeSide::NativePlane,
        vec![
            BodyScopeClass::BodyProse,
            BodyScopeClass::VisualEvidence,
            BodyScopeClass::References,
            BodyScopeClass::BackMatter,
            BodyScopeClass::PageFurniture,
            BodyScopeClass::HeaderMetadata,
        ],
        BodyJoinPolicy::VerbatimTokenConcat,
        RUNG.normalization,
        RUNG.case,
    )
}

fn body_verbatim_scope() -> BodyScope {
    BodyScope::new(
        BodyScopeSide::NativeBodyRegion,
        vec![BodyScopeClass::BodyProse],
        BodyJoinPolicy::VerbatimTokenConcat,
        RUNG.normalization,
        RUNG.case,
    )
}

fn body_word_scope() -> BodyScope {
    BodyScope::new(
        BodyScopeSide::NativeBodyRegion,
        vec![BodyScopeClass::BodyProse],
        BodyJoinPolicy::SingleSpaceBetweenWordTokens,
        RUNG.normalization,
        RUNG.case,
    )
}

fn oracle_scope() -> BodyScope {
    BodyScope::new(
        BodyScopeSide::PreservedBodyProse,
        vec![BodyScopeClass::BodyProse],
        BodyJoinPolicy::VerbatimTokenConcat,
        RUNG.normalization,
        RUNG.case,
    )
}

fn order_axes() -> Vec<OrderAxis> {
    BodyOrderLane::ALL
        .into_iter()
        .map(|lane| OrderAxis {
            lane,
            disposition: lane.disposition(),
            disposition_label: lane.disposition().wire(),
            promoted: false,
            primary_lane_unchanged: true,
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_bytes = std::fs::read(root().join("data/parse/layout_bakeoff.yaml"))?;
    let manifest: Manifest = serde_yaml::from_slice(&manifest_bytes)?;
    let provenance_bytes = std::fs::read(root().join(PROVENANCE_PATH))?;
    let oracle_provenance: Provenance = serde_yaml::from_slice(&provenance_bytes)?;

    let prior_artifacts = PRIOR_ARTIFACTS
        .into_iter()
        .map(|path| PriorArtifactBinding {
            path,
            sha256: std::fs::read(root().join(path))
                .ok()
                .map(|bytes| sha256_hex(&bytes)),
        })
        .collect::<Vec<_>>();

    let declared: Vec<String> = manifest
        .corpus
        .documents
        .iter()
        .map(|doc| doc.id.clone())
        .collect();
    let scopes = ScopeDeclaration {
        native_plane: native_plane_scope(),
        native_body_verbatim: body_verbatim_scope(),
        native_body_word: body_word_scope(),
        oracle: oracle_scope(),
    };
    let donor = PdfInspectorLayoutDonor;
    let limits = LayoutDonorLimits::default();

    let mut rows = Vec::new();
    let mut bindings = Vec::new();
    let mut s03_outcomes = Vec::new();
    let mut word_outcomes = Vec::new();
    let mut lane_localizations: BTreeMap<BodyOrderLane, BodyPlaneLocalization> = BTreeMap::new();
    let mut lane_measured: BTreeMap<BodyOrderLane, usize> = BTreeMap::new();

    for document in &manifest.corpus.documents {
        // ── PDF byte binding ────────────────────────────────────────────────
        let pdf_bytes = std::fs::read(root().join(&document.source_pdf))?;
        let pdf_observed = sha256_hex(&pdf_bytes);
        if pdf_observed != document.sha256 {
            return Err(format!(
                "sha256 drift for PDF {}: {pdf_observed} != {}",
                document.id, document.sha256
            )
            .into());
        }
        // ── Oracle byte binding (producing-run sha256 check) ────────────────
        let file = fixture_file(&document.id)
            .ok_or_else(|| format!("unknown document binding: {}", document.id))?;
        let tei_path = format!("crates/da-parse/tests/fixtures/preserved/{file}");
        let pinned = oracle_provenance
            .fixtures
            .iter()
            .find(|fixture| fixture.path.ends_with(file))
            .ok_or_else(|| format!("oracle fixture not bound in provenance: {file}"))?;
        let tei_bytes = std::fs::read(root().join(&tei_path))?;
        let tei_observed = sha256_hex(&tei_bytes);
        if tei_observed != pinned.sha256 {
            return Err(format!(
                "sha256 drift for oracle {}: {tei_observed} != {}",
                document.id, pinned.sha256
            )
            .into());
        }
        let oracle_text = body_text(std::str::from_utf8(&tei_bytes)?)
            .ok_or_else(|| format!("oracle {} carries no body text", document.id))?;
        let oracle = BodyOracleCounts {
            generator_id: ORACLE_GENERATOR.to_owned(),
            lengths: NativeBodyLengths::measure(&oracle_text),
        };
        bindings.push(InputBinding {
            document_id: document.id.clone(),
            source_pdf: document.source_pdf.clone(),
            source_pdf_sha256_pinned: document.sha256.clone(),
            source_pdf_sha256_observed: pdf_observed,
            oracle_tei_path: tei_path,
            oracle_tei_sha256_pinned: pinned.sha256.clone(),
            oracle_tei_sha256_observed: tei_observed,
        });

        // ── Native plane ────────────────────────────────────────────────────
        let observation = donor.extract_shadow(&pdf_bytes, &document.id, limits)?;
        let oracle_units = BodyUnitInventory::from_text(&oracle_text, RUNG);

        // Identity → text and identity → admitted ordinal, both read from the
        // native side only. The oracle is never consulted for order.
        let mut text_by_id: BTreeMap<DocumentTokenId, &str> = BTreeMap::new();
        for lexeme in &observation.lexemes {
            text_by_id.insert(
                token_id(lexeme.page_ordinal, lexeme.source_ordinal),
                lexeme.text(),
            );
        }
        let mut ordinal_by_id: BTreeMap<DocumentTokenId, u32> = BTreeMap::new();
        for page in &observation.document.pages {
            for token in &page.tokens {
                ordinal_by_id.insert(
                    token_id(token.page_ordinal, token.source_ordinal),
                    token.token_ordinal,
                );
            }
        }

        // Axis 1 — delivered lexical order (what the accepted probe measures).
        let raw_entries = observation
            .lexemes
            .iter()
            .map(|lexeme| {
                let id = token_id(lexeme.page_ordinal, lexeme.source_ordinal);
                BodyOrderEntry {
                    id,
                    token_ordinal: ordinal_by_id.get(&id).copied().unwrap_or(0),
                    text_digest: lexeme.text_digest.clone(),
                }
            })
            .collect::<Vec<_>>();
        let raw_lineage = BodyOrderLineage::build(
            document.id.clone(),
            BodyOrderLane::RawSourceOrder,
            raw_entries,
        )?;

        // Axis 2 — the positioned lane's admitted token order reached through the
        // accepted word projection.
        let projection = project_native_words(&observation.document, &observation.lexemes);
        let (admitted_lineage, admitted_absence) = match &projection {
            Ok(projection) => {
                let entries = projection
                    .parents
                    .iter()
                    .map(|parent| BodyOrderEntry {
                        id: token_id(parent.page_ordinal, parent.source_ordinal),
                        token_ordinal: parent.token_ordinal,
                        text_digest: parent.parent_text_digest.clone(),
                    })
                    .collect::<Vec<_>>();
                match BodyOrderLineage::build(
                    document.id.clone(),
                    BodyOrderLane::PositionedAdmittedOrder,
                    entries,
                ) {
                    Ok(lineage) => (Some(lineage), None),
                    Err(error) => (None, Some(format!("{error:?}"))),
                }
            }
            Err(error) => (None, Some(format!("{error:?}"))),
        };

        // Axis 3 — the admitted token order after the reading-order reorder. The
        // rejected lattice keeps its typed disposition; it is measured here as a
        // diagnostic axis only, never promoted.
        let xy = reorder_shadow_with_xy_cut(observation.clone())?;
        let xy_entries = xy
            .document
            .pages
            .iter()
            .flat_map(|page| page.tokens.iter())
            .map(|token| BodyOrderEntry {
                id: token_id(token.page_ordinal, token.source_ordinal),
                token_ordinal: token.token_ordinal,
                text_digest: token.text_digest.clone(),
            })
            .collect::<Vec<_>>();
        let xy_lineage = BodyOrderLineage::build(
            document.id.clone(),
            BodyOrderLane::XyCutAdmittedOrder,
            xy_entries,
        )?;

        // ── Order-lineage audit before any comparison ──────────────────────
        let mut lineages = vec![raw_lineage, xy_lineage];
        if let Some(lineage) = admitted_lineage {
            lineages.insert(1, lineage);
        }
        let (order_audit, order_audit_absence) = match audit_body_order_lineage(&lineages) {
            Ok(audit) => (Some(audit), None),
            Err(error) => (None, Some(format!("{error:?}"))),
        };

        // ── P1 per axis, measured against the same oracle ─────────────────
        let mut order_rows = Vec::new();
        // A lane whose order map could not be built is reported as a typed
        // absence, never dropped from the axis set.
        if let Some(absence) = admitted_absence {
            order_rows.push(OrderRow {
                lane: BodyOrderLane::PositionedAdmittedOrder,
                disposition: BodyOrderLane::PositionedAdmittedOrder.disposition(),
                status: "typed_absence".to_owned(),
                absence: Some(absence),
                token_count: 0,
                plane_units: 0,
                localization: None,
                verdict: None,
            });
        }
        for lineage in &lineages {
            let fragments = match ordered_fragments(lineage, |id| text_by_id.get(&id).copied()) {
                Ok(fragments) => fragments,
                Err(error) => {
                    order_rows.push(OrderRow {
                        lane: lineage.lane,
                        disposition: lineage.disposition,
                        status: "typed_absence".to_owned(),
                        absence: Some(format!("{error:?}")),
                        token_count: lineage.len(),
                        plane_units: 0,
                        localization: None,
                        verdict: None,
                    });
                    continue;
                }
            };
            let plane_units = BodyUnitInventory::from_fragments(fragments.iter().copied(), RUNG);
            let localization = localize_body_plane_checked(&plane_units, &oracle_units)?;
            let verdict = classify_body_plane(&localization);
            lane_localizations
                .entry(lineage.lane)
                .or_insert_with(BodyPlaneLocalization::empty)
                .add(&localization);
            *lane_measured.entry(lineage.lane).or_default() += 1;
            order_rows.push(OrderRow {
                lane: lineage.lane,
                disposition: lineage.disposition,
                status: "measured".to_owned(),
                absence: None,
                token_count: lineage.len(),
                plane_units: plane_units.total(),
                localization: Some(localization),
                verdict: Some(verdict),
            });
        }
        // Declaration order, so an absent middle axis does not reorder the set.
        order_rows.sort_by_key(|row| row.lane);

        // ── P2 (unchanged primary lane) ────────────────────────────────────
        let primary_plane_units = BodyUnitInventory::from_fragments(
            observation.lexemes.iter().map(|lexeme| lexeme.text()),
            RUNG,
        );
        let s03_row = match segment_document_full(
            &observation.document,
            &observation.lattice,
            &observation.lexemes,
            SegmentationConfig::default(),
        ) {
            Ok(segmentation) => {
                match build_native_body_text(
                    &observation.document,
                    &observation.lattice,
                    &observation.lexemes,
                    &segmentation,
                    RUNG,
                ) {
                    Ok(native) => match classify_body_coverage(
                        &document.id,
                        &native,
                        &primary_plane_units,
                        &oracle,
                        &oracle_units,
                        &provenance(),
                        &scopes.native_body_verbatim,
                        &scopes.oracle,
                    ) {
                        Ok(gap) => {
                            s03_outcomes.push(BodyDocumentOutcome::Measured(gap.clone()));
                            BandRow {
                                status: "measured".to_owned(),
                                absence: None,
                                token_count: native.token_count,
                                gap: Some(gap),
                            }
                        }
                        Err(error) => {
                            let reason = absence_code(&error);
                            s03_outcomes.push(BodyDocumentOutcome::Absent {
                                document_id: document.id.clone(),
                                reason: reason.clone(),
                            });
                            BandRow {
                                status: "typed_absence".to_owned(),
                                absence: Some(reason),
                                token_count: 0,
                                gap: None,
                            }
                        }
                    },
                    Err(error) => {
                        let reason = format!("{error:?}");
                        s03_outcomes.push(BodyDocumentOutcome::Absent {
                            document_id: document.id.clone(),
                            reason: reason.clone(),
                        });
                        BandRow {
                            status: "typed_absence".to_owned(),
                            absence: Some(reason),
                            token_count: 0,
                            gap: None,
                        }
                    }
                }
            }
            Err(error) => {
                let reason = format!("{error:?}");
                s03_outcomes.push(BodyDocumentOutcome::Absent {
                    document_id: document.id.clone(),
                    reason: reason.clone(),
                });
                BandRow {
                    status: "terminal_failure".to_owned(),
                    absence: Some(reason),
                    token_count: 0,
                    gap: None,
                }
            }
        };

        let word_row = match projection {
            Ok(projection) => match assign_heading_regions(
                &observation.document,
                &observation.lattice,
                &projection,
                HeadingRegionConfig::default(),
            ) {
                Ok(heading) => match build_native_word_region_carrier(
                    &observation.document,
                    &observation.lattice,
                    &projection,
                    &heading,
                    &[FullSemanticRegion::Body],
                ) {
                    Ok(carrier) => match carrier.run(FullSemanticRegion::Body) {
                        Some(run) => {
                            let fragments: Vec<&str> =
                                run.tokens.iter().map(|token| token.text()).collect();
                            match NativeBodyText::from_fragments(
                                document.id.clone(),
                                document.sha256.clone(),
                                run.tokens.len(),
                                None,
                                BodyJoinPolicy::SingleSpaceBetweenWordTokens,
                                fragments.iter().copied(),
                                RUNG,
                            ) {
                                Ok(native) => match classify_body_coverage(
                                    &document.id,
                                    &native,
                                    &primary_plane_units,
                                    &oracle,
                                    &oracle_units,
                                    &provenance(),
                                    &scopes.native_body_word,
                                    &scopes.oracle,
                                ) {
                                    Ok(gap) => {
                                        word_outcomes
                                            .push(BodyDocumentOutcome::Measured(gap.clone()));
                                        BandRow {
                                            status: "measured".to_owned(),
                                            absence: None,
                                            token_count: run.tokens.len(),
                                            gap: Some(gap),
                                        }
                                    }
                                    Err(error) => {
                                        let reason = absence_code(&error);
                                        word_outcomes.push(BodyDocumentOutcome::Absent {
                                            document_id: document.id.clone(),
                                            reason: reason.clone(),
                                        });
                                        BandRow {
                                            status: "typed_absence".to_owned(),
                                            absence: Some(reason),
                                            token_count: 0,
                                            gap: None,
                                        }
                                    }
                                },
                                Err(error) => {
                                    let reason = format!("{error:?}");
                                    word_outcomes.push(BodyDocumentOutcome::Absent {
                                        document_id: document.id.clone(),
                                        reason: reason.clone(),
                                    });
                                    BandRow {
                                        status: "typed_absence".to_owned(),
                                        absence: Some(reason),
                                        token_count: 0,
                                        gap: None,
                                    }
                                }
                            }
                        }
                        None => {
                            word_outcomes.push(BodyDocumentOutcome::Absent {
                                document_id: document.id.clone(),
                                reason: "NoBodyTokens".to_owned(),
                            });
                            BandRow {
                                status: "typed_absence".to_owned(),
                                absence: Some("NoBodyTokens".to_owned()),
                                token_count: 0,
                                gap: None,
                            }
                        }
                    },
                    Err(error) => {
                        let reason = format!("{error:?}");
                        word_outcomes.push(BodyDocumentOutcome::Absent {
                            document_id: document.id.clone(),
                            reason: reason.clone(),
                        });
                        BandRow {
                            status: "terminal_failure".to_owned(),
                            absence: Some(reason),
                            token_count: 0,
                            gap: None,
                        }
                    }
                },
                Err(error) => {
                    let reason = format!("{error:?}");
                    word_outcomes.push(BodyDocumentOutcome::Absent {
                        document_id: document.id.clone(),
                        reason: reason.clone(),
                    });
                    BandRow {
                        status: "terminal_failure".to_owned(),
                        absence: Some(reason),
                        token_count: 0,
                        gap: None,
                    }
                }
            },
            Err(error) => {
                let reason = format!("{error:?}");
                word_outcomes.push(BodyDocumentOutcome::Absent {
                    document_id: document.id.clone(),
                    reason: reason.clone(),
                });
                BandRow {
                    status: "terminal_failure".to_owned(),
                    absence: Some(reason),
                    token_count: 0,
                    gap: None,
                }
            }
        };

        let native_lexical_tokens = observation
            .document
            .pages
            .iter()
            .flat_map(|page| page.tokens.iter())
            .filter(|token| token.kind.is_lexical())
            .count();
        let pdf_inspector_spans = observation.lexemes.len();
        for row in &order_rows {
            println!(
                "order-lineage {document}: axis {} [status {} tokens {} plane_units {} exact {} boundary {} uncertain {} absent {} verdict {:?}]",
                row.disposition.wire(),
                row.status,
                row.token_count,
                row.plane_units,
                row.localization
                    .as_ref()
                    .map(|l| l.exact_in_plane)
                    .unwrap_or(0),
                row.localization
                    .as_ref()
                    .map(|l| l.boundary_variant_from_plane)
                    .unwrap_or(0),
                row.localization
                    .as_ref()
                    .map(BodyPlaneLocalization::uncertain_from_plane)
                    .unwrap_or(0),
                row.localization
                    .as_ref()
                    .map(|l| l.absent_from_plane)
                    .unwrap_or(0),
                row.verdict.as_ref().map(|v| v.verdict),
                document = document.id,
            );
        }
        println!(
            "order-lineage {id}: pages {} spans {} s03 [{} tokens {:?}] word [{} tokens {:?}] oracle {:?}",
            observation.document.pages.len(),
            pdf_inspector_spans,
            s03_row.token_count,
            s03_row.gap.as_ref().map(|g| g.verdict),
            word_row.token_count,
            word_row.gap.as_ref().map(|g| g.verdict),
            oracle.lengths.zero_width_normalized,
            id = document.id,
        );

        rows.push(DocumentRow {
            document_id: document.id.clone(),
            pages: observation.document.pages.len(),
            native_lexical_tokens,
            pdf_inspector_spans,
            order_audit,
            order_audit_absence,
            orders: order_rows,
            p2_primary_lane: P2Row {
                s03_parent_span: s03_row,
                word_heading: word_row,
            },
            oracle: oracle.lengths,
        });
    }

    let mut order_axis_aggregate = Vec::new();
    for lane in BodyOrderLane::ALL {
        let localization = lane_localizations
            .get(&lane)
            .cloned()
            .unwrap_or_else(BodyPlaneLocalization::empty);
        let measured = lane_measured.get(&lane).copied().unwrap_or(0);
        let verdict = plane_verdict_for_population(&localization, declared.len(), measured);
        order_axis_aggregate.push(AggregateOrderRow {
            lane,
            disposition: lane.disposition(),
            documents_declared: declared.len(),
            documents_measured: measured,
            population_complete: !declared.is_empty() && measured == declared.len(),
            localization,
            verdict,
        });
    }

    let locks = [
        ("claim_scope", "diagnostic_only"),
        ("product_coverage_effect", "none"),
        ("promotion_state", "candidate"),
        ("auto_promote", "forbidden"),
        ("graph_mutation", "forbidden"),
        ("import_eligible", "false"),
        ("primary_lane", "unchanged"),
    ]
    .into_iter()
    .collect();

    let s03_aggregate = aggregate_body_coverage(&BodyAggregateInput {
        corpus_id: manifest.corpus.id.clone(),
        declared_documents: declared.clone(),
        outcomes: s03_outcomes,
    });
    let word_aggregate = aggregate_body_coverage(&BodyAggregateInput {
        corpus_id: manifest.corpus.id.clone(),
        declared_documents: declared.clone(),
        outcomes: word_outcomes,
    });

    let report = Report {
        schema_version: REPORT_SCHEMA,
        corpus_id: CORPUS_ID.to_owned(),
        page_denominator: manifest.corpus.page_denominator,
        lane: LANE,
        claim_scope: "diagnostic_only",
        product_coverage_effect: "none",
        thresholds: "not_accepted",
        superseded_artifact: "artifacts/parse_compat/native_body_character_scope_bound.json",
        clearance_note: NO_CLEARANCE,
        oracle: OracleIdentity {
            generator_id: ORACLE_GENERATOR,
            provenance: PROVENANCE_PATH,
            independence: "independent_of_sut",
            binding_check: "sha256_verified_from_bytes",
        },
        order_axes: order_axes(),
        scopes,
        prior_artifacts,
        input_bindings: bindings,
        documents: rows,
        aggregate: Aggregate {
            order_axes: order_axis_aggregate,
            p2_primary_lane: P2AggregateRow {
                s03_parent_span: s03_aggregate,
                word_heading: word_aggregate,
            },
        },
        locks,
    };

    let path = report_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "native-body-order-lineage report written to {}",
        path.display()
    );
    Ok(())
}
