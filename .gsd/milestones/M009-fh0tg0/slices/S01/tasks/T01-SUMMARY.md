---
id: T01
parent: S01
milestone: M009-fh0tg0
key_files:
  - src/arxiv_archive/validation_batch_provenance.py
key_decisions:
  - Implement provenance as a new isolated module rather than extending ValidationBatchState or changing CLI behavior in S01.
  - Use sha256+size as authoritative freshness signals; mtime is recorded for context only.
  - Do not capture stdout/stderr content in S01; record paths as optional metadata only.
duration: 
verification_result: passed
completed_at: 2026-05-20T04:37:00.458Z
blocker_discovered: false
---

# T01: Implemented isolated validation-batch provenance and freshness primitives.

**Implemented isolated validation-batch provenance and freshness primitives.**

## What Happened

Created `validation_batch_provenance.py` with commit-safe hashing, fingerprinting, argv redaction, git commit lookup, provenance JSONL append/read, entry selection, and freshness report generation. The implementation hashes file bytes without storing contents, redacts secret-like CLI flag values, preserves standard validation safety flags, and reports stale/missing/unsafe provenance through diagnostics.

## Verification

Smoke check passed: fingerprint_file returns sha256 metadata without raw content, and redact_cli_args removes secret-like values.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python - <<'PY' ... fingerprint_file/redact_cli_args smoke ... PY` | 0 | ✅ pass — provenance-primitives-ok | 11600ms |

## Deviations

None.

## Known Issues

CLI commands are not wired to emit provenance yet; S01 only provides library primitives and sample artifacts. CLI integration is deferred to later slices.

## Files Created/Modified

- `src/arxiv_archive/validation_batch_provenance.py`
