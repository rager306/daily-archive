//! Figure candidate assembly shim (M409 S01).
//!
//! Closes the M400 production gap: the multimodal test built
//! `FigureCandidate`s by hand, but no production code converted
//! `FigureSemanticContext` + `FigureAnalysis` into a candidate. This module
//! provides the deterministic conversion plus the caption-driven section
//! locator the per-image journey driver needs.
//!
//! Grounding policy (deterministic, fail-visible): a figure's caption is
//! located in exactly one parsed section by an exact case-insensitive
//! substring match inside the section text (LaTeX embeds figures in source
//! order, so the caption text occurs in the containing section's text
//! window — the same discipline GROBID uses for TEI table section
//! assignment). Zero or multiple matches are typed errors with the match
//! count; a candidate is never grounded on a guess.
//!
//! Candidate identity: the VID is recomputed from the revision-bound
//! lineage (`figure_candidate_vid`) and must equal the assembled value;
//! `candidate_metadata_sha256` is the SHA-256 of the canonical JSON with the
//! metadata and diagnostics fields blanked, so replay is byte-deterministic.
//! Authority and import locks are hard-false (D127).

use std::sync::Arc;

use da_domain::figure_analysis::FigureAnalysis;
use da_domain::figure_candidate::{FigureCandidate, figure_candidate_vid};
use da_domain::figure_context::FigureSemanticContext;
use da_ports::parser::ParsedArticle;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FigureGroundingError {
    #[error("no section contains the figure caption {caption:?}")]
    CaptionNotFound { caption: String },
    #[error("figure caption {caption:?} matches {count} sections; grounding is ambiguous")]
    AmbiguousCaption { caption: String, count: usize },
}

/// Locate the containing section for a figure caption by exact
/// case-insensitive substring match over section text.
///
/// Returns the section's `section_id`, `path_id` and 0-based ordinal. A
/// candidate is only grounded when the caption occurs in exactly one
/// section.
pub fn locate_figure_section(
    caption: &str,
    parsed: &ParsedArticle,
) -> Result<(String, String, usize), FigureGroundingError> {
    let needle = caption.trim().to_lowercase();
    if needle.is_empty() {
        return Err(FigureGroundingError::CaptionNotFound {
            caption: caption.to_string(),
        });
    }
    let matches: Vec<&da_ports::parser::Section> = parsed
        .sections
        .iter()
        .filter(|section| section.text.to_lowercase().contains(&needle))
        .collect();
    match matches.as_slice() {
        [section] => {
            let section_id = section.section_id.clone().ok_or_else(|| {
                FigureGroundingError::CaptionNotFound {
                    caption: caption.to_string(),
                }
            })?;
            let path_id =
                section
                    .path_id
                    .clone()
                    .ok_or_else(|| FigureGroundingError::CaptionNotFound {
                        caption: caption.to_string(),
                    })?;
            let ordinal = parsed
                .sections
                .iter()
                .position(|candidate| std::ptr::eq(candidate, *section))
                .expect("section came from parsed.sections");
            Ok((section_id, path_id, ordinal))
        }
        [] => Err(FigureGroundingError::CaptionNotFound {
            caption: caption.to_string(),
        }),
        many => Err(FigureGroundingError::AmbiguousCaption {
            caption: caption.to_string(),
            count: many.len(),
        }),
    }
}

/// Assemble a candidate-only `FigureCandidate` from the grounded context and
/// the typed analysis payload.
#[allow(clippy::too_many_arguments)]
pub fn figure_candidate_from_analysis(
    context: &FigureSemanticContext,
    analysis: &FigureAnalysis,
    detector_name: &str,
    source_pdf_sha256: &str,
    revision_vid: &str,
    section_vid: &str,
    section_title: &str,
    section_ordinal: u32,
) -> Result<FigureCandidate, String> {
    if analysis.artifact_id != context.artifact_id {
        return Err(format!(
            "analysis artifact '{}' does not match context '{}'",
            analysis.artifact_id, context.artifact_id
        ));
    }
    let vid = figure_candidate_vid(
        revision_vid,
        &context.artifact_id,
        &context.detector_fingerprint,
        &context.asset_sha256,
        section_vid,
    )
    .map_err(|error| format!("figure candidate vid: {error}"))?;
    let mut candidate = FigureCandidate {
        vid,
        revision_vid: revision_vid.to_string(),
        artifact_id: context.artifact_id.clone(),
        kind: context.kind,
        detector_name: detector_name.to_string(),
        detector_fingerprint: context.detector_fingerprint.clone(),
        detector_metadata_sha256: context.detector_metadata_sha256.clone(),
        source_pdf_sha256: source_pdf_sha256.to_string(),
        image_sha256: context.asset_sha256.clone(),
        asset_path: context.asset_path.clone(),
        page_index: context.page_index,
        section_vid: section_vid.to_string(),
        section_title: section_title.to_string(),
        section_ordinal,
        caption: context.caption.clone(),
        candidate_metadata_sha256: String::new(),
        authoritative: false,
        import_eligible: false,
        schema_version: 1,
        materialization_diagnostics: None,
    };
    candidate.candidate_metadata_sha256 = candidate_metadata_digest(&candidate);
    Ok(candidate)
}

/// Deterministic digest of the candidate record with the metadata and
/// replay-diagnostics fields blanked (identity-independent of its own
/// digest and of runtime state).
pub fn candidate_metadata_digest(candidate: &FigureCandidate) -> String {
    let mut clone = candidate.clone();
    clone.candidate_metadata_sha256.clear();
    clone.materialization_diagnostics = None;
    let bytes = serde_json::to_vec(&clone).expect("candidate serializes");
    hex::encode(Sha256::digest(&bytes))
}

/// Convenience wrapper for the driver: everything except the parsed article
/// is carried by the context; this resolves grounding and builds the
/// candidate or a typed error.
pub fn assemble_from_context(
    context: &FigureSemanticContext,
    analysis: &FigureAnalysis,
    detector_name: &str,
    parsed: &ParsedArticle,
    revision_vid: &str,
    source_pdf_sha256: &str,
) -> Result<FigureCandidate, String> {
    let (_section_id, _path_id, ordinal) = locate_figure_section(&context.caption, parsed)
        .map_err(|error| format!("figure grounding: {error}"))?;
    let section_vid = da_domain::vid::revision_section_vid(revision_vid, ordinal)
        .map_err(|error| format!("revision section vid: {error}"))?
        .to_string();
    let section_title = parsed
        .sections
        .get(ordinal)
        .map(|section| section.title.clone())
        .unwrap_or_default();
    figure_candidate_from_analysis(
        context,
        analysis,
        detector_name,
        source_pdf_sha256,
        revision_vid,
        &section_vid,
        &section_title,
        ordinal as u32,
    )
}

/// Store alias kept for call sites that already hold the graph store; the
/// shim itself performs no I/O.
#[allow(dead_code)]
pub type AssemblyStore = Arc<dyn da_ports::graph_store::DirectGraphStore>;

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::figure::BoundingBox72Dpi;
    use da_domain::figure_analysis::{FigureAnalysisProvenance, VisualAnalysisType};
    use da_domain::figure_context::FigureSectionContext;
    use da_ports::parser::Section;

    fn sha(seed: u64) -> String {
        format!("{seed:064x}")
    }

    fn section(ordinal: usize, text: &str) -> Section {
        Section {
            title: format!("Section {ordinal}"),
            text: text.into(),
            level: 1,
            path_id: Some(format!("{ordinal}")),
            section_id: Some(format!("sec-{ordinal}")),
            ..Default::default()
        }
    }

    fn parsed() -> ParsedArticle {
        ParsedArticle {
            paper_id: "p1".into(),
            title: "T".into(),
            abstract_text: "a".into(),
            body_text: "b".into(),
            sections: vec![
                section(0, "intro text"),
                section(
                    1,
                    "results with Figure 3: Overview of the GEPA system. inside",
                ),
            ],
            citations: vec![],
            layout_json: None,
            tei_xml: None,
            tables: Vec::new(),
            table_missingness: None,
            formulas: Vec::new(),
            formula_missingness: None,
            pdf_hash: sha(1),
        }
    }

    fn context() -> FigureSemanticContext {
        FigureSemanticContext {
            artifact_id: "Figure3".into(),
            kind: da_domain::VisualArtifactKind::Figure,
            asset_path: "figures/Figure3.png".into(),
            asset_sha256: sha(9),
            source_pdf_sha256: sha(1),
            revision_vid: Some(sha(2)),
            canonical_id: Some("p1".into()),
            section_vid: Some(sha(3)),
            detector_fingerprint: "pdffigures2-0.0.12".into(),
            detector_metadata_sha256: sha(4),
            page_index: 3,
            region: BoundingBox72Dpi {
                x1: 10.0,
                y1: 20.0,
                x2: 500.0,
                y2: 300.0,
            },
            caption: "Figure 3: Overview of the GEPA system.".into(),
            containing_section: FigureSectionContext {
                section_id: "sec-1".into(),
                path_id: "1".into(),
                title: "Section 1".into(),
                text: "results with Figure 3: Overview of the GEPA system. inside".into(),
                truncated: false,
                section_ordinal: Some(1),
                relation: da_domain::figure_context::FigureSectionRelation::Containing,
            },
            neighboring_sections: Vec::new(),
            reference_excerpts: Vec::new(),
            article_summary: None,
            diagnostics: Vec::new(),
            total_context_chars: 0,
            truncated: false,
        }
    }

    fn analysis(artifact_id: &str) -> FigureAnalysis {
        FigureAnalysis {
            candidate_id: "cand".into(),
            artifact_id: artifact_id.into(),
            visual_type: VisualAnalysisType::Plot,
            description: "desc".into(),
            components: vec![],
            relationships: vec![],
            findings: vec![],
            citations: vec![],
            confidence: 0.9,
            provenance: FigureAnalysisProvenance {
                image_sha256: sha(9),
                context_sha256: sha(3),
                route_id: "r".into(),
                protocol: "openai_chat_completions".into(),
                requested_model: "m".into(),
                resolved_model: "m".into(),
                prompt_sha256: sha(5),
                output_schema_sha256: sha(6),
                schema_id: "chart".into(),
                stage1_visual_type: "chart".into(),
                prompt_task_sha256: sha(7),
                route_policy_sha256: sha(8),
                model_binding_sha256: sha(9),
                input_sha256: sha(10),
                output_sha256: sha(11),
                input_tokens: None,
                output_tokens: None,
                latency_ms: 1,
            },
            typed_payload: None,
            authoritative: false,
            import_eligible: false,
        }
    }

    #[test]
    fn exact_caption_match_locates_one_section() {
        let parsed = parsed();
        let (section_id, path_id, ordinal) =
            locate_figure_section("Figure 3: Overview of the GEPA system.", &parsed)
                .expect("located");
        assert_eq!(section_id, "sec-1");
        assert_eq!(path_id, "1");
        assert_eq!(ordinal, 1);
    }

    #[test]
    fn grobid_path_section_id_is_returned_not_sec_ordinal() {
        let mut parsed = parsed();
        parsed.sections.push(Section {
            title: "Discussion".into(),
            text: "see Figure 7: Temporal FRBR model. in this section".into(),
            level: 1,
            path_id: Some("4".into()),
            section_id: Some("sec-4".into()),
            ..Default::default()
        });
        let (section_id, path_id, ordinal) =
            locate_figure_section("Figure 7: Temporal FRBR model.", &parsed).expect("located");
        assert_eq!(section_id, "sec-4");
        assert_eq!(path_id, "4");
        assert_eq!(ordinal, 2);
        assert_ne!(section_id, format!("sec-{ordinal}"));
    }

    #[test]
    fn missing_caption_is_a_typed_error() {
        let err = locate_figure_section("Figure 99: missing", &parsed()).unwrap_err();
        assert_eq!(
            err,
            FigureGroundingError::CaptionNotFound {
                caption: "Figure 99: missing".into()
            }
        );
    }

    #[test]
    fn repeated_caption_is_ambiguous() {
        let mut parsed = parsed();
        parsed.sections.push(section(
            2,
            "results with Figure 3: Overview of the GEPA system. again",
        ));
        let err =
            locate_figure_section("Figure 3: Overview of the GEPA system.", &parsed).unwrap_err();
        assert!(matches!(
            err,
            FigureGroundingError::AmbiguousCaption { count: 2, .. }
        ));
    }

    #[test]
    fn candidate_vid_recomputes_and_locks_are_false() {
        let candidate = assemble_from_context(
            &context(),
            &analysis("Figure3"),
            "pdffigures2",
            &parsed(),
            &sha(2),
            &sha(1),
        )
        .expect("assembled");
        assert_eq!(
            candidate.recompute_vid().expect("recompute"),
            candidate.vid,
            "identity contract"
        );
        assert!(!candidate.authoritative);
        assert!(!candidate.import_eligible);
        assert_eq!(candidate.section_ordinal, 1);
        assert!(!candidate.candidate_metadata_sha256.is_empty());
    }

    #[test]
    fn metadata_digest_is_deterministic() {
        let a = assemble_from_context(
            &context(),
            &analysis("Figure3"),
            "pdffigures2",
            &parsed(),
            &sha(2),
            &sha(1),
        )
        .expect("a");
        let mut b = a.clone();
        b.candidate_metadata_sha256.clear();
        assert_eq!(
            candidate_metadata_digest(&a),
            candidate_metadata_digest(&b),
            "digest ignores the metadata field itself"
        );
        assert_eq!(candidate_metadata_digest(&a), candidate_metadata_digest(&a));
    }

    #[test]
    fn artifact_mismatch_fails_closed() {
        let err = assemble_from_context(
            &context(),
            &analysis("OtherFigure"),
            "pdffigures2",
            &parsed(),
            &sha(2),
            &sha(1),
        )
        .unwrap_err();
        assert!(err.contains("does not match"), "{err}");
    }
}
