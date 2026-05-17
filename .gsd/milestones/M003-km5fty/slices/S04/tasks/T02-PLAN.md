---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Implement scientific extraction contract models

Implement `src/arxiv_archive/scientific_extraction.py` with dataclasses for `Claim`, `ScientificEntity`, `ScientificRelation`, and `ExtractionPatch`. Add deterministic ID helpers and baseline validators for evidence presence, confidence range, schema version, and provenance. Done when initial extraction contract tests pass with no LLM/DSPy/storage calls.

## Inputs

- `tests/test_scientific_extraction_contracts.py`
- `src/arxiv_archive/evidence.py`

## Expected Output

- `src/arxiv_archive/scientific_extraction.py`

## Verification

uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py -q

## Observability Impact

Validation returns explicit diagnostics for invalid confidence, missing evidence, schema/version omissions, and unstable IDs.
