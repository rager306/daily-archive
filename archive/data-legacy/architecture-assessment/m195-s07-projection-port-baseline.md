# M195 S07 Graph Projection Port Baseline

## Verdict

**PASS: S07 can proceed with a narrow domain port edit.** GitNexus impact for the chosen source targets is LOW, and S07 will not touch queue dependency satisfaction, NetworkX, LadybugDB, FalkorDB, or production graph write paths.

## GitNexus evidence

| Target | Result | Notes |
|---|---|---|
| `File:src/research_graph/domain/ports.py` | LOW, impactedCount=7, processes_affected=0 | Existing domain port seam, direct import surface known. |
| `Class:src/research_graph/domain/universal_kb/contracts.py:CandidatePacket` | LOW, impactedCount=16, processes_affected=0 | S07 consumes but should not modify candidate packet behavior. |
| `Function:src/research_graph/workflows/universal_kb/rehearsal.py:run_universal_kb_no_write_rehearsal` | LOW, impactedCount=0 | Compatibility target only. |
| `Class:src/research_graph/application/graph/probe.py:GraphProbeExecutionPort` | LOW, impactedCount=3, processes_affected=0 | Existing application-level graph probe port remains separate. |

## Existing seams

- `GraphDBPort` is a persistence boundary and is not the S07 target.
- `GraphProbeExecutionPort` is an application probe adapter boundary and already allows NetworkX graph-shape probes.
- S07 needs a candidate-packet projection boundary that is backend-neutral and no-write, before S08 implements NetworkX projection rehearsal and S09 adds disabled backend seam shells.

## Minimal source target

Edit only:

- `src/research_graph/domain/ports.py`
- `tests/test_graph_projection_port.py` (new)

Do not edit:

- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/infrastructure/graph/networkx_probe.py`
- `src/research_graph/infrastructure/graph/ladybug_adapter.py`
- any FalkorDB backend
- any production graph/import path

## Port-rule justification

`KnowledgeGraphProjectionPort` is allowed by the project Ponytail port rule because at least two implementations are planned in this milestone: NetworkX rehearsal (S08) and disabled/dry-run LadybugDB/FalkorDB comparison seams (S09). The port is not speculative symmetry; it is the shared seam those slices depend on.

## Planned contract shape

- `ProjectionNodeRef`: metadata-only node ref plus kind.
- `ProjectionEdgeRef`: metadata-only edge ref plus source/target refs and kind.
- `ProjectionDiagnostic`: metadata-only code/phase/severity.
- `ProjectionRequest`: schema version plus `CandidatePacket`.
- `ProjectionResult`: schema version, backend name, node refs, edge refs, provenance refs, evidence refs, diagnostics, and `SafetyFlags` defaulting false.
- `KnowledgeGraphProjectionPort.project(request) -> ProjectionResult`.

## Boundary statement

S07 is not a NetworkX adapter implementation, not a graph DB adapter implementation, and not production graph readiness. It only creates a no-write contract seam for later rehearsal/comparison slices.
