---
id: S05
parent: M033-732r1t
milestone: M033-732r1t
provides:
  - S06-ready combined parser architecture recommendation.
  - Risk and validation gate list for bounded future quality plan.
  - Component responsibility map across GROBID, OpenDataLoader, Adaptix, quant-mind patterns, and daily-archive validators.
requires:
  - slice: S01
    provides: Current parser/refusal baseline.
  - slice: S02
    provides: GROBID scholarly sidecar verdict.
  - slice: S03
    provides: OpenDataLoader hybrid sidecar verdict.
  - slice: S04
    provides: quant-mind pattern-source verdict.
  - slice: S07
    provides: Adaptix typed adapter candidate verdict.
affects:
  []
key_files:
  - scripts/verify_m033_combined_parser_architecture.py
  - data/article_corpora/m033-combined-parser-architecture-v1/synthesis-evidence-matrix.json
  - data/article_corpora/m033-combined-parser-architecture-v1/synthesis-evidence-matrix.md
  - data/article_corpora/m033-combined-parser-architecture-v1/synthesis-events.jsonl
  - data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.json
  - data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.md
  - data/article_corpora/m033-combined-parser-architecture-v1/complexity-and-validation-gates.json
  - data/article_corpora/m033-combined-parser-architecture-v1/complexity-and-validation-gates.md
  - data/article_corpora/m033-combined-parser-architecture-v1/combined-architecture-closeout-summary.json
  - data/article_corpora/m033-combined-parser-architecture-v1/combined-architecture-closeout-report.md
key_decisions:
  - Recommend a bounded combined sidecar architecture, not external parser replacement or production adoption.
  - Assign GROBID to scholarly TEI/metadata/references/citations, OpenDataLoader to layout/OCR/table/coordinate candidates, Adaptix to typed adapter mapping, quant-mind to pattern inspiration, and daily-archive to validation/review/graph-readiness ownership.
  - Parser outputs remain candidate evidence only and cannot imply graph import, LadybugDB write, or import eligibility.
patterns_established:
  - Combined sidecars terminate in daily-archive candidate contracts before validators and review gates.
  - Recommendation artifacts must include both accepted component roles and rejected alternatives.
  - Complexity/gate artifacts become the handoff into bounded quality planning.
observability_surfaces:
  - data/article_corpora/m033-combined-parser-architecture-v1/synthesis-evidence-matrix.json
  - data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.json
  - data/article_corpora/m033-combined-parser-architecture-v1/complexity-and-validation-gates.json
  - data/article_corpora/m033-combined-parser-architecture-v1/combined-architecture-closeout-summary.json
  - data/article_corpora/m033-combined-parser-architecture-v1/combined-architecture-closeout-report.md
drill_down_paths:
  - .gsd/milestones/M033-732r1t/slices/S05/tasks/T01-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S05/tasks/T02-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S05/tasks/T03-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S05/tasks/T04-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-05T11:47:40.242Z
blocker_discovered: false
---

# S05: Combined Parser Architecture Recommendation

**Recommended a bounded combined parser sidecar architecture without production adoption or graph/import authorization.**

## What Happened

S05 synthesized the completed M033 evidence into a combined architecture recommendation. T01 compiled the evidence matrix across S01 baseline, S02 GROBID, S03 OpenDataLoader, S07 Adaptix, and S04 quant-mind, preserving all expected verdicts and false safety flags. T02 recommended `recommended-bounded-combined-sidecar-architecture`: GROBID should serve as a scholarly TEI/metadata/references/citations sidecar; OpenDataLoader-style extraction should serve as a layout/OCR/table/coordinate sidecar; Adaptix should serve as a typed adapter layer over fixed parser JSON; quant-mind should contribute architecture patterns only; and daily-archive remains owner of contracts, validators, review gates, graph-readiness, and no-write import boundaries. T03 documented complexity and validation gates for S06, including GROBID runtime/accuracy, OpenDataLoader backend/cache, layout/table/OCR fidelity, source-span anchoring, Adaptix semantic limits, quant-mind no-runtime boundaries, and graph-readiness/no-write gates. T04 added a validate-only closeout checker and passed verifier plus Ruff.

## Verification

Fresh final acceptance verification passed. `uv run python scripts/verify_m033_combined_parser_architecture.py --architecture-dir data/article_corpora/m033-combined-parser-architecture-v1` returned `status: passed`, `failure_count: 0`, `verdict: recommended-bounded-combined-sidecar-architecture`; Ruff returned `All checks passed!`; an inline verifier confirmed all required S05 artifacts exist, expected prior-slice verdicts are present, production/runtime adoption is false, graph-readiness/no-write gates are present, closeout passed, and all safety flags are false. Exit code 0.

## Requirements Advanced

- R053 — Provides the bounded external parser architecture recommendation required by the external parser evaluation.
- R050 — Defines a candidate paper knowledge architecture using sidecar outputs, typed adapters, tree/card/provenance patterns, and daily-archive validators.
- R029 — Preserves graph-readiness/no-write review boundaries by rejecting direct parser-to-graph import and keeping all safety flags false.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None.

## Known Limitations

S05 is synthesis and recommendation only. It does not run a larger parser-quality evaluation, build production adapters, benchmark OCR/table quality, or authorize production dependency adoption.

## Follow-ups

Execute S06 to turn S05 risks into a bounded future quality/integration plan with corpus strategy, metrics, artifact contracts, diagnostics, rollback/no-adoption criteria, and verifier expectations.

## Files Created/Modified

- `scripts/verify_m033_combined_parser_architecture.py` — New fail-closed S05 closeout verifier.
- `data/article_corpora/m033-combined-parser-architecture-v1/` — New synthesis matrix, recommendation, risk/gate, event, and closeout artifacts.
