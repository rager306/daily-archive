// Revision-scoped grounded summary task suite (M363/S01).
// Runs same-process after importing revision graph and materializing summaries.
// Ontology wires: HAS_REVISION, hasPart, hasSummary. D127 remains false.

// @query Revision summaries remain history-addressable | v1 and v5 each retain section digests
// @class summary
// @expect_rows 2
MATCH (r:PaperRevision)-[:hasPart]->(section:Section)-[:hasSummary]->(s:Summary) RETURN r.revision AS revision, count(s) AS summaries;

// @query Revision section summaries preserve evidence paths | every summary has exact Section owner
// @class summary
// @expect_min_value 2
MATCH (r:PaperRevision)-[:hasPart]->(section:Section)-[:hasSummary]->(s:Summary) WHERE s.source_id = section.vid RETURN count(s) AS grounded_section_summaries;

// @query Current revision summary is explicit | no implicit latest fallback
// @class summary
// @expect_rows 1
// @expect_min_value 1
MATCH (r:PaperRevision)-[:hasPart]->(section:Section)-[:hasSummary]->(s:Summary) WHERE r.is_current = true RETURN count(s) AS current_revision_summaries;

// @query Summary import remains locked | D127
// @class summary
// @expect_min_value 2
MATCH (s:Summary) WHERE s.import_eligible = false RETURN count(s) AS locked_summaries;
