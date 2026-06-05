---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: Document complexity risks and unresolved validation gates

Write the risk/unknowns and validation-gate artifact: runtime burdens, model cache risks, full-DL GROBID future accuracy option, layout/table fidelity gaps, source-span anchoring, bibliography/citation quality, reading order/OCR quality, review packet requirements, and no-write import boundary. This feeds S06 directly.

## Inputs

- `data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.json`

## Expected Output

- `data/article_corpora/m033-combined-parser-architecture-v1/complexity-and-validation-gates.json`
- `data/article_corpora/m033-combined-parser-architecture-v1/complexity-and-validation-gates.md`

## Verification

Fresh command validates all expected risk categories and validation gates are present, including graph-readiness review and no-write/import flags false.

## Observability Impact

Captures the exact unknowns S06 must turn into bounded quality-plan checks.
