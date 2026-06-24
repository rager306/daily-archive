# M161 Dynamic Script Import Triage

Schema: `daily-archive-m161-dynamic-triage.v1`

## Current audit

| Metric | Count |
|---|---:|
| `dynamic_script_import` | 8 |
| `legacy_mixed` | 23 |
| `strict_script_wrapper` | 49 |
| `strict_infrastructure` | 6 |
| `unknown` | 77 |
| guardrail violations | 0 |

## Remaining dynamic candidates

- `tests/test_m045_project_trajectory.py`
- `tests/test_m060d_s01.py`
- `tests/test_m060g_s02.py`
- `tests/test_m061_s01.py`
- `tests/test_m061_s03.py`
- `tests/test_m062_s03.py`
- `tests/test_m066_s01.py`
- `tests/test_m067_s03.py`

## Bounded recheck results

Ran with a 120s per-file cap for the six candidates not previously known to time out.

| Candidate | Result |
|---|---|
| `tests/test_m045_project_trajectory.py` | fail: `reverse_adr_audit` `rule_count` expected 8 observed 10 |
| `tests/test_m060d_s01.py` | fail: `PROJECT.md` missing ADR template reference; trajectory verdict `drift_risk` not `on_track` |
| `tests/test_m061_s03.py` | fail: protected hash drift and synthesis aggregate drift |
| `tests/test_m062_s03.py` | fail: missing legacy `src/arxiv_archive/embedder.py` path |
| `tests/test_m066_s01.py` | fail: missing `## Top-3 candidates` heading |
| `tests/test_m067_s03.py` | fail: validation missing `M045 on_track` text |

Not re-run in this slice because they are known prior 300s timeout candidates:

- `tests/test_m060g_s02.py`
- `tests/test_m061_s01.py`

## Selected repair candidate for S04

`tests/test_m045_project_trajectory.py`

Rationale: smallest deterministic baseline failure. The clean-project reverse ADR audit now reports 10 rules instead of the stale expected 8. It is quick, local, does not require restoring legacy package shims, and avoids protected artifact hash churn.
