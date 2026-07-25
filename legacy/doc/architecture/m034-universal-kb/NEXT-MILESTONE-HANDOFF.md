# M034 Next Milestone Handoff

Recommended next milestone: **Durable Evidence Pipeline Prototype Planning**.

## Ready Inputs

- ADR-TEMPLATE.md
- ADR-INDEX.md
- ADR-000/002/003/004/005/006/007
- PRD.md
- FUNCTIONAL-REQUIREMENTS.md
- NON-FUNCTIONAL-REQUIREMENTS.md
- CONTRACTS.md
- SAFETY-INVARIANTS.md
- STATUS-MATRIX.md
- FAILURE-TAXONOMY.md
- ARTIFACT-DEPENDENCY-MODEL.md
- ROADMAP-GATES.md
- CONFLICT-RESOLUTION-PLAN.md
- OPEN-QUESTIONS.md

## Recommended Prototype Slices

1. State model and queue semantics design gate.
2. Minimal durable job/artifact store prototype.
3. Lazy dependency/stale detection prototype.
4. One no-write sidecar worker simulation.
5. Failure/retry/resume verifier.
6. Review packet/readiness handoff no-write verifier.

## Must Not Implement Yet

- Final GraphDB selection.
- GraphDB writes.
- Production graph import.
- Agentic orchestration.
- Parser output as accepted knowledge.

## Safety Defaults

```text
graph_import_allowed=false
graphdb_written=false
ladybugdb_written=false
production_import_attempted=false
import_eligible=false
```
