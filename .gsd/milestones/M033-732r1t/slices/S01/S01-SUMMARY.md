---
id: S01
parent: M033-732r1t
milestone: M033-732r1t
provides:
  - Current daily-archive parser/conversion/refusal baseline matrix with input/output/diagnostic/safety contracts.
  - Comparison baseline for S02 GROBID, S03 OpenDataLoader, S04 quant-mind, S05 synthesis, and S06 quality plan.
requires:
  []
affects:
  []
key_files:
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
key_decisions:
  - Use the M031 fail-closed refusal-boundary chain as the baseline for external parser comparison.
  - Judge external parser outputs against provenance, diagnostics, source spans, and no-import safety flags, not text existence alone.
patterns_established:
  - Baseline-before-tool-evaluation: compare external parser claims against current stage contracts before running integration probes.
  - External parser success remains candidate evidence until mapped, reviewed, and explicitly authorized in a later milestone.
observability_surfaces:
  - data/article_corpora/m033-current-parser-baseline-v1/current-baseline-closeout.json
  - data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.md
  - data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.md
drill_down_paths:
  - .gsd/milestones/M033-732r1t/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S01/tasks/T03-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S01/tasks/T04-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S01/tasks/T05-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-05T07:36:18.524Z
blocker_discovered: false
---

# S01: Current Parser Baseline Map

**Mapped the current daily-archive parser/conversion/refusal baseline for external parser research.**

## What Happened

S01 created a baseline evidence package under `data/article_corpora/m033-current-parser-baseline-v1/`. T01 inventoried current pipeline entrypoints across catalog intake, source acquisition, loader evidence, parser/conversion, chunk/evidence, graph-readiness handoff, and no-write import boundary. T02 mapped existing M031 stage artifacts, contracts, counters, provenance expectations, and downstream consumers. T03 documented refusal diagnostics and fail-closed safety boundaries, including low-quality source handling and required false graph/import/LadybugDB flags. T04 synthesized a comparison matrix for GROBID, OpenDataLoader, and quant-mind research. T05 produced a closeout checklist proving the baseline artifacts are complete enough for downstream slices.

## Verification

Fresh slice-level `gsd_exec` verified all ten expected S01 artifacts are non-empty, `current-baseline-closeout.json` contains `status: passed`, the comparison baseline mentions OpenDataLoader and GROBID, and the safety-boundary artifact contains `graph_import_allowed`. Exit code 0.

## Requirements Advanced

- R053 — Established the current daily-archive baseline required before bounded comparison with opendataloader-pdf and GROBID.
- R027 — Documented current graph-readiness quality gaps for conversion fidelity, table/figure handling, section hierarchy, and evidence provenance as comparison targets only.
- R029 — Documented current chunk/evidence and import-readiness boundaries, preserving no positive import-ready claim.
- R050 — Mapped current pre-KG artifact candidate boundaries that external parser outputs may improve without KG import.

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

S01 does not evaluate GROBID, OpenDataLoader, or quant-mind directly. It prepares the baseline for S02, S03, S04, S05, and S06.

## Follow-ups

Proceed to S02 GROBID study, S03 OpenDataLoader hands-on probe, and S04 quant-mind pattern study using the S01 baseline artifacts.

## Files Created/Modified

- `data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.json` — Machine-readable stage entrypoint inventory.
- `data/article_corpora/m033-current-parser-baseline-v1/current-pipeline-entrypoints.md` — Human-readable stage entrypoint inventory.
- `data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.json` — Machine-readable current artifact contract map.
- `data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.md` — Human-readable current artifact contract map.
- `data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.json` — Machine-readable refusal and safety boundary model.
- `data/article_corpora/m033-current-parser-baseline-v1/refusal-and-safety-boundaries.md` — Human-readable refusal and safety boundary model.
- `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.json` — Machine-readable external parser comparison matrix.
- `data/article_corpora/m033-current-parser-baseline-v1/external-parser-comparison-baseline.md` — Human-readable external parser comparison matrix.
- `data/article_corpora/m033-current-parser-baseline-v1/current-baseline-closeout.json` — Machine-readable S01 closeout checklist.
- `data/article_corpora/m033-current-parser-baseline-v1/current-baseline-closeout.md` — Human-readable S01 closeout checklist.
