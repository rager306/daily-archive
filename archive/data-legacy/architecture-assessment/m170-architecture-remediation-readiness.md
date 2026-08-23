# M170 Architecture Remediation Readiness

## Verdict

**Architecture backlog targets B and C are closed for M170.**

M170 improves the M165 async and multithread readiness backlog with two bounded pieces of evidence:

1. same-key CLI/PDF cache coordination policy is explicit and verified;
2. longer UniversalKBQueue process soak completed successfully.

## Target B: same-key cache coordination

Status: **closed as atomic-only policy**.

Evidence:

- `data/architecture-assessment/m170-cache-coordination-policy.md`
- `data/architecture-assessment/m170-cli-cache-coordination-result.md`
- `data/architecture-assessment/m170-pdf-cache-coordination-result.md`
- `data/architecture-assessment/m170-cache-coordination-verification.md`
- Decision D092

Result:

```text
focused cache tests=3 passed
write_path_unknown=0
shared-state=4 remains visible
```

Closure rationale:

- M169 atomic writes prevent partial final files.
- M170 found no current same-key multi-writer activation requirement.
- Lock/CAS is deferred with explicit activation triggers rather than implemented speculatively.

## Target C: longer queue soak readiness

Status: **closed with bounded runtime proof**.

Evidence:

- `scripts/soak_universal_kb_queue.py`
- `data/architecture-assessment/m170-queue-soak-contract.md`
- `data/architecture-assessment/m170-queue-soak-result.json`
- `data/architecture-assessment/m170-queue-soak-result.md`

Result:

```text
jobs_per_round=64
processes=8
rounds=3
total_jobs=192
total_completed=192
unique_completed=192
worker_errors=0
stuck_workers=0
passed=true
```

Closure rationale:

- M169 proved a smaller process-level contention case.
- M170 provides a reusable harness and longer runtime proof.
- Queue internals did not require changes.

## Strictness claim boundary

M170 may claim:

```text
Architecture backlog follow-ups for shared-state review, same-key cache policy, and longer queue soak are closed for the planned scope.
```

M170 must **not** claim:

```text
Full repository strict architecture compliance.
Full production async or multithread readiness.
Global exactly-once cache population.
```

## Remaining future work

1. Run environment-specific queue soak before materially higher worker counts or shared network filesystems.
2. Add lock/CAS only when same-key concurrent CLI/PDF cache writers become real or stale-overwrite detection becomes a contract.
3. Consider richer inventory categories for visible shared-state records without hiding real risk.
