---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T03: Analyze GROBID TEI and map to daily-archive candidate contracts

Parse/summarize the GROBID TEI outputs for scholarly structure: title/header/abstract/body sections, references, citation markers, figures/tables, and coordinate hints. Compare those fields with daily-archive SourceRef/EvidencePath/PageIndex/SemanticChunk needs and write a candidate-only contract mapping verdict.

## Inputs

- `data/article_corpora/m033-grobid-probe-v1/grobid-run-summary.json`
- `data/article_corpora/m033-current-parser-baseline-v1/`

## Expected Output

- `data/article_corpora/m033-grobid-probe-v1/grobid-tei-quality-summary.json`
- `data/article_corpora/m033-grobid-probe-v1/grobid-contract-mapping.md`
- `data/article_corpora/m033-grobid-probe-v1/grobid-probe-verdict.json`

## Verification

Fresh command validates quality summary, mapping report, and verdict JSON exist, are internally consistent, and keep `graph_import_allowed`, `ladybugdb_written`, `production_import_attempted`, and `import_eligible` false.

## Observability Impact

Summarizes structural coverage and explicit gaps for later S05/S06 decisions.
