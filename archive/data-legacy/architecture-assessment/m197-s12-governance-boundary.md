# M197 S12 Governance Ratchet Boundary

## Verdict

**PASS: S12 may add governance ratchet tests and artifacts only.** Runtime source, queue semantics, rehearsal, smoke, graph backends, and schema migration code remain out of scope.

## GitNexus evidence

| Target | Risk | Result |
|---|---:|---|
| `Function:scripts/run_m197_reactive_dry_run.py:main` | LOW | impacted_count=1, affected_processes=[] |
| Query: `M197 governance ratchets no write import blocked reactive dry run tests` | n/a | Found S09/S10 tests, S09 script, M196 governance pattern, and M197 contract tests |

## Allowed S12 edits

- `tests/test_m197_governance_ratchets.py`
- S12 architecture assessment artifacts

## Disallowed S12 edits

- `scripts/run_m197_reactive_dry_run.py`
- `src/research_graph/workflows/universal_kb/reactive_runner.py`
- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/workflows/universal_kb/rehearsal.py`
- `src/research_graph/workflows/universal_kb/smoke_runner.py`
- `src/research_graph/workflows/universal_kb/smoke.py`
- Production graph backend code
- Schema migration code

## Required ratchets

- Required S09-S11 tests exist.
- Required S09-S11 architecture evidence artifacts exist.
- Reactive event contract keeps graph writes, schema migration, and import eligibility blocked.
- Dry-run script default command shape remains present.
- Dry-run script does not import queue/rehearsal/smoke modules.
- S10/S11 scope artifacts continue to disclaim queue/rehearsal/smoke edits.
- Governance tests remain compatible with M195/M196 ratchets.

## Downstream dependency map

- S13 consumes S12 as operator handoff safety proof.
- S14 consumes S12 for final compatibility sweep.
- S15 consumes S12 for validation readiness and requirement outcomes.
