---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T02: Ran a bounded GROBID CRF TEI probe on the three local S03 PDFs.

Start or reuse a local GROBID CRF Docker service on port 8070, health-check it, submit the three S03 local PDF candidates to `/api/processFulltextDocument`, and store TEI XML plus per-paper request diagnostics. If service startup fails, write a fail-closed blocker artifact and stop before claiming parser output evidence.

## Inputs

- `data/article_catalog/article_catalog/arxiv/cs-cv/2605.26525v1/source/original.pdf`
- `data/article_catalog/article_catalog/arxiv/cs-ai/2512.24601/source/original.pdf`
- `data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/source/original.pdf`

## Expected Output

- `data/article_corpora/m033-grobid-probe-v1/grobid-run-summary.json`
- `data/article_corpora/m033-grobid-probe-v1/per-paper/`

## Verification

Fresh command checks service health or blocker status and validates that each successful paper has non-empty TEI XML plus diagnostics with false graph/import safety flags.

## Observability Impact

Captures per-request status, durations, output paths, service URL, and fail-closed safety flags.
