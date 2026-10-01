#![cfg(feature = "pdf-layout")]
//! M514-1lss2s / S05 / T01 — additive uncertainty-split diagnostic (detached adapter/CLI).
//!
//! The da-parse character oracle cannot reach the positioned donor: the
//! `PdfInspectorLayoutDonor` lives in `da-adapters`, which depends on
//! `da-parse`, so a `da-parse` test can never build the real S03 plane. This
//! probe is the detached measurement the crate boundary requires: it reads the
//! three pinned PDFs declared in `data/parse/layout_bakeoff.yaml`, verifies the
//! PDF bytes and the preserved-oracle bytes against their own sha256 bindings,
//! builds the genuine `pdf_inspector_positioned` plane, runs the S03 native
//! layout band and the repaired word/heading band, and compares each native body
//! text against an independent preserved body-prose oracle through the
//! scope-bound lexical unit alignment.
//!
//! What the repair changed (finding `S05-T01-COUNTS-DO-NOT-PROVE-PRESERVATION`):
//!
//! * the oracle bytes are read with a **producing-run sha256 check** against
//!   `crates/da-parse/tests/fixtures/preserved/provenance.yaml`;
//! * every side declares its **scope**, **join policy** and **normalization**;
//! * the verdict comes from **content alignment** (oracle units missing from the
//!   whole native plane = P1 loss; units in the plane but not in the body = P2
//!   assignment gap), never from a total, ratio, or ceiling;
//! * the aggregate carries the **complete declared population** and names every
//!   contributing and absent document, so a partial band cannot look clean;
//! * the historical `native_body_character_shadow.json` is **not overwritten** —
//!   the repaired diagnostic is written to a separate artifact.
//!
//! Second bounded repair (finding `S05-T01-BOUNDARY-LOCATOR-NOT-GROUNDED`):
//!
//! * the alignment is now **ordered, occurrence-conserving and
//!   token-boundary-grounded**: exact occurrences are the optimal earliest-match
//!   monotone subsequence of the side, and residual gaps are tiled only by whole
//!   token-boundary runs. No prefix/suffix fragment can resolve a unit, and one
//!   side token can never excuse unbounded residual occurrences;
//! * unresolved occurrences are **localized by what the side still proves**
//!   (in-token, spanning tokens, character material) and stay `NotAssessable`.
//!   Non-ASCII differences are encoding uncertainty, never proof of loss, so only
//!   a deficit in ASCII character material is reported as `ContentLoss`;
//! * the P1 plane evidence uses the **plane-only type** (`BodyPlaneLocalization`),
//!   which carries no body/P2 field, so an absent body can never be fabricated;
//! * the word band passes **one fragment per word token**, so its genuine
//!   boundaries survive and no tile is invented across two separated words.
//!
//! Final cleanup (findings `S05-T01-RUNG-NOT-OBEYED`,
//! `S05-T01-P1-PLANE-VERDICT-MISSING` and `S05-T01-LOCATOR-CLAIM-OVERSTATED`):
//!
//! * every inventory is tokenized at the **declared** `BodyUnitRung`
//!   (normalization ladder plus an explicit case policy), and the rung is
//!   declared on each scope, so tokenization obeys the declaration instead of
//!   hardcoding zero-width/ligature folding and lowercasing;
//! * the P1 plane evidence carries a **typed verdict, reason and D498 trigger**
//!   (`BodyPlaneVerdict`) at both the document and the aggregate level. A zero
//!   `absent_from_plane` count is never reported as preservation: presence-only
//!   uncertainty stays `NotAssessable`, and a proven material deficit stays
//!   `ContentLoss`;
//! * the locator is documented as a **two-phase greedy decomposition, not a joint
//!   exact-plus-tile optimum**, with the `x y` / `x xy` counterexample and its
//!   conservative (`NotAssessable`) outcome named in the module and the evidence.
//!
//! Shadow/diagnostic only: no routing, promotion, graph, or import. The pinned
//! PDFs are gitignored, so this artifact is self-declared by construction
//! (`thresholds=not_accepted`, `claim_scope=diagnostic_only`).
//!
//! Additive uncertainty split (this run): the accepted localization reports one
//! bucket, `uncertain_present_in_token`. This probe calls the additive
//! `split_plane_uncertainty` / `split_body_uncertainty` siblings and writes the
//! equal-token versus proper-substring facts per document and aggregate, with
//! the conservation invariant `equal + proper == bucket` proven against the same
//! localization. Equality is tested first. The counters are **facts, not a
//! verdict**: no threshold, ratio, or matching claim is taken, a nonzero equal
//! count is not a clearance, and the accepted
//! `native_body_character_scope_bound.json` / `native_body_order_lineage.json`
//! artifacts are preserved, never overwritten. This is a **separate artifact**
//! (`native-body-uncertainty-split.v1`), not a mutation of the accepted schema.

use da_adapters::{LayoutDonorLimits, PdfInspectorLayoutDonor};
use da_parse::{
    BodyAggregateInput, BodyCasePolicy, BodyCoverageAggregate, BodyCoverageGap,
    BodyDocumentOutcome, BodyJoinPolicy, BodyNormalization, BodyOracleCounts,
    BodyOracleIndependence, BodyOracleProvenance, BodyPlaneLocalization, BodyPlaneVerdict,
    BodyScope, BodyScopeClass, BodyScopeSide, BodyUncertaintySplit, BodyUnitInventory,
    BodyUnitRung, FullSemanticRegion, HeadingRegionConfig, NativeBodyLengths, NativeBodyText,
    NativeBodyTextError, SegmentationConfig, aggregate_body_coverage, assign_heading_regions,
    build_native_body_text, build_native_word_region_carrier, classify_body_coverage,
    classify_body_plane, localize_body_plane_checked, plane_verdict_for_population,
    project_native_words, segment_document_full, split_body_uncertainty, split_plane_uncertainty,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

const REPORT_SCHEMA: &str = "native-body-uncertainty-split.v1";
const DEFAULT_REPORT: &str = "artifacts/parse_compat/native_body_uncertainty_split.json";
const DIAGNOSTIC_NOTE: &str = "Additive split of the accepted P1/P2 \
     uncertain_present_in_token bucket into equal-token and proper-substring facts. It carries a \
     fact count only: it does not decide preservation, granularity or loss, does not reopen D498, \
     and accepts no threshold.";
const CONSERVATION_INVARIANT: &str = "equal_in_token + proper_substring_in_token == the \
     uncertain_present_in_token bucket of the same localization (equality tested first)";
const CLEARANCE_NOTE: &str = "Comparison, not a quality proof: a zero uncertain count is not a \
     clearance, a nonzero equal count is not preservation, and no bucket is a threshold.";

/// Accepted in-token buckets from the frozen prior artifacts
/// `artifacts/parse_compat/native_body_character_scope_bound.json` and
/// `artifacts/parse_compat/native_body_order_lineage.json` (delivered axis). The
/// split must conserve exactly these, so a drift in the accepted measurement is
/// named rather than silently absorbed.
const ACCEPTED_P1_IN_TOKEN_BUCKET: [(&str, usize); 3] = [
    ("hub-2507.19457", 32_907),
    ("rlm-2512.24601", 15_322),
    ("reca-2605.26525v1", 21_171),
];
const ACCEPTED_P1_IN_TOKEN_BUCKET_TOTAL: usize = 69_400;
const ACCEPTED_P2_WORD_IN_TOKEN_BUCKET: [(&str, usize); 3] = [
    ("hub-2507.19457", 32_907),
    ("rlm-2512.24601", 13_389),
    ("reca-2605.26525v1", 16_006),
];
const ACCEPTED_P2_WORD_IN_TOKEN_BUCKET_TOTAL: usize = 62_302;
const CORPUS_ID: &str = "s13-same-input-pdf-corpus";
const LANE: &str = "pdf_inspector_positioned";
const ORACLE_GENERATOR: &str = "preserved-java-tei-lexical-scanner";

/// The declared tokenization rung (normalization ladder + explicit case policy)
/// the probe measures at. It is named on every scope and passed to every
/// inventory so tokenization obeys the declaration.
const RUNG: BodyUnitRung = BodyUnitRung::new(
    BodyNormalization::ZeroWidthAndLigatureNormalized,
    BodyCasePolicy::CaseFolded,
);
const PROVENANCE_PATH: &str = "crates/da-parse/tests/fixtures/preserved/provenance.yaml";

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
    diagnostic_note: &'static str,
    conservation_invariant: &'static str,
    clearance_note: &'static str,
    accepted_p1_in_token_bucket: BTreeMap<String, usize>,
    accepted_p1_in_token_bucket_total: usize,
    accepted_p2_word_in_token_bucket: BTreeMap<String, usize>,
    accepted_p2_word_in_token_bucket_total: usize,
    accepted_bucket_reproduced: bool,
    oracle: OracleIdentity,
    scopes: ScopeDeclaration,
    input_bindings: Vec<InputBinding>,
    documents: Vec<DocumentRow>,
    aggregate: Aggregate,
    locks: BTreeMap<&'static str, &'static str>,
    thresholds: &'static str,
}
#[derive(Serialize)]
struct OracleIdentity {
    generator_id: &'static str,
    provenance: &'static str,
    fixtures: &'static str,
    independence: &'static str,
    binding_check: &'static str,
}
#[derive(Serialize)]
struct ScopeDeclaration {
    native_plane: BodyScope,
    native_body_verbatim: BodyScope,
    native_body_word: BodyScope,
    oracle: BodyScope,
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
    p1_plane_preservation: PlaneRow,
    s03_parent_span: BandRow,
    word_heading: BandRow,
    uncertainty_split: UncertaintySplitRow,
    oracle: NativeBodyLengths,
}

/// The additive split facts for one document. `*_accepted_reproduced` is a
/// comparison against the frozen accepted bucket, so a drift is named instead of
/// silently absorbed; `*_conservation_ok` is the invariant
/// `equal + proper == bucket`.
#[derive(Serialize)]
struct UncertaintySplitRow {
    accepted_p1_bucket: usize,
    p1_plane: BodyUncertaintySplit,
    p1_conservation_ok: bool,
    p1_accepted_reproduced: bool,
    accepted_p2_bucket: usize,
    p2_body: Option<BodyUncertaintySplit>,
    p2_conservation_ok: Option<bool>,
    p2_accepted_reproduced: Option<bool>,
    /// Typed reason the P2 split is not available (band absence or failure).
    p2_absence: Option<String>,
}
#[derive(Serialize)]
struct PlaneRow {
    localization: BodyPlaneLocalization,
    verdict: BodyPlaneVerdict,
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
    s03_parent_span: BodyCoverageAggregate,
    word_heading: BodyCoverageAggregate,
    p1_plane_preservation: PlaneAggregate,
    uncertainty_split: SplitAggregate,
}

/// Corpus fold of the additive split. `p1_conservation_ok` is the invariant
/// over the folded counters; `accepted_bucket_reproduced` compares the folded
/// buckets to the frozen accepted totals.
#[derive(Serialize)]
struct SplitAggregate {
    documents_declared: usize,
    documents_measured: usize,
    population_complete: bool,
    accepted_p1_in_token_bucket_total: usize,
    accepted_p2_word_in_token_bucket_total: usize,
    accepted_bucket_reproduced: bool,
    p1_plane: BodyUncertaintySplit,
    p1_bucket: usize,
    p1_conservation_ok: bool,
    p2_body: BodyUncertaintySplit,
    p2_bucket: usize,
    p2_documents_measured: usize,
    p2_conservation_ok: bool,
}
#[derive(Serialize)]
struct PlaneAggregate {
    corpus_id: String,
    documents_declared: usize,
    documents_measured: usize,
    /// True only when every declared document contributed plane evidence.
    population_complete: bool,
    localization: BodyPlaneLocalization,
    /// Typed P1 verdict. A zero `absent_from_plane` count is never preservation:
    /// only a fully located plane with a complete population is `GranularityOnly`.
    verdict: BodyPlaneVerdict,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_bytes = std::fs::read(root().join("data/parse/layout_bakeoff.yaml"))?;
    let manifest: Manifest = serde_yaml::from_slice(&manifest_bytes)?;
    let provenance_bytes = std::fs::read(root().join(PROVENANCE_PATH))?;
    let oracle_provenance: Provenance = serde_yaml::from_slice(&provenance_bytes)?;

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
    let mut p1_localization = BodyPlaneLocalization::empty();
    let mut p1_measured = 0usize;
    let mut p1_split_total = BodyUncertaintySplit::default();
    let mut p1_split_bucket_total = 0usize;
    let mut p2_split_total = BodyUncertaintySplit::default();
    let mut p2_split_bucket_total = 0usize;
    let mut p2_split_documents_measured = 0usize;
    let mut accepted_bucket_reproduced_all = true;

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
        let mut ordered: Vec<_> = observation.lexemes.iter().collect();
        ordered.sort_by_key(|lexeme| (lexeme.page_ordinal, lexeme.source_ordinal));
        // The plane is tokenized per lexeme: concatenating token texts must never
        // merge the last word of one token with the first word of the next.
        let plane_units =
            BodyUnitInventory::from_fragments(ordered.iter().map(|lexeme| lexeme.text()), RUNG);
        let oracle_units = BodyUnitInventory::from_text(&oracle_text, RUNG);

        // P1: does the whole native plane carry the oracle content at all? This
        // is the plane-only localization: no body exists yet, so no P2 field is
        // invented for it. It carries its own typed verdict so a zero-absent
        // plane can never be read as preservation on its own.
        let p1 = localize_body_plane_checked(&plane_units, &oracle_units)?;
        let p1_verdict = classify_body_plane(&p1);
        let p1_split = split_plane_uncertainty(&plane_units, &oracle_units)?;
        let accepted_p1_bucket = accepted_bucket(&document.id)
            .ok_or_else(|| format!("unknown accepted P1 bucket binding: {}", document.id))?;
        let p1_conservation_ok = p1_split.total() == p1.uncertain_present_in_token_from_plane;
        let p1_accepted_reproduced = p1.uncertain_present_in_token_from_plane == accepted_p1_bucket;
        p1_localization.add(&p1);
        p1_measured += 1;

        // ── S03 parent-span band ────────────────────────────────────────────
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
                        &plane_units,
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

        // ── Repaired word/heading band ──────────────────────────────────────
        let mut p2_split: Option<BodyUncertaintySplit> = None;
        let mut p2_bucket: Option<usize> = None;
        let word_row = match project_native_words(&observation.document, &observation.lexemes) {
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
                            // One fragment per word token: the word plane's genuine
                            // boundaries must survive, so no boundary tile can be
                            // invented across two separated words.
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
                                    &plane_units,
                                    &oracle,
                                    &oracle_units,
                                    &provenance(),
                                    &scopes.native_body_word,
                                    &scopes.oracle,
                                ) {
                                    Ok(gap) => {
                                        p2_bucket = Some(
                                            gap.alignment.uncertain_present_in_token_from_body,
                                        );
                                        p2_split =
                                            split_body_uncertainty(native.units(), &oracle_units)
                                                .ok();
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

        let accepted_p2_bucket = accepted_p2_word_bucket(&document.id)
            .ok_or_else(|| format!("unknown accepted P2 bucket binding: {}", document.id))?;
        let (p2_body, p2_conservation_ok, p2_accepted_reproduced, p2_absence) =
            match (p2_split, p2_bucket) {
                (Some(split), Some(bucket)) => (
                    Some(split),
                    Some(split.total() == bucket),
                    Some(bucket == accepted_p2_bucket),
                    None,
                ),
                _ => (None, None, None, Some("NoBodyTokens".to_owned())),
            };
        let uncertainty_split = UncertaintySplitRow {
            accepted_p1_bucket,
            p1_plane: p1_split,
            p1_conservation_ok,
            p1_accepted_reproduced,
            accepted_p2_bucket,
            p2_body,
            p2_conservation_ok,
            p2_accepted_reproduced,
            p2_absence,
        };
        p1_split_total.add(&p1_split);
        p1_split_bucket_total += p1.uncertain_present_in_token_from_plane;
        if !p1_accepted_reproduced {
            accepted_bucket_reproduced_all = false;
        }
        if let (Some(body_split), Some(bucket)) = (p2_body, p2_bucket) {
            p2_split_total.add(&body_split);
            p2_split_bucket_total += bucket;
            p2_split_documents_measured += 1;
        }

        let native_lexical_tokens = observation
            .document
            .pages
            .iter()
            .flat_map(|page| page.tokens.iter())
            .filter(|token| token.kind.is_lexical())
            .count();
        let pdf_inspector_spans = observation.lexemes.len();
        println!(
            "native-body {id}: pages {} spans {} p1 [oracle {} exact {} boundary {} uncertain {} absent {} verdict {:?} d498 {}] s03 [{} tokens {:?} {:?}] word [{} tokens {:?} {:?}] split [p1 equal {} proper {} bucket {} ok {} | p2 {:?} of {:?} ok {:?}] oracle zero-width {}",
            observation.document.pages.len(),
            pdf_inspector_spans,
            p1.oracle_units,
            p1.exact_in_plane,
            p1.boundary_variant_from_plane,
            p1.uncertain_from_plane(),
            p1.absent_from_plane,
            p1_verdict.verdict,
            p1_verdict.d498_reopen_required,
            s03_row.token_count,
            s03_row.gap.as_ref().map(|g| g.verdict),
            s03_row.gap.as_ref().map(|g| g.reason.clone()),
            word_row.token_count,
            word_row.gap.as_ref().map(|g| g.verdict),
            word_row.gap.as_ref().map(|g| g.reason.clone()),
            p1_split.equal_in_token,
            p1_split.proper_substring_in_token,
            p1.uncertain_present_in_token_from_plane,
            p1_conservation_ok,
            p2_body,
            p2_bucket,
            p2_conservation_ok,
            oracle.lengths.zero_width_normalized,
            id = document.id,
        );

        rows.push(DocumentRow {
            document_id: document.id.clone(),
            pages: observation.document.pages.len(),
            native_lexical_tokens,
            pdf_inspector_spans,
            p1_plane_preservation: PlaneRow {
                localization: p1.clone(),
                verdict: p1_verdict,
            },
            s03_parent_span: s03_row,
            word_heading: word_row,
            uncertainty_split,
            oracle: oracle.lengths,
        });
    }

    let locks = [
        ("claim_scope", "diagnostic_only"),
        ("product_coverage_effect", "none"),
        ("promotion_state", "candidate"),
        ("auto_promote", "forbidden"),
        ("graph_mutation", "forbidden"),
        ("import_eligible", "false"),
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
    let p1_population_complete = !declared.is_empty() && p1_measured == declared.len();
    let p1_aggregate_verdict =
        plane_verdict_for_population(&p1_localization, declared.len(), p1_measured);
    let accepted_bucket_reproduced = accepted_bucket_reproduced_all
        && p1_split_bucket_total == ACCEPTED_P1_IN_TOKEN_BUCKET_TOTAL;
    let split_aggregate = SplitAggregate {
        documents_declared: declared.len(),
        documents_measured: p1_measured,
        population_complete: p1_population_complete,
        accepted_p1_in_token_bucket_total: ACCEPTED_P1_IN_TOKEN_BUCKET_TOTAL,
        accepted_p2_word_in_token_bucket_total: ACCEPTED_P2_WORD_IN_TOKEN_BUCKET_TOTAL,
        accepted_bucket_reproduced,
        p1_plane: p1_split_total,
        p1_bucket: p1_split_bucket_total,
        p1_conservation_ok: p1_split_total.total() == p1_split_bucket_total,
        p2_body: p2_split_total,
        p2_bucket: p2_split_bucket_total,
        p2_documents_measured: p2_split_documents_measured,
        p2_conservation_ok: p2_split_total.total() == p2_split_bucket_total,
    };

    let report = Report {
        schema_version: REPORT_SCHEMA,
        corpus_id: CORPUS_ID.to_owned(),
        page_denominator: manifest.corpus.page_denominator,
        lane: LANE,
        diagnostic_note: DIAGNOSTIC_NOTE,
        conservation_invariant: CONSERVATION_INVARIANT,
        clearance_note: CLEARANCE_NOTE,
        accepted_p1_in_token_bucket: accepted_bucket_map(&ACCEPTED_P1_IN_TOKEN_BUCKET),
        accepted_p1_in_token_bucket_total: ACCEPTED_P1_IN_TOKEN_BUCKET_TOTAL,
        accepted_p2_word_in_token_bucket: accepted_bucket_map(&ACCEPTED_P2_WORD_IN_TOKEN_BUCKET),
        accepted_p2_word_in_token_bucket_total: ACCEPTED_P2_WORD_IN_TOKEN_BUCKET_TOTAL,
        accepted_bucket_reproduced,
        oracle: OracleIdentity {
            generator_id: ORACLE_GENERATOR,
            provenance: PROVENANCE_PATH,
            fixtures: "crates/da-parse/tests/fixtures/preserved",
            independence: "independent_of_sut",
            binding_check: "sha256_verified_from_bytes",
        },
        scopes,
        input_bindings: bindings,
        documents: rows,
        aggregate: Aggregate {
            s03_parent_span: s03_aggregate,
            word_heading: word_aggregate,
            p1_plane_preservation: PlaneAggregate {
                corpus_id: manifest.corpus.id.clone(),
                documents_declared: declared.len(),
                documents_measured: p1_measured,
                population_complete: p1_population_complete,
                verdict: p1_aggregate_verdict,
                localization: p1_localization,
            },
            uncertainty_split: split_aggregate,
        },
        locks,
        thresholds: "not_accepted",
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
        "native-body uncertainty-split report written to {}",
        path.display()
    );
    Ok(())
}

/// The frozen accepted P1 in-token bucket for one document, or `None` when the
/// document is not bound in the accepted pin.
fn accepted_bucket(document_id: &str) -> Option<usize> {
    ACCEPTED_P1_IN_TOKEN_BUCKET
        .iter()
        .find(|(id, _)| *id == document_id)
        .map(|(_, bucket)| *bucket)
}

/// The frozen accepted P2 word/heading in-token bucket for one document.
fn accepted_p2_word_bucket(document_id: &str) -> Option<usize> {
    ACCEPTED_P2_WORD_IN_TOKEN_BUCKET
        .iter()
        .find(|(id, _)| *id == document_id)
        .map(|(_, bucket)| *bucket)
}

/// The accepted pins as a serialized map, so the report carries the frozen
/// expectation beside the measurement.
fn accepted_bucket_map(pins: &[(&str, usize)]) -> BTreeMap<String, usize> {
    pins.iter()
        .map(|(id, bucket)| ((*id).to_owned(), *bucket))
        .collect()
}
