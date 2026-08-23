# M170 Queue Soak Contract

## Verdict

**M170 should add a reusable longer UniversalKBQueue soak harness and run it as bounded runtime proof.**

The harness should not become a slow default CI test. It should be a small script or reusable helper with explicit parameters and JSON diagnostics.

## Starting point

M169 proof:

```text
jobs=16
processes=4
rounds=1
join_timeout_seconds=15
lease_seconds=30
repeat=5/5 focused pytest target passed
```

M169 assertions:

- no process remains alive after join timeout;
- no worker errors;
- every worker reports done;
- exactly all jobs complete;
- completed job ids are unique;
- every final job status is `succeeded`;
- every job has exactly one `claim` and one `complete` event.

Current test: `tests/test_universal_kb_queue.py::test_multiprocess_stress_claims_and_completes_each_job_once`.

## M170 longer soak parameters

Runtime proof target:

```text
jobs_per_round=64
processes=8
rounds=3
lease_seconds=30
join_timeout_seconds=30 per round
total_expected_completions=192
```

Hard bounds:

```text
max_total_seconds=120
no unbounded sleep
no external services
local temporary SQLite database per round
separate UniversalKBQueue connection per worker process
```

## Harness requirements

The harness must:

1. create a new queue database per round or isolate round job ids;
2. enqueue `jobs_per_round` jobs;
3. unblock all ready jobs;
4. start `processes` worker processes;
5. have each process open its own `UniversalKBQueue` connection;
6. claim until no work remains;
7. complete claimed jobs with deterministic output refs;
8. report structured tuples or JSON diagnostics to the parent;
9. terminate stuck workers after timeout;
10. inspect final queue state after workers exit;
11. output a JSON summary suitable for artifact capture.

## Required JSON summary fields

```text
schema_version
rounds
jobs_per_round
processes
lease_seconds
join_timeout_seconds
total_jobs
total_completed
unique_completed
worker_errors
stuck_workers
round_summaries
all_jobs_succeeded
all_jobs_completed_once
duration_seconds
```

Each `round_summaries[]` entry should include:

```text
round
jobs
completed
unique_completed
worker_done_count
worker_error_count
stuck_worker_count
claim_event_count
complete_event_count
```

## Pass conditions

The soak passes only if:

```text
total_completed == total_jobs
unique_completed == total_jobs
worker_errors == []
stuck_workers == []
all_jobs_succeeded is true
all_jobs_completed_once is true
```

## CI behavior

M170 should add a small focused unit test only if new harness code is factored into importable helpers. The long `64 x 8 x 3` soak should be run during M170 as runtime proof, not automatically added to default pre-commit or full test suite.

## Non-goals

- Do not change `UniversalKBQueue` internals unless the soak exposes a real bug.
- Do not use external services.
- Do not run an unbounded production-duration stress.
- Do not claim full high-concurrency production readiness from this bounded soak alone.

## Expected artifacts

- `scripts/soak_universal_kb_queue.py` or equivalent minimal harness if implementation needs a script.
- `data/architecture-assessment/m170-queue-soak-result.json`
- `data/architecture-assessment/m170-queue-soak-result.md`

## Residual risk

Even if this passes, it is still a bounded local SQLite process soak. It raises confidence before high-concurrency activation, but production worker counts, job durations, and filesystem behavior may still require a later environment-specific soak.
