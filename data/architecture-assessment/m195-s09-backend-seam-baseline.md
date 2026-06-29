# M195 S09 Backend Seam Shell Baseline

## Verdict

**PASS: S09 can proceed by adding a new disabled projection seam module instead of editing existing backend write adapters.** Existing Ladybug adapter impact is LOW, but S09 intentionally avoids the write-capable `GraphDBPort` adapter path.

## GitNexus evidence

| Target | Result |
|---|---|
| `Class:src/research_graph/infrastructure/graph/ladybug_adapter.py:LadybugAdapter` | LOW, impactedCount=1, processes_affected=0 |
| `File:src/research_graph/infrastructure/graph/__init__.py` | LOW, impactedCount=1, processes_affected=0 |
| `File:tests/test_ladybug_adapter_port.py` | LOW, impactedCount=0, processes_affected=0 |

## Existing backend state

- `LadybugAdapter` implements `GraphDBPort` and delegates to write-capable `ladybug_client` functions.
- `GraphDBPort` is a persistence boundary, not the S09 projection seam.
- S09 must not edit or reuse `LadybugAdapter` for projection rehearsal because that would blur disabled projection planning with write-capable persistence.

## Minimal source target

Add new files only:

- `src/research_graph/infrastructure/graph/projection_backends.py`
- `tests/test_projection_backend_seams.py`

## Planned seam behavior

- `DisabledBackendProjectionAdapter`: base disabled/dry-run adapter behind `KnowledgeGraphProjectionPort`.
- `DisabledLadybugProjectionAdapter`: reports LadybugDB projection backend disabled.
- `DisabledFalkorProjectionAdapter`: reports FalkorDB projection backend disabled.
- Dry-run mode may echo candidate node/edge/evidence/provenance refs as projection metadata.
- Disabled mode returns metadata-only diagnostics and empty graph refs.
- All results preserve false `SafetyFlags`.

## Disallowed in S09

- No LadybugDB driver imports.
- No FalkorDB driver imports.
- No `ladybug_client` import.
- No graph DB connection attempts.
- No graph write/import calls.
- No production import or import eligibility promotion.
- No queue dependency or schema edits.

## Boundary statement

S09 represents backend comparison candidates only. It is not backend readiness, not a production graph write path, and not import eligibility evidence.
