# M034 Decision Package Summary

## North Star

M034 defines daily-archive as a **local-first universal knowledge base** with **scientific articles as the primary first domain**. The project builds durable, traceable evidence chains before any graph promotion.

## Binding / Deferred ADRs

| ADR | Status | Decision |
|---|---|---|
| ADR-000 | Accepted | Universal KB north star with scientific articles as first domain. |
| ADR-002 | Deferred | Final GraphDB selection remains open pending comparison. |
| ADR-003 | Accepted | Durable lazy async evidence pipeline before scale or agents. |
| ADR-004 | Accepted | Sidecars are candidate evidence producers, not truth sources. |
| ADR-005 | Accepted | No direct extractor/parser/sidecar/LLM to GraphDB write path. |
| ADR-006 | Accepted | Agents are optional future helpers, not current core orchestrators. |
| ADR-007 | Accepted | quant-mind is pattern source, not runtime dependency. |

## Core Package Artifacts

- `ADR-TEMPLATE.md` — Mermaid-assisted enhanced ADR template.
- `ADR-INDEX.md` — ADR status/index surface.
- `R-D-CONSISTENCY-AUDIT.md` / `r-d-consistency-audit.json` — all Rxxx/Dxxx audit.
- `PRD.md` — product scope.
- `FUNCTIONAL-REQUIREMENTS.md` — generic, paper-specific, and safety requirements.
- `NON-FUNCTIONAL-REQUIREMENTS.md` — locality, reproducibility, redaction, observability, resumability, GraphDB portability.
- `CONTRACTS.md` — conceptual generic and paper-specific contracts.
- `SAFETY-INVARIANTS.md` — fail-closed defaults and non-authorization rules.
- `STATUS-MATRIX.md` — status vocabulary and transitions.
- `FAILURE-TAXONOMY.md` — failure classes and error codes.
- `ARTIFACT-DEPENDENCY-MODEL.md` — dependency graph and lazy recompute rules.
- `ROADMAP-GATES.md` — mandatory gates before implementation.
- `CONFLICT-RESOLUTION-PLAN.md` — routes all 15 S01 clarification findings.
- `OPEN-QUESTIONS.md` — open questions separated from accepted decisions.
- `NEXT-MILESTONE-HANDOFF.md` — next implementation-planning handoff.

## S01 Audit Result

- Requirements: 61
- Decisions: 67
- Audit records: 128
- Consistent: 35
- Historical scope only: 78
- Needs clarification: 15
- Blocking conflicts needing immediate user decision: 0

## Safety Defaults

```text
graph_import_allowed=false
graphdb_written=false
ladybugdb_written=false
production_import_attempted=false
import_eligible=false
```

## Must Not Infer

- Do not infer final GraphDB selection.
- Do not infer LadybugDB production adoption.
- Do not infer parser/sidecar output is graph-ready.
- Do not infer agents may orchestrate now.
- Do not infer any GraphDB write or production import is authorized.

## Next Recommended Milestone

**Durable Evidence Pipeline Prototype Planning**:

1. Resolve state model and queue semantics gates.
2. Prototype persisted job/artifact state.
3. Prototype lazy dependency/stale detection.
4. Add one no-write sidecar worker simulation.
5. Verify failure/retry/resume behavior.
6. Verify review packet/readiness handoff in no-write mode.

## Verification

Use final verifier:

```bash
uv run python scripts/verify_m034_decision_package.py --package-dir .gsd/milestones/M034-kuei9y/decision-package --requirements .gsd/REQUIREMENTS.md --decisions .gsd/DECISIONS.md
```
