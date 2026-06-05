# S06: Bounded External Parser Quality Plan

**Goal:** Create a bounded future quality and integration plan for evaluating the recommended combined parser sidecar architecture after S05 completes, without weakening daily-archive import, graph-readiness, review, or no-write safety boundaries.
**Demo:** After this: a future implementation/probe milestone can run a bounded parser quality evaluation without weakening no-import safety boundaries.

## Must-Haves

- S06 is planned as dependent on S05 and will consume S05 recommendation artifacts during execution.
- The plan defines a future implementation/probe milestone scope with corpus selection, golden fixtures, metrics, and acceptance gates.
- Quality dimensions cover GROBID bibliography/citation/header/TEI quality, OpenDataLoader layout/OCR/table/coordinate quality, Adaptix typed adapter contract coverage, quant-mind-inspired tree/card/provenance schema fit, and daily-archive review/import boundaries.
- The plan includes verifier expectations, artifact contracts, no-secret/no-raw-text diagnostic rules, model/backend cache/lifecycle checks, failure taxonomy, and rollback/no-adoption conditions.
- All S06 artifacts keep graph/import/write safety flags false and state that no production integration is authorized by M033.

## Proof Level

- This slice proves: Planning artifact with validate-only closeout; no new parser runtime, graph import, LadybugDB write, dependency adoption, or production code changes.

## Integration Closure

S06 must produce repo-local planning artifacts under `data/article_corpora/m033-external-parser-quality-plan-v1/` suitable for a future milestone brief. It must not execute the future plan in M033.

## Verification

- Produces a future probe plan with quality metrics, gates, diagnostic surfaces, and fail-closed criteria.

## Tasks

- [x] **T01: Define bounded future probe scope and corpus strategy** `est:small`
  After S05 is complete, create the future quality milestone scope from the S05 recommendation: selected corpus classes, golden/fixture paper types, required source-locality controls, no-network defaults, model/backend cache checks, and excluded production actions.
  - Files: `data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.json`, `data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.md`, `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-events.jsonl`
  - Verify: Fresh command validates scope artifacts exist, include corpus classes and excluded production actions, and keep safety flags false.

- [x] **T02: Specify quality metrics and acceptance gates** `est:medium`
  Define measurable metrics and pass/fail gates for GROBID TEI/bibliography/citation quality, OpenDataLoader layout/OCR/table/coordinate quality, Adaptix adapter contract mapping, TreeKnowledge/PageIndex/card schema fit, source-span anchoring, reading order, low-quality source detection, and review packet completion.
  - Files: `data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.json`, `data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.md`
  - Verify: Fresh command validates required metric categories and gates exist, include graph-readiness review post-check expectations, and safety flags false.

- [x] **T03: Define artifact contracts, diagnostics, and failure taxonomy** `est:medium`
  Specify the artifact tree, JSON schemas or schema-shape expectations, diagnostic event taxonomy, no-secret/no-raw-text logging rule, typed blocker states, no-write import rehearsal expectations, rollback conditions, and adoption-decision thresholds for the future milestone.
  - Files: `data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.json`, `data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.md`, `data/article_corpora/m033-external-parser-quality-plan-v1/adoption-and-rollback-criteria.md`
  - Verify: Fresh command validates artifact/diagnostic contracts exist and include no raw text/secrets, typed blockers, no-write import rehearsal, rollback, and false safety flags.

- [x] **T04: Validate and close bounded quality plan** `est:small`
  Add a validate-only closeout checker for S06 artifacts and run the acceptance gate. It must reject missing quality dimensions, missing no-write/no-import boundaries, permissive flags, or any claim that M033 authorized production integration.
  - Files: `scripts/verify_m033_external_parser_quality_plan.py`, `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-summary.json`, `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-report.md`
  - Verify: `uv run python scripts/verify_m033_external_parser_quality_plan.py --plan-dir data/article_corpora/m033-external-parser-quality-plan-v1 && uv run ruff check scripts/verify_m033_external_parser_quality_plan.py` exits 0.

## Files Likely Touched

- data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.json
- data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.md
- data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-events.jsonl
- data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.json
- data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.md
- data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.json
- data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.md
- data/article_corpora/m033-external-parser-quality-plan-v1/adoption-and-rollback-criteria.md
- scripts/verify_m033_external_parser_quality_plan.py
- data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-summary.json
- data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-report.md
