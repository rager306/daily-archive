---
id: T01
parent: S02
milestone: M061-0fib2i
key_files:
  - scripts/m060c_applicability_matrix.py
  - artifacts/m060c-benchmark/applicability-matrix.json
  - artifacts/m060c-benchmark/applicability-matrix.md
  - doc/adr/ADR-016-graph-library-selection.md
  - artifacts/m060c-benchmark/m061-m065-decision.md
key_decisions:
  - NetworkX remains the primary graph representation and correctness baseline.
  - igraph is adopted as a supplementary read-only accelerator for algorithm-heavy M060b and M061 work.
  - rustworkx is adopted when available for traversal/path hot spots with NetworkX parity or fallback checks.
  - graph-tool, PyG, DGL, NetworkX-Temporal, and GraphScope remain deferred except GraphScope as a future M063 GraphDB-selection candidate.
duration: 
verification_result: passed
completed_at: 2026-06-13T05:17:14.705Z
blocker_discovered: false
---

# T01: Built the M060c S02 applicability matrix, ADR-016, and M061-M065 decision document.

**Built the M060c S02 applicability matrix, ADR-016, and M061-M065 decision document.**

## What Happened

Added a deterministic applicability-matrix generator for 8 graph libraries across 5 future milestones, generated JSON and Markdown artifacts, authored binding ADR-016 using the M034 14-section structure with LLM Reading Notes, and wrote the Russian per-milestone decision document. The decision keeps NetworkX primary, adopts igraph as a supplementary read-only accelerator for algorithm-heavy M060b/M061 work, adopts rustworkx when available for traversal/path hot spots, and defers graph-tool, PyG, DGL, NetworkX-Temporal, and GraphScope except for future M063 GraphDB-selection evaluation.

## Verification

Generated the matrix with `uv run python scripts/m060c_applicability_matrix.py`; verified via `uv run pytest tests/test_m060c_s02.py -q` with 8 passing tests; verified no forbidden loopback hostname appears in new source or markdown.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/m060c_applicability_matrix.py` | 0 | ✅ pass | 1000ms |
| 2 | `uv run pytest tests/test_m060c_s02.py -q` | 0 | ✅ pass: 8 passed in 0.17s | 8600ms |
| 3 | `rg -n 'localhost' scripts/m060c_applicability_matrix.py tests/test_m060c_s02.py doc/adr/ADR-016-graph-library-selection.md artifacts/m060c-benchmark/applicability-matrix.md artifacts/m060c-benchmark/m061-m065-decision.md artifacts/m060c-benchmark/applicability-matrix.json || true` | 0 | ✅ pass: no matches | 1000ms |

## Deviations

The S02 plan text says 7 libraries, while the explicit task instruction requires 8 libraries. Implemented 8 libraries per the task instruction.

## Known Issues

None.

## Files Created/Modified

- `scripts/m060c_applicability_matrix.py`
- `artifacts/m060c-benchmark/applicability-matrix.json`
- `artifacts/m060c-benchmark/applicability-matrix.md`
- `doc/adr/ADR-016-graph-library-selection.md`
- `artifacts/m060c-benchmark/m061-m065-decision.md`
