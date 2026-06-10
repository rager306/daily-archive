---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Wave 2 parser runs produced 30 GROBID packets and 30 OpenDataLoader packets.

Run GROBID /api/processFulltextDocument and OpenDataLoader on each of 30 Wave 2 PDFs. Output per-pdf JSON packets + summary.json. Use existing scripts/benchmark_m055deep_grobid_fulltext.py and scripts/benchmark_m055_opendataloader_only.py.

## Inputs

- `scripts/benchmark_m055deep_grobid_fulltext.py`
- `scripts/benchmark_m055_opendataloader_only.py`
- `30 Wave 2 PDFs in data/article_catalog/`

## Expected Output

- `artifacts/m056-bfs-graph/wave-2/grobid-fulltext/summary.json`
- `artifacts/m056-bfs-graph/wave-2/opendataloader/summary.json`

## Verification

test -f artifacts/m056-bfs-graph/wave-2/grobid-fulltext/summary.json

## Observability Impact

30 GROBID + 30 OpenDataLoader Wave 2 packets.
