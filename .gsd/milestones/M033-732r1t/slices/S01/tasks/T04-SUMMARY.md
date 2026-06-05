---
id: T04
parent: S01
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.json
  - data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.md
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-05T07:34:45.857Z
blocker_discovered: false
---

# T04: Created the external parser comparison baseline matrix for GROBID, OpenDataLoader, and quant-mind research.

**Created the external parser comparison baseline matrix for GROBID, OpenDataLoader, and quant-mind research.**

## What Happened

Synthesized the entrypoint inventory, artifact contract map, and safety-boundary model into `external-parser-comparison-baseline.json` and `.md`. The matrix defines what each downstream research track must answer across scholarly metadata/bibliography, section hierarchy, tables/layout, reading order/OCR quality, provenance/evidence paths, and runtime complexity. It explicitly names downstream use for S02 GROBID, S03 OpenDataLoader, S04 quant-mind, and S05 synthesis while preserving no-import safety constraints.

## Verification

Fresh `gsd_exec` generated both baseline matrix artifacts and verified they are non-empty and mention GROBID, OpenDataLoader, and quant-mind; exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `python3 generation script plus test -s and grep GROBID/OpenDataLoader/quant-mind in comparison baseline artifacts` | 0 | ✅ pass | 58ms |

## Deviations

None.

## Known Issues

The matrix asks comparison questions; downstream slices still need to answer them with tool-specific evidence.

## Files Created/Modified

- `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.json`
- `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.md`
