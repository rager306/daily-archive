---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Selected and froze three local PDF probe inputs for the OpenDataLoader run.

Select three local scientific PDF artifacts from existing daily-archive data before full OpenDataLoader execution. Prefer challenge diversity: one figure/layout-heavy PDF, one text/section-heavy PDF, and one fallback/problem-case PDF. Record article identity, title, source path, sha256, file size, source provenance, challenge rationale, and whether network fetch was avoided. The initial candidate set should include `data/article_catalog/article_catalog/arxiv/cs-cv/2605.26525v1/source/original.pdf` (ReCA, layout/figure-heavy), `data/article_catalog/article_catalog/arxiv/cs-ai/2512.24601/source/original.pdf` (Recursive Language Models, text/section-heavy), and `data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/source/original.pdf` (GEPA, fallback/problem-case) if their hashes still match. Do not download new PDFs during this task.

## Inputs

- `data/article_catalog/index.json`
- `data/article_catalog/article_catalog/arxiv/cs-cv/2605.26525v1/article.json`
- `data/article_catalog/article_catalog/arxiv/cs-ai/2512.24601/article.json`
- `data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/article.json`
- `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.json`

## Expected Output

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.md`

## Verification

Verify `input-manifest.json` parses as JSON, contains exactly three entries, each entry has `article_key`, `title`, `source_path`, `sha256`, `size_bytes`, `challenge_role`, `challenge_rationale`, and `network_fetch_avoided: true`, and every `source_path` exists and matches its sha256.

## Observability Impact

Freezes reproducible local inputs and hashes before full execution.
