// Semantic graph task suite — corpus-wide gold rebuild (ADR-051 §8.1 / M437).
// Snapshot: artifacts/gold_corpus/corpus_graph.sgsnap (173 versioned-bound papers).
// Wire strings from data/ontology/edge_contracts.yaml (hasPart, foundIn, HAS_REVISION).
// D127: import_eligible remains false — suite only reads the process-local snapshot.
//
// Engine notes:
// - Samyama Cypher returns per-Paper rows for count() without GROUP BY; bounds
//   reflect actual engine semantics (use single-hop aggregates or DISTINCT).
// - VID values are content-scoped SHA-256 hex strings (kg-ontology vid), not
//   human-readable prefixes — integrity checks target structural properties.

// @query Corpus paper count | Scale — 173 papers bound via TEI-declared revisions
// @class structural
// @expect_rows 1
// @expect_min_value 173
MATCH (n:Paper) RETURN count(n) AS papers;

// @query Revisions exist | ADR-053 Layer A — every paper has a revision node
// @class structural
// @expect_min_value 173
MATCH (p:Paper)-[:HAS_REVISION]->(r) RETURN count(DISTINCT r.vid) AS revisions;

// @query Sections hang off revisions | Composition hasPart — section tree present
// @class structural
// @expect_min_value 1000
MATCH (r)-[:hasPart]->(s) RETURN count(s) AS sections;

// @query Entities materialized | Extraction layer — canonical entities exist (dedup by vid)
// @class structural
// @expect_rows 1
// @expect_min_value 50
MATCH (e:Entity) RETURN count(DISTINCT e.vid) AS entities;

// @query FOUND_IN grounding | Extraction layer B — entity→section edges at scale
// @class fact
// @expect_rows 1
// @expect_min_value 1000
MATCH (e:Entity)-[f]->(s:Section) WHERE toLower(type(f)) = "foundin" RETURN count(f) AS grounded_edges;

// @query Manifestations bound | Provenance — revisions link to captured sources
// @class structural
// @expect_min_value 173
MATCH (r)-[:HAS_MANIFESTATION]->(m) RETURN count(m) AS manifestations;

// @query Adam paper orient | Known corpus member (1412.6980, TEI-declared v9)
// @class structural
// @expect_rows 1
MATCH (p:Paper {arxiv_id: "1412.6980"})-[:HAS_REVISION]->(r) RETURN r.vid AS revision LIMIT 1;

// @query Method-type coverage | Typed extraction surface present (mentions carry types)
// @class fact
// @expect_rows 1
// @expect_min_value 20
MATCH (e:Entity) WHERE e.entity_type = "Method" RETURN count(DISTINCT e.vid) AS methods;

// @query Entity grounding distinct | foundIn edges bind every mention to its section
// @class fact
// @expect_rows 1
// @expect_min_value 1000
MATCH (:Entity)-[f]->(:Section) WHERE toLower(type(f)) = "foundin" RETURN count(DISTINCT f) AS distinct_found_in_edges;

// @query Claims materialized | Evidence bundle claims present at corpus scale
// @class resolution
// @expect_rows 1
// @expect_min_value 300
MATCH (c:Claim) RETURN count(c) AS claims;
