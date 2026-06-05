---
id: T01
parent: S07
milestone: M033-732r1t
key_files:
  - scripts/probe_m033_opendataloader_adaptix_adapter.py
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-summary.json
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-diagnostics.jsonl
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-report.md
key_decisions:
  - (none)
duration: 
verification_result: passed
completed_at: 2026-06-05T08:57:40.908Z
blocker_discovered: false
---

# T01: Implemented and ran an Adaptix-based typed adapter probe over OpenDataLoader S03 JSON outputs.

**Implemented and ran an Adaptix-based typed adapter probe over OpenDataLoader S03 JSON outputs.**

## What Happened

Added `scripts/probe_m033_opendataloader_adaptix_adapter.py`. The script defines typed dataclasses for OpenDataLoader documents and common elements, configures Adaptix `name_mapping` for fixed OpenDataLoader fields such as `file name`, `number of pages`, `page number`, and `bounding box`, preserves heterogeneous extra fields, loads S03 per-paper `original.json` outputs, computes element/type/page/table/figure/heading metrics, and writes review-only candidate summaries. It does not rerun OpenDataLoader, modify vendor code, write LadybugDB, or claim graph readiness. The real S03 run mapped all three papers with `status: adaptix-adapter-candidate` and `error_count: 0`.

## Verification

Fresh real probe command passed: `uv run python scripts/probe_m033_opendataloader_adaptix_adapter.py --probe-root data/article_corpora/m033-opendataloader-pdf-probe-v1 --output-dir data/article_corpora/m033-opendataloader-adaptix-probe-v1` returned `status: adaptix-adapter-candidate`, `paper_count: 3`, `error_count: 0`. Focused tests also passed earlier in the current work: `6 passed in 0.40s`.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/probe_m033_opendataloader_adaptix_adapter.py --probe-root data/article_corpora/m033-opendataloader-pdf-probe-v1 --output-dir data/article_corpora/m033-opendataloader-adaptix-probe-v1` | 0 | ✅ pass | 2800ms |
| 2 | `uv run pytest tests/test_m033_opendataloader_adaptix_adapter.py -q` | 0 | ✅ pass | 2800ms |

## Deviations

None.

## Known Issues

The adapter proves structural mapping only; it does not prove reading-order correctness, table fidelity, OCR quality, graph readiness, or production import eligibility.

## Files Created/Modified

- `scripts/probe_m033_opendataloader_adaptix_adapter.py`
- `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-summary.json`
- `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-diagnostics.jsonl`
- `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-report.md`
