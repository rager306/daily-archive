#![cfg(all(feature = "pdf-layout", feature = "pdf-classifier"))]
//! S05/T06 (M514-1lss2s) — corpus-bound content receipt runner for the native
//! body, section / reading-order, visual and callout bands.
//!
//! This example is the executable half of the S05 content contract. It:
//!
//! - reads the slice declaration and the corpus pin by pointer from
//!   `data/parse/layout_bakeoff.yaml#java_free_vertical_slice` (it never restates
//!   the boundary, the documents or the sha256 pins);
//! - binds the preserved-TEI oracle carrier and the reviewed segmentation oracle
//!   by pointer, recomputing every sha256 **from the committed bytes** and
//!   refusing drift;
//! - classifies each pinned PDF with the `pdf-classifier` plane
//!   (`classify_sync_with_options`) and assembles the total T05
//!   [`DocumentGroupPartition`];
//! - installs a `JavaInvocationCounter` on the *real* Java transport seam
//!   (`GrobidHttpBackend` + `with_invocation_observer`) and never consults it;
//! - for each pinned PDF runs the S03 native plane
//!   (`PdfInspectorLayoutDonor` -> `segment_document_full` ->
//!   `project_native_words` -> `assign_heading_regions` ->
//!   `build_native_word_region_carrier`), then the T01 body-character band, the
//!   T02 section / reading-order band, the T03 visual band and the T04 callout
//!   band;
//! - compares each band against an **independent** oracle derived from the
//!   preserved TEI (a different generator, sha256-pinned in `provenance.yaml`)
//!   and the reviewed segmentation oracle;
//! - builds the group-stratified [`ContentReceipt`], re-reads it with
//!   `validate_content_receipt` against the committed pins, and writes
//!   `artifacts/parse_compat/native_content_receipt.json`.
//!
//! It exits non-zero (and writes **no** receipt) on PDF sha256 drift, declaration
//! pin drift, an unknown document, Java observed > 0, an incomplete typed matrix,
//! a vacuous oracle, a required-family `out_of_scope`, a `claim_scope` that is not
//! `diagnostic_only`, a partitioned group that drifts from the classifier, or a
//! proven P1 body loss (which would require a new recorded D498 engine decision).
//!
//! Diagnostic only: `thresholds = not_accepted`, `product_coverage_effect = none`,
//! no promotion, no production splice, no graph write (D127 / D499).

use da_adapters::PdfInspectorLayoutDonor;
use da_adapters::layout_donors::LayoutDonorLimits;
use da_adapters::pdf_classifier::classify_sync_with_options;
use da_adapters::pdf_inspector::PdfOptions;
use da_cli::java_free_slice::{
    COMPATIBILITY_DATA_REL, SLICE_DECLARATION_DATA_REL, load_declaration, sha256_hex,
};
use da_cli::native_content_receipt::{
    CONTENT_CORPUS_POINTER, CONTENT_FAMILY_ORDER, CONTENT_ORACLE_MANIFEST_REL, ContentBodyAxis,
    ContentDeclaredCorpus, ContentDeclaredDocument, ContentExpectation, ContentFamily,
    ContentFamilyCell, ContentObservedDocument, ContentOracleDocument, ContentOracleInput,
    DEFAULT_CONTENT_ARTIFACT_REL, SEGMENTATION_ORACLE_MANIFEST_REL, SEGMENTATION_ORACLE_SOURCE_REL,
    build_content_receipt, validate_content_receipt,
};
use da_cli::native_document_group::{
    DocumentGroupEvidence, DocumentGroupPartition, RequiredFamilyUnmetReason,
};
use da_cli::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, DECLARATION_MANIFEST_REL, GuardProvenance,
    PRODUCT_COVERAGE_EFFECT_NONE, normalize_document_identity, sha256_prefixed,
};
use da_domain::pdf_classification::PdfType;
use da_parse::{
    BodyCasePolicy, BodyCoverageVerdict, BodyGapReason, BodyJoinPolicy, BodyNormalization,
    BodyOracleCounts, BodyOracleIndependence, BodyOracleProvenance, BodyScope, BodyScopeClass,
    BodyScopeSide, BodyUnitInventory, BodyUnitRung, CalloutTargetClass, FullLocation,
    FullSemanticRegion, GrobidHttpBackend, GrobidProfile, HeadingRegionConfig,
    JavaInvocationCounter, NATIVE_CALLOUT_FAMILIES, NATIVE_CALLOUT_ORACLE_SCHEMA_VERSION,
    NATIVE_MEASURED_LANE_ID, NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID,
    NATIVE_VISUAL_ORACLE_SCHEMA_VERSION, NativeBodyText, NativeBodyTextError,
    NativeCalloutFromInputOutcome, NativeCalloutOracle, NativeCalloutOraclePin,
    NativeCalloutOracleRow, NativeSectionNotAssessable, NativeSectionOracleBoundary,
    NativeSectionOracleRow, NativeSectionReadingOrderConfig, NativeSectionRowVerdict,
    NativeVisualFamily, NativeVisualOracle, NativeVisualOraclePin, NativeVisualOracleRow,
    SegmentationConfig, assign_heading_regions, build_native_callout_input,
    build_native_visual_input, build_native_word_region_carrier, callout_oracle_positive_rows,
    classify_body_coverage, classify_body_plane, derive_native_section_reading_order,
    evaluate_callout_family_rows, evaluate_native_section_rows, evaluate_visual_family_rows,
    localize_body_plane_checked, native_section_regions, oracle_content_digest,
    project_native_words, require_independent_section_oracle, require_non_vacuous_callout_oracle,
    require_non_vacuous_visual_oracle, segment_document_full, visual_oracle_positive_rows,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

/// The declared tokenization rung (normalization ladder + explicit case policy).
const RUNG: BodyUnitRung = BodyUnitRung::new(
    BodyNormalization::ZeroWidthAndLigatureNormalized,
    BodyCasePolicy::CaseFolded,
);

/// The independent oracle generator identity. It must not be any native band id.
const ORACLE_GENERATOR: &str = "da-cli.native_content_receipt.tei-lexical-scanner";
/// The backend that produced the preserved oracle bytes (the other generator).
const ORACLE_BACKEND: &str = "java-grobid-crf";
/// Identity of the classifier that produced the group partition.
const CLASSIFIER_ID: &str = "pdf-inspector";
/// Version of that classifier.
const CLASSIFIER_VERSION: &str = "0.1.0";
/// Provisional loopback endpoint; nothing below consults it.
const GUARD_ENDPOINT: &str = "http://127.0.0.1:9";
/// The gitignored-free reviewed segmentation fixture, committed.
const SEGMENTATION_FIXTURE: &str = "crates/da-parse/tests/fixtures/segmentation_oracle.yaml";

/// Subtrees stripped from the body-prose oracle (the T01 probe's exclusion set).
const EXCLUDED_SUBTREES: [&str; 5] = ["figure", "table", "formula", "note", "figDesc"];
/// Block-level tags that introduce a line break in the oracle text.
const BLOCK_TAGS: [&str; 10] = [
    "p", "head", "div", "l", "item", "label", "list", "ab", "seg", "s",
];

struct Args {
    root: PathBuf,
    report: PathBuf,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_CONTENT_ARTIFACT_REL);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--help" => {
                println!("native_content_receipt [--root DIR] [--report FILE]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    Args { root, report }
}

fn main() -> ExitCode {
    let args = parse_args();
    let _log_guard = da_cli::logging::init_pipeline_logging("native_content_receipt");
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_content_receipt: {error}");
            ExitCode::FAILURE
        }
    }
}

// ───────────────────── independent oracle scanners ─────────────────────

/// One pinned preserved-TEI fixture, as `provenance.yaml` publishes it.
#[derive(Debug, Clone)]
struct Pin {
    id: String,
    path: String,
    sha256: String,
}

/// The TEI-derived oracle facts for one document.
#[derive(Debug, Clone, Default)]
struct OracleView {
    body_text: String,
    figures: usize,
    tables: usize,
    formulas: usize,
    citations: usize,
    figure_refs: usize,
    table_refs: usize,
    equation_refs: usize,
}

/// Load the backend id and the fixture pins from the tracked provenance carrier.
fn load_provenance(text: &str) -> Result<(String, Vec<Pin>), Box<dyn Error>> {
    let value: serde_yaml::Value = serde_yaml::from_str(text)?;
    let backend = value["backend"].as_str().unwrap_or_default().to_owned();
    if backend.is_empty() {
        return Err("provenance.yaml carries no backend id".into());
    }
    let fixtures = value["fixtures"]
        .as_sequence()
        .ok_or("provenance.yaml declares no fixtures")?;
    let mut pins = Vec::with_capacity(fixtures.len());
    for fixture in fixtures {
        pins.push(Pin {
            id: fixture["id"].as_str().unwrap_or_default().to_owned(),
            path: fixture["path"].as_str().unwrap_or_default().to_owned(),
            sha256: fixture["sha256"].as_str().unwrap_or_default().to_owned(),
        });
    }
    Ok((backend, pins))
}

/// Ordered opening-tag spans `<tag ...>` in the document (non-nested).
fn open_tag_spans<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let needle = format!("<{tag}");
    let mut out = Vec::new();
    let mut at = 0usize;
    while let Some(offset) = xml[at..].find(&needle) {
        let start = at + offset;
        let rest = &xml[start + needle.len()..];
        let boundary = rest.starts_with('>')
            || rest.starts_with('/')
            || rest.starts_with(|c: char| c.is_whitespace());
        if !boundary {
            at = start + needle.len();
            continue;
        }
        match xml[start..].find('>') {
            Some(gt) => {
                out.push(&xml[start..=start + gt]);
                at = start + gt + 1;
            }
            None => break,
        }
    }
    out
}

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

/// Strip tags from a fragment, dropping the excluded subtrees and decoding
/// entities. This is `std` string work only: it never calls a native band.
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

/// The preserved body-prose oracle text: the `<body>` element with the visual
/// and note subtrees removed.
fn body_text(tei: &str) -> Option<String> {
    let start = tei.find("<body")?;
    let content_start = tei[start..].find('>')? + start + 1;
    let end = tei[content_start..].find("</body>")? + content_start;
    Some(strip_tags_and_decode(&tei[content_start..end]))
}

/// Scan the preserved TEI into the independent oracle view.
fn scan_oracle(tei: &str) -> Result<OracleView, Box<dyn Error>> {
    let body = body_text(tei).ok_or("the preserved TEI carries no body text")?;
    if body.trim().is_empty() {
        return Err("the preserved TEI body text is empty".into());
    }
    let mut view = OracleView {
        body_text: body,
        figures: open_tag_spans(tei, "figure").len(),
        tables: open_tag_spans(tei, "table").len(),
        formulas: open_tag_spans(tei, "formula").len(),
        ..OracleView::default()
    };
    for span in open_tag_spans(tei, "ref") {
        if span.contains("type=\"bibr\"") {
            view.citations += 1;
        } else if span.contains("type=\"figure\"") {
            view.figure_refs += 1;
        } else if span.contains("type=\"table\"") {
            view.table_refs += 1;
        } else if span.contains("type=\"formula\"") {
            view.equation_refs += 1;
        }
    }
    Ok(view)
}

/// The independent visual oracle, derived from the preserved TEI bytes.
fn visual_oracle(
    documents: &[(&str, &str, &OracleView)],
    pins: &[NativeVisualOraclePin],
) -> NativeVisualOracle {
    let mut rows = Vec::new();
    for (document_id, _path, view) in documents {
        for (family, spans) in [
            (NativeVisualFamily::Figure, view.figures),
            (NativeVisualFamily::Table, view.tables),
            (NativeVisualFamily::Formula, view.formulas),
        ] {
            for ordinal in 0..spans {
                rows.push(NativeVisualOracleRow {
                    document_id: (*document_id).to_owned(),
                    family,
                    ordinal: ordinal as u32,
                    page_ordinal: None,
                    coords: None,
                    caption_digest: None,
                    content_digest: oracle_content_digest(
                        format!("{document_id}:{}:{ordinal}", family.token()).as_bytes(),
                    ),
                });
            }
        }
    }
    NativeVisualOracle {
        schema_version: NATIVE_VISUAL_ORACLE_SCHEMA_VERSION.to_owned(),
        generator_id: ORACLE_GENERATOR.to_owned(),
        backend: ORACLE_BACKEND.to_owned(),
        pins: pins.to_vec(),
        rows,
    }
}

/// The independent callout oracle, derived from the preserved TEI bytes.
fn callout_oracle(
    documents: &[(&str, &str, &OracleView)],
    pins: &[NativeCalloutOraclePin],
) -> NativeCalloutOracle {
    let mut rows = Vec::new();
    for (document_id, _path, view) in documents {
        for (family, count) in [
            (CalloutTargetClass::Citation, view.citations),
            (CalloutTargetClass::Figure, view.figure_refs),
            (CalloutTargetClass::Table, view.table_refs),
            (CalloutTargetClass::Equation, view.equation_refs),
        ] {
            for ordinal in 0..count {
                rows.push(NativeCalloutOracleRow {
                    document_id: (*document_id).to_owned(),
                    family,
                    ordinal: ordinal as u32,
                    content_digest: oracle_content_digest(
                        format!("{document_id}:{family:?}:{ordinal}").as_bytes(),
                    ),
                });
            }
        }
    }
    NativeCalloutOracle {
        schema_version: NATIVE_CALLOUT_ORACLE_SCHEMA_VERSION.to_owned(),
        generator_id: ORACLE_GENERATOR.to_owned(),
        backend: ORACLE_BACKEND.to_owned(),
        pins: pins.to_vec(),
        rows,
    }
}

/// Normalize one reviewed coordinate to the native zero-based location.
fn normalize_location(location: &serde_yaml::Value, page_origin: &str) -> Option<FullLocation> {
    let values = location.as_sequence()?;
    let page = values.first()?.as_u64()? as u32;
    let page = match page_origin {
        "one_based" => page.checked_sub(1)?,
        "zero_based" => page,
        _ => return None,
    };
    Some(FullLocation {
        page_ordinal: page,
        block_ordinal: values.get(1)?.as_u64()? as u32,
        line_ordinal: values.get(2)?.as_u64()? as u32,
        token_ordinal: values.get(3)?.as_u64()? as u32,
    })
}

fn parse_region(name: &str) -> Result<FullSemanticRegion, Box<dyn Error>> {
    native_section_regions()
        .into_iter()
        .find(|region| serde_json::to_value(region).ok().as_ref() == Some(&serde_json::json!(name)))
        .ok_or_else(|| format!("`{name}` is not a member of the nine-region vocabulary").into())
}

/// Re-derive the reviewed section oracle rows from the committed fixture alone.
fn derive_section_rows(
    fixture_text: &str,
    page_origin: &str,
) -> Result<Vec<NativeSectionOracleRow>, Box<dyn Error>> {
    let fixture: serde_yaml::Value = serde_yaml::from_str(fixture_text)?;
    let mut rows = Vec::new();
    for receipt in fixture["receipts"]
        .as_sequence()
        .ok_or("the reviewed fixture declares no receipts")?
    {
        let document_id = receipt["document_id"]
            .as_str()
            .ok_or("a fixture receipt carries no document_id")?;
        let lane_id = receipt["lane_id"]
            .as_str()
            .ok_or("a fixture receipt carries no lane_id")?;
        for run in receipt["region_runs"]
            .as_sequence()
            .ok_or("a fixture receipt declares no region_runs")?
        {
            let region = parse_region(run["region"].as_str().unwrap_or_default())?;
            let boundary = if run["absence"].as_str() == Some("reviewed_absent") {
                None
            } else {
                Some(NativeSectionOracleBoundary {
                    start: normalize_location(&run["start"], page_origin)
                        .ok_or("a reviewed start coordinate did not normalize")?,
                    end: normalize_location(&run["end"], page_origin)
                        .ok_or("a reviewed end coordinate did not normalize")?,
                })
            };
            rows.push(NativeSectionOracleRow {
                document_id: document_id.to_owned(),
                lane_id: lane_id.to_owned(),
                region,
                boundary,
            });
        }
    }
    Ok(rows)
}

/// Reviewed *present* regions for one document on the measured native lane.
fn reviewed_present_regions(
    rows: &[NativeSectionOracleRow],
    document_id: &str,
    regions: &[FullSemanticRegion],
) -> u64 {
    rows.iter()
        .filter(|row| row.document_id == document_id)
        .filter(|row| row.lane_id == NATIVE_MEASURED_LANE_ID)
        .filter(|row| regions.contains(&row.region))
        .filter(|row| row.boundary.is_some())
        .count() as u64
}

// ───────────────────── band-to-cell mapping ─────────────────────

fn visual_unmet_reason(reason: da_parse::NativeVisualUnmetReason) -> RequiredFamilyUnmetReason {
    match reason {
        da_parse::NativeVisualUnmetReason::LaneAbsent => RequiredFamilyUnmetReason::NoTextLayer,
        da_parse::NativeVisualUnmetReason::NativeKindUnreachable
        | da_parse::NativeVisualUnmetReason::HeadTerminal => {
            RequiredFamilyUnmetReason::NativeLaneUnsupported
        }
    }
}

fn content_family_of_visual(family: NativeVisualFamily) -> ContentFamily {
    match family {
        NativeVisualFamily::Figure => ContentFamily::Figure,
        NativeVisualFamily::Table => ContentFamily::Table,
        NativeVisualFamily::Formula => ContentFamily::Formula,
    }
}

fn body_gap_token(reason: &BodyGapReason) -> &'static str {
    match reason {
        BodyGapReason::Matched => "matched",
        BodyGapReason::P1NativeTextAbsent { .. } => "p1_native_text_absent",
        BodyGapReason::P1LocalizationUncertain { .. } => "p1_localization_uncertain",
        BodyGapReason::P2RegionAssignmentGap { .. } => "p2_region_assignment_gap",
        BodyGapReason::P2LocalizationUncertain { .. } => "p2_localization_uncertain",
        BodyGapReason::ScopeMismatch => "scope_mismatch",
        BodyGapReason::ScopeJoinMismatch => "scope_join_mismatch",
        BodyGapReason::NormalizationMismatch => "normalization_mismatch",
        BodyGapReason::OracleEmpty => "oracle_empty",
        BodyGapReason::PopulationIncomplete { .. } => "population_incomplete",
    }
}

fn verdict_token(verdict: BodyCoverageVerdict) -> &'static str {
    match verdict {
        BodyCoverageVerdict::GranularityOnly => "granularity_only",
        BodyCoverageVerdict::ContentLoss => "content_loss",
        BodyCoverageVerdict::NotAssessable => "not_assessable",
    }
}

fn body_error_token(error: &NativeBodyTextError) -> &'static str {
    match error {
        NativeBodyTextError::NoBodyTokens | NativeBodyTextError::EmptyBodyDigest => {
            "p2_region_assignment_gap"
        }
        NativeBodyTextError::InvalidScope(_)
        | NativeBodyTextError::InventoryRungMismatch { .. }
        | NativeBodyTextError::PlaneOracleRungMismatch => "scope_mismatch",
        NativeBodyTextError::MissingLexeme { .. } => "scope_join_mismatch",
        _ => "oracle_empty",
    }
}

/// The scope the additive body band is assembled at.
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

// ────────────────────────────── runner ──────────────────────────────

#[allow(clippy::too_many_lines)]
fn run(args: &Args) -> Result<String, Box<dyn Error>> {
    // ── declaration ────────────────────────────────────────────────────────
    let layout_bytes = fs::read(args.root.join(SLICE_DECLARATION_DATA_REL))?;
    let compatibility_bytes = fs::read(args.root.join(COMPATIBILITY_DATA_REL))?;
    let declaration = load_declaration(
        std::str::from_utf8(&layout_bytes)?,
        std::str::from_utf8(&compatibility_bytes)?,
    )?;
    if declaration.claim_scope != CLAIM_SCOPE_DIAGNOSTIC_ONLY {
        return Err(format!(
            "declaration claim_scope is `{}`, expected `{CLAIM_SCOPE_DIAGNOSTIC_ONLY}`",
            declaration.claim_scope
        )
        .into());
    }
    if declaration.product_coverage_effect != PRODUCT_COVERAGE_EFFECT_NONE {
        return Err(format!(
            "declaration product_coverage_effect is `{}`, expected `{PRODUCT_COVERAGE_EFFECT_NONE}`",
            declaration.product_coverage_effect
        )
        .into());
    }
    let declaration_revision = sha256_prefixed(&layout_bytes);

    // ── oracle pins ────────────────────────────────────────────────────────
    let provenance_bytes = fs::read(args.root.join(CONTENT_ORACLE_MANIFEST_REL))?;
    let (oracle_backend, pins) = load_provenance(std::str::from_utf8(&provenance_bytes)?)?;
    if oracle_backend != ORACLE_BACKEND {
        return Err(
            format!("oracle backend is `{oracle_backend}`, expected `{ORACLE_BACKEND}`").into(),
        );
    }
    if pins.len() != declaration.documents.len() {
        return Err(format!(
            "provenance pins {} fixtures for {} declared documents",
            pins.len(),
            declaration.documents.len()
        )
        .into());
    }
    let oracle_revision = sha256_prefixed(&provenance_bytes);

    // Oracle independence: the real provenance must clear the requirement, and a
    // same-generator provenance must be refused. Both are executed against the
    // same band identity the runner uses.
    require_independent_section_oracle(ORACLE_GENERATOR)?;
    if require_independent_section_oracle(da_parse::NATIVE_SECTION_SUT_GENERATOR_ID).is_ok() {
        return Err("the same-generator section negative control was not refused".into());
    }
    if ORACLE_GENERATOR == NATIVE_VISUAL_NATIVE_INPUT_GENERATOR_ID {
        return Err("the visual oracle names the SUT generator".into());
    }
    let independent = true;
    let same_generator_refused = true;

    // ── reviewed segmentation oracle ───────────────────────────────────────
    let segmentation_manifest_bytes = fs::read(args.root.join(SEGMENTATION_ORACLE_MANIFEST_REL))?;
    let segmentation_manifest: serde_yaml::Value =
        serde_yaml::from_slice(&segmentation_manifest_bytes)?;
    let declared_segmentation_pin = segmentation_manifest["oracle"]["source_artifact_sha256"]
        .as_str()
        .ok_or("the segmentation manifest pins no source artifact")?
        .to_owned();
    let segmentation_source_bytes = fs::read(args.root.join(SEGMENTATION_ORACLE_SOURCE_REL))?;
    let segmentation_source_digest = sha256_prefixed(&segmentation_source_bytes);
    if sha256_hex(&segmentation_source_bytes) != declared_segmentation_pin {
        return Err(format!(
            "the reviewed segmentation source drifted: observed {}, pinned {declared_segmentation_pin}",
            sha256_hex(&segmentation_source_bytes)
        )
        .into());
    }
    let segmentation_revision = sha256_prefixed(&segmentation_manifest_bytes);
    let page_origin = segmentation_manifest["evaluation"]["coordinate_binding"]["page_origin"]
        .as_str()
        .ok_or("the segmentation manifest declares no page origin")?
        .to_owned();
    let segmentation_fixture = fs::read_to_string(args.root.join(SEGMENTATION_FIXTURE))?;
    let section_rows = derive_section_rows(&segmentation_fixture, &page_origin)?;

    // ── oracle views from the preserved TEI bytes ──────────────────────────
    let mut oracle_views: BTreeMap<String, (String, String, OracleView)> = BTreeMap::new();
    let mut visual_pins = Vec::new();
    let mut callout_pins = Vec::new();
    for pin in &pins {
        let bytes = fs::read(args.root.join(&pin.path))?;
        let observed = sha256_hex(&bytes);
        if observed != pin.sha256 {
            return Err(format!(
                "oracle fixture {} sha256 drifted: observed {observed}, pinned {}",
                pin.id, pin.sha256
            )
            .into());
        }
        let view = scan_oracle(std::str::from_utf8(&bytes)?)?;
        let key = normalize_document_identity(&pin.id);
        if oracle_views
            .insert(key, (pin.path.clone(), pin.sha256.clone(), view))
            .is_some()
        {
            return Err(format!("duplicate oracle fixture {}", pin.id).into());
        }
        visual_pins.push(NativeVisualOraclePin {
            document_id: pin.id.clone(),
            sha256: pin.sha256.clone(),
        });
        callout_pins.push(NativeCalloutOraclePin {
            document_id: pin.id.clone(),
            sha256: pin.sha256.clone(),
        });
    }
    // Join the declared corpus to the oracle fixtures by normalized identity.
    let mut joined: Vec<(String, String, String, OracleView)> = Vec::new();
    for document in &declaration.documents {
        let key = normalize_document_identity(&document.id);
        let (path, sha, view) = oracle_views
            .get(&key)
            .ok_or_else(|| format!("no oracle fixture for declared document {}", document.id))?
            .clone();
        joined.push((document.id.clone(), path, sha, view));
    }
    let visual_documents: Vec<(&str, &str, &OracleView)> = joined
        .iter()
        .map(|(id, path, _sha, view)| (id.as_str(), path.as_str(), view))
        .collect();
    let callout_documents = visual_documents.clone();

    // ── guard on the real Java seam ────────────────────────────────────────
    let counter = JavaInvocationCounter::new();
    let plain =
        GrobidHttpBackend::new(Some(GUARD_ENDPOINT)).with_profile(GrobidProfile::structural());
    let watched = plain.clone().with_invocation_observer(counter.clone());
    let identity_isolated = plain.identity() == watched.identity();
    let installed = watched.java_invocation_observation().is_observed();
    let observed_java_calls = counter
        .observation()
        .attempts()
        .ok_or("an installed observer must read as observed")?;
    let guard = GuardProvenance {
        installed,
        identity_isolated,
        observed_java_calls,
    };

    // ── independent oracles over the whole slice ───────────────────────────
    let visual = visual_oracle(&visual_documents, &visual_pins);
    require_non_vacuous_visual_oracle(&visual)
        .map_err(|error| format!("the visual oracle is not usable: {error}"))?;
    let callout = callout_oracle(&callout_documents, &callout_pins);
    require_non_vacuous_callout_oracle(&callout)
        .map_err(|error| format!("the callout oracle is not usable: {error}"))?;

    // ── per-document measurement ───────────────────────────────────────────
    let limits = LayoutDonorLimits::default();
    let mut evidence = Vec::with_capacity(declaration.documents.len());
    let mut observed_documents = Vec::with_capacity(declaration.documents.len());
    let mut oracle_documents = Vec::with_capacity(declaration.documents.len());
    let mut notes: Vec<String> = Vec::new();

    for (index, document) in declaration.documents.iter().enumerate() {
        let (_joined_id, _fixture_path, _fixture_sha, view) = &joined[index];
        let fixture = pins
            .iter()
            .find(|pin| {
                normalize_document_identity(&pin.id) == normalize_document_identity(&document.id)
            })
            .ok_or_else(|| format!("no provenance pin for declared document {}", document.id))?;

        let bytes = fs::read(args.root.join(&document.source_pdf))?;
        let digest = sha256_hex(&bytes);
        if digest != document.sha256 {
            return Err(format!(
                "checksum drift for {}: observed {digest}, pinned {}",
                document.id, document.sha256
            )
            .into());
        }

        // Group partition (T05): classify the PDF bytes.
        let classification = classify_sync_with_options(&bytes, PdfOptions::new())?;
        let pdf_type: PdfType = classification.pdf_type;
        let group = DocumentGroupEvidence::measured(
            document.id.clone(),
            pdf_type.clone(),
            CLASSIFIER_ID,
            CLASSIFIER_VERSION,
            digest.clone(),
        );
        let forced = group.forced_required_family_disposition();
        evidence.push(group.clone());

        // S03 native plane.
        let observation = PdfInspectorLayoutDonor.extract_shadow(&bytes, &document.id, limits)?;
        let projection = project_native_words(&observation.document, &observation.lexemes)?;
        let segmentation = segment_document_full(
            &observation.document,
            &observation.lattice,
            &observation.lexemes,
            SegmentationConfig::default(),
        )?;
        let heading = assign_heading_regions(
            &observation.document,
            &observation.lattice,
            &projection,
            HeadingRegionConfig::default(),
        )?;
        let carrier = build_native_word_region_carrier(
            &observation.document,
            &observation.lattice,
            &projection,
            &heading,
            &native_section_regions(),
        )?;
        let pipeline_digest = sha256_prefixed(
            format!(
                "{}\u{1f}{}\u{1f}{}\u{1f}{}",
                projection.digest()?,
                segmentation.order_digest,
                heading.digest()?,
                carrier.digest()?
            )
            .as_bytes(),
        );

        // ── T01 body character axis ────────────────────────────────────────
        let mut ordered: Vec<_> = observation.lexemes.iter().collect();
        ordered.sort_by_key(|lexeme| (lexeme.page_ordinal, lexeme.source_ordinal));
        let plane_units =
            BodyUnitInventory::from_fragments(ordered.iter().map(|lexeme| lexeme.text()), RUNG);
        let oracle_units = BodyUnitInventory::from_text(&view.body_text, RUNG);
        let p1 = localize_body_plane_checked(&plane_units, &oracle_units)?;
        let p1_verdict = classify_body_plane(&p1);

        let oracle_counts = BodyOracleCounts {
            generator_id: ORACLE_GENERATOR.to_owned(),
            lengths: da_parse::NativeBodyLengths::measure(&view.body_text),
        };
        let provenance = BodyOracleProvenance {
            generator_id: ORACLE_GENERATOR.to_owned(),
            independence: BodyOracleIndependence::Independent,
        };

        let mut body_token_count = 0u64;
        let (body_region, body_reason) = match carrier.run(FullSemanticRegion::Body) {
            Some(run) if !run.tokens.is_empty() => {
                let fragments: Vec<&str> = run.tokens.iter().map(|token| token.text()).collect();
                let native = NativeBodyText::from_fragments(
                    document.id.clone(),
                    document.sha256.clone(),
                    fragments.len(),
                    Some((run.start.page_ordinal, run.end.page_ordinal)),
                    BodyJoinPolicy::SingleSpaceBetweenWordTokens,
                    fragments.iter().copied(),
                    RUNG,
                )?;
                body_token_count = native.token_count as u64;
                match classify_body_coverage(
                    document.id.clone(),
                    &native,
                    &plane_units,
                    &oracle_counts,
                    &oracle_units,
                    &provenance,
                    &body_word_scope(),
                    &oracle_scope(),
                ) {
                    Ok(gap) => (verdict_token(gap.verdict), body_gap_token(&gap.reason)),
                    Err(error) => {
                        notes.push(format!("{}: body band refused: {error}", document.id));
                        ("not_assessable", body_error_token(&error))
                    }
                }
            }
            _ => ("not_assessable", "p2_region_assignment_gap"),
        };
        let body_axis = ContentBodyAxis {
            plane_preservation: verdict_token(p1_verdict.verdict).to_owned(),
            plane_d498_reopen_required: p1_verdict.d498_reopen_required,
            plane_present_in_token_units: p1.uncertain_present_in_token_from_plane as u64,
            plane_spanning_token_units: p1.uncertain_spanning_tokens_from_plane as u64,
            plane_character_material_units: p1.uncertain_character_material_from_plane as u64,
            plane_absent_units: p1.absent_from_plane as u64,
            body_region: body_region.to_owned(),
            body_reason: body_reason.to_owned(),
            body_token_count,
            oracle_units: p1.oracle_units as u64,
            native_plane_units: plane_units.total() as u64,
        };
        if p1_verdict.d498_reopen_required {
            return Err(format!(
                "{} proves P1 body content loss ({} absent units): a new D498 engine decision must be recorded before recovery, so no receipt is written",
                document.id, p1.absent_from_plane
            )
            .into());
        }

        // ── T02 sections and reading order ─────────────────────────────────
        let (section_count, reading_count, section_note) = match derive_native_section_reading_order(
            &observation.lattice,
            &carrier,
            &heading,
            NativeSectionReadingOrderConfig::default(),
        ) {
            Ok(outcome) => {
                let verdicts = evaluate_native_section_rows(
                    &outcome,
                    NATIVE_MEASURED_LANE_ID,
                    &section_rows,
                    independent,
                )?;
                let agreements = verdicts
                    .iter()
                    .filter(|row| row.verdict == NativeSectionRowVerdict::Agreement)
                    .count();
                let not_assessable = verdicts
                    .iter()
                    .filter(|row| {
                        matches!(
                            row.verdict,
                            NativeSectionRowVerdict::NotAssessable {
                                reason: NativeSectionNotAssessable::LaneNotMeasuredBySegmenter
                            }
                        )
                    })
                    .count();
                (
                    outcome.sections.len() as u64,
                    outcome.reading_order.region_rows.len() as u64,
                    format!(
                        "sections={} regions={} agreements={} segmenter_rejected={}",
                        outcome.sections.len(),
                        outcome.reading_order.region_rows.len(),
                        agreements,
                        not_assessable
                    ),
                )
            }
            Err(error) => {
                notes.push(format!("{}: section band refused: {error}", document.id));
                (0, 0, format!("section band refused: {error}"))
            }
        };

        // ── T03 visual families ────────────────────────────────────────────
        let visual_outcome = build_native_visual_input(
            &observation.document,
            &observation.lattice,
            &segmentation,
            da_parse::NativeVisualFromInputConfig::default(),
        )?;
        let visual_evaluations = evaluate_visual_family_rows(&visual_outcome, &visual)?;

        // ── T04 callout markers ────────────────────────────────────────────
        let callout_outcome: NativeCalloutFromInputOutcome = build_native_callout_input(
            &observation.document,
            &observation.lattice,
            &observation.lexemes,
            &segmentation,
            &da_parse::NATIVE_CALLOUT_REGIONS,
        )?;
        let callout_evaluations = evaluate_callout_family_rows(&callout_outcome, &callout)?;

        // ── typed family matrix ────────────────────────────────────────────
        let mut families = Vec::with_capacity(CONTENT_FAMILY_ORDER.len());
        if let Some(forced) = forced {
            for family in CONTENT_FAMILY_ORDER {
                families.push(ContentFamilyCell {
                    family,
                    disposition: forced.token().to_owned(),
                    unmet_reason: forced.unmet_reason().map(|r| r.token().to_owned()),
                    observed_count: None,
                    oracle_positive_rows: 0,
                    observed_digest: None,
                });
            }
        } else {
            for evaluation in &visual_evaluations {
                let family = content_family_of_visual(evaluation.family);
                let required = visual_oracle_positive_rows(
                    &visual,
                    Some(document.id.as_str()),
                    evaluation.family,
                ) as u64;
                match evaluation.disposition {
                    da_parse::NativeVisualFamilyDisposition::Measured { native_heads } => {
                        families.push(ContentFamilyCell::measured(
                            family,
                            native_heads as u64,
                            required,
                            Some(sha256_prefixed(
                                format!("{}:{}:{native_heads}", document.id, family.token())
                                    .as_bytes(),
                            )),
                        ));
                    }
                    da_parse::NativeVisualFamilyDisposition::Unmet { reason } => families.push(
                        ContentFamilyCell::unmet(family, visual_unmet_reason(reason), required),
                    ),
                    da_parse::NativeVisualFamilyDisposition::NotObserved => {
                        families.push(ContentFamilyCell::not_observed(family, 0));
                    }
                }
            }

            let callout_required: u64 = NATIVE_CALLOUT_FAMILIES
                .iter()
                .map(|family| {
                    callout_oracle_positive_rows(&callout, Some(document.id.as_str()), *family)
                        as u64
                })
                .sum();
            let callout_native = callout_outcome.total_markers() as u64;
            if callout_native > 0 {
                families.push(ContentFamilyCell::measured(
                    ContentFamily::CalloutMarker,
                    callout_native,
                    callout_required,
                    Some(sha256_prefixed(
                        format!("{}:callout:{callout_native}", document.id).as_bytes(),
                    )),
                ));
            } else if callout_required > 0 {
                families.push(ContentFamilyCell::unmet(
                    ContentFamily::CalloutMarker,
                    RequiredFamilyUnmetReason::NativeLaneUnsupported,
                    callout_required,
                ));
            } else {
                families.push(ContentFamilyCell::not_observed(
                    ContentFamily::CalloutMarker,
                    0,
                ));
            }

            let section_required = reviewed_present_regions(
                &section_rows,
                &document.id,
                &da_parse::SECTION_PARENT_REGIONS,
            );
            if section_count > 0 {
                families.push(ContentFamilyCell::measured(
                    ContentFamily::BodySection,
                    section_count,
                    section_required,
                    Some(sha256_prefixed(
                        format!("{}:sections:{section_count}", document.id).as_bytes(),
                    )),
                ));
            } else {
                families.push(ContentFamilyCell::unmet(
                    ContentFamily::BodySection,
                    RequiredFamilyUnmetReason::NativeLaneUnsupported,
                    section_required,
                ));
            }
            let reading_required =
                reviewed_present_regions(&section_rows, &document.id, &native_section_regions());
            if reading_count > 0 {
                families.push(ContentFamilyCell::measured(
                    ContentFamily::BodyReadingOrder,
                    reading_count,
                    reading_required,
                    Some(sha256_prefixed(
                        format!("{}:reading:{reading_count}", document.id).as_bytes(),
                    )),
                ));
            } else {
                families.push(ContentFamilyCell::unmet(
                    ContentFamily::BodyReadingOrder,
                    RequiredFamilyUnmetReason::NativeLaneUnsupported,
                    reading_required,
                ));
            }
        }

        let oracle_digest = sha256_prefixed(
            format!(
                "{}:body_units={}:figures={}:tables={}:formulas={}:citations={}:figure_refs={}:table_refs={}:equation_refs={}",
                document.id,
                oracle_counts.lengths.raw,
                view.figures,
                view.tables,
                view.formulas,
                view.citations,
                view.figure_refs,
                view.table_refs,
                view.equation_refs
            )
            .as_bytes(),
        );

        println!(
            "{}: group={:?} body={}/{} sections=[{}] families=[{}]",
            document.id,
            pdf_type,
            body_axis.plane_preservation,
            body_axis.body_region,
            section_note,
            families
                .iter()
                .map(|cell| format!("{}={}", cell.family.token(), cell.disposition))
                .collect::<Vec<_>>()
                .join(",")
        );

        observed_documents.push(ContentObservedDocument {
            id: document.id.clone(),
            sha256: digest,
            pages: document.pages,
            observed_pages: observation.document.pages.len() as u64,
            group,
            families,
            body: body_axis,
            pipeline_digest,
            oracle_digest: oracle_digest.clone(),
        });
        oracle_documents.push(ContentOracleDocument {
            document_id: document.id.clone(),
            fixture_path: fixture.path.clone(),
            sha256: fixture.sha256.clone(),
            oracle_digest,
        });
    }

    // ── partition, corpus, oracle, receipt ─────────────────────────────────
    let declared_ids: Vec<String> = declaration
        .documents
        .iter()
        .map(|document| document.id.clone())
        .collect();
    let partition =
        DocumentGroupPartition::assemble(declaration.corpus_id.clone(), &declared_ids, evidence)?;
    let corpus = ContentDeclaredCorpus {
        manifest: DECLARATION_MANIFEST_REL.to_owned(),
        pointer: CONTENT_CORPUS_POINTER.to_owned(),
        revision_sha256: sha256_prefixed(&layout_bytes),
        corpus_id: declaration.corpus_id.clone(),
        document_count: declaration.corpus_n as u64,
        page_denominator: declaration.page_denominator,
        documents: declaration
            .documents
            .iter()
            .map(|document| ContentDeclaredDocument {
                id: document.id.clone(),
                source_pdf: document.source_pdf.clone(),
                sha256: document.sha256.clone(),
                pages: document.pages,
            })
            .collect(),
    };
    let oracle_input = ContentOracleInput {
        generator_id: ORACLE_GENERATOR.to_owned(),
        backend: ORACLE_BACKEND.to_owned(),
        manifest: CONTENT_ORACLE_MANIFEST_REL.to_owned(),
        revision_sha256: oracle_revision.clone(),
        independent,
        same_generator_refused,
        segmentation_manifest: SEGMENTATION_ORACLE_MANIFEST_REL.to_owned(),
        segmentation_revision_sha256: segmentation_revision.clone(),
        segmentation_source: SEGMENTATION_ORACLE_SOURCE_REL.to_owned(),
        segmentation_source_sha256: segmentation_source_digest.clone(),
        documents: oracle_documents,
    };

    let receipt = build_content_receipt(
        &declaration_revision,
        &corpus,
        &declaration.locks,
        &declaration.claim_scope,
        &declaration.product_coverage_effect,
        guard,
        &oracle_input,
        &partition,
        &observed_documents,
    )?;

    // Independent re-read against the committed pins before anything is written.
    let fixture_pins: BTreeMap<String, String> = pins
        .iter()
        .map(|pin| (normalize_document_identity(&pin.id), pin.sha256.clone()))
        .collect();
    let expectation = ContentExpectation {
        partition: partition.clone(),
        declaration_revision_sha256: declaration_revision.clone(),
        oracle_revision_sha256: oracle_revision.clone(),
        oracle_fixture_sha256: fixture_pins,
        segmentation_revision_sha256: segmentation_revision.clone(),
        segmentation_source_sha256: segmentation_source_digest.clone(),
        required_families: vec![
            ContentFamily::Figure,
            ContentFamily::Table,
            ContentFamily::Formula,
            ContentFamily::CalloutMarker,
        ],
    };
    validate_content_receipt(&receipt, &expectation)?;

    let artifact = receipt.artifact_json_bytes()?;
    if let Some(parent) = args
        .report
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.report, &artifact)?;

    let families_measured = receipt
        .populations
        .iter()
        .flat_map(|population| population.matrix.iter())
        .flat_map(|document| document.families.iter())
        .filter(|cell| cell.disposition == "measured")
        .count();
    Ok(format!(
        "native content receipt: {} strata, {} docs, {} matrix rows, {} measured cells, guard installed={} identity_isolated={} java_calls={}, oracle independent={} same_generator_refused={}, thresholds={}, product_coverage_effect={}; notes=[{}]",
        receipt.populations.len(),
        receipt
            .populations
            .iter()
            .map(|population| population.document_count)
            .sum::<u64>(),
        receipt
            .populations
            .iter()
            .map(|population| population.matrix.len())
            .sum::<usize>(),
        families_measured,
        receipt.guard.installed,
        receipt.guard.identity_isolated,
        receipt.guard.observed_java_calls,
        receipt.oracle.independent,
        receipt.oracle.same_generator_refused,
        receipt.thresholds,
        receipt.product_coverage_effect,
        notes.join(" | ")
    ))
}
