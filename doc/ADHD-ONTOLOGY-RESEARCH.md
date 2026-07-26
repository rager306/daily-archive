# ADHD Ontology Research Results

**Date:** 2026-07-26
**Method:** ADHD skill — 5 parallel cognitive frames × 6 ideas = 30 candidates,
scored, clustered, top-3 deepened.
**Tools:** jina (arxiv search, parallel search), exa (web search), gitnexus (code analysis)
**Decision:** D134

## Research inputs

### Papers studied (via jina + exa)
- **Paper Circle** (ACL 2026, arxiv 2604.06170): typed KG schema with nodes for
  papers, sections, concepts, methods, experiments, datasets, figures, tables,
  equations. Edges: hierarchy, definition, proposal, usage, evaluation,
  illustration, dependency. ALL nodes carry provenance (source chunk IDs, page
  numbers, verification status, confidence scores, timestamps).
- **SemOpenAlex** (arxiv 2308.03671): 26B RDF triples, scholarly KG.
- **PubGraph** (arxiv 2302.02231): OpenAlex → Wikidata ontological mapping.
- **Research KG survey** (arxiv 2506.07285): entity extraction via NER, focus
  on ML models and datasets.
- **LLM KG construction survey** (arxiv 2510.20345): LLM methods for KG.
- **OntoEKG** (arxiv 2602.01276): LLM-driven domain ontology generation.

### Critical discovery
**OpenAlex Concepts are DEPRECATED** — replaced by Topics system:
- Topics: ~4,500 research topics in 4-level hierarchy
  (4 domains → 26 fields → 254 subfields → ~4,500 topics)
- Assigned via citation-based clustering (CWTS Leiden) + LLM labeling + ML classifier
- Keywords: 26,000+, BGE M3-Embedding for similarity
- Concepts (65k, 19 root, 5 levels) → being renamed to `mag_concepts`, no updates

## Three deepened branches

### 1. Provenance Ring + Agent Quarantine
- Provenance in append-only `ProvenanceEvent` ring; nodes hold offset pointer
- RuVector/SONA writes = `QuarantinedAssertion` edges (`retrieval_eligible=false`)
- Two-lane: certified (users/PPR) vs quarantine (agent rehearsal)
- PromotionCertificate flips quarantined → live

### 2. Epigenetic Deprecation + Topic Lineage
- Deprecated Concepts: `retrieval_eligible=false` (silenced, not deleted)
- Topic edges: `assignment_method` + `hierarchy_snapshot_id`
- Concept→Topic remap: reversible, with confidence
- RetrievalPolicy: single predicate all paths must use

### 3. Radical Reduction — Topic-leaf + Chunk + Event
- Two stored node types: TopicLeaf + Chunk
- Everything else = VIEW from ExtractionEvent hyperedges
- FaBiO types + CiTO intents = event payload, not node labels

## Adopted patterns (D134)

1. **`retrieval_eligible` on ALL nodes** — false for deprecated Concepts,
   quarantined agent writes, unparsed spans; true only for promoted live nodes
2. **`assignment_method` + `hierarchy_snapshot_id`** on Topic edges — audit trail
3. **Provenance ring** — `ProvenanceEvent` append-only, nodes hold offset
4. **Agent quarantine** — `QuarantinedAssertion` until `PromotionCertificate`
