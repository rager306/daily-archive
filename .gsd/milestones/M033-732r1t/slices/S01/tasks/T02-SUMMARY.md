---
id: T02
parent: S01
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.json
  - data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.md
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-05T07:33:00.670Z
blocker_discovered: false
---

# T02: Mapped the current M031 artifact contracts by stage for external parser comparison.

**Mapped the current M031 artifact contracts by stage for external parser comparison.**

## What Happened

Created `current-artifact-contracts.json` and `.md` under the M033 baseline directory. The map describes each current stage's inputs, outputs, primary artifact, key contract fields, expected counters/provenance, and downstream consumers. It covers catalog intake, source acquisition, loader evidence, parser/conversion, chunk/evidence, graph-readiness handoff, and no-write import boundary. It also identifies what external parsers must preserve and where they may improve the baseline: layout, tables, figures/captions, bibliography/citations, reading order, section hierarchy, and coordinate/source spans.

## Verification

Fresh `gsd_exec` generated the JSON/Markdown contract artifacts and verified both are non-empty with `test -s`; exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `python3 generation script plus test -s current-artifact-contracts artifacts` | 0 | ✅ pass | 65ms |

## Deviations

None.

## Known Issues

This is an artifact map, not a quality evaluation of any external tool.

## Files Created/Modified

- `data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.json`
- `data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.md`
