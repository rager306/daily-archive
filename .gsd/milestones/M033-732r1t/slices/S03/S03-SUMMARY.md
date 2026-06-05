---
id: S03
parent: M033-732r1t
milestone: M033-732r1t
provides:
  - Empirical OpenDataLoader hybrid/docling-fast evidence for three local scientific PDFs.
  - Operational requirements and cache/runtime cost for OpenDataLoader hybrid backend.
  - Bounded contract mapping and sidecar-candidate verdict for S05/S06.
requires:
  []
affects:
  []
key_files:
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/environment-readiness.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/backend-health.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/model-cache-inventory.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-runbook.md
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/input-manifest.md
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/smoke-summary.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-run-summary.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-summary.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-quality-report.md
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-contract-mapping.md
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-probe-verdict.json
key_decisions:
  - Classified OpenDataLoader hybrid/docling-fast as a bounded `hybrid-sidecar-candidate`, not graph-ready or production-ready.
  - Treat scanned/OCR and table-fidelity claims as remaining evidence gaps despite successful hybrid processing of the three local PDFs.
patterns_established:
  - External parser probes should record environment, backend lifecycle, model cache, command metadata, per-paper outputs, and bounded verdicts before synthesis.
  - Hybrid parser success is candidate sidecar evidence only until reviewed against daily-archive chunk/evidence and graph-readiness contracts.
observability_surfaces:
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/environment-readiness.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/model-cache-inventory.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/run-events.jsonl
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-run-summary.json
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-probe-verdict.json
drill_down_paths:
  - .gsd/milestones/M033-732r1t/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S03/tasks/T02-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S03/tasks/T03-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S03/tasks/T04-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S03/tasks/T05-SUMMARY.md
  - .gsd/milestones/M033-732r1t/slices/S03/tasks/T06-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-05T08:31:46.395Z
blocker_discovered: false
---

# S03: OpenDataLoader OCR Layout Table Probe

**Completed a bounded OpenDataLoader hybrid PDF probe on three local scientific PDFs.**

## What Happened

S03 prepared and executed a hands-on OpenDataLoader PDF probe. T01 recorded the confirmed environment/backend/cache readiness: OpenJDK 17, Maven 3.8.7, uv, Python 3.13, vendor JAR build, Python 3.13 wheel build/install/import, Java-only wrapper smoke, hybrid extras, backend health, hybrid smoke, and Docling model cache paths. T02 froze three local PDF inputs with hashes and challenge roles: ReCA for layout/figure-heavy, Recursive Language Models for text/section-heavy, and GEPA for fallback/problem-case. T03 promoted smoke evidence into formal setup artifacts for Java-only and hybrid modes. T04 ran all three PDFs through the Python 3.13 wrapper with hybrid docling-fast backend; all three passed without Java-only fallback and produced JSON, Markdown, HTML, and text outputs. T05 reviewed output quality across section hierarchy, reading order, tables, figures/captions, bibliography, OCR, coordinate/layout metadata, Markdown, JSON, and diagnostics, while marking scanned/OCR quality as not proven. T06 mapped results to daily-archive contracts and produced a bounded `hybrid-sidecar-candidate` verdict. No graph import, production import, or LadybugDB write was attempted or authorized.

## Verification

Fresh final acceptance `gsd_exec` verified all expected S03 artifacts exist and are non-empty, `environment-readiness.json` has `status: ready_for_hybrid_probe`, the manifest contains three entries, smoke and full run summaries have `status: passed`, all three per-paper results passed with `backend_mode: hybrid-docling-fast`, `fallback_used:false`, `model_cache_used:true`, per-paper JSON/Markdown/Text outputs are non-empty, quality summary has three reviews with ten dimensions each, verdict is `hybrid-sidecar-candidate`, safety flags are false, and all run-events lines parse as JSON. Exit code 0.

## Requirements Advanced

- R053 — Completed a bounded external parser probe for OpenDataLoader with local PDFs, environment/backend/cache evidence, quality review, and contract mapping.
- R027 — Provided candidate evidence about layout-aware conversion quality, section hierarchy, table signals, figures/captions, and coordinate metadata while preserving review gaps.
- R029 — Mapped outputs to graph-readiness boundaries and kept import eligibility false pending independent review.
- R050 — Generated candidate pre-KG sidecar artifacts and mapping evidence without promoting them to KG facts.

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

OCR quality for scanned/image-only PDFs is not proven by this run; table fidelity is qualitative because no ground-truth table benchmark was used; runtime is heavy and depends on cached Hugging Face Docling models or network access if cache is absent.

## Follow-ups

Use S03 findings in S05 synthesis and S06 bounded quality plan. Consider a future larger hybrid probe with scanned PDFs and table ground truth before any production sidecar schema commitment.

## Files Created/Modified

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/` — Created OpenDataLoader probe readiness, manifest, smoke, per-paper outputs, quality, mapping, and verdict artifacts.
