// Semantic graph task suite — GEPA hub production snap (ADR-051 §8.1 / M336).
// Snapshot: artifacts/gepa_hub_production/hub.sgsnap
// Wire strings from data/ontology/edge_contracts.yaml (hasPart, CITES, FROM_SOURCE, inCategory).
// D127: import_eligible remains false — suite only reads process-local import.
//
// M338: @expect_rows / @expect_min_value tighten PASS beyond min_rows=1.
// Engine note: Samyama Cypher returns per-Paper rows for count() without GROUP BY
// (not a single aggregate row). Bounds reflect actual engine semantics.

// @query Paper count | Scale — GEPA hub batch has 24 retrieval-eligible papers
// @class structural
// @expect_rows 1
// @expect_min_value 24
MATCH (n:Paper) WHERE n.retrieval_eligible = true RETURN count(n) AS papers;

// @query Hub paper by arxiv_id | Orient — GEPA hub 2507.19457 exists
// @class structural
// @expect_rows 1
MATCH (n:Paper {arxiv_id: "2507.19457"}) RETURN n.arxiv_id AS arxiv_id LIMIT 1;

// @query Citation fact edges | Fact edge CITES — bibliography resolution present
// @class resolution
// @expect_min_value 1
MATCH (p:Paper)-[:CITES]->(c) RETURN count(c) AS cites;

// @query Structural hasPart | Composition — sections/refs hang off papers
// @class structural
// @expect_min_value 1
MATCH (p:Paper)-[:hasPart]->(x) RETURN count(x) AS parts;

// @query Provenance FROM_SOURCE | Structural provenance — papers link to Source
// @class structural
// @expect_min_value 1
MATCH (p:Paper)-[:FROM_SOURCE]->(s) RETURN count(s) AS sources;

// @query Hub CITES multi-hop readiness | Graph-native — hub paper has outbound CITES
// @class fact
// @expect_rows 1
// @expect_min_value 1
MATCH (p:Paper {arxiv_id: "2507.19457"})-[:CITES]->(c) RETURN count(c) AS hub_cites;

// @query Category placement | Structural inCategory — at least one paper categorized
// @class structural
// @expect_min_value 1
MATCH (p:Paper)-[:inCategory]->(cat) RETURN count(p) AS categorized;

// @query Section summaries | ADR-053 Layer B — path-linked summaries are graph-reachable
// @class structural
// @expect_min_value 1
MATCH (s:Section)-[:hasSummary]->(summary:Summary) RETURN count(summary) AS section_summaries;

// @query Reviewed canonical resolution | ADR-053 Layer G — Decision audit reaches affected Entity
// @class resolution
// @expect_min_value 1
MATCH (d:Decision)-[:AFFECTS]->(e:Entity) WHERE e.canonical_vid IS NOT NULL RETURN count(e) AS resolved_entities;

// @query Concept cluster memberships | Semantic convergence — typed entities join ConceptCluster
// @class cluster
// @expect_min_value 1
MATCH (e:Entity)-[:MEMBER_OF_CLUSTER]->(c:ConceptCluster) RETURN count(e) AS clustered_entities;
