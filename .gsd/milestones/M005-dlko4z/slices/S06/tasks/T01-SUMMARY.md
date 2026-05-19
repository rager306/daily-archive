---
id: T01
parent: S06
milestone: M005-dlko4z
key_files:
  - src/arxiv_archive/chunking_benchmark.py
  - tests/test_chunking_benchmark.py
key_decisions:
  - S06 benchmark artifacts are dry-run comparison artifacts only; they cannot authorize KG import, embeddings, optimizer behavior, or production writes.
  - Benchmark validation checks both forbidden field names and unsafe safety flags at run, method, and per-paper levels.
duration: 
verification_result: passed
completed_at: 2026-05-19T10:41:10.532Z
blocker_discovered: false
---

# T01: Defined the redacted chunking benchmark contract and validator.

**Defined the redacted chunking benchmark contract and validator.**

## What Happened

Defined the S06 chunking benchmark contract with method-level and per-paper metric dataclasses, aggregate metric merging, redaction/no-write flags, and a validator. The contract captures route/type/state/refusal counts, source span coverage, parent reference resolution, annotation coverage, asset-linkage coverage, import eligibility, missing-source caveats, and recommendation status. Tests cover serialization, aggregation, missing fields, invalid ranges, nested raw/embedding leakage, and unsafe flags.

## Verification

Fresh verification after the final edit passed: chunking benchmark, source-asset, structure-aware, and import-contract tests passed, and ruff reported all checks passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_chunking_benchmark.py tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q && uv run ruff check src/arxiv_archive/chunking_benchmark.py tests/test_chunking_benchmark.py` | 0 | ✅ pass — 60 passed; ruff all checks passed | 6300ms |

## Deviations

None.

## Known Issues

T01 defines the benchmark contract only. Adapters, gold-corpus run artifacts, review samples, and independent benchmark review remain for T02-T05.

## Files Created/Modified

- `src/arxiv_archive/chunking_benchmark.py`
- `tests/test_chunking_benchmark.py`
