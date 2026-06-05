---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T05: Review OpenDataLoader hybrid and fallback output quality

Evaluate generated hybrid docling-fast outputs, Java-only fallback outputs, or blockers for each PDF against the S01 comparison baseline. Score or mark not-applicable for section hierarchy, reading order, tables, figures/captions, bibliography, OCR quality, coordinate/layout metadata, markdown usefulness, JSON usefulness, and failure diagnostics. Clearly separate observed hybrid quality, observed Java-only fallback quality, and dimensions still not proven by the probe. Record runtime/model-cache cost: cache paths, approximate sizes, first-run network dependency if cache absent, and whether cached models were used. Record qualitative examples without embedding large raw paper payloads.

## Inputs

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-run-summary.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/model-cache-inventory.json`
- `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.json`
- `data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.json`

## Expected Output

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-summary.json`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-report.md`

## Verification

Verify `opendataloader-quality-summary.json` parses as JSON, has exactly three per-paper reviews, includes every required quality dimension, distinguishes observed hybrid quality from Java-only fallback quality and not-proven dimensions, records runtime/model-cache cost and cache paths, and keeps graph/import/LadybugDB safety flags false. Verify the markdown report is non-empty and references all three article keys.

## Observability Impact

Quality summary exposes observed hybrid strengths, fallback behavior, remaining gaps, cache dependency, runtime cost, and confidence.
