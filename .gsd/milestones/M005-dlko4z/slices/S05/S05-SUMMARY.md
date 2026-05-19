---
id: S05
parent: M005-dlko4z
milestone: M005-dlko4z
provides:
  - Redacted source asset manifest contract and validator
  - Deterministic preservation of available gold-corpus Markdown/PDF files with SHA-256 metadata
  - Per-paper source/asset manifests linking S04 sidecar candidates to S03 source spans
  - Independent PASS review and S06 consumption guidance
requires:
  - slice: S04
    provides: S04 annotation sidecars and per-chunk coverage used to create asset-link records.
  - slice: S03
    provides: S03 source spans consumed for asset source-span linkage.
affects:
  - S06
  - S07
key_files:
  - src/arxiv_archive/source_asset_manifest.py
  - tests/test_source_asset_manifest.py
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/manifests
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/papers
  - .gsd/milestones/M005-dlko4z/slices/S05/source-asset-preservation-report.md
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-review-summary.md
key_decisions:
  - Source asset manifests serialize paths, hashes, provenance, spans, and linkage metadata only; no raw text/binary/base64/embeddings are serialized.
  - Missing PDFs are explicit diagnostics, not opaque failures.
  - Asset records are `linked_not_extracted` and non-factual; they are excluded from trusted KG import, production LadybugDB writes, and embedding generation.
  - S06 can consume S05 manifests as diagnostic/source-review input only.
patterns_established:
  - Preserve raw/source artifacts as files; machine artifacts store only paths, hashes, provenance, spans, linkage, counts, and safety flags.
  - Treat asset records as linked-not-extracted candidates until a later extraction/review gate authorizes more.
  - Independent review must check source/hash coverage and missing-source clarity, not just schema validity.
observability_surfaces:
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json — run-level source counts, asset counts, missing-source diagnostics, extraction states, and safety flags
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl — per-paper source hash/size/media metadata and warning counts
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/manifests/*.json — per-paper source files and linked asset records
  - .gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-review-summary.md — independent review PASS and S06 consumption assessment
drill_down_paths:
  - .gsd/milestones/M005-dlko4z/slices/S05/tasks/T01-SUMMARY.md
  - .gsd/milestones/M005-dlko4z/slices/S05/tasks/T02-SUMMARY.md
  - .gsd/milestones/M005-dlko4z/slices/S05/tasks/T03-SUMMARY.md
  - .gsd/milestones/M005-dlko4z/slices/S05/tasks/T04-SUMMARY.md
  - .gsd/milestones/M005-dlko4z/slices/S05/tasks/T05-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-19T10:31:19.095Z
blocker_discovered: false
---

# S05: Source asset preservation and multimodal manifest

**Source files and multimodal asset candidates are now preserved as redacted, hash-backed, non-fact manifests for benchmark review.**

## What Happened

S05 implemented source artifact preservation and redacted multimodal asset manifests. T01 defined the source asset manifest contract and validator. T02 copied available normalized Markdown/PDF files into deterministic per-paper workspaces and recorded hashes/provenance. T03 mapped S04 annotation diagnostics and S03 source spans into non-fact asset records for table, figure, equation, reference, and metadata candidates. T04 ran the full 10-paper dry-run and wrote per-paper manifests, run summaries, and JSONL diagnostics. T05 independently reviewed the artifacts and documented S06-safe consumption boundaries. Final artifacts cover 10 valid manifests, 12 preserved source files, hash coverage 1.0, explicit `missing_original_pdf=8`, and 283 linked-not-extracted asset records with zero promoted facts and all no-write/no-raw/no-embedding safety flags false.

## Verification

Fresh slice verification passed after final T05 commit: 53 focused tests passed, ruff passed, S05 run-evidence/report artifacts are non-empty, artifact guard confirmed 10 valid manifests, 12 source files, hash coverage 1.0, 8 missing PDFs diagnosed, 283 linked assets, zero promoted facts, review PASS, and all safety flags false.

## Requirements Advanced

- R030 — S05 preserves source artifacts and asset-link manifests with stable ids, hashes, provenance, source spans, and redaction/no-write safety flags.
- R029 — S05 strengthens the import-ready package evidence stack by providing preserved source context and asset candidate linkage for future benchmark/review gates.

## Requirements Validated

- R030 — S05 dry-run produced 10 valid per-paper manifests, preserved 12 available source files with hash coverage 1.0, surfaced 8 missing PDFs as diagnostics, linked 283 asset candidates, kept all raw/binary/base64/embedding/write flags false, and passed independent artifact review.

## New Requirements Surfaced

- None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

T04 added a small reusable `build_source_asset_run()` helper to make the full dry-run reproducible. Eight original PDFs were missing in current local source paths and are recorded as redacted diagnostics instead of treated as fatal failures.

## Known Limitations

S05 preserves available source files and links asset candidates, but does not extract figures/tables/equations/references as standalone content, perform OCR/table recovery, generate embeddings, validate multimodal retrieval, authorize KG import, or write to LadybugDB. Eight original PDFs remain missing from current local source paths.

## Follow-ups

S06 should benchmark chunking methods using these manifests as diagnostic/source-review inputs, join asset records to source files by `source_file_id` for hash context, and compare table/figure/equation/reference linkage quality. Missing PDFs should remain visible as a benchmark caveat or targeted acquisition/repair input.

## Files Created/Modified

- `src/arxiv_archive/source_asset_manifest.py` — Added source asset manifest dataclasses, validation, source preservation, sidecar-to-asset linkage, dry-run builder, and artifact writer.
- `tests/test_source_asset_manifest.py` — Added contract, preservation, linkage, dry-run, redaction, non-fact, and artifact tests.
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json` — Run-level source/asset summary with source counts, asset counts, missing-source diagnostics, and safety flags.
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl` — Per-paper diagnostics with source file hash/size/media metadata and asset counts.
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/manifests/*.json` — Per-paper manifests containing preserved source file records and linked-not-extracted asset records.
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/papers/*/source/` — Preserved source files copied into deterministic per-paper workspaces.
- `.gsd/milestones/M005-dlko4z/slices/S05/source-asset-preservation-report.md` — Final source preservation report and S06 guidance.
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-review-summary.md` — Independent artifact review summary with PASS verdict.
