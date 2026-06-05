---
id: T03
parent: S02
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-grobid-probe-v1/grobid-tei-quality-summary.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-contract-mapping.md
  - data/article_corpora/m033-grobid-probe-v1/grobid-probe-verdict.json
  - data/article_corpora/m033-grobid-probe-v1/per-paper/2605.26525v1/tei-structure-summary.json
  - data/article_corpora/m033-grobid-probe-v1/per-paper/2512.24601/tei-structure-summary.json
  - data/article_corpora/m033-grobid-probe-v1/per-paper/2507.19457/tei-structure-summary.json
key_decisions:
  - Classify GROBID as a `grobid-scholarly-sidecar-candidate` for TEI metadata, section hierarchy, bibliography, and citation markers, not as a graph-ready parser or table/OCR replacement.
duration: 
verification_result: passed
completed_at: 2026-06-05T10:17:01.386Z
blocker_discovered: false
---

# T03: Mapped GROBID TEI outputs to daily-archive candidate contracts with a fail-closed scholarly sidecar verdict.

**Mapped GROBID TEI outputs to daily-archive candidate contracts with a fail-closed scholarly sidecar verdict.**

## What Happened

Parsed the three GROBID TEI outputs and summarized scholarly structure coverage. All three outputs contained title, abstract, body sections, bibliography entries, figure/table candidates, and coordinate attributes when requested. Compared this evidence against the S01 baseline: GROBID is valuable for scholarly metadata, sections, bibliography, and citation/ref marker candidates, but it does not replace independent reading-order review, table/layout fidelity evaluation, source-span anchoring, or graph-readiness review. Wrote `grobid-tei-quality-summary.json`, `grobid-contract-mapping.md`, and `grobid-probe-verdict.json` with candidate-only safety flags false.

## Verification

Fresh T03 verification passed: quality summary has `status: grobid-tei-candidate-evidence`, `paper_count: 3`, all coverage counts are 3/3 for title, abstract, body divs, bibliography, figures, tables, and coordinates; verdict is `grobid-scholarly-sidecar-candidate` with `candidate_only: true`; every paper and top-level artifact has all safety flags false; report includes explicit `graph_import_allowed=false`, `ladybugdb_written=false`, `production_import_attempted=false`, `import_eligible=false`, and not-graph-ready language. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `python3 inline verifier over `grobid-tei-quality-summary.json`, `grobid-probe-verdict.json`, and `grobid-contract-mapping.md`` | 0 | ✅ pass | 142ms |

## Deviations

None.

## Known Issues

The CRF probe does not prove best bibliography/citation quality versus the full/DL image, and TEI coordinate/table evidence is not equivalent to machine-readable table extraction or source-span validation.

## Files Created/Modified

- `data/article_corpora/m033-grobid-probe-v1/grobid-tei-quality-summary.json`
- `data/article_corpora/m033-grobid-probe-v1/grobid-contract-mapping.md`
- `data/article_corpora/m033-grobid-probe-v1/grobid-probe-verdict.json`
- `data/article_corpora/m033-grobid-probe-v1/per-paper/2605.26525v1/tei-structure-summary.json`
- `data/article_corpora/m033-grobid-probe-v1/per-paper/2512.24601/tei-structure-summary.json`
- `data/article_corpora/m033-grobid-probe-v1/per-paper/2507.19457/tei-structure-summary.json`
