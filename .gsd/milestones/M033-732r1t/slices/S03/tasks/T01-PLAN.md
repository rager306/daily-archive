---
estimated_steps: 1
estimated_files: 4
skills_used: []
---

# T01: Recorded prepared OpenDataLoader environment, backend, and model cache readiness.

Formalize the environment, backend, and model-cache findings already proven before S03 execution: OpenJDK 17.0.19 is installed, Maven 3.8.7 is installed, uv is available, Python 3.13.12 is available, the vendor Java CLI builds successfully, the Python package builds through `uv build --python 3.13`, the wheel installs/imports in a Python 3.13 venv, the Python 3.13 wrapper converted one page in Java-only mode, hybrid extras installed in the Python 3.13 venv, `opendataloader-pdf-hybrid --port 5002` starts, `/health` returns 200, and hybrid docling-fast converted one page. Document preferred run path as Python 3.13 wrapper with hybrid docling-fast backend, Java-only wrapper as fallback, and direct Java JAR as last fallback. Document first-run model downloads/cache dependency and exact cache state: `/root/.cache/huggingface/hub/models--docling-project--docling-layout-heron` snapshot `8f39ad3c0b4c58e9c2d2c84a38465abf757272d8` (~164M) and `/root/.cache/huggingface/hub/models--docling-project--docling-models` snapshot `fc0f2d45e2218ea24bce5045f58a389aed16dc23` (~342M). Record that `HF_HOME`, `HUGGINGFACE_HUB_CACHE`, and `XDG_CACHE_HOME` were unset, so default root cache was used. Record that the observed background process termination was intentional cleanup after verification. Do not claim graph readiness or production integration.

## Inputs

- `data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.json`
- `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.json`

## Expected Output

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-runbook.md`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/backend-health.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/environment-readiness.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/model-cache-inventory.json`

## Verification

Verify `environment-readiness.json` parses as JSON and contains `status: ready_for_hybrid_probe`, observed fields for `java`, `javac`, `maven`, `uv`, `python_3_13`, `vendor_jar`, `python_313_wheel`, `python_313_wrapper_smoke`, `hybrid_extras`, `hybrid_backend_health`, `hybrid_wrapper_smoke`, `preferred_run_path`, and safety flags false. Verify `backend-health.json` records `/health: 200`, `/openapi.json: 200`, model-cache/download notes, and intentional backend cleanup. Verify `model-cache-inventory.json` records both Docling cache paths, snapshot IDs, sizes, and whether env vars were unset. Verify `opendataloader-runbook.md` is non-empty and names hybrid, Java-only, direct JAR, and cache dependency commands/paths.

## Observability Impact

Records exact prepared toolchain/backend/cache state and separates confirmed hybrid readiness from production adoption.
