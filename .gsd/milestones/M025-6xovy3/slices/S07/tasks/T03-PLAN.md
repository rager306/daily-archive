---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Finalize evidence boundary report

Validate the separated evidence artifacts and write the S07 report. The report must summarize per-article counts, missing/unsupported evidence diagnostics, provenance coverage, redaction checks, and no-import/no-write safety state.

## Inputs

- None specified.

## Expected Output

- `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-report.md`
- `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-summary.json`

## Verification

uv run python scripts/verify_m025_evidence_boundaries.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --evidence data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence --events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-events.jsonl --require-redaction --require-no-import-flags --write-summary data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-summary.json --write-report data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-report.md

## Observability Impact

Produces the handoff evidence summary S08 uses for final replay readiness and milestone validation.
