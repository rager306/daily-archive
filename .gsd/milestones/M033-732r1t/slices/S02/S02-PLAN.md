# S02: GROBID Scholarly Parsing Study

**Goal:** Evaluate GROBID as a bounded scholarly parser candidate against daily-archive contracts by using the recommended CRF Docker service path, probing local PDFs for TEI outputs, and documenting runtime/service complexity without making production import or graph-readiness claims. Vendored GROBID docs under `/root/vendor-source/grobid` are read-only research context, not formal GSD inputs.
**Demo:** After this: GROBID's TEI, bibliography, citation, metadata, runtime, and service complexity are understood against daily-archive contracts.

## Must-Haves

- GROBID runtime requirements and Docker/native tradeoffs are recorded with local environment evidence.
- GROBID CRF service is started or a typed blocker artifact explains why it could not be started.
- At least one and preferably three existing S03 local PDFs are submitted to GROBID `processFulltextDocument` when the service is available.
- TEI outputs are summarized for header, abstract, body sections, references/citations, figures/tables, and coordinate support.
- daily-archive contract mapping preserves fail-closed safety: candidate-only, no graph import, no LadybugDB writes, no production parser adoption.

## Proof Level

- This slice proves: Operational research probe with local service health evidence, real PDF request/response artifacts when available, and validate-only closeout checks.

## Integration Closure

S02 must produce repo-local artifacts under `data/article_corpora/m033-grobid-probe-v1/` for S05/S06 synthesis. No source parser integration, no graph imports, and no LadybugDB writes are allowed.

## Verification

- Produces runtime health JSON, request diagnostics JSONL, per-paper TEI outputs/summaries, a contract-mapping report, and closeout verifier artifacts.

## Tasks

- [x] **T01: Documented GROBID runtime requirements and confirmed the CRF Docker image is ready for the bounded S02 probe.** `est:small`
  Confirm local Java/Docker/runtime facts, record native-vs-Docker requirements from vendored GROBID docs, and attempt to prepare the recommended CRF Docker service path. If Docker image pull/start is blocked, record a typed blocker rather than weakening scope. Read-only research context: `/root/vendor-source/grobid/doc/Install-Grobid.md`, `/root/vendor-source/grobid/doc/Grobid-docker.md`, `/root/vendor-source/grobid/Readme.md`.
  - Files: `data/article_corpora/m033-grobid-probe-v1/grobid-runtime-readiness.json`, `data/article_corpora/m033-grobid-probe-v1/grobid-runtime-runbook.md`, `data/article_corpora/m033-grobid-probe-v1/grobid-events.jsonl`
  - Verify: Fresh command verifies runtime readiness artifacts exist, include Java version, Docker daemon status, selected image, native JDK21 requirement, and fail-closed safety flags.

- [x] **T02: Ran a bounded GROBID CRF TEI probe on the three local S03 PDFs.** `est:medium`
  Start or reuse a local GROBID CRF Docker service on port 8070, health-check it, submit the three S03 local PDF candidates to `/api/processFulltextDocument`, and store TEI XML plus per-paper request diagnostics. If service startup fails, write a fail-closed blocker artifact and stop before claiming parser output evidence.
  - Files: `data/article_corpora/m033-grobid-probe-v1/per-paper/`, `data/article_corpora/m033-grobid-probe-v1/grobid-run-summary.json`, `data/article_corpora/m033-grobid-probe-v1/grobid-events.jsonl`
  - Verify: Fresh command checks service health or blocker status and validates that each successful paper has non-empty TEI XML plus diagnostics with false graph/import safety flags.

- [x] **T03: Mapped GROBID TEI outputs to daily-archive candidate contracts with a fail-closed scholarly sidecar verdict.** `est:medium`
  Parse/summarize the GROBID TEI outputs for scholarly structure: title/header/abstract/body sections, references, citation markers, figures/tables, and coordinate hints. Compare those fields with daily-archive SourceRef/EvidencePath/PageIndex/SemanticChunk needs and write a candidate-only contract mapping verdict.
  - Files: `data/article_corpora/m033-grobid-probe-v1/grobid-tei-quality-summary.json`, `data/article_corpora/m033-grobid-probe-v1/grobid-contract-mapping.md`, `data/article_corpora/m033-grobid-probe-v1/grobid-probe-verdict.json`
  - Verify: Fresh command validates quality summary, mapping report, and verdict JSON exist, are internally consistent, and keep `graph_import_allowed`, `ladybugdb_written`, `production_import_attempted`, and `import_eligible` false.

- [x] **T04: Added and ran a validate-only closeout checker for the GROBID bounded probe.** `est:small`
  Add a validate-only closeout checker for S02 artifacts and run the full S02 acceptance gate. The closeout must pass whether the service produced TEI outputs or produced a typed service blocker, but it must reject any permissive graph/import safety flag.
  - Files: `scripts/verify_m033_grobid_probe.py`, `data/article_corpora/m033-grobid-probe-v1/grobid-closeout-summary.json`, `data/article_corpora/m033-grobid-probe-v1/grobid-closeout-report.md`
  - Verify: `uv run python scripts/verify_m033_grobid_probe.py --probe-dir data/article_corpora/m033-grobid-probe-v1 && uv run ruff check scripts/verify_m033_grobid_probe.py` exits 0.

## Files Likely Touched

- data/article_corpora/m033-grobid-probe-v1/grobid-runtime-readiness.json
- data/article_corpora/m033-grobid-probe-v1/grobid-runtime-runbook.md
- data/article_corpora/m033-grobid-probe-v1/grobid-events.jsonl
- data/article_corpora/m033-grobid-probe-v1/per-paper/
- data/article_corpora/m033-grobid-probe-v1/grobid-run-summary.json
- data/article_corpora/m033-grobid-probe-v1/grobid-tei-quality-summary.json
- data/article_corpora/m033-grobid-probe-v1/grobid-contract-mapping.md
- data/article_corpora/m033-grobid-probe-v1/grobid-probe-verdict.json
- scripts/verify_m033_grobid_probe.py
- data/article_corpora/m033-grobid-probe-v1/grobid-closeout-summary.json
- data/article_corpora/m033-grobid-probe-v1/grobid-closeout-report.md
