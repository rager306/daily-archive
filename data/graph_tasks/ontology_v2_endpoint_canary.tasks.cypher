// Bounded ontology v2 endpoint journey — process-local Samyama only.
// This suite proves materialized endpoint semantics; it is not corpus readiness.

// @query Citation endpoints | CITES must target Citation evidence
// @class ontology_endpoint
// @expect_rows 1
// @expect_value 2
MATCH (p:Paper)-[:CITES]->(c:Citation) RETURN count(c) AS citations;

// @query Independent quality endpoint | Deterministic QualitySignal evidence has a dedicated fact wire
// @class ontology_endpoint
// @expect_rows 1
// @expect_value 2
MATCH (p:Paper)-[:hasQualitySignal]->(q:QualitySignal) RETURN count(q) AS quality_signals;

// @query Forbidden citation topology | CITES must not create fake Paper nodes
// @class ontology_endpoint
// @expect_rows 1
// @expect_value 0
MATCH (p:Paper)-[:CITES]->(c) WHERE c.arxiv_id IS NOT NULL RETURN count(c) AS forbidden_paper_cites;

// @query Forbidden quality topology | hasSummary is reserved for Summary
// @class ontology_endpoint
// @expect_rows 1
// @expect_value 0
MATCH (p:Paper)-[:hasSummary]->(q) WHERE q.signal_type IS NOT NULL RETURN count(q) AS forbidden_quality_summaries;

// @query Section summary topology | Polymorphic hasSummary remains active
// @class ontology_endpoint
// @expect_rows 1
// @expect_value 1
MATCH (s:Section)-[:hasSummary]->(summary:Summary) RETURN count(summary) AS section_summaries;
