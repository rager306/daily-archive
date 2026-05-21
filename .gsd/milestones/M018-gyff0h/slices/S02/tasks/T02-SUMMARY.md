---
id: T02
parent: S02
milestone: M018-gyff0h
key_files:
  - .gsd/milestones/M018-gyff0h/slices/S02/ml-reachability-report.md
  - src/arxiv_archive/md_converter.py
  - src/arxiv_archive/thirty_paper_source_scan.py
  - src/arxiv_archive/pdf_downloader.py
key_decisions: []
duration: 
verification_result: passed
completed_at: 2026-05-21T07:06:15.224Z
blocker_discovered: false
---

# T02: Classified runtime exposure as medium when bounded source acquisition processes external PDFs, otherwise low/dormant.

**Classified runtime exposure as medium when bounded source acquisition processes external PDFs, otherwise low/dormant.**

## What Happened

Inspected the targeted runtime path around MDConverter, source acquisition, and PDFDownloader. The vulnerable ML packages are reachable through Docling only in a fallback chain: source acquisition attempts conversion, arxiv2md fails/low-quality, Marker unavailable, Docling installed, then Docling processes a local cached PDF originally downloaded from arXiv. Validation-batch CLI preflight/scan does not execute Docling. Main CLI exposure was not found in the inspected references.

## Verification

`m018-s02-reachability-guard-ok` verified report content and machine-readable classification flags.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python inline assertions over ml-reachability-map.json and report` | 0 | ✅ pass — m018-s02-reachability-guard-ok | 7600ms |

## Deviations

None.

## Known Issues

Docling fallback can process externally sourced PDFs if source acquisition is run and Marker is unavailable. This requires S03 recommendation; no code changes were made in S02.

## Files Created/Modified

- `.gsd/milestones/M018-gyff0h/slices/S02/ml-reachability-report.md`
- `src/arxiv_archive/md_converter.py`
- `src/arxiv_archive/thirty_paper_source_scan.py`
- `src/arxiv_archive/pdf_downloader.py`
