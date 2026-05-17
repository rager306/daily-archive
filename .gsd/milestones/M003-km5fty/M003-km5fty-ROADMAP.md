# M003-km5fty: Scientific Hybrid Graph RAG and RLM Navigation Base

**Vision:** Evolve the M002 LadybugDB paper-level graph-vector foundation into a traceable scientific Hybrid Graph RAG base with PageIndex document navigation, chunk/claim/entity/evidence schema, hybrid retrieval baselines, evaluation metrics, DSPy typed extraction boundaries, and staged read-only RLM navigation workflows.

## Success Criteria

- Fixture full-text ingestion can produce navigable PageIndexNode hierarchies with validation diagnostics.
- SemanticChunk, Claim, ScientificEntity, ScientificRelation, and EvidencePath contracts exist and are test-covered.
- LadybugDB stores the expanded SCI KG fixture schema idempotently and transaction-safely.
- Hybrid retrieval returns traceable evidence paths with vector, graph, and fusion metadata.
- Evaluation fixtures and ablation baselines exist before any scale or optimizer claims.
- DSPy typed extraction boundary exists without premature optimizer use.
- RLM document, workflow-in-code, and graph traversal modes are prototyped as read/draft-only capabilities with deterministic validation and benchmark comparison.

## Slices

- [x] **S01: Full text ingestion contract** `risk:medium` `depends:[]`
  > After this: After this: a representative paper fixture has stable full-text or markdown input, deterministic IDs, provenance, and parser fallbacks ready for PageIndex construction.

- [x] **S02: PageIndex document navigation** `risk:high` `depends:[S01]`
  > After this: After this: a fixture paper can be navigated as an ordered PageIndexNode tree with parent child and NEXT links.

- [x] **S03: Semantic chunks and evidence paths** `risk:medium` `depends:[S02]`
  > After this: After this: PageIndex nodes can own SemanticChunk records and EvidencePath objects can point from paper to section to chunk.

- [x] **S04: S04** `risk:high` `depends:[]`
  > After this: After this: fixture text can produce typed Claim, ScientificEntity, and ScientificRelation drafts with confidence, provenance, and validation errors.

- [ ] **S05: LadybugDB SCI KG schema expansion** `risk:high` `depends:[S02,S03,S04]`
  > After this: After this: LadybugDB stores the expanded SCI KG fixture with Paper, PageIndexNode, SemanticChunk, Claim, ScientificEntity, EvidencePath, and relation edges idempotently.

- [ ] **S06: Hybrid retrieval baseline** `risk:high` `depends:[S05]`
  > After this: After this: a query over fixtures returns fused vector and graph results with evidence paths and score metadata.

- [ ] **S07: Evaluation benchmark and ablations** `risk:medium` `depends:[S03,S04,S06]`
  > After this: After this: benchmark fixtures define expected claims, entities, evidence paths, and retrieval questions with metric calculations.

- [ ] **S08: DSPy extraction boundary** `risk:medium` `depends:[S04,S07]`
  > After this: After this: baseline extraction functions are wrapped in DSPy-compatible typed modules without changing storage schema or requiring optimizers.

- [ ] **S09: RLM document and workflow harness** `risk:high` `depends:[S02,S03,S04,S08]`
  > After this: After this: RLM can navigate a fixture PageIndex tree and run a read-only workflow-in-code extraction loop, returning a typed draft plus trajectory.

- [ ] **S10: RLM graph traversal spike** `risk:high` `depends:[S05,S06,S07,S09]`
  > After this: After this: RLM graph traversal is compared against vector-only, one-hop graph expansion, and heuristic BFS on scattered-evidence fixture questions.

## Boundary Map

```text
In M003:
  Full-text and markdown ingestion contracts
  PageIndexNode hierarchy
  SemanticChunk, Claim, ScientificEntity, ScientificRelation, EvidencePath models
  LadybugDB schema expansion for scientific KG
  Hybrid retrieval baseline: vector + graph + RRF-style fusion
  Evaluation fixtures and ablation baselines
  DSPy-compatible typed extraction boundary
  RLM read/draft-only spikes for document navigation, workflow-in-code, and graph traversal

Out of M003:
  DSPy optimizer training with GEPA/MIPROv2 unless enough reviewed examples exist
  Production-scale corpus migration
  BestBlogs/social editorial product layer
  BABOK opportunity-analysis product layer
  Autopublishing or external channel delivery
  PyO3/Rust acceleration unless profiling proves a bottleneck
```
