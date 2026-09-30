//! S04/T04 (M514-1lss2s) — family-parity runner for the declared three-document
//! slice: real native header and bibliography extraction, with an independent
//! oracle and a typed, corpus-bound receipt.
//!
//! This runner is the executable half of the S04 parity contract. It:
//!
//! - reads the slice declaration and the corpus pin by pointer from
//!   `data/parse/layout_bakeoff.yaml#java_free_vertical_slice` (it never restates
//!   the boundary, the documents or the sha256 pins);
//! - installs a [`JavaInvocationCounter`] on the *real* Java transport seam
//!   (`GrobidHttpBackend` + `with_invocation_observer`) and never consults it;
//! - for each pinned PDF runs the S03 native plane
//!   (`PdfInspectorLayoutDonor` -> `segment_document_full` ->
//!   `project_native_words` -> `assign_heading_regions` ->
//!   `build_native_word_region_carrier`) and then the T02 header and T03
//!   bibliography bands on the resulting carrier;
//! - builds the expectation side from the preserved GROBID TEI fixtures pinned
//!   in `provenance.yaml` (produced by `java-grobid-crf`, a different
//!   generator), refuses a same-generator oracle, and records a character-level
//!   coverage verdict per region;
//! - writes `artifacts/parse_compat/native_family_parity.json` and exits
//!   non-zero on PDF sha256 drift, an unknown document, Java observed > 0, an
//!   incomplete typed matrix, a non-independent oracle, or a claim-scope drift.
//!
//! It is diagnostic-only: `thresholds = not_accepted`,
//! `product_coverage_effect = none`, no promotion, no production splice, no
//! graph write (D127 / D499). Honest residuals stay visible in the matrix: the
//! `hub` References region and `reca`'s zero-font header token are typed
//! dispositions, not hidden gaps.

use da_adapters::{LayoutDonorLimits, PdfInspectorLayoutDonor};
use da_cli::java_free_slice::{
    COMPATIBILITY_DATA_REL, SLICE_DECLARATION_DATA_REL, load_declaration, sha256_hex,
};
use da_cli::native_family_receipt::{
    AtomDisposition, CLAIM_SCOPE_DIAGNOSTIC_ONLY, CoverageRegion, CoverageVerdict,
    DECLARATION_MANIFEST_REL, DEFAULT_ARTIFACT_REL, DeclaredCorpus, DeclaredDocument, FamilyCell,
    FamilyDisposition, GuardProvenance, NativeFamilyId, ORACLE_GENERATOR_ID, ORACLE_MANIFEST_REL,
    ObservedDocument, OracleDocument, OracleInput, PopulationId, RefusalReason, RegionCoverageRow,
    assert_redacted, build_parity_receipt, char_set, classify_coverage,
    normalize_document_identity, sha256_prefixed,
};
use da_parse::{
    BibliographyAtom, BibliographyAtomDisposition, FullSemanticRegion, GrobidHttpBackend,
    GrobidProfile, HeaderAtom, HeaderAtomDisposition, HeadingRegionConfig, JavaInvocationCounter,
    NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID, NATIVE_HEADER_INPUT_GENERATOR_ID,
    NativeBibliographyAtomOutcome, NativeBibliographyFromInputOutcome,
    NativeBibliographyInputError, NativeHeaderAtomOutcome, NativeHeaderFromInputOutcome,
    NativeHeaderInputError, SegmentationConfig, assign_heading_regions,
    build_native_bibliography_input, build_native_header_input, build_native_word_region_carrier,
    project_native_words, require_independent_bibliography_oracle,
    require_independent_header_oracle, segment_document_full,
};
use da_ports::{FeatureTemplate, LexiconBundle, LexiconChannel};
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

/// Provenance fixture ids, matched to the pinned corpus by normalized identity.
const FIXTURE_IDS: [&str; 3] = ["hub_2507.19457", "rlm_2512.24601", "reca_2605.26525v1"];

struct Args {
    root: PathBuf,
    report: PathBuf,
}

fn parse_args() -> Args {
    let mut root = PathBuf::from(".");
    let mut report = PathBuf::from(DEFAULT_ARTIFACT_REL);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = args.next().unwrap_or_default().into(),
            "--report" => report = args.next().unwrap_or_default().into(),
            "--help" => {
                println!("native_family_parity [--root DIR] [--report FILE]");
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
    let _log_guard = da_cli::logging::init_pipeline_logging("native_family_parity");
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native_family_parity: {error}");
            ExitCode::FAILURE
        }
    }
}

// ─────────────────────── independent oracle (TEI lexical scan) ───────────────────────

/// One pinned preserved-TEI fixture, as `provenance.yaml` publishes it.
#[derive(Debug, Clone)]
struct Pin {
    id: String,
    path: String,
    sha256: String,
}

/// The TEI-derived expectation for one document.
#[derive(Debug, Clone, Default)]
struct OracleDocumentView {
    title: Option<String>,
    authors: Vec<String>,
    dates: Vec<String>,
    affiliations: Vec<String>,
    biblstruct_count: usize,
    bibr_ref_count: usize,
    bibliography_texts: Vec<String>,
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

/// Ordered bodies of every `<tag ...>...</tag>` occurrence (non-nested).
fn element_bodies(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(offset) = xml[at..].find(&open) {
        let start = at + offset;
        let rest = &xml[start + open.len()..];
        let boundary = rest.starts_with('>') || rest.starts_with(|c: char| c.is_whitespace());
        if !boundary {
            at = start + open.len();
            continue;
        }
        let gt = start + xml[start..].find('>').expect("opening tag is closed");
        let body = &xml[gt + 1..];
        match body.find(&close) {
            Some(body_end) => {
                out.push(body[..body_end].trim().to_owned());
                at = gt + 1 + body_end + close.len();
            }
            None => break,
        }
    }
    out
}

/// Slice the `<teiHeader>` element without parsing the rest of the document.
fn header_slice(xml: &str) -> &str {
    let start = match xml.find("<teiHeader") {
        Some(start) => start,
        None => return xml,
    };
    let body = &xml[start..];
    match body.find("</teiHeader>") {
        Some(end) => &body[..end],
        None => body,
    }
}

/// Remove every `<...>` tag, leaving the element text.
fn strip_tags(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut depth = 0usize;
    for character in xml.chars() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            other if depth == 0 => out.push(other),
            _ => {}
        }
    }
    out
}

/// Count `<ref type="bibr">` occurrences in the document.
fn count_bibr_refs(xml: &str) -> usize {
    let mut count = 0;
    let mut at = 0;
    while let Some(offset) = xml[at..].find("<ref") {
        let start = at + offset;
        let rest = &xml[start + 4..];
        let boundary = rest.starts_with('>') || rest.starts_with(|c: char| c.is_whitespace());
        if !boundary {
            at = start + 4;
            continue;
        }
        let gt = start + xml[start..].find('>').unwrap_or(start);
        let tag = &xml[start..=gt];
        if tag.contains("type=\"bibr\"") {
            count += 1;
        }
        at = gt + 1;
    }
    count
}

/// Lexically scan the preserved TEI into the oracle view. This uses only `std`
/// string operations: it never calls a native band, the atoms, or the layout
/// plane, which is what makes it an oracle rather than the SUT.
fn scan_oracle(xml: &str) -> OracleDocumentView {
    let header = header_slice(xml);
    let title = element_bodies(header, "titleStmt")
        .first()
        .and_then(|statement| element_bodies(statement, "title").into_iter().next())
        .filter(|value| !value.is_empty());
    let authors = element_bodies(header, "author")
        .iter()
        .map(|author| {
            let mut parts = element_bodies(author, "forename");
            parts.extend(element_bodies(author, "surname"));
            if parts.is_empty() {
                strip_tags(author).trim().to_owned()
            } else {
                parts.join(" ")
            }
        })
        .collect();
    let bodies = element_bodies(xml, "biblStruct");
    OracleDocumentView {
        title,
        authors,
        dates: element_bodies(header, "date"),
        affiliations: element_bodies(header, "orgName"),
        biblstruct_count: bodies.len(),
        bibr_ref_count: count_bibr_refs(xml),
        bibliography_texts: bodies.iter().map(|body| strip_tags(body)).collect(),
    }
}

/// Header oracle text, in a deterministic field order.
fn oracle_header_text(view: &OracleDocumentView) -> String {
    let mut parts = Vec::new();
    if let Some(title) = &view.title {
        parts.push(title.as_str());
    }
    parts.extend(view.authors.iter().map(String::as_str));
    parts.extend(view.dates.iter().map(String::as_str));
    parts.extend(view.affiliations.iter().map(String::as_str));
    parts.join("\n")
}

/// Bibliography oracle text.
fn oracle_bibliography_text(view: &OracleDocumentView) -> String {
    view.bibliography_texts.join("\n")
}

/// Normalized joined oracle values, for the per-document oracle digest.
fn oracle_digest_text(view: &OracleDocumentView) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(title) = &view.title {
        parts.push(title.trim().to_lowercase());
    }
    parts.extend(view.authors.iter().map(|value| value.trim().to_lowercase()));
    parts.extend(view.dates.iter().map(|value| value.trim().to_lowercase()));
    parts.extend(
        view.affiliations
            .iter()
            .map(|value| value.trim().to_lowercase()),
    );
    parts.push(format!("biblstruct={}", view.biblstruct_count));
    parts.push(format!("bibr={}", view.bibr_ref_count));
    parts.join("\u{1f}")
}

// ─────────────────────── native pipeline ───────────────────────

/// Minimal project-owned lexicon (no GROBID lexicon bytes are read or
/// committed, D358).
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

fn header_atom_disposition(disposition: HeaderAtomDisposition) -> AtomDisposition {
    match disposition {
        HeaderAtomDisposition::Recognized => AtomDisposition::Recognized,
        HeaderAtomDisposition::Empty => AtomDisposition::Empty,
        HeaderAtomDisposition::Malformed => AtomDisposition::Malformed,
        HeaderAtomDisposition::Unsupported => AtomDisposition::Unsupported,
        HeaderAtomDisposition::Ambiguous => AtomDisposition::Ambiguous,
        HeaderAtomDisposition::Oversized => AtomDisposition::Oversized,
    }
}

fn bibliography_atom_disposition(disposition: BibliographyAtomDisposition) -> AtomDisposition {
    match disposition {
        BibliographyAtomDisposition::Recognized => AtomDisposition::Recognized,
        BibliographyAtomDisposition::Empty => AtomDisposition::Empty,
        BibliographyAtomDisposition::Malformed => AtomDisposition::Malformed,
        BibliographyAtomDisposition::Unsupported => AtomDisposition::Unsupported,
        BibliographyAtomDisposition::Ambiguous => AtomDisposition::Ambiguous,
        BibliographyAtomDisposition::Oversized => AtomDisposition::Oversized,
        BibliographyAtomDisposition::Truncated => AtomDisposition::Truncated,
        BibliographyAtomDisposition::OutOfScope => AtomDisposition::OutOfScope,
    }
}

fn header_refusal(error: &NativeHeaderInputError) -> RefusalReason {
    match error {
        NativeHeaderInputError::NoAdmittedTokens { .. } => RefusalReason::NoAdmittedTokens,
        NativeHeaderInputError::RegionNotRequested { .. } => RefusalReason::RegionNotRequested,
        NativeHeaderInputError::NoFeatureTokens => RefusalReason::NoFeatureTokens,
        NativeHeaderInputError::MissingFontSize { .. } => RefusalReason::MissingFontSize,
        NativeHeaderInputError::InvalidFontSize { .. } => RefusalReason::InvalidFontSize,
        NativeHeaderInputError::TokenContainsWhitespace { .. } => {
            RefusalReason::TokenContainsWhitespace
        }
        NativeHeaderInputError::FeatureRender(_) => RefusalReason::FeatureBuildRefused,
        NativeHeaderInputError::Serialization => RefusalReason::SerializationRefused,
    }
}

fn bibliography_refusal(error: &NativeBibliographyInputError) -> RefusalReason {
    match error {
        NativeBibliographyInputError::NoAdmittedTokens { .. } => RefusalReason::NoAdmittedTokens,
        NativeBibliographyInputError::RegionNotRequested { .. } => {
            RefusalReason::RegionNotRequested
        }
        NativeBibliographyInputError::NoFeatureTokens => RefusalReason::NoFeatureTokens,
        NativeBibliographyInputError::TokenContainsWhitespace { .. } => {
            RefusalReason::TokenContainsWhitespace
        }
        NativeBibliographyInputError::AnchorScanNotNative { .. } => {
            RefusalReason::AnchorScanNotNative
        }
        NativeBibliographyInputError::Feature(_) => RefusalReason::FeatureBuildRefused,
        NativeBibliographyInputError::Gazetteer(_) => RefusalReason::GazetteerRefused,
        NativeBibliographyInputError::Serialization => RefusalReason::SerializationRefused,
    }
}

fn header_atom(
    outcome: &NativeHeaderFromInputOutcome,
    atom: HeaderAtom,
) -> Option<&NativeHeaderAtomOutcome> {
    outcome.atoms.iter().find(|entry| entry.atom == atom)
}

fn bibliography_atom(
    outcome: &NativeBibliographyFromInputOutcome,
    atom: BibliographyAtom,
) -> Option<&NativeBibliographyAtomOutcome> {
    outcome.atoms.iter().find(|entry| entry.atom == atom)
}

/// A header family cell from a successful band outcome.
fn header_observed_cell(
    outcome: &NativeHeaderFromInputOutcome,
    atom: HeaderAtom,
    family: NativeFamilyId,
    oracle_count: Option<u64>,
) -> Result<FamilyCell, Box<dyn Error>> {
    let entry = header_atom(outcome, atom)
        .ok_or_else(|| format!("header band outcome omits atom {atom:?} (not a typed matrix)"))?;
    Ok(FamilyCell {
        family,
        disposition: FamilyDisposition::Observed,
        atom_disposition: Some(header_atom_disposition(entry.disposition)),
        observed_count: Some(entry.recognized_count as u64),
        observed_item_count: None,
        observed_digest: Some(entry.evidence_digest.clone()),
        oracle_count,
        refusal: None,
    })
}

/// A bibliography family cell from a successful band outcome.
fn bibliography_observed_cell(
    outcome: &NativeBibliographyFromInputOutcome,
    atom: BibliographyAtom,
    family: NativeFamilyId,
    oracle_count: Option<u64>,
    out_of_scope: bool,
) -> Result<FamilyCell, Box<dyn Error>> {
    let entry = bibliography_atom(outcome, atom).ok_or_else(|| {
        format!("bibliography band outcome omits atom {atom:?} (not a typed matrix)")
    })?;
    if out_of_scope {
        return Ok(FamilyCell {
            family,
            disposition: FamilyDisposition::OutOfScope,
            atom_disposition: Some(AtomDisposition::OutOfScope),
            observed_count: None,
            observed_item_count: None,
            observed_digest: None,
            oracle_count,
            refusal: None,
        });
    }
    Ok(FamilyCell {
        family,
        disposition: FamilyDisposition::Observed,
        atom_disposition: Some(bibliography_atom_disposition(entry.disposition)),
        observed_count: Some(entry.recognized_count as u64),
        observed_item_count: Some(entry.item_count as u64),
        observed_digest: Some(entry.evidence_digest.clone()),
        oracle_count,
        refusal: None,
    })
}

/// Every header family, typed as one unit when the band did not execute.
fn header_absent_cells(
    disposition: FamilyDisposition,
    refusal: Option<RefusalReason>,
    view: &OracleDocumentView,
) -> Vec<FamilyCell> {
    let oracle = [
        (NativeFamilyId::HeaderDate, Some(view.dates.len() as u64)),
        (
            NativeFamilyId::HeaderPersonalName,
            Some(view.authors.len() as u64),
        ),
        (
            NativeFamilyId::HeaderAffiliation,
            Some(view.affiliations.len() as u64),
        ),
        (NativeFamilyId::HeaderAffiliationLink, None),
    ];
    oracle
        .iter()
        .map(|(family, oracle_count)| FamilyCell {
            family: *family,
            disposition,
            atom_disposition: None,
            observed_count: None,
            observed_item_count: None,
            observed_digest: None,
            oracle_count: *oracle_count,
            refusal,
        })
        .collect()
}

/// Every bibliography family, typed as one unit when the band did not execute.
fn bibliography_absent_cells(
    disposition: FamilyDisposition,
    refusal: Option<RefusalReason>,
    view: &OracleDocumentView,
) -> Vec<FamilyCell> {
    let count = view.biblstruct_count as u64;
    [
        (NativeFamilyId::BibliographyCitationFields, Some(count)),
        (NativeFamilyId::BibliographyReferenceSegments, Some(count)),
        (
            NativeFamilyId::BibliographyCitationAnchors,
            Some(view.bibr_ref_count as u64),
        ),
    ]
    .iter()
    .map(|(family, oracle_count)| FamilyCell {
        family: *family,
        disposition,
        atom_disposition: None,
        observed_count: None,
        observed_item_count: None,
        observed_digest: None,
        oracle_count: *oracle_count,
        refusal,
    })
    .collect()
}

/// Join a region's carrier tokens into its native text.
fn region_text(carrier: &da_parse::NativeRegionCarrier, region: FullSemanticRegion) -> String {
    match carrier.run(region) {
        Some(run) => run
            .tokens
            .iter()
            .map(|token| token.text())
            .collect::<Vec<_>>()
            .join(" "),
        None => String::new(),
    }
}

fn coverage_row(region: CoverageRegion, oracle_text: &str, native_text: &str) -> RegionCoverageRow {
    let oracle = char_set(oracle_text);
    let native = char_set(native_text);
    let missing = oracle.difference(&native).count() as u64;
    RegionCoverageRow {
        region,
        oracle_char_count: oracle.len() as u64,
        native_char_count: native.len() as u64,
        missing_char_count: missing,
        verdict: classify_coverage(&oracle, &native),
        oracle_text_digest: sha256_prefixed(oracle_text.as_bytes()),
        native_text_digest: sha256_prefixed(native_text.as_bytes()),
    }
}

#[allow(clippy::too_many_lines)]
fn run(args: &Args) -> Result<String, Box<dyn Error>> {
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

    // The oracle pins.
    let provenance_bytes = fs::read(args.root.join(ORACLE_MANIFEST_REL))?;
    let provenance_text = std::str::from_utf8(&provenance_bytes)?;
    let (oracle_backend, pins) = load_provenance(provenance_text)?;
    if oracle_backend != "java-grobid-crf" {
        return Err(
            format!("oracle backend is `{oracle_backend}`, expected `java-grobid-crf`").into(),
        );
    }
    if pins.len() != FIXTURE_IDS.len() {
        return Err(format!(
            "provenance pins {} fixtures, expected {}",
            pins.len(),
            FIXTURE_IDS.len()
        )
        .into());
    }
    let provenance_revision = sha256_hex(&provenance_bytes);

    // Oracle independence: the real provenance must clear both requirements,
    // and a same-generator provenance must be refused. Both are executed.
    let header_provenance = da_parse::HeaderOracleProvenance {
        generator_id: ORACLE_GENERATOR_ID.to_owned(),
        backend: oracle_backend.clone(),
        sha256: provenance_revision.clone(),
    };
    let bibliography_provenance = da_parse::BibliographyOracleProvenance {
        generator_id: ORACLE_GENERATOR_ID.to_owned(),
        backend: oracle_backend.clone(),
        sha256: provenance_revision.clone(),
    };
    let independent = require_independent_header_oracle(&header_provenance).is_ok()
        && require_independent_bibliography_oracle(&bibliography_provenance).is_ok();
    if !independent {
        return Err("the oracle provenance is not independent".into());
    }
    let same_generator_refused =
        require_independent_header_oracle(&da_parse::HeaderOracleProvenance {
            generator_id: NATIVE_HEADER_INPUT_GENERATOR_ID.to_owned(),
            backend: oracle_backend.clone(),
            sha256: provenance_revision.clone(),
        })
        .is_err()
            && require_independent_bibliography_oracle(&da_parse::BibliographyOracleProvenance {
                generator_id: NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID.to_owned(),
                backend: oracle_backend.clone(),
                sha256: provenance_revision.clone(),
            })
            .is_err();
    if !same_generator_refused {
        return Err("the same-generator negative control was not refused".into());
    }

    // Build the oracle views keyed by normalized document identity.
    let mut oracle_views: BTreeMap<String, OracleDocumentView> = BTreeMap::new();
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
        let xml = std::str::from_utf8(&bytes)?;
        let key = normalize_document_identity(&pin.id);
        if oracle_views.insert(key, scan_oracle(xml)).is_some() {
            return Err(format!("duplicate oracle fixture {}", pin.id).into());
        }
    }

    // The guard on the real Java seam. Nothing below consults Java.
    let counter = JavaInvocationCounter::new();
    let plain = GrobidHttpBackend::new(Some("http://127.0.0.1:9"))
        .with_profile(GrobidProfile::structural());
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

    let limits = LayoutDonorLimits::default();
    let lexicon = lexicon();
    let mut observed_documents = Vec::with_capacity(declaration.documents.len());
    let mut oracle_documents = Vec::with_capacity(declaration.documents.len());
    let mut coverage_notes: Vec<String> = Vec::new();

    for document in &declaration.documents {
        let key = normalize_document_identity(&document.id);
        let view = oracle_views
            .get(&key)
            .ok_or_else(|| format!("no oracle fixture for declared document {}", document.id))?;
        let fixture = pins
            .iter()
            .find(|pin| normalize_document_identity(&pin.id) == key)
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

        let observation = PdfInspectorLayoutDonor.extract_shadow(&bytes, &document.id, limits)?;
        let projection = project_native_words(&observation.document, &observation.lexemes)?;

        // Repair invariant: a projected word never carries internal whitespace.
        let mut whitespace_words = 0usize;
        for parent in &projection.parents {
            for word in &parent.words {
                if word.text().chars().any(char::is_whitespace) {
                    whitespace_words += 1;
                }
            }
        }
        if whitespace_words > 0 {
            return Err(format!(
                "{} projected {} word(s) with internal whitespace; the repair invariant broke",
                document.id, whitespace_words
            )
            .into());
        }

        // The declared pipeline runs the S03 walk too; its order digest is part
        // of the per-document pipeline provenance. Region assignment for the
        // bands comes from the heading-aware walk, which supersedes it (WP-02).
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
            &[FullSemanticRegion::Header, FullSemanticRegion::References],
        )?;
        let projection_digest = projection.digest()?;
        let heading_digest = heading.digest()?;
        let carrier_digest = carrier.digest()?;
        let pipeline_digest = sha256_prefixed(
            format!(
                "{projection_digest}\u{1f}{}\u{1f}{heading_digest}\u{1f}{carrier_digest}",
                segmentation.order_digest
            )
            .as_bytes(),
        );

        // Honest-region evidence: a region is entered only through recorded
        // HeadingEvidence, and a region that was never entered is a typed
        // absence rather than a silent "everything is Header".
        let heading_evidence = heading
            .evidences
            .iter()
            .map(|evidence| {
                format!(
                    "{:?}:p{}:l{}:{:?}:{}w",
                    evidence.region,
                    evidence.page_ordinal,
                    evidence.line_ordinal,
                    evidence.matched_by,
                    evidence.word_count
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let heading_absences = heading
            .absences
            .iter()
            .map(|absence| format!("{absence:?}"))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{}: heading_evidence=[{}] typed_absences=[{}]",
            document.id, heading_evidence, heading_absences
        );

        // T02 header band.
        let mut families: Vec<FamilyCell> = match build_native_header_input(&carrier, &lexicon) {
            Ok(outcome) => vec![
                header_observed_cell(
                    &outcome,
                    HeaderAtom::Date,
                    NativeFamilyId::HeaderDate,
                    Some(view.dates.len() as u64),
                )?,
                header_observed_cell(
                    &outcome,
                    HeaderAtom::PersonalName,
                    NativeFamilyId::HeaderPersonalName,
                    Some(view.authors.len() as u64),
                )?,
                header_observed_cell(
                    &outcome,
                    HeaderAtom::Affiliation,
                    NativeFamilyId::HeaderAffiliation,
                    Some(view.affiliations.len() as u64),
                )?,
                header_observed_cell(
                    &outcome,
                    HeaderAtom::AffiliationLink,
                    NativeFamilyId::HeaderAffiliationLink,
                    None,
                )?,
            ],
            Err(NativeHeaderInputError::NoAdmittedTokens { .. }) => {
                header_absent_cells(FamilyDisposition::TypedAbsence, None, view)
            }
            Err(error) => header_absent_cells(
                FamilyDisposition::Refused,
                Some(header_refusal(&error)),
                view,
            ),
        };

        // T03 bibliography band.
        families.extend(match build_native_bibliography_input(&carrier, &lexicon) {
            Ok(outcome) => {
                if outcome.citation_names_pooled_with_biblstruct {
                    return Err(format!(
                        "{} pooled citation_names with biblstruct (R128)",
                        document.id
                    )
                    .into());
                }
                vec![
                    bibliography_observed_cell(
                        &outcome,
                        BibliographyAtom::CitationFields,
                        NativeFamilyId::BibliographyCitationFields,
                        Some(view.biblstruct_count as u64),
                        false,
                    )?,
                    bibliography_observed_cell(
                        &outcome,
                        BibliographyAtom::ReferenceSegments,
                        NativeFamilyId::BibliographyReferenceSegments,
                        Some(view.biblstruct_count as u64),
                        false,
                    )?,
                    bibliography_observed_cell(
                        &outcome,
                        BibliographyAtom::CitationAnchors,
                        NativeFamilyId::BibliographyCitationAnchors,
                        Some(view.bibr_ref_count as u64),
                        true,
                    )?,
                ]
            }
            Err(NativeBibliographyInputError::NoAdmittedTokens { .. }) => {
                bibliography_absent_cells(FamilyDisposition::TypedAbsence, None, view)
            }
            Err(error) => bibliography_absent_cells(
                FamilyDisposition::Refused,
                Some(bibliography_refusal(&error)),
                view,
            ),
        });

        let regions = vec![
            coverage_row(
                CoverageRegion::Header,
                &oracle_header_text(view),
                &region_text(&carrier, FullSemanticRegion::Header),
            ),
            coverage_row(
                CoverageRegion::References,
                &oracle_bibliography_text(view),
                &region_text(&carrier, FullSemanticRegion::References),
            ),
        ];
        for row in &regions {
            coverage_notes.push(format!(
                "{} {} verdict={} oracle_chars={} native_chars={} missing={}",
                document.id,
                row.region.as_str(),
                row.verdict.as_str(),
                row.oracle_char_count,
                row.native_char_count,
                row.missing_char_count
            ));
        }

        println!(
            "{}: spans={} words={} whitespace_words=0 header_words={} references_words={} families={}",
            document.id,
            projection.parents.len(),
            projection.identity.word_count,
            carrier
                .run(FullSemanticRegion::Header)
                .map(|run| run.tokens.len())
                .unwrap_or(0),
            carrier
                .run(FullSemanticRegion::References)
                .map(|run| run.tokens.len())
                .unwrap_or(0),
            families
                .iter()
                .map(|cell| format!("{}={}", cell.family.as_str(), cell.disposition.as_str()))
                .collect::<Vec<_>>()
                .join(",")
        );

        observed_documents.push(ObservedDocument {
            id: document.id.clone(),
            sha256: digest,
            pages: document.pages,
            observed_pages: observation.document.pages.len() as u64,
            families,
            regions,
            pipeline_digest,
            oracle_digest: sha256_prefixed(oracle_digest_text(view).as_bytes()),
        });
        oracle_documents.push(OracleDocument {
            document_id: document.id.clone(),
            fixture_path: fixture.path.clone(),
            sha256: fixture.sha256.clone(),
            oracle_digest: sha256_prefixed(oracle_digest_text(view).as_bytes()),
        });
    }

    let corpus = DeclaredCorpus {
        manifest: DECLARATION_MANIFEST_REL.to_owned(),
        pointer: "corpus.documents".to_owned(),
        revision_sha256: sha256_prefixed(&layout_bytes),
        corpus_id: declaration.corpus_id.clone(),
        document_count: declaration.corpus_n as u64,
        page_denominator: declaration.page_denominator,
        documents: declaration
            .documents
            .iter()
            .map(|document| DeclaredDocument {
                id: document.id.clone(),
                source_pdf: document.source_pdf.clone(),
                sha256: document.sha256.clone(),
                pages: document.pages,
            })
            .collect(),
    };

    let oracle_input = OracleInput {
        manifest: ORACLE_MANIFEST_REL.to_owned(),
        revision_sha256: sha256_prefixed(&provenance_bytes),
        independent,
        same_generator_refused,
        documents: oracle_documents,
    };

    let receipt = build_parity_receipt(
        &sha256_prefixed(&layout_bytes),
        &corpus,
        &declaration.locks,
        &declaration.claim_scope,
        &declaration.product_coverage_effect,
        guard,
        &oracle_input,
        &observed_documents,
    )?;
    let artifact = receipt.artifact_json_bytes()?;
    assert_redacted(std::str::from_utf8(&artifact)?)?;

    if let Some(parent) = args
        .report
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.report, &artifact)?;

    let population = receipt
        .populations
        .iter()
        .find(|population| population.population_id == PopulationId::ThreeDocumentSlice)
        .ok_or("the parity population is absent from the receipt")?;
    let header_granularity = population
        .matrix
        .iter()
        .flat_map(|document| document.regions.iter())
        .filter(|row| {
            row.region == CoverageRegion::Header && row.verdict == CoverageVerdict::GranularityOnly
        })
        .count();

    Ok(format!(
        "native family parity: {} documents, {} matrix rows, guard installed={} identity_isolated={} java_calls={}, oracle independent={} same_generator_refused={}, thresholds={}, product_coverage_effect={}, header_granularity_only={}; {}",
        population.document_count,
        population.matrix.len(),
        receipt.guard.installed,
        receipt.guard.identity_isolated,
        receipt.guard.observed_java_calls,
        receipt.oracle.independent,
        receipt.oracle.same_generator_refused,
        receipt.thresholds,
        receipt.product_coverage_effect,
        header_granularity,
        coverage_notes.join(" | ")
    ))
}
