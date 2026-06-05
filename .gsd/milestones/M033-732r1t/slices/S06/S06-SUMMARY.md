---
id: S06
parent: M033-732r1t
milestone: M033-732r1t
provides:
  - Future milestone-ready quality plan for combined parser sidecar evaluation.
  - Metric/gate framework covering GROBID, OpenDataLoader, Adaptix, quant-mind-inspired schemas, and daily-archive review/import boundaries.
  - Artifact and diagnostic contract for no-secret/no-raw-body, no-write, fail-closed parser-quality probes.
requires:
  - slice: S05
    provides: Combined parser architecture recommendation and complexity/validation gates.
affects:
  []
key_files:
  - scripts/verify_m033_external_parser_quality_plan.py
  - data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.md
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-events.jsonl
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.md
  - data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.md
  - data/article_corpora/m033-external-parser-quality-plan-v1/adoption-and-rollback-criteria.md
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-summary.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-report.md
key_decisions:
  - S06 is a future quality plan only; M033 does not execute the future probe or authorize production integration.
  - Future quality evaluation must include graph-readiness review post-check before manifest synthesis and keep no-write import rehearsal counts at zero unless separately authorized outside M033.
  - Diagnostics must use typed failure codes and must not log secrets or raw article bodies.
patterns_established:
  - Future external-parser probes should define corpus classes, quality dimensions, artifact contracts, diagnostics, rollback criteria, and verifier expectations before implementation.
  - No-network/cache preflight should be a first-class gate for hybrid parser backends.
  - Parser-quality success must remain separate from graph-readiness/import eligibility.
observability_surfaces:
  - data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-summary.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-report.md
drill_down_paths:
  - .gsd/milestones/M033-732r1t/slices/S06/tasks/T01-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S06/tasks/T02-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S06/tasks/T03-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S06/tasks/T04-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-05T11:53:27.127Z
blocker_discovered: false
---

# S06: Bounded External Parser Quality Plan

**Completed a fail-closed future quality plan for evaluating the combined parser sidecar architecture.**

## What Happened

S06 turned the S05 recommendation into a bounded future quality/integration plan without executing the future probe in M033. T01 defined corpus classes and source/runtime controls for a future milestone: native digital PDFs, long/appendix-heavy PDFs, table-heavy PDFs, figure/layout-heavy PDFs, scanned/image-only controls, and low-quality/metadata-only controls, all under no-network/local-source/cache preflight expectations. T02 specified quality metrics and acceptance gates for GROBID TEI/bibliography/citation quality, OpenDataLoader layout/OCR/table/coordinate quality, Adaptix adapter contract coverage, tree/PageIndex/card/provenance schema fit, source-span anchoring, low-quality/refusal preservation, and review packet/graph-readiness boundaries. T03 defined future artifact contracts, diagnostics, failure taxonomy, no-secret/no-raw-body logging rules, no-write import rehearsal, rollback triggers, and non-authorizations. T04 added and passed a validate-only closeout checker. The final verdict is `bounded-future-quality-plan-ready`.

## Verification

Fresh final acceptance verification passed. `uv run python scripts/verify_m033_external_parser_quality_plan.py --plan-dir data/article_corpora/m033-external-parser-quality-plan-v1` returned `status: passed`, `failure_count: 0`, `verdict: bounded-future-quality-plan-ready`; Ruff returned `All checks passed!`; an inline verifier confirmed all required S06 artifacts exist, the future probe was not executed in M033, at least six corpus classes are present, graph import is excluded, seven metric categories exist, review post-check command includes `--require-completed-review`, no-secret/no-raw-body logging rules are present, no-write rehearsal counts are zero, production integration is unauthorized, closeout passed, and all safety flags are false. Exit code 0.

## Requirements Advanced

- R053 — Completes the bounded follow-up quality/integration plan required by the external parser evaluation.
- R050 — Defines quality gates for future paper knowledge architecture sidecar candidates and typed provenance/schema fit.
- R029 — Preserves graph-readiness/no-write boundaries by requiring review post-check and no-write import rehearsal with all safety flags false.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

T04 initially failed because markdown artifacts lacked exact component/cache wording expected by the closeout checker. The markdown was clarified and the gate passed; no safety checks were weakened.

## Known Limitations

The plan does not select final concrete paper IDs, implement adapters, run a larger parser-quality benchmark, or authorize production integration. A future milestone must bind metric categories to exact thresholds after corpus selection.

## Follow-ups

Use S06 as the basis for a future implementation/probe milestone if the user wants to evaluate the recommended combined sidecar architecture on a larger corpus.

## Files Created/Modified

- `scripts/verify_m033_external_parser_quality_plan.py` — New fail-closed S06 closeout verifier.
- `data/article_corpora/m033-external-parser-quality-plan-v1/` — New future scope, metrics/gates, artifact contracts/diagnostics, rollback, event, and closeout artifacts.
