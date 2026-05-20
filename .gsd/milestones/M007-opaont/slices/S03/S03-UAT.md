# S03: Automated scan delta and outlier gates — UAT

**Milestone:** M007-opaont
**Written:** 2026-05-20T01:57:42.918Z

# S03: Automated scan delta and outlier gates — UAT

## Smoke Test

Run `validation-batch scan` over the S02 batch state with M005/S03 and M005/S06 baselines.

Expected final artifacts:

- `validation-scan-summary.json`
- `validation-scan-diagnostics.jsonl`
- `delta-report.json`
- `outlier-report.json`
- `batch-state.json`

Expected summary:

- `paper_count=30`
- `chunk_count=4289`
- `import_eligible_chunk_count=0`
- `outlier_count=11`
- `structure_delta=2458`
- `mixed_delta=1818`
- all safety flags false

## Not proven

- New +10 batch selection.
- Semantic correctness of candidates.
- Trusted KG import readiness.
- Production LadybugDB writes.
