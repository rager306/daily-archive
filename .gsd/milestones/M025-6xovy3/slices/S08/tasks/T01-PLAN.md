---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T01: Define final replay contract

Define and test the final end-to-end replay contract for the fixed 5 article corpus. The contract must require catalog/index/selection inputs, no-network execution, per-article final artifact references, baseline comparison categories, and no graph import or production write flags.

## Inputs

- `.gsd/milestones/M025-6xovy3/M025-6xovy3-ROADMAP.md`

## Expected Output

- `tests/fixtures/article_preprocessing_replay_v00_01/`
- `tests/test_article_preprocessing_replay_contract.py`

## Verification

uv run pytest tests/test_article_preprocessing_replay_contract.py -q
uv run ruff check tests/test_article_preprocessing_replay_contract.py

## Observability Impact

Locks final replay report shape before implementation so validation can distinguish pass, regression, and blocked states without inspecting raw article payloads.
