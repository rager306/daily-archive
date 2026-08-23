# M170 Combined Feasibility

## Verdict

**All three requested tracks are feasible in one milestone** if the milestone keeps thin slices and uses explicit decision gates before code changes.

Tracks:

1. Architecture backlog batch.
2. Longer UniversalKBQueue soak.
3. Same-key cache write coordination policy or implementation.

## Why combined execution is safe enough

- Baseline guards are green: test architecture violations are zero and onion violations are zero.
- Write-path inventory has `unknown=0`, so cache coordination can focus on ownership and residual same-key contention rather than scanner cleanup.
- Queue suite already has bounded process-level proof from M169, so M170 can add a longer harness without modifying queue internals unless a real bug appears.
- Architecture backlog will be converted into concrete in-scope items before edits, preventing broad refactor sprawl.

## Decision gates

### Architecture backlog

S02 and S03 must define exact in-scope items before edits. Broad backlog items must be deferred instead of becoming hidden refactor scope.

### Cache coordination

S05 must decide whether atomic replacement from M169 is enough for the current risk. If lock/CAS is needed, implementation must be bounded to same-key stable CLI and PDF cache writes.

### Queue soak

S09 must define jobs, processes, rounds, timeout, and output format before S10 implements a harness. The soak must not become an unbounded CI burden.

## Initial pass conditions

- Dynamic test allowlist remains zero.
- Legacy mixed allowlist remains zero.
- Onion allowlist remains empty.
- Write-path inventory remains `unknown=0`.
- Queue soak produces structured diagnostics and completes every job exactly once.
- Closeout quality stack passes before milestone completion.

## Scope note

M170 should prefer documented no-code closure when reconnaissance shows code is speculative. This is especially important for cache coordination: lock/CAS should be added only if S04/S05 establish a real same-key contention risk that atomic replacement alone does not address.
