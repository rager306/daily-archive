# S01: Current Parser Baseline Map

**Goal:** Map the current daily-archive article processing baseline from source/catalog inputs through acquisition, loader, parser/conversion, refusal diagnostics, chunk/evidence handoff, and no-write graph-readiness boundaries so external parser tools have a concrete comparison target.
**Demo:** After this: daily-archive's current parser/conversion/refusal contracts are mapped as the comparison baseline for external tools.

## Must-Haves

- Current parser/conversion/source-loader entrypoints and scripts are identified with GitNexus-backed references.
- Existing artifacts and data contracts are inventoried by stage: catalog, acquisition, loader, parser/conversion, chunk/evidence, graph-readiness review, no-write import refusal.
- Refusal diagnostics, low-quality source handling, fail-closed safety flags, and forbidden claims are summarized.
- Baseline gaps are explicit for layout, tables, figures/captions, bibliography/citations, OCR, section hierarchy, source spans, and output quality.
- A comparison matrix exists for S02 GROBID, S03 OpenDataLoader, S04 quant-mind, and S05 synthesis to consume.
- The slice makes no external parser adoption, graph-readiness, production import, or LadybugDB write claim.

## Proof Level

- This slice proves: research artifact with GitNexus-backed code references and existing artifact references

## Integration Closure

Produces the baseline contract consumed by S02, S03, S04, S05, and S06. It does not run external tools or modify production code.

## Verification

- Creates stable baseline artifacts under data/article_corpora/m033-current-parser-baseline-v1 so future agents can inspect current pipeline assumptions, gaps, and safety boundaries.

## Tasks

- [x] **T01: Inventory current parser pipeline entrypoints** `est:45m`
  Use GitNexus and repository evidence to identify the current daily-archive scripts/modules/tests that participate in catalog intake, source acquisition, loader evidence, parser/conversion, chunk/evidence replay, graph-readiness package generation, and no-write import refusal. Record file paths, symbol/process names where available, stage ownership, and what each entrypoint produces. Do not edit code.
  - Files: `data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.json`, `data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.md`
  - Verify: Manual review — file exists and is non-empty

- [x] **T02: Map existing stage artifacts and contracts** `est:45m`
  Build a stage-by-stage artifact contract map from M031 and current data: catalog/intake, acquisition, loader evidence, parser/conversion, chunk/evidence, graph-readiness reviewer packets, continuity audit, and no-write import rehearsal. For each stage, record inputs, outputs, key fields, expected counters, hashes/provenance, and downstream consumers.
  - Files: `data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.json`, `data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.md`
  - Verify: Manual review — file exists and is non-empty

- [x] **T03: Document refusal diagnostics and safety boundaries** `est:35m`
  Summarize the current fail-closed model: low-quality source handling, metadata-only rows, missing-source blockers, unsafe path checks, parser-ready/chunk-ready refusal rules, graph-readiness review requirements, no-write import flags, and forbidden positive claims. Separate implementation evidence from requirement scope so external parser outputs remain candidate evidence only.
  - Files: `data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.json`, `data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.md`
  - Verify: Manual review — file exists and is non-empty

- [x] **T04: Create external parser comparison baseline matrix** `est:40m`
  Synthesize T01-T03 into a comparison matrix for GROBID, OpenDataLoader, and quant-mind research. Identify current strengths, weaknesses, missing capabilities, and exact questions external tools must answer for layout, tables, figures/captions, bibliography/citations, OCR, reading order, section hierarchy, source spans, Markdown/JSON quality, runtime complexity, and provenance.
  - Files: `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.json`, `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.md`
  - Verify: Manual review — file exists and is non-empty

- [x] **T05: Validate baseline artifact completeness** `est:20m`
  Perform a final artifact completeness check for S01: all expected JSON/Markdown artifacts exist, contain the required stage names and safety/no-import language, and point to downstream consumers. Record a closeout checklist for S01 without running external tools or changing code.
  - Files: `data/article_corpora/m033-current-parser-baseline-v1/current-baseline-closeout.json`, `data/article_corpora/m033-current-parser-baseline-v1/current-baseline-closeout.md`
  - Verify: Manual review — file exists and is non-empty

## Files Likely Touched

- data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.json
- data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.md
- data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.json
- data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.md
- data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.json
- data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.md
- data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.json
- data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.md
- data/article_corpora/m033-current-parser-baseline-v1/current-baseline-closeout.json
- data/article_corpora/m033-current-parser-baseline-v1/current-baseline-closeout.md
