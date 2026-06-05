---
id: T02
parent: S02
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-grobid-probe-v1/grobid-service-health.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-run-summary.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-events.jsonl
  - data/article_corpora/m033-grobid-probe-v1/per-paper/2605.26525v1/grobid.tei.xml
  - data/article_corpora/m033-grobid-probe-v1/per-paper/2512.24601/grobid.tei.xml
  - data/article_corpora/m033-grobid-probe-v1/per-paper/2507.19457/grobid.tei.xml
key_decisions:
  - Use CRF-only TEI evidence for S02 contract mapping and defer full/DL image comparison to a future quality milestone if needed.
duration: 
verification_result: passed
completed_at: 2026-06-05T10:15:26.684Z
blocker_discovered: false
---

# T02: Ran a bounded GROBID CRF TEI probe on the three local S03 PDFs.

**Ran a bounded GROBID CRF TEI probe on the three local S03 PDFs.**

## What Happened

Started `grobid/grobid:0.9.0-crf` as a local Docker service on port 8070, waited for the API to become ready, and recorded service health/version/model status. Submitted the three S03 local PDF candidates to `/api/processFulltextDocument` with non-consolidating, candidate-only settings and coordinate options. GROBID returned TEI XML for all three PDFs, which was stored under `data/article_corpora/m033-grobid-probe-v1/per-paper/` along with request diagnostics and a run summary. All outputs remain parser evidence only with graph/import/LadybugDB safety flags false.

## Verification

Fresh T02 verification passed: `grobid-run-summary.json` has `status: tei-probe-complete`, `paper_count: 3`, `success_count: 3`, `failure_count: 0`; every result has HTTP 2xx, `status: tei_written`, non-empty TEI XML over 1000 bytes, matching output path, `teiHeader`, per-paper diagnostics, and all safety flags false. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `python3 inline verifier over `data/article_corpora/m033-grobid-probe-v1/grobid-run-summary.json` and per-paper TEI outputs` | 0 | ✅ pass | 68ms |

## Deviations

None.

## Known Issues

This run used the CRF-only image, so it proves service/API/TEI contract shape and baseline scholarly extraction behavior, not best possible DL bibliography/citation accuracy.

## Files Created/Modified

- `data/article_corpora/m033-grobid-probe-v1/grobid-service-health.json`
- `data/article_corpora/m033-grobid-probe-v1/grobid-run-summary.json`
- `data/article_corpora/m033-grobid-probe-v1/grobid-events.jsonl`
- `data/article_corpora/m033-grobid-probe-v1/per-paper/2605.26525v1/grobid.tei.xml`
- `data/article_corpora/m033-grobid-probe-v1/per-paper/2512.24601/grobid.tei.xml`
- `data/article_corpora/m033-grobid-probe-v1/per-paper/2507.19457/grobid.tei.xml`
