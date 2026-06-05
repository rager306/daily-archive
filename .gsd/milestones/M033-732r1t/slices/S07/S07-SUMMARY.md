---
id: S07
parent: M033-732r1t
milestone: M033-732r1t
provides:
  - Empirical proof that Adaptix can load OpenDataLoader fixed JSON into typed intermediate models across all three S03 PDFs.
  - A bounded adapter verdict and tests for S05/S06 parser architecture planning.
  - A reusable script/verifier pattern for future external parser adapter probes.
requires:
  []
affects:
  []
key_files:
  - scripts/probe_m033_opendataloader_adaptix_adapter.py
  - scripts/verify_m033_opendataloader_adaptix_adapter.py
  - tests/test_m033_opendataloader_adaptix_adapter.py
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-summary.json
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-diagnostics.jsonl
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-report.md
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-summary.json
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-report.md
key_decisions:
  - Use Adaptix as a structural adapter layer over OpenDataLoader's fixed JSON rather than modifying OpenDataLoader or expecting custom output schemas.
  - Treat Adaptix adapter outputs as review-only candidate summaries with graph/import/LadybugDB safety flags false.
patterns_established:
  - External parser JSON should first load into typed intermediate adapter models before mapping to daily-archive candidate contracts.
  - Adapter success is a structural boundary proof, not semantic parser-quality or graph-readiness proof.
observability_surfaces:
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-summary.json
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-diagnostics.jsonl
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-summary.json
  - data/article_corpora/m033-opendataloader-adaptix-probe-v1/adaptix-adapter-closeout-report.md
drill_down_paths:
  - .gsd/milestones/M033-732r1t/slices/S07/tasks/T01-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S07/tasks/T02-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S07/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-05T08:59:36.754Z
blocker_discovered: false
---

# S07: Adaptix OpenDataLoader Adapter Probe

**Completed a bounded Adaptix adapter probe over OpenDataLoader fixed JSON outputs.**

## What Happened

S07 tested the proposed Adaptix + OpenDataLoader architecture with real S03 outputs. T01 implemented `scripts/probe_m033_opendataloader_adaptix_adapter.py`, defining typed dataclasses for OpenDataLoader documents/elements, using Adaptix `name_mapping` for space-containing OpenDataLoader field names, preserving extra heterogeneous fields, and generating review-only candidate summaries for SourceRef/PageIndex/semantic signals. T02 added focused tests for alias mapping, extra-field preservation, malformed fail-closed behavior, verifier acceptance, permissive flag rejection, and missing-output handling. T03 added a validate-only verifier, ran the probe over all three S03 OpenDataLoader JSON outputs, validated closeout artifacts, and passed focused pytest plus Ruff. The result is a bounded `adaptix-adapter-candidate` verdict: Adaptix is suitable as a structural adapter over OpenDataLoader fixed JSON, but not as a semantic quality, graph-readiness, or import-eligibility gate.

## Verification

Fresh final acceptance `gsd_exec` verified all expected S07 artifacts exist and are non-empty, `adaptix-adapter-summary.json` has `status: adaptix-adapter-candidate`, `paper_count: 3`, `error_count: 0`, closeout has `status: passed`, `failure_count: 0`, all three results have `status: mapped_candidate_only`, candidate-only SourceRef summaries, positive top-level element counts, false safety flags, no error diagnostics, valid JSONL diagnostics, and report safety lines for `graph_import_allowed=false` and `ladybugdb_written=false`. Exit code 0. The full T03 gate also passed: probe status candidate, verifier passed, `6 passed`, and Ruff `All checks passed!`.

## Requirements Advanced

- R053 — Adds Adaptix adapter evidence to the bounded external parser evaluation track without changing import safety.
- R050 — Tests a typed pre-KG adapter pattern for candidate sidecar artifacts while keeping graph readiness false.
- R029 — Preserves graph-readiness boundaries by separating structural adapter success from semantic/review/import eligibility.

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

Adaptix proves structural mapping only. It does not validate reading-order correctness, table fidelity, OCR quality, citation correctness, graph readiness, or production import eligibility.

## Follow-ups

Use S07 findings in S05 synthesis: recommend OpenDataLoader fixed JSON -> Adaptix typed intermediate adapter -> daily-archive candidate contracts -> independent validators/review gates. Consider expanding the adapter into production only in a future implementation milestone with broader fixtures and contract tests.

## Files Created/Modified

- `scripts/probe_m033_opendataloader_adaptix_adapter.py` — New Adaptix-based OpenDataLoader JSON adapter probe.
- `scripts/verify_m033_opendataloader_adaptix_adapter.py` — New validate-only verifier for adapter artifacts.
- `tests/test_m033_opendataloader_adaptix_adapter.py` — Focused tests for adapter mapping and safety invariants.
- `data/article_corpora/m033-opendataloader-adaptix-probe-v1/` — Generated adapter summary, diagnostics, report, and closeout artifacts.
