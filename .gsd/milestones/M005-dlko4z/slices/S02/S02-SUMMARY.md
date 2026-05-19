---
id: S02
parent: M005-dlko4z
milestone: M005-dlko4z
provides:
  - A reproducible baseline measurement tool and artifacts for comparing S03 structure-aware chunking.
  - A clear no-go boundary: 345 current chunks measured, 0 import-ready chunks, no production writes.
  - Review samples showing where structure-aware chunking must improve lineage and route typing.
requires:
  []
affects:
  - S03 — Structure-aware chunk construction should use S02 baseline distributions and review samples as before/after evidence.
  - S04+ — KG import rehearsal remains blocked until improved chunks pass contract and review gates.
key_files:
  - src/arxiv_archive/chunk_baseline_measurement.py
  - tests/test_chunk_baseline_measurement.py
  - .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json
  - .gsd/milestones/M005-dlko4z/slices/S02/review/baseline-review-samples.md
  - .gsd/milestones/M005-dlko4z/slices/S02/baseline-chunk-quality-report.md
  - .gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-review-summary.md
key_decisions:
  - All current baseline chunks are classified as `retrieval_only` / `ok_for_retrieval_only`, not import-ready.
  - Machine artifacts remain redacted; bounded snippets are restricted to markdown review artifacts.
  - S02 is a baseline measurement and no-go import gate, not an implementation of improved chunking.
patterns_established:
  - Use explicit refusal reasons for baseline no-go evidence, not count-only summaries.
  - Separate bounded human-review snippets from redacted machine logs.
  - Treat independent artifact review as required evidence for semantic reports.
observability_surfaces:
  - baseline-summary.json aggregate counts and safety flags
  - baseline-package-diagnostics.jsonl per-paper redacted diagnostics
  - review-sample-index.json redacted review coverage
  - baseline-chunk-quality-report.md final go/no-go summary
  - baseline-review-summary.md independent review evidence
drill_down_paths:
  - .gsd/milestones/M005-dlko4z/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M005-dlko4z/slices/S02/tasks/T02-SUMMARY.md
  - .gsd/milestones/M005-dlko4z/slices/S02/tasks/T03-SUMMARY.md
  - .gsd/milestones/M005-dlko4z/slices/S02/tasks/T04-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-19T06:39:54.238Z
blocker_discovered: false
---

# S02: Baseline chunk quality measurement

**Measured current chunking against the S01 contract and confirmed it is retrieval-only baseline, not KG-import-ready.**

## What Happened

S02 built and exercised a reproducible baseline measurement path for current chunking. T01 mapped current full-text/PageIndex/SemanticChunk output into S01 contract-shaped packages while conservatively marking chunks retrieval-only. T02 ran that baseline over the ten-paper gold corpus and tightened the summary so refusal reasons and route/state/type distributions are explicit. T03 generated bounded markdown review samples for the six-paper inner review set while keeping the machine sample index redacted. T04 wrote the baseline quality report and passed independent review. The slice proves the current baseline is useful for retrieval diagnostics but not for KG import: all 345 measured chunks are retrieval-only, zero are import-eligible, and no production KG writes or import-readiness claims were made.

## Verification

Fresh verification passed: `uv run pytest tests/test_chunk_baseline_measurement.py tests/test_chunk_import_contract.py -q` produced 24 passed, T04 report/review artifacts exist, and ruff reported all checks passed. Independent review returned PASS with no required fixes.

## Requirements Advanced

- R029 — S02 measured current chunks against the S01 import-ready typed chunk package contract and confirmed the existing baseline does not yet satisfy import readiness.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

T02 required a small reporting improvement because the first baseline output had `refused_chunk_count=345` but empty `refusal_counts`. The builder was tightened to report `baseline_retrieval_only_not_import_ready` and route/state/type counts before committing the baseline evidence.

## Known Limitations

The baseline uses current PageIndex/SemanticChunk output. It proves current chunks are retrieval-only and not KG-import-ready; it does not validate semantic retrieval, production persistence, claim extraction, entity extraction, relation extraction, table extraction, citation graph construction, metadata graph construction, or broad corpus scaling.

## Follow-ups

S03 should implement deterministic structure-aware chunking and then rerun the S02 measurement/report path to compare against this baseline. Keep KG import and corpus scaling blocked until later dry-run/review evidence passes.

## Files Created/Modified

- `src/arxiv_archive/chunk_baseline_measurement.py` — Baseline package measurement, aggregate summary, review sample generation, and CLI support.
- `tests/test_chunk_baseline_measurement.py` — Focused coverage for retrieval-only baseline classification, missing full-text blockers, redacted diagnostics, and markdown-vs-machine sample separation.
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json` — Gold-corpus baseline run summary showing 345 retrieval-only chunks and zero import-ready chunks.
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl` — Per-paper redacted baseline diagnostics JSONL.
- `.gsd/milestones/M005-dlko4z/slices/S02/review/baseline-review-samples.md` — Bounded human-review sample markdown for the six-paper inner review set.
- `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/review-sample-index.json` — Redacted machine index for review sample coverage.
- `.gsd/milestones/M005-dlko4z/slices/S02/baseline-chunk-quality-report.md` — Final S02 report and independent review summary.
