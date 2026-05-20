# Validation batch scan dry-run report

## Summary

S03 ran the new `validation-batch scan` command over the S02 source-ready 30-paper batch state. The command produced M007-named scan, delta, outlier, and updated batch-state artifacts while reusing the existing redacted deviation scanner.

No source acquisition, conversion, production KG import, or LadybugDB write was performed.

## Evidence

| Metric | Value |
|---|---:|
| Papers scanned | 30 |
| Structure-aware chunks | 4,289 |
| Import-eligible chunks | 0 |
| Outlier papers | 11 |
| M005/S03 structure-aware chunk delta | +2,458 |
| M005/S06 mixed benchmark chunk delta | +1,818 |
| Production import attempted | false |
| LadybugDB written | false |

## Baseline separation

The scan writes two distinct comparison surfaces:

1. `structure_aware_baseline` — M005/S03 apples-to-apples structure-aware baseline.
2. `mixed_benchmark_context` — M005/S06 mixed benchmark context only.

This keeps the M006 review correction intact: S06 is not treated as a direct route-share baseline.

## Route deltas against M005/S03

Largest route-share changes:

- `retrieval_only`: -0.0632
- `method_extraction`: +0.0292
- `citation_graph`: +0.0126
- `claim_extraction`: +0.0114
- `table_extraction`: +0.0105

These are routing/review-prioritization signals only. They do not validate semantic correctness or KG import readiness.

## Import gate

The scan produced:

```text
import_eligible_chunk_count = 0
```

Therefore no import-gate blocker was added. If a future batch produces non-zero import eligibility outside a reviewed promotion path, the workflow will add an `unexpected_import_eligible_chunks` blocker and move the batch to review-required.

## Artifact paths

- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-diagnostics.jsonl`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/delta-report.json`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/outlier-report.json`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/batch-state.json`
- `.gsd/milestones/M007-opaont/slices/S03/run-evidence/scan-response.json`

## Result

S03 proves that M007 can now automate the M006 scan/delta/outlier evidence path through a resumable validation-batch state file and CLI command. The next slice should independently review whether these automated artifacts are useful enough to drive the next +10-paper batch.
