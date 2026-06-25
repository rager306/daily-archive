# M171 Environment Soak Contract

## Verdict

**M171 will use the existing queue soak harness with explicit environment profiles.**

No new runner is required unless the existing harness cannot run the activation-candidate profile.

## Profiles

### local-fast

Use for quick smoke during development:

```text
jobs_per_round=8
processes=4
rounds=2
join_timeout_seconds=10
max_total_seconds=60
total_jobs=16
```

### activation-candidate

Use for M171 environment-specific proof:

```text
jobs_per_round=128
processes=12
rounds=4
join_timeout_seconds=45
max_total_seconds=180
total_jobs=512
```

## Command template

```text
uv run python scripts/soak_universal_kb_queue.py \
  --jobs-per-round <jobs_per_round> \
  --processes <processes> \
  --rounds <rounds> \
  --join-timeout-seconds <join_timeout_seconds> \
  --max-total-seconds <max_total_seconds> \
  --json-out <json_out>
```

## M171 output paths

```text
data/architecture-assessment/m171-environment-soak-result.json
data/architecture-assessment/m171-environment-soak-result.md
```

## Pass conditions

The activation-candidate soak passes only if:

```text
passed=true
total_jobs=512
total_completed=512
unique_completed=512
worker_errors=[]
stuck_workers=[]
timeout_exceeded=false
all_jobs_succeeded=true
all_jobs_completed_once=true
```

## Runtime bounds

- No external services.
- Temporary local SQLite databases only.
- No production queue path.
- No long-running daemon.
- Max total seconds bounded at 180 for M171.

## Failure handling

If the profile fails:

1. preserve JSON output and stdout evidence;
2. do not retry blindly;
3. inspect worker errors, stuck workers, bad jobs, and timeout flag;
4. replan only if failure indicates a queue bug or harness bug.

## Non-production boundary

This is an environment-specific local soak profile, not a production activation. Passing it supports readiness assessment but does not start real production workers.
