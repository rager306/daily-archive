# S07: Assets Tables Links Identity Replay

**Goal:** Separate assets, tables, links, identity, and evidence metadata from chunk text while replaying the same fixed corpus.
**Demo:** After this: assets, tables, links, and identity evidence are separated from chunks and replayed on the same corpus with metadata-safe provenance-bearing outputs.

## Must-Haves

- Assets, tables, links, and identity are represented as separate metadata-safe artifacts.
- Outputs preserve provenance to article/source/element/chunk identifiers.
- Missing or unsupported evidence types are reported as diagnostics, not silent absence.
- Diagnostics do not embed raw payload text or binary data.
- No artifact marks production import or graph readiness true.

## Proof Level

- This slice proves: Same-corpus evidence-boundary replay with metadata-safe artifact inspection.

## Integration Closure

S08 consumes chunk outputs plus separated evidence artifacts to run the full local preprocessing replay without crossing into KG import.

## Verification

- Records asset/table/link/identity counts, extraction outcomes, provenance pointers, missing evidence diagnostics, and redaction checks.

## Tasks

- [x] **T01: Define evidence boundary contracts** `est:medium`
  Define the separated metadata-safe evidence artifact contracts for assets, tables, links, and identity. The contracts must reference article/source/element/chunk identifiers without embedding raw payload text or binary data, and must keep graph import and production write flags false. At execution time this task consumes S01 catalog/index/selection outputs and S06 chunking outputs, but those future artifacts are intentionally not listed as static inputs for pre-execution validation.
  - Files: `tests/test_article_evidence_boundaries.py`, `tests/fixtures/article_evidence_boundaries_v00_01/`
  - Verify: uv run pytest tests/test_article_evidence_boundaries.py -q
uv run ruff check tests/test_article_evidence_boundaries.py

- [ ] **T02: Replay separated evidence artifacts** `est:medium`
  Implement or adapt a local evidence replay command that reads the fixed corpus outputs from S06 and writes separate assets, tables, links, and identity artifacts per article. Unsupported evidence types must produce diagnostics rather than silent empty outputs. The command must read the catalog index and corpus selection at runtime and fail clearly if expected S06 chunking artifacts are absent.
  - Files: `src/arxiv_archive/`, `scripts/verify_m025_evidence_boundaries.py`, `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/`
  - Verify: uv run python scripts/verify_m025_evidence_boundaries.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --chunks data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/chunking --evidence data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence --write-events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-events.jsonl

- [ ] **T03: Finalize evidence boundary report** `est:small`
  Validate the separated evidence artifacts and write the S07 report. The report must summarize per-article counts, missing/unsupported evidence diagnostics, provenance coverage, redaction checks, and no-import/no-write safety state.
  - Files: `scripts/verify_m025_evidence_boundaries.py`, `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/`
  - Verify: uv run python scripts/verify_m025_evidence_boundaries.py --catalog data/article_catalog/catalog.json --index data/article_catalog/index.json --selection data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/selection.json --evidence data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence --events data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-events.jsonl --require-redaction --require-no-import-flags --write-summary data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-summary.json --write-report data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/evidence-report.md

## Files Likely Touched

- tests/test_article_evidence_boundaries.py
- tests/fixtures/article_evidence_boundaries_v00_01/
- src/arxiv_archive/
- scripts/verify_m025_evidence_boundaries.py
- data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/
