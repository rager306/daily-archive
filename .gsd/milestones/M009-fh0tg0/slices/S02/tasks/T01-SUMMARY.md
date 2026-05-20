---
id: T01
parent: S02
milestone: M009-fh0tg0
key_files:
  - src/arxiv_archive/cli.py
key_decisions:
  - Expose freshness verification as a new additive `validation-batch verify-artifacts` command rather than changing existing init/preflight/scan behavior.
  - Exit 0 only for `fresh`; stale/missing/invalid provenance exits 1.
duration: 
verification_result: passed
completed_at: 2026-05-20T04:43:56.366Z
blocker_discovered: false
---

# T01: Added the additive `validation-batch verify-artifacts` CLI command.

**Added the additive `validation-batch verify-artifacts` CLI command.**

## What Happened

Added a new `validation-batch verify-artifacts` Typer command. It reads a provenance JSONL log, selects a run by run-id or batch/command, builds a freshness report, optionally writes that report to disk, prints JSON when requested, and exits nonzero unless the verdict is `fresh`. Existing validation-batch commands are unchanged.

## Verification

CLI help includes `verify-artifacts`.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python -m arxiv_archive validation-batch --help | grep -Fq 'verify-artifacts'` | 0 | ✅ pass — verify-artifacts command visible | 6000ms |

## Deviations

None.

## Known Issues

The command verifies existing provenance logs but init/preflight/scan do not emit provenance logs automatically yet. That integration remains a later slice.

## Files Created/Modified

- `src/arxiv_archive/cli.py`
