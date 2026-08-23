# M162 Timeout Candidate Bounded Probes

## Summary

M161 left two timeout-risk dynamic candidates: `tests/test_m060g_s02.py` and `tests/test_m061_s01.py`. S03 used only bounded probes and did not repeat any 300 second baseline run.

After S03:

- `allowlisted_dynamic_script_import`: 3
- `allowlisted_legacy_mixed`: 18
- `strict_script_wrapper`: 54
- `violations`: 0

## M060G

Prior issue: earlier full baseline attempt timed out at 300 seconds.

Bounded probes:

| Command | Exit | Duration | Result |
|---|---:|---:|---|
| `timeout 45 uv run pytest tests/test_m060g_s02.py --collect-only -q` | 0 | 933 ms | 11 tests collected quickly |
| `timeout 45 uv run pytest tests/test_m060g_s02.py -q -k 'not figure_judge_runs'` | 1 | 1032 ms | quick stale import failure: `article_artifact_worker` |
| normal `scripts.m060g_figure_judge` import probe | 0 | 144 ms | import works |
| `uv run pytest tests/test_m060g_s02.py -q -k 'not figure_judge_runs'` after repair | 0 | 230 ms | 10 passed, 1 deselected |

Repair:

- Replaced dynamic `m060g_figure_judge` import with normal `from scripts import m060g_figure_judge`.
- Replaced stale `article_artifact_worker.HttpTransport` import with canonical `research_graph.infrastructure.papers.artifacts.worker.HttpTransport`.
- Promoted the test from dynamic and legacy allowlists to strict script-wrapper.

Remaining limit: the live `figure_judge_runs` test remains guarded by API-key skip and was not forced.

## M061

Prior issue: earlier full baseline attempt timed out at 300 seconds.

Bounded probes:

| Command | Exit | Duration | Result |
|---|---:|---:|---|
| `timeout 45 uv run pytest tests/test_m061_s01.py --collect-only -q` | 0 | 915 ms | 8 tests collected quickly |
| normal `scripts.m061_anchor_pilot` import probe | 0 | 185 ms | import works |
| default output probe | 0 | 195 ms | existing `pipeline-summary.json` present |
| `uv run pytest tests/test_m061_s01.py --collect-only -q` after repair | 0 | 190 ms | 8 tests collected |

Repair:

- Replaced dynamic `m061_anchor_pilot` import with normal `from scripts import m061_anchor_pilot`.
- Promoted the test from dynamic and legacy allowlists to strict script-wrapper for import architecture.

Remaining limit: full file still uses a session fixture that runs the real 30-paper pilot; S03 intentionally did not rerun that full path.

## Verification

- `uv run python scripts/verify_test_architecture.py --json` passed with `violations=0`.
- `uv run ruff check tests/test_m060g_s02.py tests/test_m061_s01.py scripts/m060g_figure_judge.py scripts/m061_anchor_pilot.py` passed.
- `uv run pyrefly check tests/test_m060g_s02.py tests/test_m061_s01.py scripts/m060g_figure_judge.py scripts/m061_anchor_pilot.py` passed with 0 errors.
