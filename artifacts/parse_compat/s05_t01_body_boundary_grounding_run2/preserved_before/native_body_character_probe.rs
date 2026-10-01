#![cfg(feature = "pdf-layout")]
//! M514-1lss2s / S05 / T01 — native body-character probe (detached adapter/CLI).
//!
//! The da-parse character oracle cannot reach the positioned donor: the
//! `PdfInspectorLayoutDonor` lives in `da-adapters`, which depends on
//! `da-parse`, so a `da-parse` test can never build the real S03 plane. This
//! probe is the detached measurement the crate boundary requires: it reads the
//! three pinned PDFs declared in `data/parse/layout_bakeoff.yaml`, builds the
//! genuine `pdf_inspector_positioned` plane, runs the S03 native layout band and
//! the repaired word/heading band, and compares each native body text against
//! an independent preserved-TEI character oracle. It records per-document and
//! aggregate normalization numbers and typed verdicts
//! (`GranularityOnly` / `ContentLoss` / `NotAssessable`) plus the D498 flag.
//!
//! Shadow/diagnostic only: no routing, promotion, graph, or import. The pinned
//! PDFs are gitignored, so this artifact is self-declared by construction
//! (`thresholds=not_accepted`, `claim_scope=diagnostic_only`).

use da_adapters::{LayoutDonorLimits, PdfInspectorLayoutDonor};
use da_parse::{
    BodyCoverageAggregate, BodyCoverageGap, BodyOracleCounts, BodyOracleIndependence,
    BodyOracleProvenance, FullSemanticRegion, HeadingRegionConfig, NativeBodyLengths,
    NativeBodyTextError, SegmentationConfig, aggregate_body_coverage, assign_heading_regions,
    build_native_body_text, build_native_word_region_carrier, classify_body_coverage,
    classify_body_coverage_from_lengths, project_native_words, segment_document_full,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;

const REPORT_SCHEMA: &str = "native-body-character-shadow.v1";
const CORPUS_ID: &str = "s13-same-input-pdf-corpus";
const LANE: &str = "pdf_inspector_positioned";
const ORACLE_GENERATOR: &str = "preserved-java-tei-lexical-scanner";
const DEFAULT_REPORT: &str = "artifacts/parse_compat/native_body_character_shadow.json";

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
    documents: Vec<ManifestDocument>,
}
#[derive(Deserialize)]
struct ManifestDocument {
    id: String,
    source_pdf: String,
    sha256: String,
}

#[derive(Serialize)]
struct Report {
    schema_version: &'static str,
    corpus_id: &'static str,
    lane: &'static str,
    oracle: OracleIdentity,
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
}
#[derive(Serialize)]
struct DocumentRow {
    document_id: String,
    source_sha256: String,
    pages: usize,
    native_lexical_tokens: usize,
    pdf_inspector_spans: usize,
    s03_parent_span: PathRow,
    word_heading: PathRow,
    oracle: NativeBodyLengths,
}
#[derive(Serialize)]
struct PathRow {
    status: String,
    absence: Option<String>,
    token_count: usize,
    gap: Option<BodyCoverageGap>,
}
#[derive(Serialize)]
struct Aggregate {
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

fn oracle_counts(tei: &str) -> BodyOracleCounts {
    let text = body_text(tei).unwrap_or_default();
    BodyOracleCounts {
        generator_id: ORACLE_GENERATOR.to_owned(),
        lengths: NativeBodyLengths::measure(&text),
    }
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_bytes = std::fs::read(root().join("data/parse/layout_bakeoff.yaml"))?;
    let manifest: Manifest = serde_yaml::from_slice(&manifest_bytes)?;
    let donor = PdfInspectorLayoutDonor;
    let limits = LayoutDonorLimits::default();

    let mut rows = Vec::new();
    let mut s03_gaps = Vec::new();
    let mut word_gaps = Vec::new();

    for document in &manifest.corpus.documents {
        let pdf_bytes = std::fs::read(root().join(&document.source_pdf))?;
        let observed = sha256_hex(&pdf_bytes);
        if observed != document.sha256 {
            return Err(format!(
                "sha256 drift for {}: {observed} != {}",
                document.id, document.sha256
            )
            .into());
        }
        let observation = donor.extract_shadow(&pdf_bytes, &document.id, limits)?;
        let oracle = match fixture_file(&document.id) {
            Some(file) => {
                let bytes = std::fs::read(
                    root()
                        .join("crates/da-parse/tests/fixtures/preserved")
                        .join(file),
                )?;
                oracle_counts(std::str::from_utf8(&bytes)?)
            }
            None => BodyOracleCounts {
                generator_id: ORACLE_GENERATOR.to_owned(),
                lengths: NativeBodyLengths {
                    raw: 0,
                    whitespace_collapsed: 0,
                    zero_width_normalized: 0,
                },
            },
        };

        // S03 parent-span band: the genuine layout-plane region walk.
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
                ) {
                    Ok(native) => {
                        let gap =
                            classify_body_coverage(&document.id, &native, &oracle, &provenance())?;
                        let row = PathRow {
                            status: "measured".to_owned(),
                            absence: None,
                            token_count: native.token_count,
                            gap: Some(gap.clone()),
                        };
                        s03_gaps.push(gap);
                        row
                    }
                    Err(error) => PathRow {
                        status: "typed_absence".to_owned(),
                        absence: Some(absence_code(&error)),
                        token_count: 0,
                        gap: None,
                    },
                }
            }
            Err(error) => PathRow {
                status: "terminal_failure".to_owned(),
                absence: Some(format!("{error:?}")),
                token_count: 0,
                gap: None,
            },
        };

        // Repaired word/heading band: heading-aware regions over word tokens.
        let word_row = match project_native_words(&observation.document, &observation.lexemes) {
            Ok(projection) => {
                match assign_heading_regions(
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
                                let joined = run
                                    .tokens
                                    .iter()
                                    .map(|token| token.text())
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                let lengths = NativeBodyLengths::measure(&joined);
                                let gap = classify_body_coverage_from_lengths(
                                    &document.id,
                                    lengths,
                                    &oracle,
                                    &provenance(),
                                )?;
                                let row = PathRow {
                                    status: "measured".to_owned(),
                                    absence: None,
                                    token_count: run.tokens.len(),
                                    gap: Some(gap.clone()),
                                };
                                word_gaps.push(gap);
                                row
                            }
                            None => PathRow {
                                status: "typed_absence".to_owned(),
                                absence: Some("NoBodyTokens".to_owned()),
                                token_count: 0,
                                gap: None,
                            },
                        },
                        Err(error) => PathRow {
                            status: "typed_absence".to_owned(),
                            absence: Some(format!("{error:?}")),
                            token_count: 0,
                            gap: None,
                        },
                    },
                    Err(error) => PathRow {
                        status: "terminal_failure".to_owned(),
                        absence: Some(format!("{error:?}")),
                        token_count: 0,
                        gap: None,
                    },
                }
            }
            Err(error) => PathRow {
                status: "terminal_failure".to_owned(),
                absence: Some(format!("{error:?}")),
                token_count: 0,
                gap: None,
            },
        };

        let native_lexical_tokens = observation
            .document
            .pages
            .iter()
            .flat_map(|page| page.tokens.iter())
            .filter(|token| token.kind.is_lexical())
            .count();
        let pdf_inspector_spans = observation.lexemes.len();
        println!(
            "native-body {id}: pages {} spans {} s03 [{} tokens {:?} {:?} d498 {:?}] word [{} tokens {:?} {:?}] oracle zero-width {}",
            observation.document.pages.len(),
            pdf_inspector_spans,
            s03_row.token_count,
            s03_row.gap.as_ref().map(|g| g.coverage_milli),
            s03_row.gap.as_ref().map(|g| g.verdict),
            s03_row.gap.as_ref().map(|g| g.d498_reopen_required),
            word_row.token_count,
            word_row.gap.as_ref().map(|g| g.coverage_milli),
            word_row.gap.as_ref().map(|g| g.verdict),
            oracle.lengths.zero_width_normalized,
            id = document.id,
        );

        rows.push(DocumentRow {
            document_id: document.id.clone(),
            source_sha256: observed,
            pages: observation.document.pages.len(),
            native_lexical_tokens,
            pdf_inspector_spans,
            s03_parent_span: s03_row,
            word_heading: word_row,
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

    let report = Report {
        schema_version: REPORT_SCHEMA,
        corpus_id: CORPUS_ID,
        lane: LANE,
        oracle: OracleIdentity {
            generator_id: ORACLE_GENERATOR,
            provenance: "crates/da-parse/tests/fixtures/preserved/provenance.yaml",
            fixtures: "crates/da-parse/tests/fixtures/preserved",
        },
        documents: rows,
        aggregate: Aggregate {
            s03_parent_span: aggregate_body_coverage(&s03_gaps),
            word_heading: aggregate_body_coverage(&word_gaps),
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
    println!("native-body report written to {}", path.display());
    Ok(())
}
