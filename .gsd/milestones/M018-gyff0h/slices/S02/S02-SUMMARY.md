---
id: S02
parent: M018-gyff0h
milestone: M018-gyff0h
provides:
  - ML reachability map
  - Runtime exposure classification
requires:
  []
affects:
  []
key_files:
  - .gsd/milestones/M018-gyff0h/slices/S02/run-evidence/ml-reachability-map.json
  - .gsd/milestones/M018-gyff0h/slices/S02/ml-reachability-report.md
key_decisions:
  - Torch/transformers findings are not directly imported by source code.
  - Docling fallback is reachable from source acquisition helpers and can process external PDFs.
  - Validation-batch CLI scan/preflight does not execute Docling fallback.
patterns_established:
  - Do not score dependency CVEs solely by package presence; classify import and input reachability.
  - Lazy fallback imports can still matter when they process external files.
observability_surfaces:
  - ml-reachability-map.json
  - ml-reachability-report.md
drill_down_paths:
  - .gsd/milestones/M018-gyff0h/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M018-gyff0h/slices/S02/tasks/T02-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-21T07:06:54.270Z
blocker_discovered: false
---

# S02: ML package reachability map

**S02 proved vulnerable ML packages are transitive and fallback-reachable, not directly used in main source paths.**

## What Happened

S02 mapped package references and runtime exposure. No direct torch/transformers imports exist in project source. The only direct docling source import is lazy inside `MDConverter._try_docling`. That path is reachable from source acquisition helpers when arxiv2md fails and Marker is unavailable, and it may process externally downloaded arXiv PDFs. Main validation-batch scan/preflight does not execute Docling. Production KG import/write remains disabled.

## Verification

Inline guard over ml-reachability-map.json and report passed.

## Requirements Advanced

- R046 — Adds reachability and runtime exposure evidence required for security triage.

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

S02 did not patch code or change dependencies; it only classified current reachability.

## Follow-ups

S03 must make an explicit recommendation: isolate/gate Docling fallback, defer broad torch/transformers upgrade, or remove/optionalize Docling if conversion fallback is not currently needed.

## Files Created/Modified

- `.gsd/milestones/M018-gyff0h/slices/S02/run-evidence/ml-reachability-map.json` — Machine-readable reachability map and classification.
- `.gsd/milestones/M018-gyff0h/slices/S02/ml-reachability-report.md` — Human-readable runtime exposure report.
