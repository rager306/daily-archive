---
id: S04
parent: M005-dlko4z
milestone: M005-dlko4z
provides:
  - Deterministic annotation sidecar model and generation path over structure-aware chunks
  - Executable contract-boundary tests for sidecar references, redaction, non-fact status, and import ineligibility
  - Redacted gold-corpus annotation dry-run artifacts with per-chunk sidecar coverage evidence
requires:
  - slice: S03
    provides: Structure-aware package construction, route/state assignment, and redacted chunk-level diagnostics consumed as the annotation substrate.
affects:
  - S05
  - S06
  - S07
key_files:
  - src/arxiv_archive/structure_aware_chunking.py
  - src/arxiv_archive/chunk_import_contract.py
  - tests/test_structure_aware_chunking.py
  - .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-summary.json
  - .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl
  - .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-review-summary.md
key_decisions:
  - Annotations are deterministic sidecars generated from chunk metadata only, not KG facts.
  - Annotation values cannot authorize import eligibility; chunk state/route/allowed uses/evidence remain the only eligibility sources.
  - Nested raw-text leakage inside annotation values is now a contract violation.
  - Semantic artifact review requires per-chunk sidecar coverage evidence, not only aggregate counts.
patterns_established:
  - Generate sidecars from structural metadata only; never inspect raw chunk text for annotation sidecar construction.
  - Semantic evidence artifacts need per-object redacted coverage evidence when aggregate counts are insufficient.
  - Contract validators must check nested annotation values for forbidden raw/vector/secret fields.
observability_surfaces:
  - .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-summary.json — run-level coverage, counts, import/no-write flags, and safety flags
  - .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl — per-paper and per-chunk redacted sidecar coverage
  - .gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-review-summary.md — independent review PASS and semantic evidence notes
drill_down_paths:
  - .gsd/milestones/M005-dlko4z/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M005-dlko4z/slices/S04/tasks/T02-SUMMARY.md
  - .gsd/milestones/M005-dlko4z/slices/S04/tasks/T03-SUMMARY.md
  - .gsd/milestones/M005-dlko4z/slices/S04/tasks/T04-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-19T08:47:22.644Z
blocker_discovered: false
---

# S04: Chunk annotation sidecars

**Deterministic chunk annotation sidecars now exist as redacted, reviewable, non-fact metadata over the gold corpus.**

## What Happened

S04 added deterministic annotation sidecars to the structure-aware chunk package model. T01 defined redacted sidecar serialization with `promoted_to_fact=false`. T02 generated section-role, route-hint, structural-type, review-blocker, and asset-link sidecars from chunk metadata without inspecting or persisting raw chunk text. T03 added contract-boundary tests and fixed nested redaction validation so annotation values cannot hide raw text or other forbidden fields. T04 ran the gold-corpus dry run and, after independent review blocked aggregate-only diagnostics, remediated artifacts with redacted per-chunk annotation coverage evidence. The final artifacts show 10 valid packages, 1,831 chunks, 1,831 annotated chunks, 7,448 annotations, 100% per-chunk coverage, zero promoted facts, zero import-eligible chunks, and all no-write/no-raw/no-embedding safety flags false.

## Verification

Fresh slice verification passed after final artifact remediation and commit: 38 focused tests passed, ruff passed, S04 run-evidence artifacts are non-empty, artifact guard confirmed coverage and safety flags, and independent re-review returned PASS.

## Requirements Advanced

- R029 — S04 adds deterministic annotations, route hints, structural cues, review blockers, source spans through chunks, and sidecar diagnostics to the import-ready package model while keeping import blocked.
- R030 — S04 emits asset-link hints for table/figure chunks that S05 can resolve into preserved source asset manifests without promoting assets to KG facts.

## Requirements Validated

None.

## New Requirements Surfaced

- None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

T03 expected tests only, but a negative raw-text leakage test exposed and fixed a real contract-validator gap in nested annotation values. T04 independent review initially blocked because artifact diagnostics were aggregate-heavy; S04 artifacts were remediated with redacted per-chunk annotation coverage evidence and re-review passed.

## Known Limitations

S04 proves deterministic annotation sidecar coverage and contract boundaries only. It does not validate asset preservation, source file manifests, real chunking library quality, semantic/vector retrieval, claim/entity/relation extraction quality, production KG writes, or broad corpus scaling.

## Follow-ups

S05 must add source asset preservation and multimodal manifests before any asset-link hints can be treated as resolved. S06 must benchmark real chunking methods against these redacted structure/annotation diagnostics. S07 remains blocked until S04/S05/S06 all pass.

## Files Created/Modified

- `src/arxiv_archive/structure_aware_chunking.py` — Added annotation sidecar dataclasses, deterministic metadata-derived generation, annotation diagnostics, and run-summary counts.
- `src/arxiv_archive/chunk_import_contract.py` — Extended contract validation to reject nested forbidden raw/embedding/vector/secret/optimizer fields in annotation values without logging raw values.
- `tests/test_structure_aware_chunking.py` — Added schema, generation, boundary, and artifact tests for annotation sidecars.
- `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-summary.json` — Redacted run-level annotation dry-run evidence with 10 papers, 1,831 chunks, 7,448 annotations, 100% per-chunk coverage, zero promoted facts, and all safety flags false.
- `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl` — Redacted per-paper diagnostics with per-chunk annotation coverage entries.
- `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-review-summary.md` — Independent review summary recording initial blocker, remediation, and final PASS.
