---
id: S02
parent: M033-732r1t
milestone: M033-732r1t
provides:
  - GROBID runtime/container requirement evidence for S05/S06.
  - Real TEI outputs for three local PDFs.
  - A fail-closed GROBID scholarly sidecar verdict and contract mapping against daily-archive baseline.
requires:
  - slice: S01
    provides: Current parser/conversion/refusal baseline and external parser comparison matrix.
affects:
  []
key_files:
  - scripts/verify_m033_grobid_probe.py
  - data/article_corpora/m033-grobid-probe-v1/grobid-runtime-readiness.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-runtime-runbook.md
  - data/article_corpora/m033-grobid-probe-v1/grobid-service-health.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-run-summary.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-tei-quality-summary.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-contract-mapping.md
  - data/article_corpora/m033-grobid-probe-v1/grobid-probe-verdict.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-closeout-summary.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-closeout-report.md
  - data/article_corpora/m033-grobid-probe-v1/per-paper/
key_decisions:
  - Use GROBID CRF Docker for bounded S02 because native build requires JDK 21 and local Java is 17.
  - Classify GROBID as `grobid-scholarly-sidecar-candidate`, not as a graph-ready parser or production import trigger.
  - Defer full/DL GROBID image comparison to a future quality milestone if bibliography/citation accuracy needs deeper measurement.
patterns_established:
  - External scholarly parser services should produce sidecar TEI/candidate artifacts with independent validators and false graph/import/write flags.
  - GROBID TEI is best treated as a scholarly structure source; layout/table/OCR evidence should remain a separate sidecar concern.
observability_surfaces:
  - data/article_corpora/m033-grobid-probe-v1/grobid-events.jsonl
  - data/article_corpora/m033-grobid-probe-v1/grobid-service-health.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-run-summary.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-tei-quality-summary.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-closeout-summary.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-closeout-report.md
drill_down_paths:
  - .gsd/milestones/M033-732r1t/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S02/tasks/T02-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S02/tasks/T03-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S02/tasks/T04-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-05T10:19:26.763Z
blocker_discovered: false
---

# S02: GROBID Scholarly Parsing Study

**Completed a bounded GROBID CRF Docker study with real TEI outputs and fail-closed contract mapping.**

## What Happened

S02 evaluated GROBID as a scholarly parser candidate against daily-archive contracts. T01 documented runtime requirements and confirmed that vendored GROBID source requires JDK 21 while the local runtime is Java 17, so the bounded probe used Docker. The `grobid/grobid:0.9.0-crf` image was pulled and verified. T02 started the GROBID CRF service on port 8070, waited for API readiness, recorded health/version/model status, and submitted the three S03 local PDFs to `/api/processFulltextDocument`; all three produced non-empty TEI XML with request diagnostics. T03 parsed the TEI outputs and mapped them to daily-archive needs: GROBID is a strong candidate for scholarly metadata, abstract, section hierarchy, bibliography, and citation/ref marker sidecars, with partial coordinate/table/figure support. It does not prove OCR quality, table fidelity, reading-order correctness, graph readiness, or production import eligibility. T04 added a validate-only closeout checker and verified all artifacts and safety boundaries.

## Verification

Fresh final acceptance verification passed. `uv run python scripts/verify_m033_grobid_probe.py --probe-dir data/article_corpora/m033-grobid-probe-v1` returned `status: passed`, `failure_count: 0`, `verdict: grobid-scholarly-sidecar-candidate`; Ruff returned `All checks passed!`; an additional inline verifier confirmed all required artifacts exist, run summary has `tei-probe-complete` with `success_count: 3` and `failure_count: 0`, quality summary has `grobid-tei-candidate-evidence`, verdict is candidate-only, closeout passed with zero failures, per-paper TEI/diagnostics exist, and all safety flags remain false. Exit code 0.

## Requirements Advanced

- R053 — Adds bounded GROBID evidence to the external parser evaluation requirement without weakening import safety.
- R050 — Contributes a scholarly TEI sidecar candidate pattern for pre-KG article processing.
- R029 — Preserves graph-readiness boundaries by keeping GROBID output candidate-only and requiring independent review.

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

The probe used the CRF-only Docker image, not the full/DL image; therefore it proves API/TEI contract shape and useful scholarly sidecar evidence, but not best possible bibliography/citation accuracy. TEI tables/figures/coordinates remain candidates and need independent source-span/layout validation before any downstream use.

## Follow-ups

Use S02 findings in S05 synthesis with S03/S07: GROBID is the scholarly TEI/bibliography/section sidecar candidate; OpenDataLoader remains the layout/table/OCR sidecar candidate; Adaptix remains a typed adapter pattern. If accuracy matters, plan a future full/DL GROBID comparison under the same fail-closed constraints.

## Files Created/Modified

- `scripts/verify_m033_grobid_probe.py` — New validate-only closeout verifier for S02 GROBID probe artifacts.
- `data/article_corpora/m033-grobid-probe-v1/` — New runtime, service health, TEI output, quality, contract mapping, verdict, and closeout artifacts.
