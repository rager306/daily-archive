---
id: T01
parent: S04
milestone: M003-km5fty
key_files:
  - tests/test_scientific_extraction_contracts.py
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-17T17:50:40.672Z
blocker_discovered: false
---

# T01: Added red contract tests for S04 Claim, ScientificEntity, ScientificRelation, and ExtractionPatch APIs.

**Added red contract tests for S04 Claim, ScientificEntity, ScientificRelation, and ExtractionPatch APIs.**

## What Happened

Added S04 T01 red contract tests defining the expected public scientific extraction API: Claim, ScientificEntity, ScientificRelation, ExtractionPatch, validate_claim, and validate_extraction_patch. The tests consume the deterministic S03 EvidencePath fixture path and assert storage-ready fields for IDs, paper IDs, confidence, evidence paths, schema/extractor versions, provenance, and validation diagnostics. The negative cases cover missing evidence, invalid confidence, missing versions/provenance, invalid relation endpoint, and paper mismatch. No DSPy, LLM, embeddings, retrieval, or LadybugDB behavior was introduced.

## Verification

`uv run pytest tests/test_scientific_extraction_contracts.py -q` failed as expected with `ModuleNotFoundError: No module named 'arxiv_archive.scientific_extraction'`. `uv run pytest tests/test_evidence_paths.py -q` passed with 9 tests, confirming the S03 substrate remains green.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_scientific_extraction_contracts.py -q` | 2 | ✅ expected red: missing arxiv_archive.scientific_extraction module | 3200ms |
| 2 | `uv run pytest tests/test_evidence_paths.py -q` | 0 | ✅ pass: 9 passed | 3000ms |

## Deviations

None.

## Known Issues

The new test module intentionally fails to import `arxiv_archive.scientific_extraction`; T02 is planned to implement it.

## Files Created/Modified

- `tests/test_scientific_extraction_contracts.py`
