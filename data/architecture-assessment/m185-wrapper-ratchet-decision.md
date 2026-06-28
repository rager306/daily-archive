# M185 Wrapper Ratchet Decision

## Decision

**No executable ratchet change in S07.**

## Rationale

The current ratchets already enforce the relevant failure modes:

- write-path canonical ratchet: `script-only <= 4`, `unknown == 0`, `shared-state == 0`;
- test architecture guard: no unallowlisted dynamic imports or legacy-mixed regressions;
- onion guard: application layer cannot import outward layers.

S03 and S04 prove two extraction patterns, but the sample size is still small and S05-S06 show that verifier boundaries require cohesive package design before movement. A new numeric wrapper-extraction ratchet would be premature.

## Guardrail posture

Keep existing tests and guards as the executable ratchet for now. Future ratchet candidates:

- assert `scripts/audit_test_architecture.py` stays a thin wrapper;
- assert `scripts/audit_pipeline_scripts.py` stays a thin wrapper;
- add a verifier-boundary no-move allowlist only if repeated verifier probes accumulate.

These should wait until after manifest/cache probes and final M185 quality stack.
