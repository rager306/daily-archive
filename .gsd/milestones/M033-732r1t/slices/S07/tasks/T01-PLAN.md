---
estimated_steps: 1
estimated_files: 4
skills_used: []
---

# T01: Implemented and ran an Adaptix-based typed adapter probe over OpenDataLoader S03 JSON outputs.

Create a small script that defines typed dataclasses for the OpenDataLoader document and common elements, configures Adaptix name mappings for fields such as `file name`, `number of pages`, `page number`, and `bounding box`, loads S03 `original.json` files, preserves unknown fields as extras where useful, computes element/type/page/table/figure/heading metrics, and writes review-only adapter summary plus diagnostics. The script must not modify OpenDataLoader, rerun the backend, write LadybugDB, or claim graph readiness.

## Inputs

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-run-summary.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/per-paper`

## Expected Output

- `scripts/probe_m033_opendataloader_adaptix_adapter.py`
- `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-summary.json`
- `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-diagnostics.jsonl`
- `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-report.md`

## Verification

uv run python scripts/probe_m033_opendataloader_adaptix_adapter.py --probe-root data/article_corpora/m033-opendataloader-pdf-probe-v1 --output-dir data/article_corpora/m033-opendataloader-adaptix-probe-v1

## Observability Impact

Records adapter metrics, per-paper mapping status, unmapped/extra field counts, and fail-closed safety flags.
