# S07: Adaptix OpenDataLoader Adapter Probe

**Goal:** Test whether Adaptix can safely map fixed OpenDataLoader PDF JSON into typed intermediate models and review-only daily-archive candidate summaries without changing OpenDataLoader or weakening graph/import safety boundaries.
**Demo:** After this: OpenDataLoader fixed JSON has been tested through an Adaptix typed adapter into review-only daily-archive candidate summaries, with safety flags fail-closed.

## Must-Haves

- ## Must-Haves
- Use existing S03 OpenDataLoader outputs as inputs; do not rerun the hybrid backend unless verification requires it.
- Implement a small Adaptix-based adapter that loads OpenDataLoader JSON with space-containing field names into typed Python models.
- Produce review-only candidate summaries for SourceRef, EvidencePath/PageIndex-like element counts, table/figure/heading signals, diagnostics, and safety flags.
- Keep `graph_import_allowed=false`, `ladybugdb_written=false`, `production_import_attempted=false`, and no positive import eligibility claims.
- Verify with focused tests and a real run over at least one S03 `original.json`, preferably all three if cheap.
- ## Verification
- `uv run pytest tests/test_m033_opendataloader_adaptix_adapter.py -q`
- `uv run python scripts/probe_m033_opendataloader_adaptix_adapter.py --probe-root data/article_corpora/m033-opendataloader-pdf-probe-v1 --output-dir data/article_corpora/m033-opendataloader-adaptix-probe-v1`
- `uv run python scripts/verify_m033_opendataloader_adaptix_adapter.py --probe-root data/article_corpora/m033-opendataloader-pdf-probe-v1 --adapter-dir data/article_corpora/m033-opendataloader-adaptix-probe-v1`
- `uv run ruff check scripts/probe_m033_opendataloader_adaptix_adapter.py scripts/verify_m033_opendataloader_adaptix_adapter.py tests/test_m033_opendataloader_adaptix_adapter.py`

## Proof Level

- This slice proves: Contract/probe proof over existing local JSON artifacts. No parser rerun, network call, graph write, LadybugDB write, or production import is allowed.

## Integration Closure

Consumes S03 OpenDataLoader run artifacts and model-cache/runtime evidence. Produces adapter proof artifacts for S05 synthesis and S06 quality planning. Does not modify production article pipeline modules.

## Verification

- Adds adapter-level summary, diagnostics JSONL, verification closeout, and report surfaces showing which OpenDataLoader fields mapped cleanly, which remained extra/unmapped, and why outputs are review-only candidates.

## Tasks

- [x] **T01: Implement Adaptix OpenDataLoader typed adapter probe** `est:90m`
  Create a small script that defines typed dataclasses for the OpenDataLoader document and common elements, configures Adaptix name mappings for fields such as `file name`, `number of pages`, `page number`, and `bounding box`, loads S03 `original.json` files, preserves unknown fields as extras where useful, computes element/type/page/table/figure/heading metrics, and writes review-only adapter summary plus diagnostics. The script must not modify OpenDataLoader, rerun the backend, write LadybugDB, or claim graph readiness.
  - Files: `scripts/probe_m033_opendataloader_adaptix_adapter.py`, `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-summary.json`, `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-diagnostics.jsonl`, `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-report.md`
  - Verify: uv run python scripts/probe_m033_opendataloader_adaptix_adapter.py --probe-root data/article_corpora/m033-opendataloader-pdf-probe-v1 --output-dir data/article_corpora/m033-opendataloader-adaptix-probe-v1

- [x] **T02: Add focused tests for adapter mapping and safety flags** `est:60m`
  Add tests that exercise Adaptix loading against a small fixture with OpenDataLoader field names, verify aliases and bounding boxes map correctly, verify malformed documents fail closed with diagnostics, and verify generated summaries keep graph/import/LadybugDB flags false. Keep tests local-only and independent of the hybrid backend.
  - Files: `tests/test_m033_opendataloader_adaptix_adapter.py`
  - Verify: uv run pytest tests/test_m033_opendataloader_adaptix_adapter.py -q

- [x] **T03: Verify real S03 adapter run and close bounded verdict** `est:45m`
  Add a validate-only verifier for adapter artifacts and run the full T01/T02 verification sequence over the S03 outputs. The verifier should check three per-paper results, JSON/JSONL parseability, candidate-only verdict, safety flags false, non-empty report, and no production graph/import claims. Close the slice with a clear verdict for S05 synthesis.
  - Files: `scripts/verify_m033_opendataloader_adaptix_adapter.py`, `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-summary.json`, `data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-report.md`
  - Verify: uv run python scripts/probe_m033_opendataloader_adaptix_adapter.py --probe-root data/article_corpora/m033-opendataloader-pdf-probe-v1 --output-dir data/article_corpora/m033-opendataloader-adaptix-probe-v1 && uv run python scripts/verify_m033_opendataloader_adaptix_adapter.py --probe-root data/article_corpora/m033-opendataloader-pdf-probe-v1 --adapter-dir data/article_corpora/m033-opendataloader-adaptix-probe-v1 && uv run pytest tests/test_m033_opendataloader_adaptix_adapter.py -q && uv run ruff check scripts/probe_m033_opendataloader_adaptix_adapter.py scripts/verify_m033_opendataloader_adaptix_adapter.py tests/test_m033_opendataloader_adaptix_adapter.py

## Files Likely Touched

- scripts/probe_m033_opendataloader_adaptix_adapter.py
- data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-summary.json
- data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-diagnostics.jsonl
- data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-report.md
- tests/test_m033_opendataloader_adaptix_adapter.py
- scripts/verify_m033_opendataloader_adaptix_adapter.py
- data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-summary.json
- data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-report.md
