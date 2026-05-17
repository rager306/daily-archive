---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Implement relation and extraction patch validators

Add relation and patch validators that reject invalid relation endpoints, unsupported relation types, paper mismatches between entities/claims/evidence, and evidence paths that fail S03 validation. Add tests for valid fixture drafts and invalid endpoint/mismatch cases. Done when S05 can use the patch contract as a pre-storage validation boundary.

## Inputs

- `src/arxiv_archive/scientific_extraction.py`
- `src/arxiv_archive/evidence.py`
- `tests/test_scientific_extraction_contracts.py`

## Expected Output

- `src/arxiv_archive/scientific_extraction.py`
- `tests/test_scientific_extraction_contracts.py`

## Verification

uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py -q

## Observability Impact

Patch validation reports relation endpoint, evidence path, paper mismatch, unsupported relation type, and duplicate ID diagnostics as data.
