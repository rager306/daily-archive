---
id: S05
parent: M003-km5fty
milestone: M003-km5fty
provides:
  - Idempotent SCI KG schema initialization in `init_scientific_kg_schema`.
  - Transaction-safe `upsert_scientific_kg` for PageIndex, SemanticChunk, EvidencePath, Claim, ScientificEntity, and ScientificRelation fixture records.
  - Contract tests that S06/S07/S10 can extend for retrieval and traversal fixtures.
requires:
  - slice: S02
    provides: PageIndexDocument and PageIndexNode IDs consumed by SCI KG persistence.
  - slice: S03
    provides: SemanticChunk and EvidencePath contracts consumed by SCI KG persistence.
  - slice: S04
    provides: ExtractionPatch, Claim, ScientificEntity, and ScientificRelation contracts consumed by SCI KG persistence.
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
  - Patch-embedded claim/entity/relation EvidencePath references must be present in the persisted `evidence_paths` list.
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

**LadybugDB now stores the expanded SCI KG fixture schema idempotently, transaction-safely, and with enforced EvidencePath membership.**

## What Happened

S05 expanded the LadybugDB graph persistence layer from paper/author/keyword/category records into a scientific KG fixture schema. The implementation adds idempotent schema creation for PageIndexNode, SemanticChunk, EvidencePath, Claim, ScientificEntity, and ScientificRelation nodes plus relationship tables for document structure, chunk attachment, evidence anchoring, and scientific relation endpoints. `upsert_scientific_kg()` validates document/chunk/evidence/patch consistency before writes, uses explicit transactions and parameterized MERGE statements, and rolls back on mid-write failures.

A post-review regression check found a traceability gap: a patch could embed claim/entity/relation EvidencePath references that were not present in the persisted `evidence_paths` list. S05 now rejects that payload before opening a write transaction. The regression test proves `evidence_paths=[]` with a patch that references evidence raises `ValueError` before `BEGIN TRANSACTION`, preventing ungrounded Claim/ScientificEntity/ScientificRelation records.

The S05 roadmap metadata was also repaired through `gsd_plan_milestone` so the completed roadmap entry preserves the real title and dependencies: `LadybugDB SCI KG schema expansion` with `depends:[S02,S03,S04]`. Milestone success criteria and boundary map were restored in the same GSD repair.

## Verification

Fresh post-review verification after the last code and GSD metadata changes passed: 42 pytest tests, Ruff on touched files, Pyrefly on src, Ty on src plus S05 test, CLI help smoke, LSP diagnostics, and GitNexus detect_changes review. GitNexus reported high scope because the full S05 LadybugDB persistence expansion remains uncommitted and affects the persistence surface; this is expected for S05 and covered by the verification set.

## Requirements Advanced

- R019 — Provides storage substrate required by S06 hybrid retrieval and S10 graph traversal, while preserving S07 evaluation gating.
- R020 — Maintains the requirement that evaluation and metrics still precede DSPy/RLM quality claims.

## Requirements Validated

- R018 — S05 post-review verification passed: 42 pytest tests, Ruff, Pyrefly, Ty, CLI help smoke, LSP diagnostics, GitNexus change review, and explicit regression coverage for rejecting patch evidence not included in persisted EvidencePath records before transaction open.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

- **Health signal**: SCI KG schema helpers and write path log schema readiness and per-paper record counts.
- **Failure signal**: Invalid payloads raise aggregated `ValueError` diagnostics before transaction; write failures log paper id and phase then roll back.
- **Recovery**: Idempotent MERGE-based writes can be rerun for fixture payloads after correcting invalid inputs.
- **Monitoring gaps**: No production corpus monitoring yet; S05 remains fixture-level persistence only.

## Deviations

The final type gate exposed pre-existing PageIndex annotation imprecision, so S05 included a small type-only `_HeadingSection` cleanup in `src/arxiv_archive/page_index.py`. Post-review, S05 added one validation guard requiring patch-embedded EvidencePath references to be present in the persisted `evidence_paths` list before any write transaction opens. Runtime PageIndex behavior was unchanged and covered by PageIndex/evidence tests.

## Known Limitations

Fixture-level persistence only. No hybrid retrieval/fusion scoring, no benchmark metrics, no DSPy typed LM modules, no RLM workflow/traversal, and no production corpus migration.

## Follow-ups

S06 should build the hybrid retrieval baseline against the persisted SCI KG fixture schema. S07 still must define evaluation metrics before any DSPy/RLM/optimizer quality claims.

## Files Created/Modified

- `src/arxiv_archive/ladybug_client.py` — Added idempotent base/scientific schema helpers, `upsert_scientific_kg`, deterministic EvidencePath IDs, validation-before-transaction, EvidencePath membership validation, rollback-on-failure, and SCI KG MERGE writes.
- `src/arxiv_archive/page_index.py` — Added `_HeadingSection` typed shape to make PageIndex heading parsing type-check cleanly without changing runtime behavior.
- `tests/test_ladybug_scientific_kg.py` — Added S05 contract/integration tests for schema creation, idempotent fixture persistence, invalid patch rejection before transaction, EvidencePath membership validation, and rollback on mid-write failure.

## Forward Intelligence

### What the next slice should know
- `upsert_scientific_kg()` now guarantees persisted claims/entities/relations cannot reference EvidencePath objects omitted from the persisted evidence path list.
- S06 can rely on `EvidencePath` nodes and `EVIDENCED_BY` edges for traceability in fixture retrieval tests.

### What's fragile
- LadybugDB path-list fields are slash-delimited strings rather than array properties; S06 retrieval should treat them as compatibility storage, not rich path objects.
- The current schema is fixture-level and in-memory-test verified, not production migration verified.

### Authoritative diagnostics
- `tests/test_ladybug_scientific_kg.py` — best first stop for storage contract expectations.
- `uv run pytest tests/test_ladybug_scientific_kg.py -q` — fastest focused verification for S05 storage behavior.

### What assumptions changed
- Original assumption: validating the separate EvidencePath list and validating embedded patch evidence independently was sufficient. Actual result: persistence must also verify embedded draft evidence references are included in the persisted EvidencePath list.
