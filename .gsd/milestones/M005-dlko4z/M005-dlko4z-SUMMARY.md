---
id: M005-dlko4z
title: "Chunking Import Model Deepening"
status: complete
completed_at: 2026-05-19T12:43:32.207Z
key_decisions:
  - Current M005 outputs improve observability and safety but do not authorize positive KG import.
  - Zero import eligibility is an explicit milestone finding, not an execution failure.
  - Future positive import requires a reviewed non-zero import-eligible subset.
key_files:
  - src/arxiv_archive/chunk_import_contract.py
  - src/arxiv_archive/structure_aware_chunking.py
  - src/arxiv_archive/source_asset_manifest.py
  - src/arxiv_archive/chunking_benchmark.py
  - src/arxiv_archive/import_boundary_rehearsal.py
  - .gsd/milestones/M005-dlko4z/M005-dlko4z-VALIDATION.md
  - .gsd/milestones/M005-dlko4z/slices/S06/run-evidence/chunking-benchmark-summary.json
  - .gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-summary.json
lessons_learned:
  - Aggregate counts alone are insufficient for semantic review; redacted chunk/candidate-level diagnostics are needed.
  - A negative import rehearsal is the correct safety proof when benchmark evidence shows zero import eligibility.
  - Source/asset preservation should expose missing artifacts as diagnostics, not hide them or fail opaque downstream gates.
---

# M005-dlko4z: Chunking Import Model Deepening

**M005 delivered a typed chunk/import evidence model and proved current candidates are safely rejected before KG import, while positive import remains blocked.**

## What Happened

M005 deepened the daily-archive chunk/import model from generic retrieval chunks into a typed, traceable, reviewable package model. The milestone defined the import contract, selected a gold corpus, measured the baseline, implemented deterministic structure-aware chunking, added sidecar annotations, preserved source and asset manifests, benchmarked three current methods, and rehearsed the import boundary negatively. The final result is intentionally conservative: structure, annotations, source spans, asset linkage, benchmark diagnostics, and import-boundary safety are now observable; however, all 2,471 benchmarked candidates remain refused and positive trusted KG import remains blocked. S07 proves that this blocker is enforced safely before production writes.

## Success Criteria Results

## Success criteria results

- ✅ Import-ready chunk model is defined, implemented, and versioned.
- ✅ Representative benchmark documents improved observability over baseline and the blocker: zero import eligibility.
- ✅ Only eligible chunks/routes can proceed; current candidates all remain rejected.
- ✅ Independent reviews confirmed semantic artifact quality and blocked over-claims.
- ✅ Production KG writes remain blocked; S07 has zero accepted imports and no writes.

## Definition of Done Results

## Definition of done results

- ✅ All seven slices complete.
- ✅ Full verification passed: 362 tests, ruff clean.
- ✅ S06 benchmark evidence documents zero import eligibility rather than over-claiming readiness.
- ✅ S07 negative rehearsal proves safe rejection with zero accepted imports and no production writes.
- ✅ Independent reviews are recorded for semantic artifacts and final import-boundary scope.
- ✅ Production KG writes, embeddings, broad scaling, positive import, semantic/vector retrieval, and asset-to-fact promotion remain blocked.

## Requirement Outcomes

## Requirement outcomes

- R029 — Advanced and validated for the negative boundary: import-ready package contract exists, current candidates are measured, and ineligible chunks are rejected before trusted KG import. Positive import subset remains future work.
- R030 — Advanced and validated for the gold corpus: source artifacts and linked assets are preserved as files/metadata with hashes and provenance; missing PDFs are explicit diagnostics; assets are not KG facts.

## Deviations

S07 was re-scoped from positive isolated import rehearsal to negative import-boundary rehearsal after S06 independently blocked positive import readiness. This preserves the milestone safety boundary and is recorded in the roadmap assessment.

## Follow-ups

Add a future remediation milestone/slice to create a small independently reviewed import-eligible subset, then run a positive isolated import rehearsal only against that subset. Broad KG import, semantic/vector retrieval claims, entity/relation extraction, and production LadybugDB writes remain blocked.
