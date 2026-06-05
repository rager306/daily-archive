# S01: Current Parser Baseline Map — UAT

**Milestone:** M033-732r1t
**Written:** 2026-06-05T07:36:18.524Z

# S01 UAT

A future agent can open `data/article_corpora/m033-current-parser-baseline-v1/` and understand the current daily-archive parser/conversion/refusal baseline before evaluating external tools.

Checks:

- `current-pipeline-entrypoints.*` identifies current stage entrypoints and produced artifacts.
- `current-artifact-contracts.*` maps stage inputs, outputs, counters, provenance, and downstream consumers.
- `refusal-and-safety-boundaries.*` records low-quality/refusal states and no-import safety flags.
- `external-parser-comparison-baseline.*` states what GROBID, OpenDataLoader, and quant-mind must answer.
- `current-baseline-closeout.*` records `status: passed`.

This UAT confirms S01 is a comparison baseline only. It does not adopt external parsers, authorize graph import, or write to LadybugDB.
