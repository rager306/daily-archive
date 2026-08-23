# M171 Environment Soak Runner

## Verdict

**Reuse the existing M170 soak harness. No new runner code is needed.**

The existing script already supports M171 environment profiles through explicit command-line parameters:

```text
scripts/soak_universal_kb_queue.py
```

## local-fast verification

Command evidence: `gsd_exec[277b30d0-0588-44f1-8786-bbc8c8113b23]`.

Parameters:

```text
jobs_per_round=8
processes=4
rounds=2
join_timeout_seconds=10
max_total_seconds=60
total_jobs=16
```

Result:

```text
total_completed=16
unique_completed=16
worker_errors=[]
stuck_workers=[]
timeout_exceeded=false
```

## Decision rationale

Adding another wrapper would duplicate a working command surface. The M170 harness already emits JSON, validates pass conditions via exit status, and accepts all profile parameters needed by S05.

## S07 command shape

S07 should run the activation-candidate profile directly:

```text
uv run python scripts/soak_universal_kb_queue.py \
  --jobs-per-round 128 \
  --processes 12 \
  --rounds 4 \
  --join-timeout-seconds 45 \
  --max-total-seconds 180 \
  --json-out data/architecture-assessment/m171-environment-soak-result.json
```

## Boundary

This remains local runtime proof only. It does not start production workers.
