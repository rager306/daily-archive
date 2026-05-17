---
estimated_steps: 1
estimated_files: 1
skills_used: []
---

# T01: Add claim entity relation contract tests

Create red contract tests for `Claim`, `ScientificEntity`, `ScientificRelation`, and extraction patch/draft models over S03 EvidencePath fixtures. Tests must define stable IDs, confidence fields, provenance, evidence-path references, schema/extractor version fields, and validation diagnostics for missing evidence and invalid confidence. Done when tests fail for missing extraction-contract implementation while S03 evidence tests still pass.

## Inputs

- `.gsd/milestones/M003-km5fty/slices/S03/S03-SUMMARY.md`
- `src/arxiv_archive/evidence.py`
- `tests/test_evidence_paths.py`

## Expected Output

- `tests/test_scientific_extraction_contracts.py`

## Verification

uv run pytest tests/test_scientific_extraction_contracts.py -q

## Observability Impact

Defines expected extraction validation diagnostics before implementation, including evidence reference, confidence, schema version, and provenance fields.
