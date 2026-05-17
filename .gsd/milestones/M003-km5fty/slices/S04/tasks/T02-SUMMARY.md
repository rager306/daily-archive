---
id: T02
parent: S04
milestone: M003-km5fty
key_files:
  - src/arxiv_archive/scientific_extraction.py
  - tests/test_scientific_extraction_contracts.py
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-17T17:52:04.502Z
blocker_discovered: false
---

# T02: Implemented deterministic scientific extraction contract dataclasses and baseline validators.

**Implemented deterministic scientific extraction contract dataclasses and baseline validators.**

## What Happened

Implemented the S04 scientific extraction contract module with frozen dataclasses for Claim, ScientificEntity, ScientificRelation, and ExtractionPatch. Added deterministic ID helper functions and validators for confidence range, evidence presence, schema/extractor versions, provenance, stable ID prefixes, relation endpoints, supported relation types, patch membership, paper mismatches, and evidence-path warnings. No DSPy, LLM, embeddings, retrieval, or LadybugDB behavior was introduced.

## Verification

`uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py -q` passed with 13 tests. `uv run ruff check src/arxiv_archive/scientific_extraction.py tests/test_scientific_extraction_contracts.py` passed with all checks clean.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py -q` | 0 | ✅ pass: 13 passed | 4700ms |
| 2 | `uv run ruff check src/arxiv_archive/scientific_extraction.py tests/test_scientific_extraction_contracts.py` | 0 | ✅ pass: All checks passed | 5200ms |

## Deviations

Implemented baseline relation and patch validation in T02 as part of making the initial contract tests pass; T03 can now deepen edge cases without changing the public contract shape.

## Known Issues

T03 still needs broader relation/patch validator edge cases for unsupported relation types, duplicate IDs, and explicit S03 evidence-path validation failures.

## Files Created/Modified

- `src/arxiv_archive/scientific_extraction.py`
- `tests/test_scientific_extraction_contracts.py`
