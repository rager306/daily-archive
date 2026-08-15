// Semantic graph task suite — ADR-053 Layer A document tree navigation.
// Snapshot: artifacts/d127_unlock/hub_unlocked_t04.sgsnap (after T04 ingest wiring).
// Wire strings from data/ontology/edge_contracts.yaml (parent_section, first_child, next_sibling).
// D127: import_eligible remains false — suite only reads process-local structure.
//
// Template queries (M362-d4qoaf/S01 T05):
//   1. First-child navigation (descend into document tree)
//   2. Next-sibling chain (linear traversal of children under one parent)
//   3. Parent-section navigation (ascend to container)
//   4. Sibling chain walk (two-hop chains — chains continue past one hop)
//
// Deep-tree contract (M398 S01): every non-last sibling has a DIRECT
// next_sibling edge to its actual same-parent successor even when descendants
// intervene (`1.1 -> 1.2` exists although `1.1.1` sits between them in
// document order); chains are complete and acyclic; root sections chain to
// the next root section. The deterministic proof lives in the bounded Rust
// fixture `crates/da-application/tests/section_navigation_test.rs`
// (graph-walk tests materialize the tree into a store and walk next_sibling
// edges read back from the graph). The Governor consumes the same contract
// from `scripts/governor/checks/section_navigation_contract.yaml` against the
// NavigationReport fixtures in `data/navigation_contract/`.
//
// Note: queries return Section nodes populated by the ingest use case (T04)
// using path_id / parent_section_id / order_in_parent fields computed by
// GrobidParser::extract_sections (T02). Flat-section parsers produce no
// navigation edges — this suite will return 0 rows on such snapshots.
//
// Samyama Cypher note: count() returns per-row counts, not aggregates.
// Use direct edge traversal RETURN (returns one row per match).

// @query First-child navigation | Structural — sections with first_child edge
MATCH (s:Section)-[:first_child]->(child:Section) RETURN child;

// @query Next-sibling chain | Structural — sections with next_sibling edge
MATCH (s:Section)-[:next_sibling]->(sibling:Section) RETURN sibling;

// @query Parent-section navigation | Structural — sections with parent_section edge
MATCH (s:Section)-[:parent_section]->(parent:Section) RETURN parent;

// @query Sibling chain walk | Structural — two-hop next_sibling chains
// Walks two consecutive sibling hops; a non-empty result proves chains
// continue past the first hop and terminate at the last sibling (acyclic).
// On flat GROBID output (depth <= 1) this is the root-section chain.
MATCH (a:Section)-[:next_sibling]->(b:Section)-[:next_sibling]->(c:Section) RETURN c;
