---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T03: Verify real S03 adapter run and close bounded verdict

Add a validate-only verifier for adapter artifacts and run the full T01/T02 verification sequence over the S03 outputs. The verifier should check three per-paper results, JSON/JSONL parseability, candidate-only verdict, safety flags false, non-empty report, and no production graph/import claims. Close the slice with a clear verdict for S05 synthesis.

## Inputs

- `scripts/probe_m033_opendataloader_adaptix_adapter.py`
- `tests/test_m033_opendataloader_adaptix_adapter.py`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-run-summary.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/per-paper`

## Expected Output

- `scripts/verify_m033_opendataloader_adaptix_adapter.py`
- `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-summary.json`
- `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-report.md`

## Verification

uv run python scripts/probe_m033_opendataloader_adaptix_adapter.py --probe-root data/article_corpora/m033-opendataloader-pdf-probe-v1 --output-dir data/article_corpora/m033-opendataloader-adaptix-probe-v1 && uv run python scripts/verify_m033_opendataloader_adaptix_adapter.py --probe-root data/article_corpora/m033-opendataloader-pdf-probe-v1 --adapter-dir data/article_corpora/m033-opendataloader-adaptix-probe-v1 && uv run pytest tests/test_m033_opendataloader_adaptix_adapter.py -q && uv run ruff check scripts/probe_m033_opendataloader_adaptix_adapter.py scripts/verify_m033_opendataloader_adaptix_adapter.py tests/test_m033_opendataloader_adaptix_adapter.py

## Observability Impact

Adds final closeout signal and bounded verdict for S05/S06.
