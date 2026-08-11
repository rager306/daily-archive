// Revision-scoped deterministic agent readiness tasks (M363/S03).

// @query Each revision has exactly one current readiness signal
// @class readiness
// @expect_rows 2
MATCH (r:PaperRevision)-[:hasQualitySignal]->(q:QualitySignal) WHERE q.signal_type = "agent_readiness" AND q.is_current = true RETURN r.revision AS revision, q.state AS state;

// @query Current readiness binds exact revision source identity
// @class readiness
// @expect_min_value 2
MATCH (r:PaperRevision)-[:hasQualitySignal]->(q:QualitySignal) WHERE q.signal_type = "agent_readiness" AND q.is_current = true AND q.source_id = r.vid RETURN count(q) AS exact_revision_signals;

// @query Real legal ontology revisions are partial because Results evidence is absent
// @class readiness
// @expect_min_value 2
MATCH (q:QualitySignal) WHERE q.signal_type = "agent_readiness" AND q.is_current = true AND q.state = "partial" RETURN count(q) AS honest_partial_signals;

// @query Readiness is derived evidence and D127 remains locked
// @class readiness
// @expect_min_value 2
MATCH (q:QualitySignal) WHERE q.signal_type = "agent_readiness" AND q.import_eligible = false RETURN count(q) AS locked_readiness_signals;
