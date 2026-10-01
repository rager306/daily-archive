#![cfg(all(feature = "pdf-layout", feature = "pdf-classifier"))]
//! S06/T05 (M514-1lss2s) — Java-forbidden end-to-end native-route runner.
//!
//! This is the executable half of the S06 route contract. It turns the DATA
//! declaration (`data/parse/layout_bakeoff.yaml#java_free_vertical_slice`), the
//! frozen prospective limits (`scripts/governor/checks/grobid_absorption_progress.yaml#native_route_e2e.limit_policy`)
//! and the pinned 3-document corpus into a corpus-bound
//! [`da_cli::native_route_receipt::RouteReceipt`] at
//! `artifacts/parse_compat/native_route_e2e.json`.
//!
//! What it does, and deliberately does not do:
//!
//! - It reads the declaration, the cascade stage ids, the corpus pin and the
//!   freeze **by pointer**. It never restates the boundary, the stage list, the
//!   document identities or the ceiling dictionaries.
//! - It recomputes the declared `freeze_revision` from the committed carrier
//!   bytes and refuses the run when it drifted (`fail-closed`). The limits it
//!   binds into the receipt are therefore the frozen ones, never a tuned set.
//! - It installs a [`JavaInvocationCounter`] on the *real* Java transport seam
//!   (`GrobidHttpBackend` + `with_invocation_observer`) **first**, then records
//!   a positive control on a separate unreachable seam that must raise the
//!   counter. That is what makes the observed zero a measurement rather than an
//!   absence.
//! - For every pinned PDF it verifies the sha256 before any work, runs the S03
//!   native plane, folds it through T01's [`build_native_route_result`] with the
//!   S04/S05 band oracles, and runs T04's [`attempt_native_route`] over the
//!   result so each document carries typed failure-recovery dispositions. The
//!   route never consults Java and never consumes a pre-existing TEI: the
//!   preserved TEI is only an **independent oracle**, sha256-pinned in
//!   `provenance.yaml`.
//! - It builds the T03 receipt (guard provenance, per-stage P0..P9 stances from
//!   DATA, per-family typed dispositions, the reproduced S05 body axis, the
//!   frozen limits, the five closed locks and redaction), re-reads it through
//!   `validate_route_receipt`, and writes the canonical bytes.
//!
//! Provisional lexicon: the header/bibliography bands take a
//! `da_ports::LexiconBundle`. No GROBID lexicon bytes are read or committed
//! (D358); the runner provisions a minimal project-owned bundle exactly as
//! `examples/native_family_parity.rs` does, so a band the lexicon does not
//! cover lands as a typed `refused`/`unmet` cell rather than a silent zero.
//!
//! Exit status is non-zero — and **no** receipt is written — on PDF sha256
//! drift, an unknown document, malformed/absent input bytes, a drifted freeze
//! revision, an opened lock, a non-diagnostic claim scope, observed Java > 0, a
//! guard-less zero, an incomplete stage/family matrix, or a proven P1 content
//! loss (which would require a recorded D498 engine decision).
//!
//! Diagnostic only: `claim_scope = diagnostic_only`, `product_coverage_effect
//! = none`, `thresholds = not_accepted`, no promotion, no production splice, no
//! graph write (D127 / D499).

use da_adapters::PdfInspectorLayoutDonor;
use da_adapters::layout_donors::LayoutDonorLimits;
use da_cli::java_free_slice::{
    COMPATIBILITY_DATA_REL, SLICE_DECLARATION_DATA_REL, load_declaration, sha256_hex,
};
use da_cli::native_family_receipt::{
    CLAIM_SCOPE_DIAGNOSTIC_ONLY, GuardProvenance, PRODUCT_COVERAGE_EFFECT_NONE,
    normalize_document_identity, sha256_prefixed,
};
use da_cli::native_route_receipt::{
    DEFAULT_ROUTE_ARTIFACT_REL, ROUTE_DIRECTION_VOCABULARY, ROUTE_FAMILY_ORDER,
    ROUTE_FLOOR_METRIC_VOCABULARY, ROUTE_FREEZE_SCOPE, ROUTE_FREEZE_STATE_FROZEN,
    ROUTE_SILENT_CLAMP_FORBIDDEN, ROUTE_TUNED_AFTER_SCORING_FORBIDDEN, RouteBodyAxis,
    RouteBoundaryInput, RouteDeclaredCorpus, RouteDeclaredDocument, RouteExpectation,
    RouteFamilyCell, RouteFreezeDeclaration, RouteLimitsBinding, RouteMetricDeclaration,
    RouteMetricValue, RouteObservedDocument, RoutePopulationId, RoutePopulationInput, RouteReceipt,
    RouteStageStanceCell, assert_route_redacted, build_route_receipt, validate_route_receipt,
};
use da_parse::{
    BodyCasePolicy, BodyNormalization, BodyUnitInventory, BodyUnitRung, CalloutTargetClass,
    FullLocation, FullSemanticRegion, GrobidHttpBackend, GrobidProfile, JavaInvocationCounter,
    NATIVE_CALLOUT_ORACLE_SCHEMA_VERSION, NATIVE_MEASURED_LANE_ID, NATIVE_SECTION_SUT_GENERATOR_ID,
    NATIVE_VISUAL_ORACLE_SCHEMA_VERSION, NativeCalloutOracle, NativeCalloutOraclePin,
    NativeCalloutOracleRow, NativeRouteBodyAxis, NativeRouteBodyOracle, NativeRouteBoundary,
    NativeRouteFamilyDisposition, NativeRouteInput, NativeRouteOutOfSliceReason,
    NativeRouteOutOfSliceStage, NativeRouteStageStance, NativeRouteUnmetReason,
    NativeSectionOracleBoundary, NativeSectionOracleRow, NativeVisualFamily, NativeVisualOracle,
    NativeVisualOraclePin, NativeVisualOracleRow, ParseBackend, ParseBackendRequest,
    RouteInputSpec, RouteRecoveryDisposition, attempt_native_route, build_native_route_result,
    native_section_regions, oracle_content_digest, require_independent_section_oracle,
    require_non_vacuous_callout_oracle, require_non_vacuous_visual_oracle,
};
use da_ports::{FeatureTemplate, LexiconBundle, LexiconChannel};
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
const ORACLE_GENERATOR: &str = "da-cli.native_route_e2e.tei-lexical-scanner";
/// The backend that produced the preserved oracle bytes (the other generator).
const ORACLE_BACKEND: &str = "java-grobid-crf";
/// Provisional loopback endpoint; the real route never consults it.
const GUARD_ENDPOINT: &str = "http://127.0.0.1:9";
/// An unroutable seam for the guard's positive control: the request fails at URL
/// parsing, so the attempt is counted before any socket exists.
const CONTROL_ENDPOINT: &str = "http://";

/// The governor sibling DATA block that carries the boundary declaration and the
/// prospective limit freeze (T02).
const GOVERNOR_REL: &str = "scripts/governor/checks/grobid_absorption_progress.yaml";
/// Pointer inside the governor block.
const GOVERNOR_POINTER: &str = "native_route_e2e";

/// The preserved-TEI oracle pin carrier (an independent generator, not the SUT).
const PROVENANCE_REL: &str = "crates/da-parse/tests/fixtures/preserved/provenance.yaml";
/// The reviewed segmentation oracle manifest.
const SEGMENTATION_MANIFEST_REL: &str = "data/parse/document_segmentation.yaml";
/// The reviewed segmentation oracle source artifact.
const SEGMENTATION_SOURCE_REL: &str =
    "artifacts/parse_compat/document_segmentation_oracle_source.json";
/// The committed reviewed segmentation fixture.
const SEGMENTATION_FIXTURE: &str = "crates/da-parse/tests/fixtures/segmentation_oracle.yaml";

/// Subtrees stripped from the body-prose oracle (the T01 probe's exclusion set).
const EXCLUDED_SUBTREES: [&str; 5] = ["figure", "table", "formula", "note", "figDesc"];
/// Block-level tags that introduce a line break in the oracle text.
const BLOCK_TAGS: [&str; 10] = [
    "p", "head", "div", "l", "item", "label", "list", "ab", "seg", "s",
];

// ─────────────────────────────── arguments ───────────────────────────────

struct Args {
    root: PathBuf,
    report: PathBuf,
    self_check: bool,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_ROUTE_ARTIFACT_REL);
    let mut self_check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--self-check" => self_check = true,
            "--help" => {
                println!("native_route_e2e [--root DIR] [--report FILE] [--self-check]");
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
    let _log_guard = da_cli::logging::init_pipeline_logging("native_route_e2e");
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_route_e2e: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Minimal project-owned lexicon (no GROBID lexicon bytes are read or committed,
/// D358). Provisioned exactly as `examples/native_family_parity.rs#lexicon()`.
fn lexicon() -> LexiconBundle {
    LexiconBundle {
        schema_version: "grobid-lexicon.v1".into(),
        resource_version: "0.9.0-crf".into(),
        channels: vec![LexiconChannel {
            key: "properName".into(),
            entries: vec!["vaswani".into()],
        }],
        feature_templates: vec![FeatureTemplate {
            key: "header".into(),
            channel_keys: vec!["properName".into()],
            feature_inputs: vec!["token.text".into()],
        }],
    }
}

// ───────────────────── independent oracle scanners ─────────────────────
//
// The preserved TEI is an *independent oracle*, never a route input: the route
// reads PDF bytes only. These scanners are the same shape the S05 content
// receipt uses, so the reproduced S05 body axis is measured against the same
// bytes with the same algorithm.

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
    documents: &[(&str, &OracleView)],
    pins: &[NativeVisualOraclePin],
) -> NativeVisualOracle {
    let mut rows = Vec::new();
    for (document_id, view) in documents {
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
    documents: &[(&str, &OracleView)],
    pins: &[NativeCalloutOraclePin],
) -> NativeCalloutOracle {
    let mut rows = Vec::new();
    for (document_id, view) in documents {
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

// ──────────────────────────── DATA resolution ────────────────────────────

fn yaml(bytes: &[u8]) -> Result<serde_yaml::Value, Box<dyn Error>> {
    Ok(serde_yaml::from_slice(bytes)?)
}

fn out_of_slice_reason(token: &str) -> Result<NativeRouteOutOfSliceReason, Box<dyn Error>> {
    match token {
        "java_authority" => Ok(NativeRouteOutOfSliceReason::JavaAuthority),
        "visual_heads_not_measured" => Ok(NativeRouteOutOfSliceReason::VisualHeadsNotMeasured),
        "java_tei_coordinates" => Ok(NativeRouteOutOfSliceReason::JavaTeiCoordinates),
        "profile_dependent_consolidation" => {
            Ok(NativeRouteOutOfSliceReason::ProfileDependentConsolidation)
        }
        "java_tei_lift" => Ok(NativeRouteOutOfSliceReason::JavaTeiLift),
        other => Err(format!("`{other}` is not an out-of-slice reason token in DATA").into()),
    }
}

/// The T01 boundary, resolved from the declaration (which already resolved the
/// cascade by pointer and classified every stage).
fn native_boundary(
    declaration: &da_cli::java_free_slice::SliceDeclaration,
) -> Result<NativeRouteBoundary, Box<dyn Error>> {
    let admission = declaration
        .stages
        .iter()
        .find(|stage| stage.scope == da_cli::java_free_slice::StageScope::IdentityAdmission)
        .ok_or("the declaration classifies no identity-admission stage")?;
    let in_slice_stage_ids = declaration
        .in_slice_stage_ids()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let mut out_of_slice = Vec::new();
    for stage in &declaration.stages {
        if stage.scope != da_cli::java_free_slice::StageScope::OutOfSlice {
            continue;
        }
        let reason = stage
            .out_of_slice_reason
            .as_deref()
            .ok_or("an out-of-slice stage carries no reason")?;
        out_of_slice.push(NativeRouteOutOfSliceStage {
            stage_id: stage.stage_id.clone(),
            reason: out_of_slice_reason(reason)?,
        });
    }
    let boundary = NativeRouteBoundary {
        admission_stage_id: admission.stage_id.clone(),
        in_slice_stage_ids,
        out_of_slice,
        lane_ids: declaration.lane_ids.clone(),
    };
    let cascade: Vec<String> = declaration
        .stages
        .iter()
        .map(|stage| stage.stage_id.clone())
        .collect();
    boundary
        .validate(&cascade)
        .map_err(|error| -> Box<dyn Error> { error.to_string().into() })?;
    Ok(boundary)
}

/// The T03 boundary descriptor, derived from the same declaration.
fn route_boundary_input(
    declaration: &da_cli::java_free_slice::SliceDeclaration,
) -> Result<RouteBoundaryInput, Box<dyn Error>> {
    let admission = declaration
        .stages
        .iter()
        .find(|stage| stage.scope == da_cli::java_free_slice::StageScope::IdentityAdmission)
        .ok_or("the declaration classifies no identity-admission stage")?;
    Ok(RouteBoundaryInput {
        cascade_stages: declaration
            .stages
            .iter()
            .map(|stage| stage.stage_id.clone())
            .collect(),
        admission_stage: admission.stage_id.clone(),
        in_slice_stages: declaration
            .in_slice_stage_ids()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        out_of_slice_stages: declaration
            .out_of_slice_stage_ids()
            .into_iter()
            .map(str::to_owned)
            .collect(),
    })
}

fn route_corpus(declaration: &da_cli::java_free_slice::SliceDeclaration) -> RouteDeclaredCorpus {
    RouteDeclaredCorpus {
        corpus_id: declaration.corpus_id.clone(),
        document_count: declaration.corpus_n as u64,
        page_denominator: declaration.page_denominator,
        documents: declaration
            .documents
            .iter()
            .map(|document| RouteDeclaredDocument {
                id: document.id.clone(),
                source_pdf: document.source_pdf.clone(),
                sha256: document.sha256.clone(),
                pages: document.pages,
            })
            .collect(),
    }
}

/// The declared freeze revision, recomputed from the committed carriers it pins.
///
/// `sha256(join("<path>:<sha256-hex>", "\n"))` over the carriers in the declared
/// order — the T02 rule, re-executed rather than trusted.
fn recompute_freeze_revision(
    root: &std::path::Path,
    policy: &serde_yaml::Value,
) -> Result<String, Box<dyn Error>> {
    let carriers = policy["freeze_revision"]["carriers"]
        .as_sequence()
        .ok_or("the freeze declares no carriers")?;
    let mut lines = Vec::with_capacity(carriers.len());
    for carrier in carriers {
        let path = carrier["path"]
            .as_str()
            .ok_or("a freeze carrier declares no path")?;
        let bytes = fs::read(root.join(path))?;
        lines.push(format!("{path}:{}", sha256_hex(&bytes)));
    }
    Ok(sha256_prefixed(lines.join("\n").as_bytes()))
}

/// Build the freeze declaration and the limits binding from the governor block.
fn load_freeze(
    root: &std::path::Path,
) -> Result<(RouteFreezeDeclaration, RouteLimitsBinding), Box<dyn Error>> {
    let governor = fs::read(root.join(GOVERNOR_REL))?;
    let governor = yaml(&governor)?;
    let policy = governor
        .get(GOVERNOR_POINTER)
        .and_then(|block| block.get("limit_policy"))
        .ok_or("the governor block carries no native_route_e2e.limit_policy")?
        .clone();

    let declared_revision = policy["freeze_revision"]["value"]
        .as_str()
        .ok_or("the freeze declares no revision value")?
        .to_owned();
    let recomputed = recompute_freeze_revision(root, &policy)?;
    if recomputed != declared_revision {
        return Err(format!(
            "the freeze revision drifted: recomputed {recomputed}, declared {declared_revision}"
        )
        .into());
    }

    let route_directions: BTreeMap<String, String> = policy["route_metrics"]
        .as_sequence()
        .ok_or("the freeze declares no route_metrics")?
        .iter()
        .map(|metric| {
            Ok((
                metric["id"]
                    .as_str()
                    .ok_or("a route metric declares no id")?
                    .to_owned(),
                metric["direction"]
                    .as_str()
                    .ok_or("a route metric declares no direction")?
                    .to_owned(),
            ))
        })
        .collect::<Result<_, Box<dyn Error>>>()?;
    let metrics: Vec<RouteMetricDeclaration> = policy["metrics"]
        .as_sequence()
        .ok_or("the freeze declares no metrics")?
        .iter()
        .map(|metric| {
            let id = metric
                .as_str()
                .ok_or("a frozen metric is not a string")?
                .to_owned();
            let direction = route_directions.get(&id).cloned().unwrap_or_else(|| {
                if ROUTE_FLOOR_METRIC_VOCABULARY.contains(&id.as_str()) {
                    "floor".to_owned()
                } else {
                    "ceiling".to_owned()
                }
            });
            Ok(RouteMetricDeclaration { id, direction })
        })
        .collect::<Result<_, Box<dyn Error>>>()?;

    let direction_vocabulary: Vec<String> = policy["direction_vocabulary"]
        .as_sequence()
        .ok_or("the freeze declares no direction_vocabulary")?
        .iter()
        .map(|value| {
            Ok(value
                .as_str()
                .ok_or("a direction is not a string")?
                .to_owned())
        })
        .collect::<Result<_, Box<dyn Error>>>()?;

    let freeze = RouteFreezeDeclaration {
        freeze_revision: declared_revision.clone(),
        metrics,
        direction_vocabulary,
    };

    let required_str = |key: &str| -> Result<String, Box<dyn Error>> {
        Ok(policy[key]
            .as_str()
            .ok_or_else(|| format!("the freeze declares no `{key}`"))?
            .to_owned())
    };
    let required_bool = |key: &str| -> Result<bool, Box<dyn Error>> {
        policy[key]
            .as_bool()
            .ok_or_else(|| format!("the freeze declares no `{key}`").into())
    };

    let limits = RouteLimitsBinding {
        freeze_revision: declared_revision,
        freeze_state: required_str("state")?,
        freeze_owner: required_str("freeze_owner")?,
        freeze_scope: required_str("freeze_scope")?,
        tuned_after_scoring: required_str("tuned_after_scoring")?,
        zero_is_not_a_measurement: required_bool("zero_is_not_a_measurement")?,
        silent_clamp: required_str("silent_clamp")?,
        pass_requires_frozen_threshold: required_bool("pass_requires_frozen_threshold")?,
        claim_without_freeze: required_str("claim_without_freeze")?,
        direction_vocabulary: freeze.direction_vocabulary.clone(),
        metrics: freeze.metrics.clone(),
    };

    // The freeze is a requirement, and the receipt's own constants are the
    // closed vocabulary: a drifted DATA block must fail rather than be adopted.
    if limits.freeze_state != ROUTE_FREEZE_STATE_FROZEN
        || limits.freeze_scope != ROUTE_FREEZE_SCOPE
        || limits.tuned_after_scoring != ROUTE_TUNED_AFTER_SCORING_FORBIDDEN
        || limits.silent_clamp != ROUTE_SILENT_CLAMP_FORBIDDEN
        || !limits.zero_is_not_a_measurement
        || !limits.pass_requires_frozen_threshold
        || limits.direction_vocabulary != ROUTE_DIRECTION_VOCABULARY
    {
        return Err("the declared freeze is not the frozen diagnostic ceiling".into());
    }

    Ok((freeze, limits))
}

// ──────────────────────── result-to-receipt mapping ────────────────────────

fn unmet_reason_token(reason: &NativeRouteUnmetReason) -> &'static str {
    match reason {
        NativeRouteUnmetReason::RegionNoAdmittedTokens { .. } => "region_no_admitted_tokens",
        NativeRouteUnmetReason::HeaderAtom { .. } => "header_atom",
        NativeRouteUnmetReason::BibliographyAtom { .. } => "bibliography_atom",
        NativeRouteUnmetReason::Visual { .. } => "visual",
        NativeRouteUnmetReason::Callout { .. } => "callout",
        NativeRouteUnmetReason::Section { .. } => "section",
    }
}

fn stage_cell(stage: &da_parse::NativeRouteStage) -> RouteStageStanceCell {
    let reason = match stage.stance {
        NativeRouteStageStance::OutOfScope { reason } => Some(reason.token().to_owned()),
        _ => None,
    };
    RouteStageStanceCell {
        stage: stage.stage_id.clone(),
        stance: stage.stance.token().to_owned(),
        reason,
    }
}

fn family_cell(cell: &da_parse::NativeRouteFamilyCell) -> RouteFamilyCell {
    match cell.disposition {
        NativeRouteFamilyDisposition::Measured { observed } => RouteFamilyCell {
            family: cell.family.token().to_owned(),
            disposition: "measured".to_owned(),
            reason: None,
            observed_count: Some(observed),
        },
        NativeRouteFamilyDisposition::Unmet { reason } => RouteFamilyCell {
            family: cell.family.token().to_owned(),
            disposition: "unmet".to_owned(),
            reason: Some(unmet_reason_token(&reason).to_owned()),
            observed_count: None,
        },
        NativeRouteFamilyDisposition::NotObserved => RouteFamilyCell {
            family: cell.family.token().to_owned(),
            disposition: "not_observed".to_owned(),
            reason: None,
            observed_count: None,
        },
        NativeRouteFamilyDisposition::Refused { reason } => RouteFamilyCell {
            family: cell.family.token().to_owned(),
            disposition: "refused".to_owned(),
            reason: Some(reason.token().to_owned()),
            observed_count: None,
        },
    }
}

/// The T03 body axis. The T01 axis must be measured here: the receipt has no
/// representation for a refused axis, and a refusal would be a real failure
/// rather than a diagnostic.
fn route_body_axis(axis: &NativeRouteBodyAxis) -> Result<RouteBodyAxis, Box<dyn Error>> {
    match axis {
        NativeRouteBodyAxis::Measured {
            verdict,
            reason,
            d498_reopen_required,
            plane_absent_units,
            plane_present_in_token_units,
            plane_spanning_token_units,
            plane_character_material_units,
            ..
        } => Ok(RouteBodyAxis {
            plane_preservation: verdict.token().to_owned(),
            plane_gap_reason: reason.token().to_owned(),
            plane_d498_reopen_required: *d498_reopen_required,
            plane_character_material_units: *plane_character_material_units,
            plane_absent_units: *plane_absent_units,
            plane_present_in_token_units: *plane_present_in_token_units,
            plane_spanning_token_units: *plane_spanning_token_units,
        }),
        NativeRouteBodyAxis::Refused { reason } => Err(format!(
            "the reproduced P1 body axis refused ({}); no receipt is written",
            reason.token()
        )
        .into()),
    }
}

/// The receipt default metric vocabulary used by the S06 route population.
fn population_metrics(
    corpus: &RouteDeclaredCorpus,
    unmeasured_stages: u64,
) -> Vec<RouteMetricValue> {
    [
        ("native_route_documents", "floor", corpus.document_count),
        ("native_route_pages", "floor", corpus.page_denominator),
        ("native_route_java_calls", "ceiling", 0),
        (
            "native_route_unmeasured_stages",
            "ceiling",
            unmeasured_stages,
        ),
        ("native_route_unclassified_families", "ceiling", 0),
    ]
    .iter()
    .map(|(id, direction, value)| RouteMetricValue {
        id: (*id).to_owned(),
        direction: (*direction).to_owned(),
        value: *value,
    })
    .collect()
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

    // ── frozen limits (before any measurement) ────────────────────────────
    let (freeze, limits) = load_freeze(&args.root)?;

    // ── guard on the real Java seam, installed first ───────────────────────
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

    // ── independent oracles over the whole slice ───────────────────────────
    let provenance_bytes = fs::read(args.root.join(PROVENANCE_REL))?;
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
    require_independent_section_oracle(ORACLE_GENERATOR)?;
    if require_independent_section_oracle(NATIVE_SECTION_SUT_GENERATOR_ID).is_ok() {
        return Err("the same-generator section negative control was not refused".into());
    }

    let mut views: BTreeMap<String, OracleView> = BTreeMap::new();
    let mut visual_pins = Vec::new();
    let mut callout_pins = Vec::new();
    for pin in &pins {
        let key = normalize_document_identity(&pin.id);
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
        views.insert(key, view);
        visual_pins.push(NativeVisualOraclePin {
            document_id: pin.id.clone(),
            sha256: pin.sha256.clone(),
        });
        callout_pins.push(NativeCalloutOraclePin {
            document_id: pin.id.clone(),
            sha256: pin.sha256.clone(),
        });
    }

    // The segmentation oracle is a reviewed, committed carrier: re-derive the
    // section rows from it alone and refuse a drifted source artifact.
    let segmentation_manifest_bytes = fs::read(args.root.join(SEGMENTATION_MANIFEST_REL))?;
    let segmentation_manifest: serde_yaml::Value =
        serde_yaml::from_slice(&segmentation_manifest_bytes)?;
    let declared_segmentation_pin = segmentation_manifest["oracle"]["source_artifact_sha256"]
        .as_str()
        .ok_or("the segmentation manifest pins no source artifact")?
        .to_owned();
    let segmentation_source_bytes = fs::read(args.root.join(SEGMENTATION_SOURCE_REL))?;
    if sha256_hex(&segmentation_source_bytes) != declared_segmentation_pin {
        return Err("the reviewed segmentation source drifted".into());
    }
    let page_origin = segmentation_manifest["evaluation"]["coordinate_binding"]["page_origin"]
        .as_str()
        .ok_or("the segmentation manifest declares no page origin")?
        .to_owned();
    let segmentation_fixture = fs::read_to_string(args.root.join(SEGMENTATION_FIXTURE))?;
    let section_rows = derive_section_rows(&segmentation_fixture, &page_origin)?;

    let mut oracle_documents: Vec<(&str, &OracleView)> = Vec::new();
    for document in &declaration.documents {
        let key = normalize_document_identity(&document.id);
        let view = views
            .get(&key)
            .ok_or_else(|| format!("no oracle fixture for declared document {}", document.id))?;
        oracle_documents.push((document.id.as_str(), view));
    }
    let visual = visual_oracle(&oracle_documents, &visual_pins);
    require_non_vacuous_visual_oracle(&visual)
        .map_err(|error| format!("the visual oracle is not usable: {error}"))?;
    let callout = callout_oracle(&oracle_documents, &callout_pins);
    require_non_vacuous_callout_oracle(&callout)
        .map_err(|error| format!("the callout oracle is not usable: {error}"))?;

    // ── boundary / corpus ─────────────────────────────────────────────────
    let boundary = native_boundary(&declaration)?;
    let route_boundary = route_boundary_input(&declaration)?;
    let corpus = route_corpus(&declaration);
    let cascade_stage_ids: Vec<String> = declaration
        .stages
        .iter()
        .map(|stage| stage.stage_id.clone())
        .collect();
    let declared_ids: Vec<String> = declaration
        .documents
        .iter()
        .map(|document| document.id.clone())
        .collect();

    // ── per-document measurement ──────────────────────────────────────────
    let lexicon = lexicon();
    let limits_donor = LayoutDonorLimits::default();
    let mut observed_documents = Vec::with_capacity(declaration.documents.len());
    let mut typed_failures = 0usize;

    for (index, document) in declaration.documents.iter().enumerate() {
        let (_, view) = oracle_documents[index];

        let bytes = fs::read(args.root.join(&document.source_pdf))?;
        let spec = RouteInputSpec {
            declared_document_ids: &declared_ids,
            document_id: document.id.as_str(),
            input_bytes: Some(&bytes),
        };
        if let Err(failure) = da_parse::classify_route_input(&spec) {
            return Err(format!(
                "route input for {} refused: {} ({})",
                document.id,
                failure.reason_token(),
                failure.disposition_token()
            )
            .into());
        }
        let digest = sha256_hex(&bytes);
        if digest != document.sha256 {
            return Err(format!(
                "checksum drift for {}: observed {digest}, pinned {}",
                document.id, document.sha256
            )
            .into());
        }

        // S03 native plane. `build_native_route_result` folds this plane itself;
        // the runner needs only the plane identity and the ordered lexemes for
        // the body oracle.
        let observation =
            PdfInspectorLayoutDonor.extract_shadow(&bytes, &document.id, limits_donor)?;

        // The reproduced S05 body axis needs the same tokenization the S05
        // receipt used: ordered plane lexemes against the preserved body prose.
        let mut ordered: Vec<_> = observation.lexemes.iter().collect();
        ordered.sort_by_key(|lexeme| (lexeme.page_ordinal, lexeme.source_ordinal));
        let plane_units =
            BodyUnitInventory::from_fragments(ordered.iter().map(|lexeme| lexeme.text()), RUNG);
        let oracle_units = BodyUnitInventory::from_text(&view.body_text, RUNG);
        let body_oracle = NativeRouteBodyOracle {
            native_plane_units: plane_units,
            oracle_units,
        };

        let input = NativeRouteInput {
            document: &observation.document,
            lattice: &observation.lattice,
            lexemes: &observation.lexemes,
            lexicon: &lexicon,
            boundary: &boundary,
            cascade_stage_ids: &cascade_stage_ids,
            body_oracle: &body_oracle,
            section_lane_id: NATIVE_MEASURED_LANE_ID,
            section_oracle_independent: true,
            section_oracle: &section_rows,
            visual_oracle: &visual,
            callout_oracle: &callout,
        };
        let result = build_native_route_result(&input).map_err(|error| {
            format!(
                "the native route result for {} was refused: {error}",
                document.id
            )
        })?;

        // T04: run the attempt and require the report to be self-consistent and
        // to have observed zero Java consults.
        let report = attempt_native_route(&spec, counter.observation(), || Ok(result.clone()));
        if !report.is_consistent() {
            return Err(format!(
                "the route attempt report for {} is inconsistent (measured={}, failures={})",
                document.id,
                report.is_measured(),
                report.failures.len()
            )
            .into());
        }
        if !report.java.is_observed_zero() {
            return Err(format!(
                "the route attempt for {} did not observe a guard-owned zero",
                document.id
            )
            .into());
        }
        typed_failures += report.failures.len();

        let body = route_body_axis(&result.body_axis)?;
        if result
            .body_axis
            .is_frozen_s05_residual()
            .then_some(())
            .is_none()
        {
            return Err(format!(
                "{} did not reproduce the frozen S05 body residual (not_assessable / p1_localization_uncertain)",
                document.id
            )
            .into());
        }
        observed_documents.push(RouteObservedDocument {
            id: document.id.clone(),
            sha256: digest,
            pages: document.pages,
            stages: result.stages.iter().map(stage_cell).collect(),
            families: result.families.iter().map(family_cell).collect(),
            body,
            route_digest: result.digest.clone(),
        });
    }

    // ── populations ───────────────────────────────────────────────────────
    let unmeasured_stages: u64 = observed_documents
        .iter()
        .map(|document| {
            document
                .stages
                .iter()
                .filter(|cell| cell.stance != "native_measured")
                .count() as u64
        })
        .sum();
    let identities = corpus
        .documents
        .iter()
        .map(
            |document| da_cli::native_route_receipt::RouteDocumentIdentity {
                id: document.id.clone(),
                source_pdf: document.source_pdf.clone(),
                sha256: document.sha256.clone(),
                pages: document.pages,
            },
        )
        .collect();
    let populations = vec![RoutePopulationInput {
        population: RoutePopulationId::NativeRoute,
        documents: identities,
        metrics: population_metrics(&corpus, unmeasured_stages),
    }];

    // ── receipt ───────────────────────────────────────────────────────────
    let receipt = build_route_receipt(
        &declaration_revision,
        &corpus,
        &route_boundary,
        &freeze,
        &limits,
        CLAIM_SCOPE_DIAGNOSTIC_ONLY,
        PRODUCT_COVERAGE_EFFECT_NONE,
        guard,
        &observed_documents,
        &populations,
    )?;
    let expectation = RouteExpectation {
        declaration_revision_sha256: declaration_revision.clone(),
        corpus: corpus.clone(),
        boundary: route_boundary.clone(),
        freeze: freeze.clone(),
        required_families: ROUTE_FAMILY_ORDER.iter().map(|f| (*f).to_owned()).collect(),
    };
    validate_route_receipt(&receipt, &expectation)?;

    let artifact = receipt.artifact_json_bytes()?;
    assert_route_redacted(std::str::from_utf8(&artifact)?)?;
    write_artifact(&args.report, &artifact)?;

    Ok(summary(&receipt, typed_failures))
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

fn summary(receipt: &RouteReceipt, typed_failures: usize) -> String {
    format!(
        "native route `{}`: {} documents, {} populations, {} stage cells, {} family cells; \
         java calls observed {} via installed={} identity_isolated={}; body residual `{}`/`{}`; \
         {} typed failure dispositions recorded; freeze {}",
        receipt.schema_version,
        receipt.documents.len(),
        receipt.populations.len(),
        receipt
            .documents
            .iter()
            .map(|document| document.stages.len())
            .sum::<usize>(),
        receipt
            .documents
            .iter()
            .map(|document| document.families.len())
            .sum::<usize>(),
        receipt.guard.observed_java_calls,
        receipt.guard.installed,
        receipt.guard.identity_isolated,
        receipt.residuals.body.plane_preservation,
        receipt.residuals.body.plane_gap_reason,
        typed_failures,
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
    let request = ParseBackendRequest::new("native-route-guard-control", b"%PDF-1.4".to_vec());
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
/// fails the self-check. This is the runner's own half of the negative matrix;
/// `tests/native_route_e2e_contract.rs` audits the committed artifact's shape.
fn run_self_check(root: &std::path::Path) -> Result<String, Box<dyn Error>> {
    let mut checked = 0usize;

    let declared = vec!["hub-2507.19457".to_owned(), "rlm-2512.24601".to_owned()];

    // (a) unknown document
    let unknown = RouteInputSpec {
        declared_document_ids: &declared,
        document_id: "unknown-9999.00000",
        input_bytes: None,
    };
    let failure = da_parse::classify_route_input(&unknown)
        .err()
        .ok_or("an unknown document was admitted")?;
    if failure.reason_token() != "unknown_document"
        || failure.disposition_token() != "unknown_document"
    {
        return Err(format!("unexpected unknown-document refusal: {failure:?}").into());
    }
    checked += 1;

    // (b) absent bytes
    let absent = RouteInputSpec {
        declared_document_ids: &declared,
        document_id: "hub-2507.19457",
        input_bytes: None,
    };
    let failure = da_parse::classify_route_input(&absent)
        .err()
        .ok_or("absent bytes were admitted")?;
    if failure.disposition_token() != RouteRecoveryDisposition::InputBytesAbsent.token() {
        return Err(format!("unexpected absent-bytes refusal: {failure:?}").into());
    }
    checked += 1;

    // (c) malformed bytes
    let malformed = RouteInputSpec {
        declared_document_ids: &declared,
        document_id: "hub-2507.19457",
        input_bytes: Some(b"not a pdf at all"),
    };
    let failure = da_parse::classify_route_input(&malformed)
        .err()
        .ok_or("malformed bytes were admitted")?;
    if failure.disposition_token() != RouteRecoveryDisposition::InputBytesMalformed.token() {
        return Err(format!("unexpected malformed-bytes refusal: {failure:?}").into());
    }
    checked += 1;

    // (d) sha256 drift
    if verify_document_pin("hub-2507.19457", &"0".repeat(64), &"1".repeat(64)).is_ok() {
        return Err("a sha256 drift was admitted by the pin predicate".into());
    }
    checked += 1;

    // (e)/(f) receipt-level refusals the runner must never bypass.
    let (corpus, boundary, freeze, limits) = synthetic_receipt_inputs(root)?;
    let observed = synthetic_observed(&corpus, &boundary);
    let populations = synthetic_populations(&corpus);

    let non_diagnostic = build_route_receipt(
        &sha256_prefixed(b"self-check"),
        &corpus,
        &boundary,
        &freeze,
        &limits,
        "product",
        PRODUCT_COVERAGE_EFFECT_NONE,
        GuardProvenance {
            installed: true,
            identity_isolated: true,
            observed_java_calls: 0,
        },
        &observed,
        &populations,
    )
    .err()
    .ok_or("a non-diagnostic claim scope was admitted")?;
    if !format!("{non_diagnostic:?}").contains("ClaimScopeDrift") {
        return Err(format!("unexpected claim-scope refusal: {non_diagnostic:?}").into());
    }
    checked += 1;

    let java_calls = build_route_receipt(
        &sha256_prefixed(b"self-check"),
        &corpus,
        &boundary,
        &freeze,
        &limits,
        CLAIM_SCOPE_DIAGNOSTIC_ONLY,
        PRODUCT_COVERAGE_EFFECT_NONE,
        GuardProvenance {
            installed: true,
            identity_isolated: true,
            observed_java_calls: 1,
        },
        &observed,
        &populations,
    )
    .err()
    .ok_or("a positive Java count was admitted")?;
    if !format!("{java_calls:?}").contains("ObservedJavaOnRoute") {
        return Err(format!("unexpected java-count refusal: {java_calls:?}").into());
    }
    checked += 1;

    Ok(format!(
        "native route self-check: {checked} named refusals executed, all refused"
    ))
}

fn verify_document_pin(
    document_id: &str,
    observed: &str,
    pinned: &str,
) -> Result<(), Box<dyn Error>> {
    if observed != pinned {
        return Err(format!(
            "checksum drift for {document_id}: observed {observed}, pinned {pinned}"
        )
        .into());
    }
    Ok(())
}

/// Self-check fixtures derived from the live DATA, so the negative controls run
/// against the real boundary/corpus/freeze rather than a restated copy.
fn synthetic_receipt_inputs(
    root: &std::path::Path,
) -> Result<
    (
        RouteDeclaredCorpus,
        RouteBoundaryInput,
        RouteFreezeDeclaration,
        RouteLimitsBinding,
    ),
    Box<dyn Error>,
> {
    let layout_bytes = fs::read(root.join(SLICE_DECLARATION_DATA_REL))?;
    let compatibility_bytes = fs::read(root.join(COMPATIBILITY_DATA_REL))?;
    let declaration = load_declaration(
        std::str::from_utf8(&layout_bytes)?,
        std::str::from_utf8(&compatibility_bytes)?,
    )?;
    let (freeze, limits) = load_freeze(root)?;
    Ok((
        route_corpus(&declaration),
        route_boundary_input(&declaration)?,
        freeze,
        limits,
    ))
}

fn synthetic_observed(
    corpus: &RouteDeclaredCorpus,
    boundary: &RouteBoundaryInput,
) -> Vec<RouteObservedDocument> {
    let stages: Vec<RouteStageStanceCell> = boundary
        .cascade_stages
        .iter()
        .map(|stage| {
            if *stage == boundary.admission_stage {
                RouteStageStanceCell {
                    stage: stage.clone(),
                    stance: "admission_boundary".to_owned(),
                    reason: None,
                }
            } else if boundary.out_of_slice_stages.contains(stage) {
                RouteStageStanceCell {
                    stage: stage.clone(),
                    stance: "out_of_scope".to_owned(),
                    reason: Some("java_authority".to_owned()),
                }
            } else {
                RouteStageStanceCell {
                    stage: stage.clone(),
                    stance: "native_measured".to_owned(),
                    reason: None,
                }
            }
        })
        .collect();
    let families: Vec<RouteFamilyCell> = ROUTE_FAMILY_ORDER
        .iter()
        .map(|family| RouteFamilyCell {
            family: (*family).to_owned(),
            disposition: "measured".to_owned(),
            reason: None,
            observed_count: Some(1),
        })
        .collect();
    let body = RouteBodyAxis {
        plane_preservation: "not_assessable".to_owned(),
        plane_gap_reason: "p1_localization_uncertain".to_owned(),
        plane_d498_reopen_required: false,
        plane_character_material_units: 0,
        plane_absent_units: 0,
        plane_present_in_token_units: 0,
        plane_spanning_token_units: 0,
    };
    corpus
        .documents
        .iter()
        .map(|document| RouteObservedDocument {
            id: document.id.clone(),
            sha256: document.sha256.clone(),
            pages: document.pages,
            stages: stages.clone(),
            families: families.clone(),
            body: body.clone(),
            route_digest: sha256_prefixed(format!("{}:self-check", document.id).as_bytes()),
        })
        .collect()
}

fn synthetic_populations(corpus: &RouteDeclaredCorpus) -> Vec<RoutePopulationInput> {
    let identities = corpus
        .documents
        .iter()
        .map(
            |document| da_cli::native_route_receipt::RouteDocumentIdentity {
                id: document.id.clone(),
                source_pdf: document.source_pdf.clone(),
                sha256: document.sha256.clone(),
                pages: document.pages,
            },
        )
        .collect();
    vec![RoutePopulationInput {
        population: RoutePopulationId::NativeRoute,
        documents: identities,
        metrics: population_metrics(corpus, 0),
    }]
}
