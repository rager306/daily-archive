---
id: T02
parent: S03
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.md
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-05T08:15:34.707Z
blocker_discovered: false
---

# T02: Selected and froze three local PDF probe inputs for the OpenDataLoader run.

**Selected and froze three local PDF probe inputs for the OpenDataLoader run.**

## What Happened

Created the S03 input manifest with three local PDFs: ReCA as layout/figure-heavy, Recursive Language Models as text/section-heavy, and GEPA as fallback/problem-case. Each entry records article identity, title, catalog/source provenance, local source path, SHA-256, file size, challenge role, rationale, and `network_fetch_avoided: true`. Catalog-provided PDF hashes were checked against local file hashes where available.

## Verification

Fresh `gsd_exec` generated `input-manifest.json` and `.md`, parsed JSON, verified exactly three entries, required fields, local file existence, SHA-256 matches, and `network_fetch_avoided: true`; exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gsd_exec T02 input manifest generation and hash verification` | 0 | ✅ pass | 214ms |

## Deviations

None.

## Known Issues

The manifest fixes probe inputs only; parser quality is evaluated in later tasks.

## Files Created/Modified

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.md`
