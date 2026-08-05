// Semantic graph task suite — GEPA hub production snap (ADR-051 §8.1 / M336).
// Snapshot: artifacts/gepa_hub_production/hub.sgsnap
// Wire strings from data/ontology/edge_contracts.yaml (hasPart, CITES, FROM_SOURCE, inCategory).
// D127: import_eligible remains false — suite only reads process-local import.

// @query Paper count | Scale — GEPA hub batch has 24 retrieval-eligible papers
MATCH (n:Paper) WHERE n.retrieval_eligible = true RETURN count(n) AS papers;

// @query Hub paper by arxiv_id | Orient — GEPA hub 2507.19457 exists
MATCH (n:Paper {arxiv_id: "2507.19457"}) RETURN n.arxiv_id AS arxiv_id LIMIT 1;

// @query Citation fact edges | Fact edge CITES — bibliography resolution present
MATCH (p:Paper)-[:CITES]->(c) RETURN count(c) AS cites;

// @query Structural hasPart | Composition — sections/refs hang off papers
MATCH (p:Paper)-[:hasPart]->(x) RETURN count(x) AS parts;

// @query Provenance FROM_SOURCE | Structural provenance — papers link to Source
MATCH (p:Paper)-[:FROM_SOURCE]->(s) RETURN count(s) AS sources;

// @query Hub CITES multi-hop readiness | Graph-native — hub paper has outbound CITES
MATCH (p:Paper {arxiv_id: "2507.19457"})-[:CITES]->(c) RETURN count(c) AS hub_cites;

// @query Category placement | Structural inCategory — at least one paper categorized
MATCH (p:Paper)-[:inCategory]->(cat) RETURN count(p) AS categorized;
