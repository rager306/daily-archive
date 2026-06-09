---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T04: Ran the OpenDataLoader hybrid probe on all three selected PDFs without needing Java-only fallback.

Run OpenDataLoader against the three selected PDFs. Prefer Python 3.13 wrapper with `--hybrid docling-fast --hybrid-url http://127.0.0.1:5002`; start the backend on demand, health-check it, confirm the expected Hugging Face model cache paths, and stop it after the run. If hybrid startup or per-paper hybrid processing fails, fall back to Java-only fast mode for that paper and record both the hybrid blocker and fallback result. Persist per-paper outputs under a stable artifact directory. Capture json, markdown, html, and text outputs where supported, plus command metadata, exit code, duration, output sizes, runtime mode, backend mode, fallback use, model-cache notes, and diagnostics.

## Inputs

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-summary.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/environment-readiness.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/backend-health.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/model-cache-inventory.json`

## Expected Output

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/per-paper`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-run-summary.json`

## Verification

Verify `opendataloader-run-summary.json` parses as JSON, contains exactly three per-paper results matching `input-manifest.json`, and each result has `status` in `passed|blocked|failed`, `article_key`, `source_path`, `exit_code`, `duration_ms`, `runtime_mode`, `backend_mode`, `fallback_used`, `model_cache_used`, `output_paths`, and `diagnostics`. Verify `per-paper/` exists; for passed papers JSON/Markdown/Text outputs must be non-empty, and failed/blocked papers must have typed blocker files.

## Observability Impact

Per-paper run summary prevents silent partial success and preserves hybrid/fallback/runtime/output/model-cache diagnostics.
