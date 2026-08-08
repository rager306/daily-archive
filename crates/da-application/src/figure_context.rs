//! Deterministic figure semantic-context assembly (M371 S02).

use da_domain::{
    ArticleSummaryContext, DetectedVisualArtifact, FigureContextError, FigureReferenceExcerpt,
    FigureSectionContext, FigureSectionRelation, FigureSemanticContext,
};
use da_ports::parser::{ParsedArticle, Section};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FigureSectionLocator {
    SectionId(String),
    PathId(String),
}

impl FigureSectionLocator {
    fn display(&self) -> &str {
        match self {
            Self::SectionId(value) | Self::PathId(value) => value,
        }
    }

    fn matches(&self, section: &Section) -> bool {
        match self {
            Self::SectionId(value) => section.section_id.as_ref() == Some(value),
            Self::PathId(value) => section.path_id.as_ref() == Some(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FigureContextBudget {
    pub caption_chars: usize,
    pub containing_section_chars: usize,
    pub neighbor_section_chars: usize,
    pub reference_excerpt_chars: usize,
    pub max_reference_excerpts: usize,
    pub article_summary_chars: usize,
    pub max_total_chars: usize,
}

impl Default for FigureContextBudget {
    fn default() -> Self {
        Self {
            caption_chars: 2_000,
            containing_section_chars: 6_000,
            neighbor_section_chars: 1_500,
            reference_excerpt_chars: 1_200,
            max_reference_excerpts: 3,
            article_summary_chars: 3_000,
            max_total_chars: 18_000,
        }
    }
}

impl FigureContextBudget {
    fn validate(self) -> Result<Self, FigureContextError> {
        let component_limit = self
            .caption_chars
            .saturating_add(self.containing_section_chars)
            .saturating_add(self.neighbor_section_chars.saturating_mul(2))
            .saturating_add(
                self.reference_excerpt_chars
                    .saturating_mul(self.max_reference_excerpts),
            )
            .saturating_add(self.article_summary_chars);
        if component_limit > self.max_total_chars {
            return Err(FigureContextError::Invalid(format!(
                "component context budgets total {component_limit}, exceeding max_total_chars {}",
                self.max_total_chars
            )));
        }
        Ok(self)
    }
}

pub struct FigureContextAssembler {
    budget: FigureContextBudget,
}

impl FigureContextAssembler {
    pub fn new(budget: FigureContextBudget) -> Result<Self, FigureContextError> {
        Ok(Self {
            budget: budget.validate()?,
        })
    }

    pub fn with_default_budget() -> Self {
        Self {
            budget: FigureContextBudget::default(),
        }
    }

    pub fn assemble(
        &self,
        artifact: &DetectedVisualArtifact,
        article: &ParsedArticle,
        locator: Option<&FigureSectionLocator>,
        article_summary: Option<&ArticleSummaryContext>,
    ) -> Result<FigureSemanticContext, FigureContextError> {
        artifact
            .validate()
            .map_err(|error| FigureContextError::Invalid(error.to_string()))?;
        let locator = locator.ok_or(FigureContextError::MissingSectionLocator)?;
        let matches: Vec<_> = article
            .sections
            .iter()
            .enumerate()
            .filter(|(_, section)| locator.matches(section))
            .map(|(index, _)| index)
            .collect();
        let containing_index = match matches.as_slice() {
            [] => {
                return Err(FigureContextError::SectionNotFound(
                    locator.display().to_owned(),
                ));
            }
            [index] => *index,
            _ => {
                return Err(FigureContextError::AmbiguousSectionLocator {
                    locator: locator.display().to_owned(),
                    matches: matches.len(),
                });
            }
        };
        let containing = &article.sections[containing_index];
        require_provenance(containing, locator.display())?;

        let mut diagnostics = Vec::new();
        let (caption, caption_truncated) =
            truncate_chars(&artifact.caption, self.budget.caption_chars);
        if caption_truncated {
            diagnostics.push("caption_truncated".into());
        }
        let containing_section = section_context(
            containing,
            FigureSectionRelation::Containing,
            self.budget.containing_section_chars,
        )?;
        if containing_section.truncated {
            diagnostics.push("containing_section_truncated".into());
        }

        let mut neighboring_sections = Vec::new();
        for (index, relation) in [
            containing_index
                .checked_sub(1)
                .map(|index| (index, FigureSectionRelation::Previous)),
            (containing_index + 1 < article.sections.len())
                .then_some((containing_index + 1, FigureSectionRelation::Next)),
        ]
        .into_iter()
        .flatten()
        {
            match section_context(
                &article.sections[index],
                relation,
                self.budget.neighbor_section_chars,
            ) {
                Ok(section) => {
                    if section.truncated {
                        diagnostics.push(format!("neighbor_section_truncated:{}", section.path_id));
                    }
                    neighboring_sections.push(section);
                }
                Err(FigureContextError::MissingSectionProvenance(value)) => diagnostics.push(
                    format!("neighbor_section_skipped_missing_provenance:{value}"),
                ),
                Err(error) => return Err(error),
            }
        }

        let reference_excerpts = find_references(
            artifact,
            article,
            self.budget.reference_excerpt_chars,
            self.budget.max_reference_excerpts,
            &mut diagnostics,
        );
        if reference_excerpts.is_empty() {
            diagnostics.push("no_in_text_reference_found".into());
        }
        let article_summary = article_summary.map(|summary| {
            let (text, truncated) =
                truncate_chars(&summary.text, self.budget.article_summary_chars);
            if truncated {
                diagnostics.push("article_summary_truncated".into());
            }
            ArticleSummaryContext {
                source_id: summary.source_id.clone(),
                source_text_sha256: summary.source_text_sha256.clone(),
                text,
                truncated,
            }
        });

        let total_context_chars = caption.chars().count()
            + containing_section.text.chars().count()
            + neighboring_sections
                .iter()
                .map(|section| section.text.chars().count())
                .sum::<usize>()
            + reference_excerpts
                .iter()
                .map(|reference| reference.excerpt.chars().count())
                .sum::<usize>()
            + article_summary
                .as_ref()
                .map(|summary| summary.text.chars().count())
                .unwrap_or_default();
        let truncated = caption_truncated
            || containing_section.truncated
            || neighboring_sections.iter().any(|section| section.truncated)
            || reference_excerpts
                .iter()
                .any(|reference| reference.truncated)
            || diagnostics
                .iter()
                .any(|diagnostic| diagnostic == "article_summary_truncated");

        let context = FigureSemanticContext {
            artifact_id: artifact.artifact_id.clone(),
            kind: artifact.kind,
            asset_path: artifact.asset_path.clone(),
            asset_sha256: artifact.asset_sha256.clone(),
            source_pdf_sha256: artifact.source_pdf_sha256.clone(),
            detector_fingerprint: artifact.detector_fingerprint.clone(),
            detector_metadata_sha256: artifact.metadata_sha256.clone(),
            page_index: artifact.page_index,
            region: artifact.region,
            caption,
            containing_section,
            neighboring_sections,
            reference_excerpts,
            article_summary,
            diagnostics,
            total_context_chars,
            truncated,
        };
        context.validate()?;
        Ok(context)
    }
}

fn require_provenance(section: &Section, locator: &str) -> Result<(), FigureContextError> {
    if section.section_id.as_deref().is_none_or(str::is_empty)
        || section.path_id.as_deref().is_none_or(str::is_empty)
    {
        return Err(FigureContextError::MissingSectionProvenance(
            locator.to_owned(),
        ));
    }
    Ok(())
}

fn section_context(
    section: &Section,
    relation: FigureSectionRelation,
    max_chars: usize,
) -> Result<FigureSectionContext, FigureContextError> {
    let locator = section
        .section_id
        .as_deref()
        .or(section.path_id.as_deref())
        .unwrap_or("unknown");
    require_provenance(section, locator)?;
    let (text, truncated) = truncate_chars(&section.text, max_chars);
    Ok(FigureSectionContext {
        relation,
        section_id: section.section_id.clone().expect("validated section_id"),
        path_id: section.path_id.clone().expect("validated path_id"),
        title: section.title.clone(),
        text,
        truncated,
    })
}

fn find_references(
    artifact: &DetectedVisualArtifact,
    article: &ParsedArticle,
    excerpt_chars: usize,
    max_excerpts: usize,
    diagnostics: &mut Vec<String>,
) -> Vec<FigureReferenceExcerpt> {
    let forms = reference_forms(&artifact.artifact_id);
    let mut excerpts = Vec::new();
    for section in &article.sections {
        if excerpts.len() >= max_excerpts {
            diagnostics.push("reference_excerpt_limit_reached".into());
            break;
        }
        let Some(section_id) = section
            .section_id
            .as_ref()
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(path_id) = section.path_id.as_ref().filter(|value| !value.is_empty()) else {
            continue;
        };
        let Some((matched_form, byte_index)) = forms.iter().find_map(|form| {
            find_ascii_case_insensitive(&section.text, form).map(|index| (form.clone(), index))
        }) else {
            continue;
        };
        let (excerpt, truncated) = excerpt_around(&section.text, byte_index, excerpt_chars);
        excerpts.push(FigureReferenceExcerpt {
            section_id: section_id.clone(),
            path_id: path_id.clone(),
            title: section.title.clone(),
            matched_form,
            excerpt,
            truncated,
        });
    }
    excerpts
}

fn find_ascii_case_insensitive(value: &str, needle: &str) -> Option<usize> {
    if !needle.is_ascii() {
        return value.find(needle);
    }
    value
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn reference_forms(artifact_id: &str) -> Vec<String> {
    let number = artifact_id
        .trim_start_matches("Figure")
        .trim_start_matches("Table")
        .split('-')
        .next()
        .unwrap_or_default();
    if number.is_empty() {
        return vec![artifact_id.to_lowercase()];
    }
    let noun = if artifact_id.starts_with("Table") {
        "table"
    } else {
        "figure"
    };
    let short = if noun == "figure" { "fig" } else { "tab" };
    vec![
        format!("{noun} {number}"),
        format!("{short}. {number}"),
        format!("{short} {number}"),
    ]
}

fn truncate_chars(value: &str, max_chars: usize) -> (String, bool) {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    let had_more = chars.next().is_some();
    (truncated, had_more)
}

fn excerpt_around(value: &str, match_byte_index: usize, max_chars: usize) -> (String, bool) {
    let match_char_index = value[..match_byte_index].chars().count();
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return (value.to_owned(), false);
    }
    let half = max_chars / 2;
    let start = match_char_index.saturating_sub(half);
    let end = (start + max_chars).min(chars.len());
    let start = end.saturating_sub(max_chars);
    (chars[start..end].iter().collect(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::{BoundingBox72Dpi, VisualArtifactKind};
    use da_ports::parser::CitationEntry;

    fn artifact() -> DetectedVisualArtifact {
        DetectedVisualArtifact {
            artifact_id: "Figure3".into(),
            kind: VisualArtifactKind::Figure,
            detector_name: "pdffigures2".into(),
            detector_fingerprint: "pdffigures2-0.0.12".into(),
            source_pdf_sha256: "a".repeat(64),
            page_index: 4,
            region: BoundingBox72Dpi {
                x1: 10.0,
                y1: 20.0,
                x2: 100.0,
                y2: 120.0,
            },
            caption_region: BoundingBox72Dpi {
                x1: 10.0,
                y1: 125.0,
                x2: 100.0,
                y2: 140.0,
            },
            caption: "Figure 3: GEPA proposes reflective prompt mutation.".into(),
            image_text: vec!["GEPA".into()],
            render_dpi: 150,
            asset_path: "figures/Figure3.png".into(),
            asset_sha256: "b".repeat(64),
            metadata_sha256: "c".repeat(64),
        }
    }

    fn section(path: &str, title: &str, text: &str) -> Section {
        Section {
            title: title.into(),
            text: text.into(),
            level: 1,
            path_id: Some(path.into()),
            parent_section_id: None,
            order_in_parent: Some(0),
            section_id: Some(format!("section-{path}")),
        }
    }

    fn article() -> ParsedArticle {
        ParsedArticle {
            paper_id: "2507.19457".into(),
            title: "GEPA".into(),
            abstract_text: "Reflective prompt evolution.".into(),
            body_text: String::new(),
            sections: vec![
                section(
                    "2",
                    "Background",
                    "Earlier prompt optimizers use scalar rewards.",
                ),
                section(
                    "3",
                    "GEPA System",
                    "As shown in Figure 3, GEPA mutates prompts and uses Pareto-based sampling.",
                ),
                section(
                    "3.1",
                    "Candidate Sampling",
                    "Fig. 3 illustrates candidate selection.",
                ),
            ],
            citations: Vec::<CitationEntry>::new(),
            layout_json: None,
            tei_xml: None,
            pdf_hash: "a".repeat(64),
        }
    }

    #[test]
    fn explicit_section_resolution_assembles_grounded_context() {
        let context = FigureContextAssembler::with_default_budget()
            .assemble(
                &artifact(),
                &article(),
                Some(&FigureSectionLocator::PathId("3".into())),
                Some(&ArticleSummaryContext {
                    source_id: "summary-node-1".into(),
                    source_text_sha256: "d".repeat(64),
                    text: "GEPA improves 6% with 35x fewer rollouts.".into(),
                    truncated: false,
                }),
            )
            .unwrap();
        assert_eq!(context.containing_section.path_id, "3");
        assert_eq!(context.neighboring_sections.len(), 2);
        assert_eq!(context.reference_excerpts.len(), 2);
        assert!(context.reference_excerpts[0].excerpt.contains("Figure 3"));
        let summary = context.article_summary.unwrap();
        assert!(summary.text.contains("35x"));
        assert_eq!(summary.source_id, "summary-node-1");
        assert_eq!(context.asset_sha256, "b".repeat(64));
    }

    #[test]
    fn missing_locator_or_unknown_section_never_selects_first_section() {
        let assembler = FigureContextAssembler::with_default_budget();
        assert_eq!(
            assembler.assemble(&artifact(), &article(), None, None),
            Err(FigureContextError::MissingSectionLocator)
        );
        assert_eq!(
            assembler.assemble(
                &artifact(),
                &article(),
                Some(&FigureSectionLocator::PathId("missing".into())),
                None,
            ),
            Err(FigureContextError::SectionNotFound("missing".into()))
        );
    }

    #[test]
    fn unicode_truncation_is_deterministic_and_preserves_reference() {
        let mut article = article();
        article.sections[1].text = format!(
            "{} Figure 3 {}",
            "модель ".repeat(100),
            "результат ".repeat(100)
        );
        let budget = FigureContextBudget {
            caption_chars: 100,
            containing_section_chars: 120,
            neighbor_section_chars: 20,
            reference_excerpt_chars: 80,
            max_reference_excerpts: 1,
            article_summary_chars: 20,
            max_total_chars: 360,
        };
        let context = FigureContextAssembler::new(budget)
            .unwrap()
            .assemble(
                &artifact(),
                &article,
                Some(&FigureSectionLocator::SectionId("section-3".into())),
                Some(&ArticleSummaryContext {
                    source_id: "summary-node-1".into(),
                    source_text_sha256: "d".repeat(64),
                    text: "глобальное резюме статьи".into(),
                    truncated: false,
                }),
            )
            .unwrap();
        assert!(context.truncated);
        assert_eq!(context.containing_section.text.chars().count(), 120);
        assert!(context.reference_excerpts[0].excerpt.contains("Figure 3"));
        assert!(context.total_context_chars <= 360);
    }

    #[test]
    fn duplicate_section_locator_is_rejected_as_ambiguous() {
        let mut article = article();
        article.sections.push(section(
            "3",
            "Duplicate identity",
            "A different section must not share path 3.",
        ));
        let error = FigureContextAssembler::with_default_budget()
            .assemble(
                &artifact(),
                &article,
                Some(&FigureSectionLocator::PathId("3".into())),
                None,
            )
            .unwrap_err();
        assert_eq!(
            error,
            FigureContextError::AmbiguousSectionLocator {
                locator: "3".into(),
                matches: 2,
            }
        );
    }

    #[test]
    fn section_without_navigation_provenance_is_rejected() {
        let mut article = article();
        article.sections[1].section_id = None;
        let error = FigureContextAssembler::with_default_budget()
            .assemble(
                &artifact(),
                &article,
                Some(&FigureSectionLocator::PathId("3".into())),
                None,
            )
            .unwrap_err();
        assert_eq!(
            error,
            FigureContextError::MissingSectionProvenance("3".into())
        );
    }
}
