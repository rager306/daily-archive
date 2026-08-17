// Revision-grounded EquationCandidate semantic graph task suite (M400 S03/T04).
// Same-process run after materializing the revision plane (PaperRevision +
// revision-scoped Sections) and EquationCandidate nodes.
// Ontology wires: PaperRevision -[:hasPart]-> EquationCandidate -[:groundedIn]-> Section.
// Wire strings from data/ontology/edge_contracts.yaml + da-domain relation
// constants (structure::HAS_PART, structure::GROUNDED_IN).
// D127 / ADR-053: candidates are candidate evidence, never graph truth —
// every task below fails if any candidate unlocks import or authority.
// Extraction missingness is typed at conversion (ungrounded/malformed formulas
// never become candidates); the required-formula task fails visibly when the
// grounded fixture formula is absent (missing), loses revision/section
// lineage (ungrounded), or unlocks D127 (unlocked).
//
// Fixture (materialized by crates/da-application/tests/equation_candidate_test.rs
// `equation_graph_task`): 2 revisions (v1, v2), 3 sections each, one
// EquationCandidate per revision grounded in section ordinal 2 with
// formula_id 'formula_0', tex_math (LaTeX) representation, source span
// 120..158 — totals: 2 candidates.

// @query Grounded equation candidates with exact section lineage | revision hasPart candidate groundedIn section
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (r:PaperRevision)-[:hasPart]->(c:EquationCandidate)-[:groundedIn]->(s:Section) WHERE c.revision_vid = r.vid AND c.section_vid = s.vid RETURN count(c) AS grounded_candidates;

// @query Required grounded formula by revision and section | fixture formula_0 stays grounded in a revision-scoped section
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (r:PaperRevision)-[:hasPart]->(c:EquationCandidate)-[:groundedIn]->(s:Section) WHERE c.formula_id = 'formula_0' AND c.revision_vid = r.vid AND c.section_vid = s.vid AND s.work_vid = c.revision_vid RETURN count(c) AS required_grounded_formulas;

// @query Candidate revision identity | every candidate binds a materialized PaperRevision via hasPart
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (r:PaperRevision)-[:hasPart]->(c:EquationCandidate) WHERE c.revision_vid = r.vid RETURN count(c) AS revision_bound_candidates;

// @query Candidate section identity | groundedIn section belongs to the candidate revision
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (c:EquationCandidate)-[:groundedIn]->(s:Section) WHERE c.section_vid = s.vid AND s.work_vid = c.revision_vid RETURN count(c) AS section_bound_candidates;

// @query Formula identity present | every candidate carries a TEI formula id and section path
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (c:EquationCandidate) WHERE c.formula_id IS NOT NULL AND c.section_path_id IS NOT NULL AND c.section_ordinal IS NOT NULL RETURN count(c) AS identified_candidates;

// @query MathML or LaTeX representation present | every candidate carries a typed representation kind
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (c:EquationCandidate) WHERE c.representation_kind = 'tex_math' OR c.representation_kind = 'mathml' RETURN count(c) AS represented_candidates;

// @query Representation digest present | every candidate carries representation text and digest
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (c:EquationCandidate) WHERE c.representation_text IS NOT NULL AND c.representation_sha256 IS NOT NULL RETURN count(c) AS digested_candidates;

// @query Source span present | every candidate exposes source offsets
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (c:EquationCandidate) WHERE c.span_start IS NOT NULL AND c.span_end IS NOT NULL RETURN count(c) AS spanned_candidates;

// @query No orphan candidates | a candidate must never lose revision or section identity
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (c:EquationCandidate) WHERE c.revision_vid IS NULL OR c.section_vid IS NULL RETURN count(c) AS orphan_candidates;

// @query D127 import lock preserved | candidate evidence is never import-eligible
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (c:EquationCandidate) WHERE c.import_eligible = true RETURN count(c) AS unlocked_import_candidates;

// @query All candidates remain import-locked | D127
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (c:EquationCandidate) WHERE c.import_eligible = false RETURN count(c) AS locked_candidates;

// @query Authority lock preserved | candidate evidence is never graph truth
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (c:EquationCandidate) WHERE c.authoritative = true RETURN count(c) AS authoritative_candidates;
