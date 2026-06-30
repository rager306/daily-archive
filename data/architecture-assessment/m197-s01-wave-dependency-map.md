# M197 S01 Wave Dependency Map

## Verdict

**PASS: M197 should execute as six gated waves, from contracts to additive async pilot to compatibility and governance.**

## Wave 1: Inventory and contracts

Slices: S01-S02

- S01 maps GitNexus impacts and high-risk seams.
- S02 defines the reactive event contract before implementation.
- No production source edits should be required.

Exit gate:

- Event schema names required state fields.
- Payload safety and false graph/import flags are contractually required.
- M195/M196 governance compatibility remains assumed, not yet altered.

## Wave 2: Deterministic baseline

Slice: S03

- Lock current sync no-write rehearsal behavior.
- Capture expected artifact count, key fields, safety flags, and schema/projection diagnostics.

Exit gate:

- Sync baseline tests pass.
- Baseline artifact provides comparison target for async runner.

## Wave 3: Additive async foundation

Slices: S04-S06

- S04 adds a small async stage runner next to existing sync APIs.
- S05 adds bounded concurrency and deterministic ordering.
- S06 adds timeout and cancellation semantics.

Exit gate:

- Async runner outputs contract-compliant metadata events.
- No existing sync call path is broken.
- No queue dependency semantics are changed.

## Wave 4: Queue and artifact observability

Slices: S07-S08

- S07 models attempts, heartbeat, and lease observability without first editing `_dependencies_satisfied`.
- S08 adds artifact lineage and payload safety.

Exit gate:

- Any queue semantic edit has exact GitNexus impact and explicit HIGH-risk warning.
- Payload leak checks pass.
- M196 run artifact observability remains green.

## Wave 5: Operator integration and batch rehearsal

Slices: S09-S11

- S09 adds dry-run script integration.
- S10 verifies queue, rehearsal, smoke, and governance compatibility.
- S11 runs a small multi-document no-write batch rehearsal.

Exit gate:

- Default command path is no-write.
- `import_eligible=false` remains visible in runtime artifacts.
- No production graph backend is contacted.

## Wave 6: Governance and handoff

Slice: S12

- Final M197 tests run.
- M195 and M196 ratchets pass.
- R073-R075 are advanced or validated.
- Handoff states future production import remains a separate milestone.

## Dependency policy

- Do not implement async code before S02 contract and S03 baseline exist.
- Do not change queue dependency semantics before S07 and exact GitNexus impact.
- Do not expose a script command before S08 payload safety exists.
- Do not run batch rehearsal before S10 compatibility passes.
- Do not claim import readiness in M197.

## Verification ladder

1. Artifact assertions for planning slices.
2. Unit tests for contract and runner slices.
3. Compatibility suites for queue/rehearsal/smoke slices.
4. Runtime no-write smoke for batch slices.
5. GSD milestone validation for final closeout.
