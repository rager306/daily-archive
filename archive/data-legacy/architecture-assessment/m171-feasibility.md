# M171 Combined Feasibility

## Verdict

**All three requested tracks are feasible together in M171.**

Tracks:

1. production-style queue activation readiness;
2. environment-specific longer queue soak;
3. richer write-path inventory categories.

## Why combined execution is safe

- The guard baseline is green: dynamic=0, legacy=0, onion violations=0.
- Write-path inventory starts at unknown=0, so category work can focus on precision rather than cleanup.
- M170 added a working queue soak harness, so M171 can add profile/runbook evidence rather than changing queue internals first.
- Production activation can be handled as readiness evidence and local runbook; no external production mutation is required or allowed.

## Scope boundaries

M171 may:

- create activation readiness artifacts and local runbook/checklist;
- run bounded local environment-specific soak profiles;
- improve write-path inventory categories and tests;
- add small scripts or tests if needed.

M171 must not:

- push remote changes;
- start real production workers against external systems;
- weaken architecture guardrails;
- hide real shared-state risks behind broad category names;
- claim full production readiness beyond local bounded evidence.

## Initial pass criteria

- Test architecture guard remains dynamic=0, legacy=0, violations=0.
- Onion guard remains violation_count=0 and allowed_violation_count=0.
- Write-path inventory remains unknown=0.
- Environment soak produces structured pass evidence.
- Richer inventory categories are covered by focused tests.
- Final quality stack and GitNexus review pass.
