# S08: End to End Preprocessing Replay

**Goal:** Run the complete refactored preprocessing pipeline end to end on the fixed 5 article local corpus and produce final comparison, readiness, and blocker reports.
**Demo:** After this: the full refactored preprocessing pipeline runs on the same smoke corpus, writes final per-article artifacts, compares against baseline, and states whether larger preprocessing validation is ready or blocked.

## Must-Haves

- Full refactored preprocessing pipeline runs locally from catalog/corpus inputs.
- Final per-article artifacts and metrics are persisted.
- Comparison report explains preserved, improved, regressed, and blocked behaviors.
- No network fetch occurs during replay.
- No graph import, production LadybugDB write, or graph readiness acceptance is claimed.

## Proof Level

- This slice proves: End to end local replay proof over the fixed 5 article corpus.

## Integration Closure

Milestone validation consumes this final replay report to decide whether preprocessing readiness is sufficient for a later larger validation milestone.

## Verification

- Writes final run summary, baseline comparison, diagnostics, readiness blockers, no-network proof, and no-write safety evidence.

## Tasks

- [x] **T01: Define final replay contract** `est:medium`
  Define and test the final end-to-end replay contract for the fixed 5 article corpus. The contract must require catalog/index/selection inputs, no-network execution, per-article final artifact references, baseline comparison categories, and no graph import or production write flags.
  - Files: `tests/test_article_preprocessing_replay_contract.py`, `tests/fixtures/article_preprocessing_replay_v00_01/`
  - Verify: uv run pytest tests/test_article_preprocessing_replay_contract.py -q
uv run ruff check tests/test_article_preprocessing_replay_contract.py

- [x] **T02: Run final local preprocessing replay** `est:medium`
  Implement or adapt the final local preprocessing replay command. It must read `data/article_catalog/catalog.json`, `data/article_catalog/index.json`, and the M025 corpus selection; it must reuse local artifacts from earlier slices; it must fail if a network fetch would be required during replay; and it must write final per-article artifacts and metrics.
  - Files: `src/arxiv_archive/`, `scripts/verify_m025_final_preprocessing_replay.py`, `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/`
  - Verify: uv run python scripts/verify_m025_final_preprocessing_replay.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --baseline data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline --final data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay --write-events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-events.jsonl --no-network

- [ ] **T03: Finalize preprocessing replay report** `est:small`
  Write the final S08 report and machine-readable readiness decision. The report must compare final outputs against the baseline, classify behaviors as preserved/improved/regressed/blocked, summarize diagnostics, and explicitly state that M025 makes no graph readiness claim.
  - Files: `scripts/verify_m025_final_preprocessing_replay.py`, `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/`
  - Verify: uv run python scripts/verify_m025_final_preprocessing_replay.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --baseline data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline --final data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay --events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-events.jsonl --require-no-network --require-no-import-flags --write-summary data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json --write-report data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-report.md --write-decision data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json

## Files Likely Touched

- tests/test_article_preprocessing_replay_contract.py
- tests/fixtures/article_preprocessing_replay_v00_01/
- src/arxiv_archive/
- scripts/verify_m025_final_preprocessing_replay.py
- data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/
