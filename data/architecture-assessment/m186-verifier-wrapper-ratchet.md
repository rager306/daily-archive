# M186 Verifier Wrapper Ratchet Decision

## Decision

**Do not add a new numeric verifier wrapper ratchet yet.**

## Rationale

The verifier wave made real progress, but only two verifier primitive clusters have moved. A numeric ratchet now would likely reward wrapper shape rather than semantic boundary quality. The correct near-term guard remains contract evidence:

- exact GitNexus impact before helper movement;
- focused verifier tests for moved helpers;
- script wrappers preserving existing names and exceptions;
- strict write-path drift staying clean;
- no broad classifier rules.

## Ratchet candidate for later

A future numeric ratchet is justified after one more verifier or manifest lifecycle wave. Candidate measure:

```text
verifier_reusable_primitive_clusters_moved >= 2
verifier_milestone_specific_builders_moved == 0 unless shared by >=2 verifiers
script-only <= 4 unless manifest lifecycle movement completes
unknown == 0
shared-state == 0
```

## Current enforcement

For M186, the enforcement surface is the verifier wave artifacts plus tests, not a new global threshold.
