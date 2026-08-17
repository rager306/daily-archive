//! Idempotent EquationCandidate graph materializer (M400 S03 T03).
//!
//! Application use case over [`DirectGraphStore`]: materializes a validated,
//! revision-bound [`EquationCandidate`] as non-authoritative DATA. Candidate
//! evidence is never graph truth (D127 / ADR-053): `authoritative` and
//! `import_eligible` are hard-locked to `false` on the node.
//!
//! Graph wiring:
//! - `PaperRevision -[:hasPart]-> EquationCandidate` (revision owns the candidate)
//! - `EquationCandidate -[:groundedIn]-> Section` (revision-scoped section lineage)
//!
//! Identity contract (mirrors `TableCandidateMaterializer`, M400 S02):
//! - the candidate VID is recomputed from its revision-bound lineage and must
//!   match the stored VID ([`EquationCandidate::validate`]);
//! - the PaperRevision and revision-scoped Section the candidate grounds in
//!   must already exist in the graph (grounding targets are never invented);
//! - an identical replay is idempotent: `applied=false`, `unchanged=true`, and
//!   zero node/edge writes; a replay with divergent properties — or a partially
//!   written candidate (missing edge) — is rejected (Conflict) instead of
//!   silently overwriting the deterministic identity.
//!
//! This module also owns the application-boundary conversion from the ports
//! wire type ([`da_ports::parser::ExtractedFormula`]) to the domain projection
//! ([`EquationCandidateInput`]) — deferred from T02 so da-domain stays
//! dependency-free. Formulas without a section identity (outside every section
//! window), formulas whose content is unusable, and formulas referencing an
//! out-of-range section ordinal are typed extraction-missingness errors, never
//! silently dropped.

use std::sync::Arc;

use da_domain::equation_candidate::{
    EquationCandidate, EquationCandidateError, EquationCandidateInput, EquationRepresentationKind,
};
use da_domain::relation::structure;
use da_domain::vid::revision_section_vid;
use da_ports::graph_store::DirectGraphStore;
use da_ports::parser::{ExtractedFormula, FormulaRepresentation, Section};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Deterministic replay-outcome diagnostic written by the materializer.
pub const DIAGNOSTIC_APPLIED: &str = "applied";
/// Diagnostic returned for an idempotent replay of an identical candidate.
pub const DIAGNOSTIC_UNCHANGED_REPLAY: &str = "unchanged:replay";

/// Node label of the materialized candidate.
pub const EQUATION_CANDIDATE_LABEL: &str = "EquationCandidate";

/// Lossless application-boundary conversion from the deterministic TEI
/// formula wire type (T01) to the domain projection the contract consumes
/// (T02). Preserves formula id, label, coordinates, representation kind and
/// content verbatim, source span, and the formula source digest.
///
/// Section lineage is resolved against the revision's section list by the
/// formula's `section_ordinal`: the section VID derives deterministically
/// from `(revision_vid, ordinal)` (mirroring `revision_materialization`),
/// and the section title is read from the resolved `Section`. A formula with
/// no section identity, unusable content, or an out-of-range section ordinal
/// is a typed extraction-missingness error — never a silently dropped
/// formula and never an invented representation.
pub fn equation_candidate_input_from_extracted(
    formula: &ExtractedFormula,
    revision_vid: &str,
    source_pdf_sha256: &str,
    sections: &[Section],
) -> Result<EquationCandidateInput, EquationMaterializationError> {
    if let Some(reason) = &formula.missingness {
        return Err(EquationMaterializationError::MalformedFormula(
            formula.formula_id.clone(),
            format!("{reason:?}"),
        ));
    }
    let section_ordinal = formula.section_ordinal.ok_or_else(|| {
        EquationMaterializationError::UngroundedFormula(formula.formula_id.clone())
    })?;
    let section_path_id = formula.section_path_id.as_deref().ok_or_else(|| {
        EquationMaterializationError::UngroundedFormula(formula.formula_id.clone())
    })?;
    let section = sections.get(section_ordinal).ok_or_else(|| {
        EquationMaterializationError::SectionOrdinalOutOfRange {
            formula: formula.formula_id.clone(),
            ordinal: section_ordinal,
            count: sections.len(),
        }
    })?;
    let section_vid = revision_section_vid(revision_vid, section_ordinal)
        .map_err(EquationMaterializationError::Invalid)?;
    let representation_kind = match &formula.representation {
        FormulaRepresentation::MathML(_) => EquationRepresentationKind::MathMl,
        FormulaRepresentation::TeXMath(_) => EquationRepresentationKind::TexMath,
        FormulaRepresentation::PlainText(_) => EquationRepresentationKind::PlainText,
    };
    Ok(EquationCandidateInput {
        revision_vid: revision_vid.to_string(),
        source_pdf_sha256: source_pdf_sha256.to_string(),
        formula_id: formula.formula_id.clone(),
        section_vid,
        section_path_id: section_path_id.to_string(),
        section_ordinal: section_ordinal as u32,
        section_title: section.title.clone(),
        label: formula.label.clone(),
        coordinates: formula.coordinates.clone(),
        representation_kind,
        representation_text: formula.representation.as_str().to_string(),
        span_start: formula.span.map(|span| span.start),
        span_end: formula.span.map(|span| span.end),
        source_digest: formula.source_digest.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquationMaterializationResult {
    /// Deterministic candidate VID written (or already present).
    pub candidate_vid: String,
    pub nodes_created: usize,
    pub nodes_reused: usize,
    pub edges_created: usize,
    pub edges_reused: usize,
    /// True when this run wrote the candidate (first application).
    pub applied: bool,
    /// True when the candidate already existed with identical lineage
    /// properties (idempotent replay).
    pub unchanged: bool,
    /// Hard-locked false: candidate evidence is never import-eligible (D127).
    pub import_eligible: bool,
    /// `applied` on first write, `unchanged:replay` on identical replay.
    pub materialization_diagnostics: String,
}

#[derive(Debug, Error)]
pub enum EquationMaterializationError {
    #[error("invalid equation candidate: {0}")]
    Invalid(String),
    #[error(
        "formula {0} has no section identity (outside every section window); cannot ground the candidate"
    )]
    UngroundedFormula(String),
    #[error("formula {0} is unusable: {1}")]
    MalformedFormula(String, String),
    #[error(
        "formula {formula} references section ordinal {ordinal} which is out of range (revision has {count} sections)"
    )]
    SectionOrdinalOutOfRange {
        formula: String,
        ordinal: usize,
        count: usize,
    },
    #[error("revision {0} is not materialized in the graph")]
    MissingRevision(String),
    #[error("section {0} is not materialized in the graph")]
    MissingSection(String),
    #[error(
        "section {section} does not belong to revision {revision}: grounding lineage is broken"
    )]
    SectionRevisionMismatch { section: String, revision: String },
    #[error(
        "equation candidate {0} is already materialized with divergent properties; refusing to overwrite deterministic identity"
    )]
    Conflict(String),
    #[error("graph write failed during {operation}: {source}")]
    Graph {
        operation: &'static str,
        #[source]
        source: kg_storage::GraphStoreError,
    },
}

impl From<EquationCandidateError> for EquationMaterializationError {
    fn from(error: EquationCandidateError) -> Self {
        Self::Invalid(error.to_string())
    }
}

pub struct EquationCandidateMaterializer {
    graph_store: Arc<dyn DirectGraphStore>,
}

impl EquationCandidateMaterializer {
    pub fn new(graph_store: Arc<dyn DirectGraphStore>) -> Self {
        Self { graph_store }
    }

    /// Materialize one validated equation candidate as non-authoritative
    /// graph evidence. Idempotent: an identical replay applies zero
    /// nodes/edges and reports `unchanged` with positive reuse evidence.
    pub async fn materialize(
        &self,
        candidate: &EquationCandidate,
    ) -> Result<EquationMaterializationResult, EquationMaterializationError> {
        candidate.validate()?;

        // Grounding identity must already exist in the graph: we never invent
        // the revision or the revision-scoped section the candidate grounds in.
        let revision_node = self
            .graph_store
            .find_node_by_string_property("PaperRevision", "vid", &candidate.revision_vid)
            .await
            .ok_or_else(|| {
                EquationMaterializationError::MissingRevision(candidate.revision_vid.clone())
            })?;
        let section_node = self
            .graph_store
            .find_node_by_string_property("Section", "vid", &candidate.section_vid)
            .await
            .ok_or_else(|| {
                EquationMaterializationError::MissingSection(candidate.section_vid.clone())
            })?;
        let section_work_vid = self
            .graph_store
            .get_node_property_string(section_node, "work_vid")
            .await;
        if section_work_vid.as_deref() != Some(candidate.revision_vid.as_str()) {
            return Err(EquationMaterializationError::SectionRevisionMismatch {
                section: candidate.section_vid.clone(),
                revision: candidate.revision_vid.clone(),
            });
        }

        let mut result = EquationMaterializationResult {
            candidate_vid: candidate.vid.clone(),
            nodes_created: 0,
            nodes_reused: 0,
            edges_created: 0,
            edges_reused: 0,
            applied: false,
            unchanged: false,
            import_eligible: false,
            materialization_diagnostics: DIAGNOSTIC_APPLIED.to_string(),
        };

        if let Some(existing) = self
            .graph_store
            .find_node_by_string_property(EQUATION_CANDIDATE_LABEL, "vid", &candidate.vid)
            .await
        {
            // Replay: every deterministic part must already exist and match —
            // a partially written or divergent candidate is a Conflict, never
            // a silent repair of a deterministic identity.
            if !self.candidate_node_matches(existing, candidate).await {
                return Err(EquationMaterializationError::Conflict(
                    candidate.vid.clone(),
                ));
            }
            result.nodes_reused += 1;

            // Edges are part of the deterministic materialization: an
            // unchanged node must also keep its revision/section edges.
            self.reuse_edges(revision_node, existing, section_node, &mut result)
                .await?;

            result.applied = false;
            result.unchanged = true;
            result.materialization_diagnostics = DIAGNOSTIC_UNCHANGED_REPLAY.to_string();
            tracing::info!(
                candidate_vid = %candidate.vid,
                revision_vid = %candidate.revision_vid,
                section_vid = %candidate.section_vid,
                formula_id = %candidate.formula_id,
                applied = false,
                unchanged = true,
                "equation candidate replay unchanged"
            );
            return Ok(result);
        }

        // First application: candidate node + lineage edges in one pass.
        // The string literal keeps the schema_audit_test scanner
        // (create_node call-site audit against node_schemas.yaml) covering
        // this label.
        let candidate_node = self
            .graph_store
            .create_node(EQUATION_CANDIDATE_LABEL)
            .await
            .map_err(|source| EquationMaterializationError::Graph {
                operation: "create EquationCandidate node",
                source,
            })?;
        result.nodes_created += 1;
        self.write_candidate_properties(candidate_node, candidate, DIAGNOSTIC_APPLIED)
            .await?;

        self.ensure_edge(
            revision_node,
            candidate_node,
            structure::HAS_PART,
            &mut result,
        )
        .await?;
        self.ensure_edge(
            candidate_node,
            section_node,
            structure::GROUNDED_IN,
            &mut result,
        )
        .await?;

        result.applied = true;
        result.materialization_diagnostics = DIAGNOSTIC_APPLIED.to_string();
        tracing::info!(
            candidate_vid = %candidate.vid,
            revision_vid = %candidate.revision_vid,
            section_vid = %candidate.section_vid,
            formula_id = %candidate.formula_id,
            applied = true,
            unchanged = false,
            "equation candidate materialized"
        );
        Ok(result)
    }

    /// Verify every expected edge already exists on replay, counting them
    /// into `edges_reused`. A missing edge means a partial write — Conflict.
    async fn reuse_edges(
        &self,
        revision_node: u64,
        candidate_node: u64,
        section_node: u64,
        result: &mut EquationMaterializationResult,
    ) -> Result<(), EquationMaterializationError> {
        self.reuse_edge(
            revision_node,
            candidate_node,
            structure::HAS_PART,
            "revision hasPart candidate",
            result,
        )
        .await?;
        self.reuse_edge(
            candidate_node,
            section_node,
            structure::GROUNDED_IN,
            "candidate groundedIn section",
            result,
        )
        .await?;
        Ok(())
    }

    async fn reuse_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &'static str,
        operation: &'static str,
        result: &mut EquationMaterializationResult,
    ) -> Result<(), EquationMaterializationError> {
        if self
            .graph_store
            .get_edges_between(source, target, edge_type)
            .await
            .is_empty()
        {
            return Err(EquationMaterializationError::Conflict(format!(
                "replay is missing edge {edge_type} ({operation})"
            )));
        }
        result.edges_reused += 1;
        Ok(())
    }

    /// Write every deterministic candidate property (formula identity,
    /// revision/section lineage, representation digest and verbatim text,
    /// source span, schema, authority locks, replay diagnostic) onto the node.
    async fn write_candidate_properties(
        &self,
        node: u64,
        candidate: &EquationCandidate,
        diagnostic: &str,
    ) -> Result<(), EquationMaterializationError> {
        self.set_string(node, "vid", &candidate.vid).await?;
        self.set_string(node, "revision_vid", &candidate.revision_vid)
            .await?;
        self.set_string(node, "source_pdf_sha256", &candidate.source_pdf_sha256)
            .await?;
        self.set_string(node, "formula_id", &candidate.formula_id)
            .await?;
        self.set_string(node, "section_vid", &candidate.section_vid)
            .await?;
        self.set_string(node, "section_path_id", &candidate.section_path_id)
            .await?;
        self.set_int(node, "section_ordinal", candidate.section_ordinal as i64)
            .await?;
        self.set_string(node, "section_title", &candidate.section_title)
            .await?;
        self.set_optional_string(node, "label", candidate.label.as_deref())
            .await?;
        self.set_optional_string(node, "coordinates", candidate.coordinates.as_deref())
            .await?;
        self.set_string(
            node,
            "representation_kind",
            candidate.representation_kind.as_str(),
        )
        .await?;
        self.set_string(node, "representation_text", &candidate.representation_text)
            .await?;
        self.set_string(
            node,
            "representation_sha256",
            &candidate.representation_sha256,
        )
        .await?;
        self.set_optional_int(node, "span_start", candidate.span_start.map(i64::from))
            .await?;
        self.set_optional_int(node, "span_end", candidate.span_end.map(i64::from))
            .await?;
        self.set_string(node, "source_digest", &candidate.source_digest)
            .await?;
        self.set_string(
            node,
            "candidate_metadata_sha256",
            &candidate.candidate_metadata_sha256,
        )
        .await?;
        // Authority locks: candidate evidence is never graph truth (D127).
        self.set_bool(node, "authoritative", false).await?;
        self.set_bool(node, "import_eligible", false).await?;
        self.set_int(node, "schema_version", candidate.schema_version as i64)
            .await?;
        self.set_string(node, "materialization_diagnostics", diagnostic)
            .await?;
        Ok(())
    }

    /// Deterministic replay comparison for the candidate node. The diagnostic
    /// property is excluded — it records the first-application outcome and is
    /// not part of identity.
    async fn candidate_node_matches(&self, node: u64, candidate: &EquationCandidate) -> bool {
        let string_fields: &[(&str, &str)] = &[
            ("vid", &candidate.vid),
            ("revision_vid", &candidate.revision_vid),
            ("source_pdf_sha256", &candidate.source_pdf_sha256),
            ("formula_id", &candidate.formula_id),
            ("section_vid", &candidate.section_vid),
            ("section_path_id", &candidate.section_path_id),
            ("section_title", &candidate.section_title),
            (
                "representation_kind",
                candidate.representation_kind.as_str(),
            ),
            ("representation_text", &candidate.representation_text),
            ("representation_sha256", &candidate.representation_sha256),
            ("source_digest", &candidate.source_digest),
            (
                "candidate_metadata_sha256",
                &candidate.candidate_metadata_sha256,
            ),
        ];
        for (key, expected) in string_fields {
            if self
                .graph_store
                .get_node_property_string(node, key)
                .await
                .as_deref()
                != Some(*expected)
            {
                return false;
            }
        }
        let optional_string_fields: &[(&str, Option<&str>)] = &[
            ("label", candidate.label.as_deref()),
            ("coordinates", candidate.coordinates.as_deref()),
        ];
        for (key, expected) in optional_string_fields {
            if self
                .graph_store
                .get_node_property_string(node, key)
                .await
                .as_deref()
                != *expected
            {
                return false;
            }
        }
        let int_fields: &[(&str, i64)] = &[
            ("section_ordinal", candidate.section_ordinal as i64),
            ("schema_version", candidate.schema_version as i64),
        ];
        for (key, expected) in int_fields {
            if self.graph_store.get_node_property_int(node, key).await != Some(*expected) {
                return false;
            }
        }
        let optional_int_fields: &[(&str, Option<i64>)] = &[
            ("span_start", candidate.span_start.map(i64::from)),
            ("span_end", candidate.span_end.map(i64::from)),
        ];
        for (key, expected) in optional_int_fields {
            if self.graph_store.get_node_property_int(node, key).await != *expected {
                return false;
            }
        }
        let bool_fields: &[(&str, bool)] = &[("authoritative", false), ("import_eligible", false)];
        for (key, expected) in bool_fields {
            if self.graph_store.get_node_property_bool(node, key).await != Some(*expected) {
                return false;
            }
        }
        true
    }

    async fn ensure_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &'static str,
        result: &mut EquationMaterializationResult,
    ) -> Result<(), EquationMaterializationError> {
        if !self
            .graph_store
            .get_edges_between(source, target, edge_type)
            .await
            .is_empty()
        {
            result.edges_reused += 1;
            return Ok(());
        }
        self.graph_store
            .create_edge(source, target, edge_type)
            .await
            .map_err(|source| EquationMaterializationError::Graph {
                operation: "create lineage edge",
                source,
            })?;
        result.edges_created += 1;
        Ok(())
    }

    async fn set_string(
        &self,
        node: u64,
        key: &'static str,
        value: &str,
    ) -> Result<(), EquationMaterializationError> {
        self.graph_store
            .set_node_property_string(node, key, value.to_string())
            .await
            .map_err(|source| EquationMaterializationError::Graph {
                operation: "set string property",
                source,
            })
    }

    /// Set an optional string property only when present (absent on None).
    async fn set_optional_string(
        &self,
        node: u64,
        key: &'static str,
        value: Option<&str>,
    ) -> Result<(), EquationMaterializationError> {
        if let Some(value) = value {
            self.set_string(node, key, value).await?;
        }
        Ok(())
    }

    async fn set_int(
        &self,
        node: u64,
        key: &'static str,
        value: i64,
    ) -> Result<(), EquationMaterializationError> {
        self.graph_store
            .set_node_property_int(node, key, value)
            .await
            .map_err(|source| EquationMaterializationError::Graph {
                operation: "set integer property",
                source,
            })
    }

    /// Set an optional integer property only when present (absent on None).
    async fn set_optional_int(
        &self,
        node: u64,
        key: &'static str,
        value: Option<i64>,
    ) -> Result<(), EquationMaterializationError> {
        if let Some(value) = value {
            self.set_int(node, key, value).await?;
        }
        Ok(())
    }

    async fn set_bool(
        &self,
        node: u64,
        key: &'static str,
        value: bool,
    ) -> Result<(), EquationMaterializationError> {
        self.graph_store
            .set_node_property_bool(node, key, value)
            .await
            .map_err(|source| EquationMaterializationError::Graph {
                operation: "set boolean property",
                source,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_ports::parser::{CharSpan, FormulaMissingReason};

    fn sha(seed: u64) -> String {
        format!("{seed:064x}")
    }

    fn section(title: &str, ordinal: usize) -> Section {
        Section {
            title: title.into(),
            text: format!("text {ordinal}"),
            level: 1,
            path_id: Some(ordinal.to_string()),
            section_id: Some(format!("sec-{ordinal}")),
            ..Default::default()
        }
    }

    fn ports_formula(representation: FormulaRepresentation) -> ExtractedFormula {
        ExtractedFormula {
            formula_id: "formula_0".into(),
            section_path_id: Some("2".into()),
            section_ordinal: Some(2),
            label: Some("(1)".into()),
            coordinates: Some("10,20,30,40;5".into()),
            representation,
            span: Some(CharSpan {
                start: 120,
                end: 158,
            }),
            source_digest: sha(7),
            missingness: None,
        }
    }

    #[test]
    fn conversion_preserves_representation_kind_text_and_span() {
        let formula = ports_formula(FormulaRepresentation::TeXMath(
            "L(\\theta)=\\mathbb{E}[\\log p_\\theta(x)]".into(),
        ));
        let sections = vec![
            section("Intro", 0),
            section("Methods", 1),
            section("Results", 2),
        ];
        let input = equation_candidate_input_from_extracted(&formula, &sha(10), &sha(1), &sections)
            .unwrap();
        let candidate = EquationCandidate::new(input).unwrap();
        candidate.validate().unwrap();
        assert_eq!(candidate.formula_id, "formula_0");
        assert_eq!(candidate.section_path_id, "2");
        assert_eq!(candidate.section_ordinal, 2);
        assert_eq!(candidate.section_title, "Results");
        assert_eq!(
            candidate.section_vid,
            revision_section_vid(&sha(10), 2).unwrap()
        );
        assert_eq!(candidate.label.as_deref(), Some("(1)"));
        assert_eq!(candidate.coordinates.as_deref(), Some("10,20,30,40;5"));
        assert_eq!(
            candidate.representation_kind,
            EquationRepresentationKind::TexMath
        );
        assert_eq!(
            candidate.representation_text,
            "L(\\theta)=\\mathbb{E}[\\log p_\\theta(x)]"
        );
        assert_eq!(candidate.span_start, Some(120));
        assert_eq!(candidate.span_end, Some(158));
        assert_eq!(candidate.source_digest, sha(7));
        assert!(!candidate.authoritative && !candidate.import_eligible);
        // The representation digest is deterministic and content-bound.
        assert_eq!(candidate.representation_sha256.len(), 64);
    }

    #[test]
    fn conversion_maps_every_representation_kind_verbatim() {
        for (ports, expected) in [
            (
                FormulaRepresentation::MathML("<math><mi>x</mi></math>".into()),
                EquationRepresentationKind::MathMl,
            ),
            (
                FormulaRepresentation::TeXMath("x^2".into()),
                EquationRepresentationKind::TexMath,
            ),
            (
                FormulaRepresentation::PlainText("x squared".into()),
                EquationRepresentationKind::PlainText,
            ),
        ] {
            let formula = ports_formula(ports);
            let sections = vec![section("A", 0), section("B", 1), section("C", 2)];
            let input =
                equation_candidate_input_from_extracted(&formula, &sha(10), &sha(1), &sections)
                    .unwrap();
            assert_eq!(input.representation_kind, expected);
        }
    }

    #[test]
    fn ungrounded_formula_without_section_identity_is_typed_missingness() {
        let sections = vec![section("A", 0)];
        // No section ordinal: formula sits outside every section window.
        let mut formula = ports_formula(FormulaRepresentation::PlainText("x".into()));
        formula.section_ordinal = None;
        assert!(matches!(
            equation_candidate_input_from_extracted(&formula, &sha(10), &sha(1), &sections),
            Err(EquationMaterializationError::UngroundedFormula(_))
        ));
        // Ordinal present but no path_id.
        let mut formula = ports_formula(FormulaRepresentation::PlainText("x".into()));
        formula.section_path_id = None;
        assert!(matches!(
            equation_candidate_input_from_extracted(&formula, &sha(10), &sha(1), &sections),
            Err(EquationMaterializationError::UngroundedFormula(_))
        ));
    }

    #[test]
    fn malformed_formula_content_is_typed_missingness() {
        let sections = vec![section("A", 0), section("B", 1), section("C", 2)];
        let mut formula = ports_formula(FormulaRepresentation::PlainText("".into()));
        formula.missingness = Some(FormulaMissingReason::Malformed(
            "empty formula with no representation".into(),
        ));
        assert!(matches!(
            equation_candidate_input_from_extracted(&formula, &sha(10), &sha(1), &sections),
            Err(EquationMaterializationError::MalformedFormula(_, _))
        ));
    }

    #[test]
    fn out_of_range_section_ordinal_is_typed_missingness() {
        let sections = vec![section("A", 0)];
        let mut formula = ports_formula(FormulaRepresentation::PlainText("x".into()));
        formula.section_ordinal = Some(5);
        assert!(matches!(
            equation_candidate_input_from_extracted(&formula, &sha(10), &sha(1), &sections),
            Err(EquationMaterializationError::SectionOrdinalOutOfRange {
                ordinal: 5,
                count: 1,
                ..
            })
        ));
    }
}
