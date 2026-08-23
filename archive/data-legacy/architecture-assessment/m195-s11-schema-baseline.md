# M195 S11 Schema Governance Baseline

## Verdict

**PASS: S11 can proceed with a separate schema gate module.** GitNexus impact for existing candidate/projection contract surfaces is LOW, and S11 will avoid mutating `CandidatePacket` or projection result behavior directly.

## GitNexus evidence

| Target | Result |
|---|---|
| `Class:src/research_graph/domain/universal_kb/contracts.py:CandidatePacket` | LOW, impactedCount=16, processes_affected=0 |
| `File:src/research_graph/domain/ports.py` | LOW, impactedCount=7, processes_affected=0 |

## Schema inventory

| Surface | Current schema version |
|---|---|
| Candidate packet | `universal-kb-candidate.v1` |
| Projection request/result | `knowledge-graph-projection.v1` |
| Review assistance | `review-assistance.v1` |
| Queue payload metadata | optional `schema_version` metadata code |
| Rehearsal projection artifact | projection result schema from S07/S10 |

## Minimal source target

Add new files only:

- `src/research_graph/domain/graph_projection_schema.py`
- `tests/test_graph_projection_schema_gate.py`

Do not edit:

- `src/research_graph/domain/universal_kb/contracts.py`
- `src/research_graph/domain/ports.py`
- queue schema or dependency semantics
- projection adapters
- graph backend adapters

## Planned gate behavior

- `GraphProjectionSchemaGate.validate(request)` checks candidate and projection schema versions.
- Current versions pass with metadata-only diagnostic `schema_versions_current`.
- Unsupported versions fail closed with `schema_migration_required` and a migration placeholder.
- Gate result preserves `SafetyFlags()` defaults and never sets import eligibility.

## Boundary statement

S11 is schema governance only. It is not migration execution, not graph import eligibility, not production graph readiness, and not backend adapter activation.
