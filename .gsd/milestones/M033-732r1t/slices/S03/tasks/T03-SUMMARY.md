---
id: T03
parent: S03
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-summary.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-output
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-05T08:17:06.627Z
blocker_discovered: false
---

# T03: Promoted Java-only and hybrid smoke evidence into formal OpenDataLoader run setup.

**Promoted Java-only and hybrid smoke evidence into formal OpenDataLoader run setup.**

## What Happened

Created formal smoke artifacts for the first manifest PDF. The task ran Java-only fast mode through the Python 3.13 wrapper and hybrid docling-fast mode through an on-demand `opendataloader-pdf-hybrid` backend on port 5002. The backend was started, health-checked, used for one-page conversion, and intentionally stopped after capture. Smoke outputs include JSON, Markdown, HTML, and text outputs for both modes, plus run events, backend log, command metadata, durations, output paths, model-cache references, and safety flags.

## Verification

Fresh `gsd_exec` generated `smoke-summary.json`, `run-events.jsonl`, and smoke-output directories; verified JSON parse, `status: passed`, Java-only exit code 0, hybrid exit code 0, valid JSONL events, and non-empty JSON/Markdown/Text outputs for both modes. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gsd_exec T03 Java-only and hybrid smoke setup plus JSON/output verification` | 0 | ✅ pass | 41005ms |

## Deviations

None.

## Known Issues

Hybrid runtime remains heavier than Java-only and depends on model cache/network if cache is absent; this is recorded for quality/verdict tasks.

## Files Created/Modified

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-summary.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-output`
