---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T03: Promote Java-only and hybrid smoke evidence into formal run setup

Create formal smoke/setup artifacts from the prepared Python 3.13 wrapper, hybrid docling-fast backend, Java-only fallback, direct JAR fallback, and Hugging Face model cache. Re-run or validate smoke conversions on the first selected manifest PDF: one Java-only fast-mode conversion and one hybrid docling-fast conversion with `--hybrid-url http://127.0.0.1:5002`. Start the backend on demand, verify `/health`, verify expected model cache paths exist before or after startup, run the hybrid smoke, and stop the backend after capture. Capture command metadata, cwd, runtime mode, backend mode, exit code, duration, output paths, stderr/stdout summary, model-cache paths/sizes/snapshot IDs, first-run download/cache notes, and typed blockers. Treat `opendataloader-pdf` with no input returning usage exit 2 as non-blocking CLI behavior, not a failed build.

## Inputs

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/environment-readiness.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/backend-health.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/model-cache-inventory.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-runbook.md`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.json`

## Expected Output

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-summary.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-output`

## Verification

Verify `smoke-summary.json` parses as JSON and has `status: passed`, `selected_pdf`, `java_only_smoke`, `hybrid_smoke`, `commands`, `duration_ms`, `model_cache`, and non-empty outputs. Verify each smoke has `exit_code: 0`. Verify `run-events.jsonl` is non-empty and contains valid JSON lines. Verify smoke output includes at least one JSON, Markdown, and text output file for Java-only and hybrid modes.

## Observability Impact

Records formal Java-only and hybrid smoke command evidence, backend lifecycle, and model-cache state before full three-PDF run.
