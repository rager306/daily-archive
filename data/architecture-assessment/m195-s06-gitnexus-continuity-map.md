# M195 S06 GitNexus Continuity Map

## Verdict

**PASS as audit-only with HIGH source-edit caution.** GitNexus confirms `UniversalKBQueue` is the central continuity seam for queue tests, soak script, substrate rehearsal, smoke runner, and no-write rehearsal. `_dependencies_satisfied` is HIGH impact at depth 3, so future edits to dependency unblocking require explicit pre-edit warning and targeted compatibility tests.

## GitNexus evidence

| Target | Result | Affected processes |
|---|---|---|
| `UniversalKBQueue` class context | Direct import surface identified | `tests/test_universal_kb_queue.py`, `scripts/soak_universal_kb_queue.py`, `substrate_rehearsal.py`, `smoke_runner.py`, `rehearsal.py` |
| `UniversalKBQueue._dependencies_satisfied#1` | HIGH, impactedCount=5, direct=2, processes_affected=3 | `run_universal_kb_no_write_rehearsal`, `run_article`, `smoke.py main` |
| `run_universal_kb_no_write_rehearsal` | LOW, impactedCount=0 | none upstream |
| `smoke_runner.run_article` | LOW, impactedCount=3, processes_affected=1 | `smoke.py main` |

## Continuity consumers

- `tests/test_universal_kb_queue.py`: direct contract and state-machine tests.
- `scripts/soak_universal_kb_queue.py`: operational soak surface.
- `src/research_graph/workflows/universal_kb/substrate_rehearsal.py`: no-write substrate rehearsal boundary.
- `src/research_graph/workflows/universal_kb/smoke_runner.py`: smoke article runner.
- `src/research_graph/workflows/universal_kb/rehearsal.py`: no-write rehearsal execution.
- `src/research_graph/workflows/universal_kb/smoke.py`: CLI/main path downstream of `run_article`.

## Source-edit caution

Because `_dependencies_satisfied` is HIGH impact after S05, S06 makes no source edits. Any future change to dependency satisfaction or unblocking must be preceded by fresh exact GitNexus impact and a user-visible HIGH/CRITICAL warning before editing.

## Required compatibility tests for future dependency edits

- `uv run pytest tests/test_universal_kb_queue.py -q`
- `uv run pytest tests/test_universal_kb_rehearsal.py tests/test_universal_kb_substrate_rehearsal.py -q`
- targeted smoke runner tests if the next change affects article execution paths

## Boundary statement

This map is artifact-only. It does not change queue behavior, graph projection, backend adapters, or import eligibility.
