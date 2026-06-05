---
id: T04
parent: S03
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/per-paper
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-run-summary.json
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-05T08:28:03.268Z
blocker_discovered: false
---

# T04: Ran the OpenDataLoader hybrid probe on all three selected PDFs without needing Java-only fallback.

**Ran the OpenDataLoader hybrid probe on all three selected PDFs without needing Java-only fallback.**

## What Happened

Executed OpenDataLoader against ReCA, Recursive Language Models, and GEPA using the Python 3.13 wrapper with `--hybrid docling-fast` and the local backend on port 5002. The backend was started on demand, health-checked, used for all three papers, and stopped after the run. All three papers completed with `status: passed`, `exit_code: 0`, `backend_mode: hybrid-docling-fast`, `fallback_used: false`, and `model_cache_used: true`. Outputs were persisted per paper under `per-paper/`, with JSON, Markdown, HTML, text, logs, and per-paper `result.json` metadata. The full run took about 256s for ReCA, 215s for Recursive Language Models, and 106s for GEPA.

## Verification

Fresh `gsd_exec` parsed `opendataloader-run-summary.json`, verified exactly three per-paper results matching the manifest, required fields, `status: passed`, exit code 0, `backend_mode: hybrid-docling-fast`, `fallback_used: false`, `model_cache_used: true`, per-paper directories, non-empty JSON/Markdown/Text outputs, and valid JSONL run events. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gsd_exec T04 run-summary/per-paper/output/JSONL verification` | 0 | ✅ pass | 62ms |

## Deviations

None.

## Known Issues

Hybrid runtime is heavy: the three-paper run took roughly 9.6 minutes total and depends on cached Docling models.

## Files Created/Modified

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/per-paper`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-run-summary.json`
