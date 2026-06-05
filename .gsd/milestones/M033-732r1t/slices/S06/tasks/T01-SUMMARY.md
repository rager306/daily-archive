---
id: T01
parent: S06
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.md
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-events.jsonl
key_decisions:
  - Future combined-parser quality evaluation must be no-network by default and must treat missing caches or external fetch needs as typed blockers rather than implicit downloads.
duration: 
verification_result: passed
completed_at: 2026-06-05T11:48:23.842Z
blocker_discovered: false
---

# T01: Defined the bounded future parser-quality probe scope and corpus strategy.

**Defined the bounded future parser-quality probe scope and corpus strategy.**

## What Happened

Created `future-probe-scope.json`, `.md`, and the S06 event log. The scope derives from S05's `recommended-bounded-combined-sidecar-architecture` verdict and frames a future milestone intent rather than executing the future probe in M033. It defines six corpus classes: native digital arXiv PDFs, long/appendix-heavy PDFs, table-heavy PDFs, figure/layout-heavy PDFs, scanned/image-only controls, and known low-quality/metadata-only controls. It also records source-locality controls, no-network defaults, model/backend cache preflights, and excluded production actions including production parser integration, dependency adoption, graph import, LadybugDB writes, positive import eligibility, quant-mind runtime, and unreviewed parser-to-SemanticChunk promotion.

## Verification

Fresh T01 verification passed in `gsd_exec[5dd5e098-e736-401e-871d-47f2e309d470]`: the script validated the S05 recommendation source, at least six corpus classes, excluded production actions including graph import, and all false safety flags. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `python3 future-scope-generation validation script via gsd_exec purpose 'M033 S06 T01 create bounded future probe scope'` | 0 | ✅ pass | 66ms |

## Deviations

None.

## Known Issues

This is a future probe scope only; it does not select final concrete paper IDs or run parser quality checks in M033.

## Files Created/Modified

- `data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.json`
- `data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.md`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-events.jsonl`
