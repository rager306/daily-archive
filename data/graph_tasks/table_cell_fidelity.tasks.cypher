// Table-cell fidelity semantic graph task suite (M400 S02 T04).
// Same-process run after materializing the revision plane (PaperRevision +
// revision-scoped Sections) and TableCandidate/TableCell/TableConflict nodes.
// Ontology wires: PaperRevision -[:hasPart]-> TableCandidate -[:hasPart]->
// TableCell; TableCandidate -[:groundedIn]-> Section (revision-scoped);
// TableCandidate -[:CONFLICTS_OVER]-> TableConflict (typed visual-versus-TEI
// disagreement, ADR-047 conflict plane — never an overwrite).
// Wire strings from data/ontology/edge_contracts.yaml + da-domain relation
// constants (structure::HAS_PART, structure::GROUNDED_IN,
// conflict::CONFLICTS_OVER).
// D127 / ADR-053: candidates are candidate evidence, never graph truth —
// every task below fails if any candidate/cell/conflict unlocks import or
// authority. Coverage gaps (missing_tei_cell / missing_visual_cell) are
// counters on the candidate node (coverage_unmatched_*), never conflict
// nodes and never invented cells (R087 no-invention rule).
//
// Fixture (materialized by crates/da-application/tests/table_candidate_test.rs
// `table_cell_fidelity_graph_task`): 2 perfect candidates (6 cells each, zero
// findings), 1 conflicting candidate (4 cells, header_mismatch +
// numeric_mismatch), 1 no-visual-payload candidate (2 cells, typed
// missingness) — totals: 4 candidates, 18 cells, 2 conflicts.

// @query Grounded table cells with exact revision lineage | revision hasPart candidate hasPart cell
// @class structural
// @expect_rows 1
// @expect_value 18
MATCH (r:PaperRevision)-[:hasPart]->(c:TableCandidate)-[:hasPart]->(cell:TableCell) WHERE c.revision_vid = r.vid RETURN count(cell) AS grounded_cells;

// @query Candidate revision identity | every candidate binds a materialized PaperRevision via hasPart
// @class structural
// @expect_rows 1
// @expect_value 4
MATCH (r:PaperRevision)-[:hasPart]->(c:TableCandidate) WHERE c.revision_vid = r.vid RETURN count(c) AS revision_bound_candidates;

// @query Candidate section identity | groundedIn section belongs to the candidate revision
// @class structural
// @expect_rows 1
// @expect_value 4
MATCH (c:TableCandidate)-[:groundedIn]->(s:Section) WHERE c.section_vid = s.vid AND s.work_vid = c.revision_vid RETURN count(c) AS section_bound_candidates;

// @query No orphan candidates | a candidate must never lose revision or section identity
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (c:TableCandidate) WHERE c.revision_vid IS NULL OR c.section_vid IS NULL RETURN count(c) AS orphan_candidates;

// @query D127 import lock preserved | candidate evidence is never import-eligible
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (c:TableCandidate) WHERE c.import_eligible = true RETURN count(c) AS unlocked_import_candidates;

// @query All candidates remain import-locked | D127
// @class structural
// @expect_rows 1
// @expect_value 4
MATCH (c:TableCandidate) WHERE c.import_eligible = false RETURN count(c) AS locked_candidates;

// @query Authority lock preserved | candidate evidence is never graph truth
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (c:TableCandidate) WHERE c.authoritative = true RETURN count(c) AS authoritative_candidates;

// @query Typed conflict nodes | candidate conflictsOver typed disagreement, never an overwrite
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (c:TableCandidate)-[:CONFLICTS_OVER]->(x:TableConflict) WHERE x.import_eligible = false RETURN count(x) AS typed_conflicts;

// @query Header mismatches typed | header disagreement is a typed conflict node
// @class structural
// @expect_rows 1
// @expect_value 1
MATCH (x:TableConflict) WHERE x.kind = 'header_mismatch' RETURN count(x) AS header_mismatches;

// @query Numeric mismatches typed | body numeric disagreement is a typed conflict node
// @class structural
// @expect_rows 1
// @expect_value 1
MATCH (x:TableConflict) WHERE x.kind = 'numeric_mismatch' RETURN count(x) AS numeric_mismatches;

// @query No coverage-gap conflict nodes | missing cells are counters, not conflicts (R087)
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (x:TableConflict) WHERE x.kind = 'missing_tei_cell' OR x.kind = 'missing_visual_cell' RETURN count(x) AS gap_conflict_nodes;

// @query TEI text preserved verbatim | conflict node keeps the structural text unchanged
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (x:TableConflict) WHERE x.tei_text = 'F1' OR x.tei_text = '0.90' RETURN count(x) AS tei_verbatim_conflicts;

// @query Visual text preserved verbatim | conflict node keeps the visual text unchanged
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (x:TableConflict) WHERE x.visual_text = 'F1-score' OR x.visual_text = '0.87' RETURN count(x) AS visual_verbatim_conflicts;

// @query Typed cell fidelity kinds | disagreeing cells carry the typed disposition
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (cell:TableCell) WHERE cell.fidelity_kind IS NOT NULL RETURN count(cell) AS typed_cells;

// @query Agreeing cells carry no fidelity kind | agreement is silent, not typed
// @class structural
// @expect_rows 1
// @expect_value 16
MATCH (cell:TableCell) WHERE cell.fidelity_kind IS NULL RETURN count(cell) AS agreeing_cells;

// @query Exact positional coverage | matched visual cells equal visual cells with no unmatched positions
// @class structural
// @expect_rows 1
// @expect_value 3
MATCH (c:TableCandidate) WHERE c.coverage_visual_cells = c.coverage_matched AND c.coverage_unmatched_visual = 0 AND c.coverage_unmatched_tei = 0 RETURN count(c) AS fully_matched_candidates;

// @query Typed source-level missingness | no-visual-payload is a typed candidate disposition
// @class structural
// @expect_rows 1
// @expect_value 1
MATCH (c:TableCandidate) WHERE c.missingness_kind = 'no_visual_payload' RETURN count(c) AS typed_missingness;

// @query Missingness keeps TEI cells materialized | cells survive without visual evidence
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (c:TableCandidate)-[:hasPart]->(cell:TableCell) WHERE c.missingness_kind IS NOT NULL RETURN count(cell) AS missingness_cells;

// @query No candidate without TEI grounding | a candidate is always TEI-grounded (R087)
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (c:TableCandidate) WHERE c.coverage_tei_cells = 0 RETURN count(c) AS zero_tei_candidates;

// @query Conflict nodes stay locked | D127 applies to conflict evidence too
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (x:TableConflict) WHERE x.authoritative = true OR x.import_eligible = true RETURN count(x) AS unlocked_conflicts;

// @query All cells remain import-locked | D127
// @class structural
// @expect_rows 1
// @expect_value 18
MATCH (cell:TableCell) WHERE cell.import_eligible = false RETURN count(cell) AS locked_cells;
