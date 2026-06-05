# S03: OpenDataLoader OCR Layout Table Probe

**Goal:** Perform a hands-on OpenDataLoader PDF probe using the confirmed local toolchain and model cache: Java/Maven, uv + Python 3.13 wrapper, direct Java JAR fallback, verified hybrid docling-fast backend, and documented Hugging Face Docling model cache. Freeze three PDF inputs, formalize readiness/build/backend/cache evidence, run hybrid with Java-only fallback, review output quality, and map results to daily-archive contracts without production graph import or LadybugDB writes.
**Demo:** After this: OpenDataLoader PDF has been tested or blocked on three local PDFs with backend health, outputs, quality review, and contract mapping evidence.

## Must-Haves

- Environment readiness records confirmed OpenJDK 17, Maven 3.8.7, uv, Python 3.13, vendor JAR build success, Python 3.13 wheel build/install/import success, Java-only wrapper conversion success, hybrid extras installation, backend `/health` success, hybrid docling-fast one-page conversion success, and Hugging Face model cache locations.
- Backend/cache lifecycle is documented: start on demand, health-check on port 5002, use cached models when present, first-run network/model-download dependency when cache is missing, cache paths/sizes/snapshot IDs, and stop backend after probe; prior bg process termination is recorded as intentional cleanup, not parser crash.
- Three local PDF articles are selected before full execution with path, sha256, size, provenance, and challenge rationale.
- Preferred run path is Python 3.13 + uv-built `opendataloader-pdf` wrapper with `--hybrid docling-fast --hybrid-url http://127.0.0.1:5002` for quality probe; Java-only fast mode and direct Java JAR are documented fallbacks.
- All three PDFs have OpenDataLoader outputs or typed per-paper blockers with command metadata and diagnostics.
- Quality review distinguishes Java-only quality from hybrid/docling-fast quality and documents runtime/model-cache cost.
- Contract mapping covers SourceRef, EvidencePath, PageIndex, SemanticChunk, table artifact, refusal diagnostic, and graph-readiness packet boundaries.
- Graph/import/LadybugDB safety flags remain false in all verdict artifacts.

## Proof Level

- This slice proves: Hands-on local OpenDataLoader probe with verified Java-only and hybrid/docling-fast execution paths, executable command evidence, generated artifacts, diagnostics, backend/cache lifecycle notes, and bounded quality/contract review.

## Integration Closure

Consumes S01 baseline artifacts and local catalog PDFs; uses `/root/vendor-source/opendataloader-pdf` as read-only research context plus the locally built JAR/Python wheel/hybrid backend and `/root/.cache/huggingface/hub` model cache as probe tooling/runtime state; provides empirical OpenDataLoader evidence to S05 and S06. No production integration, graph import, or LadybugDB write is introduced.

## Verification

- Environment readiness, backend health, model-cache inventory, build evidence, run events, smoke/run summaries, per-paper outputs/blockers, quality summary, and verdict artifacts preserve command, exit code, duration, runtime mode, backend mode, output paths, diagnostics, model-cache paths/sizes/snapshot IDs, backend lifecycle, and recovery notes for future agents.

## Tasks

- [x] **T01: Record prepared OpenDataLoader environment, backend, and model cache readiness** `est:30m`
  Formalize the environment, backend, and model-cache findings already proven before S03 execution: OpenJDK 17.0.19 is installed, Maven 3.8.7 is installed, uv is available, Python 3.13.12 is available, the vendor Java CLI builds successfully, the Python package builds through `uv build --python 3.13`, the wheel installs/imports in a Python 3.13 venv, the Python 3.13 wrapper converted one page in Java-only mode, hybrid extras installed in the Python 3.13 venv, `opendataloader-pdf-hybrid --port 5002` starts, `/health` returns 200, and hybrid docling-fast converted one page. Document preferred run path as Python 3.13 wrapper with hybrid docling-fast backend, Java-only wrapper as fallback, and direct Java JAR as last fallback. Document first-run model downloads/cache dependency and exact cache state: `/root/.cache/huggingface/hub/models--docling-project--docling-layout-heron` snapshot `8f39ad3c0b4c58e9c2d2c84a38465abf757272d8` (~164M) and `/root/.cache/huggingface/hub/models--docling-project--docling-models` snapshot `fc0f2d45e2218ea24bce5045f58a389aed16dc23` (~342M). Record that `HF_HOME`, `HUGGINGFACE_HUB_CACHE`, and `XDG_CACHE_HOME` were unset, so default root cache was used. Record that the observed background process termination was intentional cleanup after verification. Do not claim graph readiness or production integration.
  - Files: `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-runbook.md`, `data/article_corpora/m033-opendataloader-pdf-probe-v1/backend-health.json`, `data/article_corpora/m033-opendataloader-pdf-probe-v1/environment-readiness.json`, `data/article_corpora/m033-opendataloader-pdf-probe-v1/model-cache-inventory.json`
  - Verify: Verify `environment-readiness.json` parses as JSON and contains `status: ready_for_hybrid_probe`, observed fields for `java`, `javac`, `maven`, `uv`, `python_3_13`, `vendor_jar`, `python_313_wheel`, `python_313_wrapper_smoke`, `hybrid_extras`, `hybrid_backend_health`, `hybrid_wrapper_smoke`, `preferred_run_path`, and safety flags false. Verify `backend-health.json` records `/health: 200`, `/openapi.json: 200`, model-cache/download notes, and intentional backend cleanup. Verify `model-cache-inventory.json` records both Docling cache paths, snapshot IDs, sizes, and whether env vars were unset. Verify `opendataloader-runbook.md` is non-empty and names hybrid, Java-only, direct JAR, and cache dependency commands/paths.

- [x] **T02: Select and freeze three local PDF probe inputs** `est:30m`
  Select three local scientific PDF artifacts from existing daily-archive data before full OpenDataLoader execution. Prefer challenge diversity: one figure/layout-heavy PDF, one text/section-heavy PDF, and one fallback/problem-case PDF. Record article identity, title, source path, sha256, file size, source provenance, challenge rationale, and whether network fetch was avoided. The initial candidate set should include `data/article_catalog/article_catalog/arxiv/cs-cv/2605.26525v1/source/original.pdf` (ReCA, layout/figure-heavy), `data/article_catalog/article_catalog/arxiv/cs-ai/2512.24601/source/original.pdf` (Recursive Language Models, text/section-heavy), and `data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/source/original.pdf` (GEPA, fallback/problem-case) if their hashes still match. Do not download new PDFs during this task.
  - Files: `data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.json`, `data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.md`
  - Verify: Verify `input-manifest.json` parses as JSON, contains exactly three entries, each entry has `article_key`, `title`, `source_path`, `sha256`, `size_bytes`, `challenge_role`, `challenge_rationale`, and `network_fetch_avoided: true`, and every `source_path` exists and matches its sha256.

- [x] **T03: Promote Java-only and hybrid smoke evidence into formal run setup** `est:1h`
  Create formal smoke/setup artifacts from the prepared Python 3.13 wrapper, hybrid docling-fast backend, Java-only fallback, direct JAR fallback, and Hugging Face model cache. Re-run or validate smoke conversions on the first selected manifest PDF: one Java-only fast-mode conversion and one hybrid docling-fast conversion with `--hybrid-url http://127.0.0.1:5002`. Start the backend on demand, verify `/health`, verify expected model cache paths exist before or after startup, run the hybrid smoke, and stop the backend after capture. Capture command metadata, cwd, runtime mode, backend mode, exit code, duration, output paths, stderr/stdout summary, model-cache paths/sizes/snapshot IDs, first-run download/cache notes, and typed blockers. Treat `opendataloader-pdf` with no input returning usage exit 2 as non-blocking CLI behavior, not a failed build.
  - Files: `data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl`, `data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-summary.json`, `data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-output`
  - Verify: Verify `smoke-summary.json` parses as JSON and has `status: passed`, `selected_pdf`, `java_only_smoke`, `hybrid_smoke`, `commands`, `duration_ms`, `model_cache`, and non-empty outputs. Verify each smoke has `exit_code: 0`. Verify `run-events.jsonl` is non-empty and contains valid JSON lines. Verify smoke output includes at least one JSON, Markdown, and text output file for Java-only and hybrid modes.

- [x] **T04: Run OpenDataLoader hybrid probe on three PDFs with Java-only fallback** `est:1h30m`
  Run OpenDataLoader against the three selected PDFs. Prefer Python 3.13 wrapper with `--hybrid docling-fast --hybrid-url http://127.0.0.1:5002`; start the backend on demand, health-check it, confirm the expected Hugging Face model cache paths, and stop it after the run. If hybrid startup or per-paper hybrid processing fails, fall back to Java-only fast mode for that paper and record both the hybrid blocker and fallback result. Persist per-paper outputs under a stable artifact directory. Capture json, markdown, html, and text outputs where supported, plus command metadata, exit code, duration, output sizes, runtime mode, backend mode, fallback use, model-cache notes, and diagnostics.
  - Files: `data/article_corpora/m033-opendataloader-pdf-probe-v1/per-paper`, `data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl`, `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-run-summary.json`
  - Verify: Verify `opendataloader-run-summary.json` parses as JSON, contains exactly three per-paper results matching `input-manifest.json`, and each result has `status` in `passed|blocked|failed`, `article_key`, `source_path`, `exit_code`, `duration_ms`, `runtime_mode`, `backend_mode`, `fallback_used`, `model_cache_used`, `output_paths`, and `diagnostics`. Verify `per-paper/` exists; for passed papers JSON/Markdown/Text outputs must be non-empty, and failed/blocked papers must have typed blocker files.

- [x] **T05: Review OpenDataLoader hybrid and fallback output quality** `est:1h`
  Evaluate generated hybrid docling-fast outputs, Java-only fallback outputs, or blockers for each PDF against the S01 comparison baseline. Score or mark not-applicable for section hierarchy, reading order, tables, figures/captions, bibliography, OCR quality, coordinate/layout metadata, markdown usefulness, JSON usefulness, and failure diagnostics. Clearly separate observed hybrid quality, observed Java-only fallback quality, and dimensions still not proven by the probe. Record runtime/model-cache cost: cache paths, approximate sizes, first-run network dependency if cache absent, and whether cached models were used. Record qualitative examples without embedding large raw paper payloads.
  - Files: `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-summary.json`, `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-report.md`
  - Verify: Verify `opendataloader-quality-summary.json` parses as JSON, has exactly three per-paper reviews, includes every required quality dimension, distinguishes observed hybrid quality from Java-only fallback quality and not-proven dimensions, records runtime/model-cache cost and cache paths, and keeps graph/import/LadybugDB safety flags false. Verify the markdown report is non-empty and references all three article keys.

- [x] **T06: Map OpenDataLoader probe results to daily-archive contracts** `est:45m`
  Create a contract mapping matrix from OpenDataLoader hybrid outputs, Java-only fallback outputs, or blockers to daily-archive SourceRef, EvidencePath, PageIndex, SemanticChunk, table artifact, refusal diagnostic, and graph-readiness packet boundaries. Classify the bounded tool verdict as `hybrid-sidecar-candidate`, `java-only-candidate`, `needs-larger-hybrid-probe`, `blocked-by-runtime`, or `reject-for-now`. The verdict must remain bounded research only and must not claim graph readiness, production import eligibility, or LadybugDB write readiness. Include backend/cache operational requirements in the verdict: Python 3.13 venv, hybrid extras, server lifecycle, Hugging Face cache paths, cache size, and network dependency if cache is absent.
  - Files: `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-contract-mapping.md`, `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-probe-verdict.json`
  - Verify: Verify `opendataloader-probe-verdict.json` parses as JSON, contains one of the allowed bounded verdict values, includes `graph_import_allowed:false`, `ladybugdb_written:false`, `production_import_attempted:false`, references the quality summary and model-cache inventory, and states backend/runtime/cache cost and remaining evidence gaps. Verify the contract mapping markdown is non-empty and covers SourceRef, EvidencePath, PageIndex, SemanticChunk, table artifact, refusal diagnostic, and graph-readiness packet boundaries.

## Files Likely Touched

- data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-runbook.md
- data/article_corpora/m033-opendataloader-pdf-probe-v1/backend-health.json
- data/article_corpora/m033-opendataloader-pdf-probe-v1/environment-readiness.json
- data/article_corpora/m033-opendataloader-pdf-probe-v1/model-cache-inventory.json
- data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.json
- data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.md
- data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl
- data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-summary.json
- data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-output
- data/article_corpora/m033-opendataloader-pdf-probe-v1/per-paper
- data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-run-summary.json
- data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-summary.json
- data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-report.md
- data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-contract-mapping.md
- data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-probe-verdict.json
