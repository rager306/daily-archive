---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Specified measurable quality metrics and acceptance gates for the future combined-parser probe.

Define measurable metrics and pass/fail gates for GROBID TEI/bibliography/citation quality, OpenDataLoader layout/OCR/table/coordinate quality, Adaptix adapter contract mapping, TreeKnowledge/PageIndex/card schema fit, source-span anchoring, reading order, low-quality source detection, and review packet completion.

## Inputs

- `data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.json`

## Expected Output

- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.json`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.md`

## Verification

Fresh command validates required metric categories and gates exist, include graph-readiness review post-check expectations, and safety flags false.

## Observability Impact

Provides concrete criteria for future parser quality evaluation.
