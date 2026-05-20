---
id: T03
parent: S02
milestone: M009-fh0tg0
key_files:
  - .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json
  - .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json
  - .gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/sample-cli-run-log.jsonl
key_decisions:
  - S02 evidence includes both a verifier pass report and an intentional stale-output failure report.
  - The stale verification command is expected to exit 1 and is treated as a pass for the negative test.
duration: 
verification_result: passed
completed_at: 2026-05-20T04:46:12.189Z
blocker_discovered: false
---

# T03: Generated freshness verifier pass/fail sample reports and ran focused regression.

**Generated freshness verifier pass/fail sample reports and ran focused regression.**

## What Happened

Generated S02 verifier sample artifacts. A fresh sample provenance log verified successfully and wrote `freshness-pass-report.json`; then the output was intentionally mutated and the verifier failed with exit code 1 while writing `freshness-stale-report.json`. Focused regression tests and ruff passed.

## Verification

Fresh and stale sample reports exist. Provenance, CLI freshness, and CLI contract tests passed, and ruff passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `generate sample provenance; verify fresh; mutate output; verify stale expected exit 1; uv run pytest tests/test_validation_batch_provenance.py tests/test_validation_batch_cli_freshness.py tests/test_validation_batch_cli_contract.py -q; uv run ruff check ...` | 0 | ✅ pass — 20 tests passed; ruff passed; pass/stale reports present | 5900ms |

## Deviations

None.

## Known Issues

S02 sample uses synthetic files. Real validation-batch commands still need provenance-log emission before the verifier can audit actual init/preflight/scan runs automatically.

## Files Created/Modified

- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/sample-cli-run-log.jsonl`
