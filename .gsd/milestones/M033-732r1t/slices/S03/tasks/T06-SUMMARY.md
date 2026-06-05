---
id: T06
parent: S03
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-contract-mapping.md
  - data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-probe-verdict.json
key_decisions:
  - Classify OpenDataLoader hybrid/docling-fast as a bounded `hybrid-sidecar-candidate`, not graph-ready or production-ready.
duration: 
verification_result: passed
completed_at: 2026-06-05T08:30:32.321Z
blocker_discovered: false
---

# T06: Mapped OpenDataLoader hybrid probe results to daily-archive contracts with a bounded sidecar-candidate verdict.

**Mapped OpenDataLoader hybrid probe results to daily-archive contracts with a bounded sidecar-candidate verdict.**

## What Happened

Created `opendataloader-contract-mapping.md` and `opendataloader-probe-verdict.json`. The mapping covers SourceRef, EvidencePath, PageIndex, SemanticChunk, table artifact, refusal diagnostic, and graph-readiness packet boundaries. The bounded verdict is `hybrid-sidecar-candidate`: all three PDFs processed successfully with hybrid docling-fast and produced useful outputs, but OCR on scanned PDFs, table fidelity, independent review, chunk validation, and graph-readiness remain unproven. Operational requirements include the Python 3.13 venv, OpenJDK/Maven, backend lifecycle, Hugging Face cache paths/snapshots/sizes, and network dependency if cache is absent. Safety flags remain false.

## Verification

Fresh `gsd_exec` generated and verified the verdict and mapping artifacts; parsed JSON; checked the verdict is in the allowed bounded set, safety flags false, references to quality summary and model-cache inventory exist, all seven contract boundaries are covered, and the markdown mapping is non-empty. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gsd_exec T06 verdict/mapping generation and verification` | 0 | ✅ pass | 156ms |

## Deviations

None.

## Known Issues

The verdict is a bounded research sidecar candidate, not a production adoption decision. A larger hybrid probe and table/OCR-specific checks remain needed before schema or integration commitments.

## Files Created/Modified

- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-contract-mapping.md`
- `data/article_corpora/m033-opendataloader-pdf-probe-v1/opendataloader-probe-verdict.json`
