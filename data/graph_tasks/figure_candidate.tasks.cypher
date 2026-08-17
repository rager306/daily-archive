// Revision-grounded FigureCandidate semantic graph task suite (M400 S01/T04).
// Same-process run after materializing the revision plane (PaperRevision +
// revision-scoped Sections) and FigureCandidate nodes.
// Ontology wires: PaperRevision -[:hasPart]-> FigureCandidate -[:groundedIn]-> Section.
// D127 / ADR-053: candidates are candidate evidence, never graph truth —
// every task below fails if any candidate unlocks import or authority.

// @query Grounded figure candidates with exact section lineage | revision hasPart candidate groundedIn section
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (r:PaperRevision)-[:hasPart]->(c:FigureCandidate)-[:groundedIn]->(s:Section) WHERE c.revision_vid = r.vid AND c.section_vid = s.vid RETURN count(c) AS grounded_candidates;

// @query Candidate revision identity | every candidate binds a materialized PaperRevision via hasPart
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (r:PaperRevision)-[:hasPart]->(c:FigureCandidate) WHERE c.revision_vid = r.vid RETURN count(c) AS revision_bound_candidates;

// @query Candidate section identity | groundedIn section belongs to the candidate revision
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (c:FigureCandidate)-[:groundedIn]->(s:Section) WHERE c.section_vid = s.vid AND s.work_vid = c.revision_vid RETURN count(c) AS section_bound_candidates;

// @query No orphan candidates | a candidate must never lose revision or section identity
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (c:FigureCandidate) WHERE c.revision_vid IS NULL OR c.section_vid IS NULL RETURN count(c) AS orphan_candidates;

// @query D127 import lock preserved | candidate evidence is never import-eligible
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (c:FigureCandidate) WHERE c.import_eligible = true RETURN count(c) AS unlocked_import_candidates;

// @query All candidates remain import-locked | D127
// @class structural
// @expect_rows 1
// @expect_value 2
MATCH (c:FigureCandidate) WHERE c.import_eligible = false RETURN count(c) AS locked_candidates;

// @query Authority lock preserved | candidate evidence is never graph truth
// @class structural
// @expect_rows 1
// @expect_value 0
MATCH (c:FigureCandidate) WHERE c.authoritative = true RETURN count(c) AS authoritative_candidates;
