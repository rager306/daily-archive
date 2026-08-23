# M171 Queue Activation Checklist

## Verdict

**Local activation-readiness checklist is defined.**

This checklist is for future production activation planning. M171 does not start production workers or mutate external systems.

## Preflight gates

- Test architecture guard passes with dynamic=0 and legacy=0.
- Onion guard passes with zero violations and zero allowed violations.
- Write-path inventory passes with unknown=0.
- Queue soak harness smoke passes.
- Activation-candidate environment soak passes.
- Persisted diagnostics contain no secret-shaped payloads.
- Safety flags remain false for graph write, production import, and promotion paths.

## Runtime gates for future activation

- Worker count and lease seconds are declared before start.
- Queue database path and storage class are declared before start.
- Worker logs include worker id, job id, event type, and redacted diagnostics only.
- Every claimed job has one terminal outcome.
- Stuck-worker timeout and termination policy are configured.
- Queue inspect surface is available for failed jobs.

## Stop conditions

Stop activation immediately if any of these occur:

- worker process hangs past configured join/health timeout;
- duplicate completion is observed for one job id;
- worker emits secret-shaped diagnostics;
- write-path inventory regresses to unknown entries;
- safety flags indicate production import, graph write, or promotion;
- job failure rate exceeds the threshold chosen for the real activation milestone.

## Rollback and recovery

- Stop workers.
- Preserve queue database and diagnostic artifacts.
- Record stuck workers, worker errors, and bad jobs.
- Re-run local soak only after diagnosing the failure.
- Do not promote outputs from failed activation attempts.

## Required evidence bundle

- Activation contract.
- Soak JSON result.
- Soak markdown summary.
- Guard outputs.
- Write-path inventory output.
- GitNexus risk report.
- Final status/scope hygiene.

## Non-activation boundary

M171 can declare local readiness evidence only. Real production activation requires a separate milestone with explicit environment, worker count, storage target, rollback owner, and user confirmation.
