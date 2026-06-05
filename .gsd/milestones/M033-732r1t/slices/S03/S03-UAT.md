# S03: OpenDataLoader OCR Layout Table Probe — UAT

**Milestone:** M033-732r1t
**Written:** 2026-06-05T08:31:46.395Z

# S03 UAT

A future agent can inspect `data/article_corpora/m033-opendataloader-pdf-probe-v1/` and verify that OpenDataLoader was evaluated as bounded research evidence only.

Checks:

- `environment-readiness.json` records `ready_for_hybrid_probe` and the prepared Java/Maven/uv/Python 3.13/backend/cache state.
- `input-manifest.json` freezes exactly three local PDFs with hashes and `network_fetch_avoided:true`.
- `smoke-summary.json` records successful Java-only and hybrid smoke conversions.
- `opendataloader-run-summary.json` records three successful hybrid/docling-fast per-paper runs with no fallback.
- `opendataloader-quality-summary.json` reviews all three papers across required quality dimensions.
- `opendataloader-probe-verdict.json` records the bounded `hybrid-sidecar-candidate` verdict with graph/import/LadybugDB flags false.

This UAT confirms the probe generated candidate evidence only; it does not authorize graph readiness or production import.
