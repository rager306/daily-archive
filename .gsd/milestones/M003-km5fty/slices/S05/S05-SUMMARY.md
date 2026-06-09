---
id: S05
parent: M003-km5fty
milestone: M003-km5fty
provides:
  - Idempotent SCI KG schema initialization in `init_scientific_kg_schema`.
  - Transaction-safe `upsert_scientific_kg` for PageIndex, SemanticChunk, EvidencePath, Claim, ScientificEntity, and ScientificRelation fixture records.
  - Contract tests that S06/S07/S10 can extend for retrieval and traversal fixtures.
requires:
  []
affects:
  - S06: Hybrid retrieval baseline can now consume persisted SCI KG fixture records
  - S07: Evaluation fixtures can validate storage-backed evidence paths and graph records
  - S10: RLM graph traversal has a storage schema substrate but no traversal behavior yet
key_files:
  - src/arxiv_archive/ladybug_client.py
  - src/arxiv_archive/page_index.py
  - tests/test_ladybug_scientific_kg.py
key_decisions:
  - Kept the S05 storage boundary in `arxiv_archive.ladybug_client` instead of adding a new persistence module so the existing LadybugDB entrypoint remains the public integration surface.
  - Stored PageIndex and EvidencePath path lists as slash-delimited strings in LadybugDB properties for compatibility with the current embedded schema.
  - Validation occurs before opening the SCI KG write transaction so invalid ExtractionPatch/EvidencePath payloads cannot create partial graph state.
patterns_established:
  - Validate SCI KG payloads before opening a LadybugDB write transaction.
  - Use deterministic EvidencePath node IDs based on PageIndexNode ID and SemanticChunk ID.
  - Keep later retrieval/DSPy/RLM behavior gated behind storage and evaluation slices.
observability_surfaces:
  - Schema helpers log created/existing statement counts per schema group.
  - SCI KG writes log paper id and record counts for nodes, chunks, evidence paths, claims, entities, and relations without logging full paper text or embeddings.
  - Failure logs include paper id, write phase, and exception text; invalid payloads return aggregated validation diagnostics.
drill_down_paths:
  - .gsd/milestones/M003-km5fty/slices/S05/tasks/T01-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S05/tasks/T02-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S05/tasks/T03-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S05/tasks/T04-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S05/tasks/T05-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-17T18:46:17.252Z
blocker_discovered: false
---

# S05: LadybugDB SCI KG schema expansion

**LadybugDB now stores the expanded SCI KG fixture schema idempotently and transaction-safely.**

## What Happened

S05 expanded the LadybugDB graph persistence layer from paper/author/keyword/category records into a scientific KG fixture schema. The implementation adds idempotent schema creation for PageIndexNode, SemanticChunk, EvidencePath, Claim, ScientificEntity, and ScientificRelation nodes plus relationship tables for document structure, chunk attachment, evidence anchoring, and scientific relation endpoints. `upsert_scientific_kg` validates document/chunk/evidence/patch consistency before writes, uses explicit transactions and parameterized MERGE statements, and rolls back on mid-write failures. The new tests prove duplicate reruns do not duplicate fixture records and invalid patches are rejected before transaction open.

## Verification

Fresh final verification after the last code change passed: 41 pytest tests, Ruff on touched files, Pyrefly on src, Ty on src plus S05 test, CLI help smoke, LSP diagnostics, and GitNexus detect_changes review.

## Requirements Advanced

- R019 — Provides storage substrate required by S06 hybrid retrieval and S10 graph traversal, while preserving S07 evaluation gating.
- R020 — Maintains the requirement that evaluation and metrics still precede DSPy/RLM quality claims.

## Requirements Validated

- R018 — S05 final verification passed: 41 pytest tests, Ruff, Pyrefly, Ty, CLI help smoke, LSP diagnostics, and GitNexus change review.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The final type gate exposed pre-existing PageIndex annotation imprecision, so S05 included a small type-only `_HeadingSection` cleanup in `src/arxiv_archive/page_index.py`. The runtime PageIndex behavior was unchanged and covered by PageIndex/evidence tests.

## Known Limitations

Fixture-level persistence only. No hybrid retrieval/fusion scoring, no benchmark metrics, no DSPy typed LM modules, no RLM workflow/traversal, and no production corpus migration.

## Follow-ups

S06 should build the hybrid retrieval baseline against the new persisted SCI KG fixture schema. S07 still must define evaluation metrics before any DSPy/RLM/optimizer quality claims.

## Files Created/Modified

- `src/arxiv_archive/ladybug_client.py` — Added idempotent base/scientific schema helpers, `upsert_scientific_kg`, deterministic EvidencePath IDs, validation-before-transaction, rollback-on-failure, and SCI KG MERGE writes.
- `src/arxiv_archive/page_index.py` — Added `_HeadingSection` typed shape to make PageIndex heading parsing type-check cleanly without changing runtime behavior.
- `tests/test_ladybug_scientific_kg.py` — Added S05 contract/integration tests for schema creation, idempotent fixture persistence, invalid patch rejection before transaction, and rollback on mid-write failure.
