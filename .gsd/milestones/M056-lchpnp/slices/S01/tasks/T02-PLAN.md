---
estimated_steps: 7
estimated_files: 2
skills_used: []
---

# T02: Ran GROBID fulltext and OpenDataLoader probes over the 30 acquired Wave 1 PDFs.

Run GROBID /api/processFulltextDocument and OpenDataLoader on each of 30 acquired PDFs. Per-PDF:
- GROBID: TEI with body, refs, bibl, equations, figures, sections
- OpenDataLoader: markdown with tables, images, sections, pages, bboxes
- Both: 5-flag safety defaults explicit
- Fail-closed on errors
Output: artifacts/m056-bfs-graph/wave-1/grobid-fulltext/per-pdf/*.json + summary.json, and opendataloader/per-pdf/*.json + summary.json.
Use scripts/benchmark_m055deep_grobid_fulltext.py and scripts/benchmark_m055_opendataloader_only.py.

## Inputs

- `scripts/benchmark_m055deep_grobid_fulltext.py`
- `scripts/benchmark_m055_opendataloader_only.py`
- `30 Wave 1 PDFs in data/article_catalog/`

## Expected Output

- `artifacts/m056-bfs-graph/wave-1/grobid-fulltext/summary.json`
- `artifacts/m056-bfs-graph/wave-1/opendataloader/summary.json`

## Verification

test -f artifacts/m056-bfs-graph/wave-1/grobid-fulltext/summary.json

## Observability Impact

30 GROBID + 30 OpenDataLoader packets captured.
