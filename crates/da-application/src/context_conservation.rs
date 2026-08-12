//! Pure ACAP diagnostic observers. They never mutate graph truth.

use std::collections::{BTreeMap, BTreeSet};

use da_domain::{
    assurance::{BoundaryMode, MissingnessReason},
    context_conservation::StageContextSnapshot,
    extractive_summary::ExtractiveSummaryBatch,
    revision_readiness::{RevisionReadinessAssessment, RevisionReadinessObservation},
};
use da_ports::{extractor::ExtractedEntity, parser::ParsedArticle};

use crate::revision_materialization::{RevisionContentContext, RevisionMaterializationResult};

pub fn observe_parsed_article(
    parsed: &ParsedArticle,
    source_bytes: u64,
    navigation_noise_titles: &BTreeSet<String>,
) -> Result<StageContextSnapshot, String> {
    let mut missingness = BTreeMap::new();
    if parsed.abstract_text.trim().is_empty() {
        missingness.insert(
            "abstract_missing".into(),
            MissingnessReason::GenuinelyAbsent,
        );
    }
    if parsed.sections.is_empty() {
        missingness.insert(
            "sections_missing".into(),
            MissingnessReason::GenuinelyAbsent,
        );
    }
    if parsed.citations.is_empty() {
        missingness.insert(
            "citations_missing".into(),
            MissingnessReason::GenuinelyAbsent,
        );
    }
    if parsed.tei_xml.is_none() {
        missingness.insert(
            "figures_unobserved".into(),
            MissingnessReason::NotMaterialized,
        );
        missingness.insert(
            "tables_unobserved".into(),
            MissingnessReason::NotMaterialized,
        );
    }
    if parsed
        .sections
        .iter()
        .any(|section| section.path_id.is_none() || section.section_id.is_none())
    {
        missingness.insert(
            "navigation_identity_missing".into(),
            MissingnessReason::NotMaterialized,
        );
    }
    let tei = parsed
        .tei_xml
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let value = StageContextSnapshot {
        stage_id: "parse".into(),
        mode: BoundaryMode::LosslessProjection,
        artifact_sha256: StageContextSnapshot::digest_json(parsed)?,
        source_bytes,
        text_chars: text_chars(parsed),
        token_count: None,
        tokenizer_lineage: None,
        sections: parsed.sections.len() as u64,
        citations: parsed.citations.len() as u64,
        figures: tei.matches("<figure").count() as u64,
        tables: tei.matches("type=\"table\"").count() as u64,
        grounded_spans: 0,
        grounded_section_vids: 0,
        entities: 0,
        evidence_items: 0,
        summaries: 0,
        manifestations: 0,
        duplicate_section_titles: duplicate_titles(parsed),
        navigation_noise_sections: parsed
            .sections
            .iter()
            .filter(|s| navigation_noise_titles.contains(&normalize(&s.title)))
            .count() as u64,
        missingness,
        no_claim: None,
        import_eligible: false,
    };
    value.validate()?;
    Ok(value)
}

pub fn observe_revision_materialization(
    parsed: &ParsedArticle,
    context: &RevisionContentContext,
    result: &RevisionMaterializationResult,
) -> Result<StageContextSnapshot, String> {
    let mut missingness = BTreeMap::new();
    if result.sections_created + result.nodes_reused < parsed.sections.len() {
        missingness.insert(
            "section_materialization_unobserved".into(),
            MissingnessReason::NotMaterialized,
        );
    }
    if result.manifestations_created + result.nodes_reused < context.manifestations.len() {
        missingness.insert(
            "manifestation_materialization_unobserved".into(),
            MissingnessReason::NotMaterialized,
        );
    }
    let value = StageContextSnapshot {
        stage_id: "revision_materialization".into(),
        mode: BoundaryMode::FullFidelity,
        artifact_sha256: StageContextSnapshot::digest_json(result)?,
        source_bytes: 0,
        text_chars: text_chars(parsed),
        token_count: None,
        tokenizer_lineage: None,
        sections: parsed.sections.len() as u64,
        citations: parsed.citations.len() as u64,
        figures: 0,
        tables: 0,
        grounded_spans: 0,
        grounded_section_vids: parsed.sections.len() as u64,
        entities: 0,
        evidence_items: 0,
        summaries: 0,
        manifestations: context.manifestations.len() as u64,
        duplicate_section_titles: duplicate_titles(parsed),
        navigation_noise_sections: 0,
        missingness,
        no_claim: None,
        import_eligible: result.import_eligible,
    };
    value.validate()?;
    Ok(value)
}

pub fn observe_extracted_entities(
    parsed: &ParsedArticle,
    entities: &[ExtractedEntity],
) -> Result<StageContextSnapshot, String> {
    let mut missingness = BTreeMap::new();
    let mut grounded = 0;
    let mut ordinals = BTreeSet::new();
    for entity in entities {
        match entity.section_ordinal {
            Some(i)
                if i < parsed.sections.len()
                    && entity.char_start < entity.char_end
                    && entity.char_end <= parsed.sections[i].text.len() =>
            {
                grounded += 1;
                ordinals.insert(i);
            }
            Some(_) => {
                missingness.insert(
                    "invalid_entity_grounding".into(),
                    MissingnessReason::TerminalFailure,
                );
            }
            None => {
                missingness.insert(
                    "entity_section_ordinal_missing".into(),
                    MissingnessReason::NotMaterialized,
                );
            }
        }
    }
    let value = StageContextSnapshot {
        stage_id: "extraction_compute".into(),
        mode: BoundaryMode::ReviewCandidate,
        artifact_sha256: StageContextSnapshot::digest_json(&entities)?,
        source_bytes: 0,
        text_chars: parsed.body_text.chars().count() as u64,
        token_count: None,
        tokenizer_lineage: None,
        sections: parsed.sections.len() as u64,
        citations: parsed.citations.len() as u64,
        figures: 0,
        tables: 0,
        grounded_spans: grounded,
        grounded_section_vids: ordinals.len() as u64,
        entities: entities.len() as u64,
        evidence_items: entities.len() as u64,
        summaries: 0,
        manifestations: 0,
        duplicate_section_titles: duplicate_titles(parsed),
        navigation_noise_sections: 0,
        missingness,
        no_claim: None,
        import_eligible: false,
    };
    value.validate()?;
    Ok(value)
}

pub fn observe_summary_readiness(
    batch: &ExtractiveSummaryBatch,
    observation: &RevisionReadinessObservation,
    assessment: &RevisionReadinessAssessment,
) -> Result<StageContextSnapshot, String> {
    let mut missingness = BTreeMap::new();
    for code in assessment
        .blocker_codes
        .iter()
        .chain(&assessment.diagnostic_codes)
    {
        missingness.insert(code.clone(), MissingnessReason::PolicySkipped);
    }
    let summaries =
        batch.section_summaries.len() as u64 + u64::from(batch.abstract_summary.is_some());
    let value = StageContextSnapshot {
        stage_id: "readiness".into(),
        mode: BoundaryMode::NonAuthoritativeDiagnostic,
        artifact_sha256: StageContextSnapshot::digest_json(&(batch, observation, assessment))?,
        source_bytes: 0,
        text_chars: batch
            .section_summaries
            .iter()
            .map(|x| x.char_count as u64)
            .sum::<u64>()
            + batch
                .abstract_summary
                .as_ref()
                .map_or(0, |x| x.char_count as u64),
        token_count: None,
        tokenizer_lineage: None,
        sections: observation.section_count as u64,
        citations: 0,
        figures: 0,
        tables: 0,
        grounded_spans: 0,
        grounded_section_vids: observation
            .grounded_summary_count
            .min(observation.section_count) as u64,
        entities: 0,
        evidence_items: summaries,
        summaries,
        manifestations: observation.manifestation_count as u64,
        duplicate_section_titles: 0,
        navigation_noise_sections: 0,
        missingness,
        no_claim: None,
        import_eligible: false,
    };
    value.validate()?;
    Ok(value)
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}
fn text_chars(parsed: &ParsedArticle) -> u64 {
    parsed.title.chars().count() as u64
        + parsed.abstract_text.chars().count() as u64
        + parsed.body_text.chars().count() as u64
}
fn duplicate_titles(parsed: &ParsedArticle) -> u64 {
    let mut counts = BTreeMap::new();
    for section in &parsed.sections {
        *counts.entry(normalize(&section.title)).or_insert(0_u64) += 1;
    }
    counts.values().map(|x| x.saturating_sub(1)).sum()
}
