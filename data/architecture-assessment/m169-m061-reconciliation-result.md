# M169 M061 Reconciliation Result

## Verdict

**M061 dynamic import debt is closed.**

S04 applied the bounded reconciliation from S03. `tests/test_m061_s03.py` now uses a normal `scripts.m061_synthesis` import, the focused M061 test passes, and the test architecture guard now reports zero dynamic and zero legacy allowlist entries.

## Impact analysis

GitNexus impact before editing:

```text
load_synthesis_module
  risk=LOW
  impactedCount=1
  affected_processes=0

test_synthesis_collect_summary_matches_written_artifact
  risk=LOW
  impactedCount=0
  affected_processes=0
```

No high or critical risk was reported.

## Changes

### `tests/test_m061_s03.py`

- Removed `importlib.util` and `sys` dynamic loader mechanics.
- Added `from scripts import m061_synthesis`.
- Removed `load_synthesis_module()` helper.
- Updated `test_synthesis_collect_summary_matches_written_artifact()` to call `m061_synthesis.collect_summary(...)` directly.
- Updated two stale protected hash expectations to match current tracked artifacts.
- Updated stale throughput expectations from current tracked decision and summary artifacts.

### `artifacts/m061-2hop/m061-summary.json`

Updated only the bounded S03-approved fields:

```text
anchors[0].average_pacing_delay_seconds = 2.6414815602045447
anchors[0].real_paper_throughput_per_min = 6.3918567952554275
aggregate.average_pacing_delay_seconds = 2.8385302972259456
aggregate.cumulative_real_paper_throughput_per_min = 6.929166867747222
```

### `data/test-architecture-alignment/test-architecture-allowlist.json`

- Removed `tests/test_m061_s03.py` from `dynamic_script_import`.
- Removed `tests/test_m061_s03.py` from `legacy_mixed`.
- Added `tests/test_m061_s03.py` to `strict_script_wrapper`.
- Added an M169 note.

Generated guardrail files updated after running the guard.

## Verification

| Check | Result | Evidence |
|---|---|---|
| Focused M061 test after bounded edits | PASS: 7 passed | `gsd_exec[99da0216-1c40-401e-9ca5-f287f5d9f091]` |
| Test architecture guard | PASS: dynamic=0, legacy=0, violations=0 | `gsd_exec[09a6ed55-f96e-40be-b7c8-2d451d8900d4]` |
| M061 plus guardrail tests | PASS: 13 passed | `gsd_exec[4dd1c75b-810d-4a43-a58f-101da23451fb]` |
| Scoped ruff | PASS | `gsd_exec[e42bb5bd-46b8-483d-8a15-4a54131cd5f3]` |
| Bounded diff check | PASS: expected import, allowlist, and metric changes present | `gsd_exec[7d012c0a-3464-4802-904f-27a9d9fd529d]` |

A first bounded diff check used plain `python` and failed with `python: command not found`; it was rerun with `uv run python` and passed.

## Final test architecture counts

```text
allowlisted_dynamic_script_import=0
allowlisted_legacy_mixed=0
strict_script_wrapper=57
strict_workflows=15
violations=0
```

## Residual risk

`artifacts/m061-2hop/m061-summary.json` is a historical artifact, but the rewrite was bounded to deterministic values from current tracked source artifacts and preserved safety, graph, decision, anchor ordering, request total, and HTTP 429 invariants from S03.
