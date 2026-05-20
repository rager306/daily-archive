---
id: T02
parent: S03
milestone: M009-fh0tg0
key_files:
  - src/arxiv_archive/validation_batch_provenance.py
  - tests/test_validation_batch_provenance.py
key_decisions:
  - Store expected artifact metadata on provenance entries as `expected_artifact_metadata`.
  - Treat `artifact_metadata_mismatch` as a stale verdict even when file hashes match the current recorded output hash.
duration: 
verification_result: passed
completed_at: 2026-05-20T05:13:40.113Z
blocker_discovered: false
---

# T02: Extended freshness verification to catch artifact milestone/batch metadata mismatches.

**Extended freshness verification to catch artifact milestone/batch metadata mismatches.**

## What Happened

Extended freshness verification with artifact lineage metadata checks. Provenance entries can now carry expected artifact metadata such as `milestone_id` and `batch_id`; the freshness report reads recorded JSON outputs and emits `artifact_metadata_mismatch` diagnostics when values differ. Tests prove both the matching case and a stale M006-style milestone mismatch.

## Verification

Provenance unit tests passed and ruff passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run pytest tests/test_validation_batch_provenance.py -q && uv run ruff check src/arxiv_archive/validation_batch_provenance.py tests/test_validation_batch_provenance.py` | 0 | ✅ pass — 11 tests passed; ruff passed | 8800ms |

## Deviations

None.

## Known Issues

The metadata expectation currently applies to all recorded JSON outputs in a provenance entry; non-JSON outputs with metadata expectations fail as unreadable. This is acceptable for validation-batch JSON artifacts.

## Files Created/Modified

- `src/arxiv_archive/validation_batch_provenance.py`
- `tests/test_validation_batch_provenance.py`
