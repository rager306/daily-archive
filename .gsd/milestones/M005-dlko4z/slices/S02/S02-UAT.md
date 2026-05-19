# S02: Baseline chunk quality measurement — UAT

**Milestone:** M005-dlko4z
**Written:** 2026-05-19T06:39:54.238Z

# UAT — M005/S02 Baseline Chunk Quality Measurement

## Scenario

Given the S01 import-ready chunk contract and the ten-paper gold corpus, run the current `PageIndex → SemanticChunk` baseline and verify that the output is measured honestly without production KG writes or import-readiness overclaims.

## Evidence

- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl`
- `.gsd/milestones/M005-dlko4z/slices/S02/review/baseline-review-samples.md`
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/review-sample-index.json`
- `.gsd/milestones/M005-dlko4z/slices/S02/baseline-chunk-quality-report.md`
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-review-summary.md`

## Acceptance Checks

- Baseline run measured 10 gold-corpus papers.
- 345 chunks were measured.
- 345 chunks were classified as `retrieval_only` / `ok_for_retrieval_only` / `retrieval_context`.
- 0 chunks were import-eligible.
- 0 packages were import-ready.
- `refusal_counts.baseline_retrieval_only_not_import_ready == 345`.
- Machine JSON/JSONL artifacts report no raw text, no embeddings, no production import attempt, and no LadybugDB writes.
- Six-paper inner review set has bounded markdown samples and a redacted machine index.
- Independent review returned PASS with no required fixes.

## Verdict

PASS for S02 baseline measurement. NO-GO for KG import or corpus scaling. Proceed to S03 structure-aware chunk construction only.
