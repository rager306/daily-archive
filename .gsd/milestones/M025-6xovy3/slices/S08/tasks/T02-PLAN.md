---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T02: Run final local preprocessing replay

Implement or adapt the final local preprocessing replay command. It must read `data/article_catalog/catalog.json`, `data/article_catalog/index.json`, and the M025 corpus selection; it must reuse local artifacts from earlier slices; it must fail if a network fetch would be required during replay; and it must write final per-article artifacts and metrics.

## Inputs

- None specified.

## Expected Output

- `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay/`
- `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-events.jsonl`

## Verification

uv run python scripts/verify_m025_final_preprocessing_replay.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --baseline data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline --final data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay --write-events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-events.jsonl --no-network

## Observability Impact

Persists per-article final phase, artifact paths, metric counts, diagnostics, no-network evidence, and no-write safety state.
