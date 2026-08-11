// Cross-layer revision preparation reading tasks (M363/S04).

// @query v1 exposes grounded Methods evidence and its readiness reason set
// @class readiness
// @expect_min_value 2
MATCH (r:PaperRevision)-[:hasPart]->(section:Section)-[:hasSummary]->(s:Summary) WHERE r.revision = 1 AND s.summary_type = "methods" AND s.source_id = section.vid RETURN count(s) AS v1_methods;

// @query current v5 honestly reports Methods and Results missing
// @class readiness
// @expect_rows 1
MATCH (r:PaperRevision)-[:hasQualitySignal]->(q:QualitySignal) WHERE r.revision = 5 AND r.is_current = true AND q.signal_type = "agent_readiness" AND q.is_current = true AND q.state = "partial" RETURN q.diagnostic_codes AS reasons;

// @query exact revision preparation keeps two manifestations per revision
// @class structural
// @expect_rows 2
MATCH (r:PaperRevision)-[:HAS_MANIFESTATION]->(m:SourceManifestation) RETURN r.revision AS revision, count(m) AS manifestations;

// @query preparation evidence remains candidate-only
// @class readiness
// @expect_min_value 2
MATCH (r:PaperRevision)-[:hasQualitySignal]->(q:QualitySignal) WHERE q.import_eligible = false RETURN count(q) AS locked_signals;
