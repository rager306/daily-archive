---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T04: Created the external parser comparison baseline matrix for GROBID, OpenDataLoader, and quant-mind research.

Synthesize T01-T03 into a comparison matrix for GROBID, OpenDataLoader, and quant-mind research. Identify current strengths, weaknesses, missing capabilities, and exact questions external tools must answer for layout, tables, figures/captions, bibliography/citations, OCR, reading order, section hierarchy, source spans, Markdown/JSON quality, runtime complexity, and provenance.

## Inputs

- `data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.json`
- `data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.json`
- `data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.json`

## Expected Output

- `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.json`
- `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.md`

## Verification

Manual review — file exists and is non-empty

## Observability Impact

Comparison baseline is the handoff surface for S02, S03, S04, and S05 research.
