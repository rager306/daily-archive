---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Finalized the S08 replay report and readiness decision, classifying all five articles as blocked by missing baseline while preserving no-network and no-write safety evidence.

Write the final S08 report and machine-readable readiness decision. The report must compare final outputs against the baseline, classify behaviors as preserved/improved/regressed/blocked, summarize diagnostics, and explicitly state that M025 makes no graph readiness claim.

## Inputs

- None specified.

## Expected Output

- `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json`
- `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-report.md`
- `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json`

## Verification

uv run python scripts/verify_m025_final_preprocessing_replay.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --baseline data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline --final data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay --events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-events.jsonl --require-no-network --require-no-import-flags --write-summary data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json --write-report data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-report.md --write-decision data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json

## Observability Impact

Creates milestone validation input with readiness blockers, no-network proof, no-write proof, and per-article comparison outcomes.
