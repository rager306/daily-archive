# M195 S08 NetworkX Projection Rehearsal Baseline

## Verdict

**PASS: S08 can proceed with a targeted NetworkX adapter source edit.** Exact GitNexus impact for the planned adapter file, existing adapter class, and test file is LOW with no affected processes.

## GitNexus evidence

| Target | Result |
|---|---|
| `File:src/research_graph/infrastructure/graph/networkx_probe.py` | LOW, impactedCount=0, processes_affected=0 |
| `Class:src/research_graph/infrastructure/graph/networkx_probe.py:NetworkXGraphProbeAdapter` | LOW, impactedCount=0, processes_affected=0 |
| `File:tests/test_networkx_graph_probe_adapter.py` | LOW, impactedCount=0, processes_affected=0 |

## Implementation boundary

Edit only:

- `src/research_graph/infrastructure/graph/networkx_probe.py`
- `tests/test_networkx_graph_probe_adapter.py`

Use S07 contracts:

- `KnowledgeGraphProjectionPort`
- `ProjectionRequest`
- `ProjectionResult`
- `ProjectionNodeRef`
- `ProjectionEdgeRef`
- `ProjectionDiagnostic`

## Planned adapter behavior

- Implement `NetworkXProjectionAdapter.project(request)`.
- Build an in-memory `DiGraph` from `CandidatePacket.graph_node_refs` and `graph_edge_refs` only.
- Return `ProjectionResult` with backend `networkx`, node refs, edge refs, evidence refs, provenance refs, and diagnostics.
- Preserve `SafetyFlags()` default false.
- Missing NetworkX or graph construction failures return metadata-only diagnostics without raw payload values.

## Disallowed in S08

- No graph DB connection.
- No LadybugDB/FalkorDB import or writes.
- No production import.
- No import eligibility promotion.
- No queue dependency or queue schema edits.
- No raw candidate payload logging.

## Compatibility tests

- `tests/test_networkx_graph_probe_adapter.py`
- `tests/test_graph_projection_port.py`
- Universal KB contract and no-write rehearsal tests at closeout.
