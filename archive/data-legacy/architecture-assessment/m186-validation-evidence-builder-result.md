# M186 Validation Evidence Builder Result

## Decision

**No-move for `build_evidence` in M186 S06.**

## Rationale

S03 already extracted the reusable path primitives from the M031 verifier into `research_graph.application.validation.evidence_paths`. The remaining `build_evidence` function is a milestone-specific dossier assembler with one production caller and one test helper caller. It depends on M031 constants, default artifact paths, requirement row generation, verification class row generation, stale-assessment detection, 65-pass signal detection, full-repo collection debt detection, and recursive safety flag scanning.

Moving it now would require either:

1. moving a large M031-specific helper cluster into application code; or
2. adding a configuration/data-builder abstraction for one implementation.

Both options are more architecture than the code has earned. The safer crystallization boundary is the one already landed in S03: reusable path primitives in application code, M031-specific evidence assembly in the verifier script.

## Future extraction gate

Revisit `build_evidence` only when at least one is true:

- another validation remediation verifier needs the same evidence-builder shape;
- M029/M031/M033 validation remediation builders are unified under a shared contract;
- a test contract requires mocking the evidence-builder boundary;
- M031 constants/default path policy is moved into a proper typed config shared by more than one verifier.

## Current safe boundary

- Application layer owns reusable path safety primitives.
- Script layer owns M031-specific dossier assembly and milestone constants.
- Focused M031 tests remain the enforcement surface.
