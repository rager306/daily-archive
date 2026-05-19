---
id: T03
parent: S04
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/chunk_import_contract.py
  - tests/test_structure_aware_chunking.py
key_decisions:
  - Annotation sidecar contract validation now rejects nested forbidden raw, embedding, vector, secret, and optimizer field names using redacted field-path diagnostics.
  - Annotation metadata cannot create import eligibility; eligibility remains derived from chunk state, route, allowed uses, excluded uses, source spans, and evidence paths.
duration: 
verification_result: passed
completed_at: 2026-05-19T08:33:14.261Z
blocker_discovered: false
---

# T03: Validated annotation sidecar contract boundaries and blocked nested raw-text leakage.

**Validated annotation sidecar contract boundaries and blocked nested raw-text leakage.**

## What Happened

Added annotation boundary tests for unresolved chunk references, promoted facts, nested raw-text leakage, and annotation values that try to imply trusted import eligibility. The nested leakage test exposed that the contract validator only checked top-level annotation fields. Fixed the validator with recursive forbidden-field detection that reports safe field paths rather than raw values. The tests confirm sidecars remain non-facts, unresolved annotation chunk references are rejected, nested raw text is blocked, and annotation values do not authorize KG import.

## Verification

Fresh verification after the last edit passed: focused structure-aware and import-contract tests passed, and ruff reported all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunk_import_contract.py src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py` | 0 | ✅ pass — 38 passed; ruff all checks passed | 4800ms |

## Deviations

T03 expected tests only, but the negative raw-text leakage test exposed a real contract-validator gap; `src/arxiv_archive/chunk_import_contract.py` was updated to detect forbidden field names nested inside annotation values/warnings without logging raw values.

## Known Issues

Recursive redaction currently may report the same nested leak at both package and object scope; diagnostics remain safe and redacted, but a future cleanup could de-duplicate findings if needed.

## Files Created/Modified

- `src/arxiv_archive/chunk_import_contract.py`
- `tests/test_structure_aware_chunking.py`
