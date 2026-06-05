---
id: T01
parent: S01
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.json
  - data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.md
key_decisions:
  - Treat current daily-archive parser/conversion success as candidate evidence only; graph import and LadybugDB writes remain out of scope for M033.
duration: 
verification_result: passed
completed_at: 2026-06-05T07:31:48.836Z
blocker_discovered: false
---

# T01: Inventoried the current daily-archive parser pipeline entrypoints for external parser comparison.

**Inventoried the current daily-archive parser pipeline entrypoints for external parser comparison.**

## What Happened

Created the M033 S01 entrypoint inventory under `data/article_corpora/m033-current-parser-baseline-v1/`. The artifact maps the current staged pipeline from catalog intake through source acquisition, loader evidence, parser/conversion, chunk/evidence, graph-readiness handoff, and no-write import boundary. It records relevant scripts, GitNexus-discovered symbols where available, stage roles, produced artifacts, downstream consumers, and fail-closed safety context. No production code was modified.

## Verification

Fresh `gsd_exec` generated and verified both `current-pipeline-entrypoints.json` and `current-pipeline-entrypoints.md` with `test -s`; exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `python3 generation script plus test -s current-pipeline-entrypoints artifacts` | 0 | ✅ pass | 139ms |

## Deviations

None.

## Known Issues

This is an inventory artifact only; it does not yet map detailed artifact fields or compare external tools.

## Files Created/Modified

- `data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.json`
- `data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.md`
