# M195 S02 Contract Baseline

## Verdict

**PASS: S02 may edit the active current-layout Universal KB contract with LOW GitNexus impact, but must keep the change minimal and no-write.**

## GitNexus impact

| Target | Exact UID | Result |
|---|---|---|
| `CandidatePacket` | `Class:src/research_graph/domain/universal_kb/contracts.py:CandidatePacket` | LOW, impactedCount=26, direct=3, processes_affected=0 |
| `SafetyFlags` | `Class:src/research_graph/domain/universal_kb/contracts.py:SafetyFlags` | LOW, impactedCount=26, direct=3, processes_affected=0 |

Name-only lookup is ambiguous because `archive/package-rename-waves/wave-17/src/arxiv_archive/universal_kb_contracts.py` has historical copies. S02 must edit only `src/research_graph/domain/universal_kb/contracts.py` unless replanned.

## Current contract state

Active file:

- `src/research_graph/domain/universal_kb/contracts.py`

Existing relevant dataclasses:

- `SafetyFlags`: all five write/import flags default false and reject unsafe true values.
- `EvidenceArtifactRecord`: metadata-only artifact evidence record.
- `CandidatePacket`: currently has `candidate_id`, `evidence_refs`, `candidate_type`, `review_state`, and `safety_flags`.
- `ReviewPacket`, `ProcessingJob`, `DependencyRecord`, `FailureRecord`, `ToolInvocationRecord` already cover review, queue, failure, and helper evidence metadata.

Existing tests:

- `tests/test_universal_kb_contracts.py`
- `tests/test_universal_kb_sidecar_boundary.py`
- `tests/test_universal_kb_review_assistance.py`
- `tests/test_validation_batch_state.py`

## Gap for M195 S02

`CandidatePacket` is already no-write and evidence-backed, but it does not yet carry graph projection shape metadata required by D110 and R069:

- `schema_version`
- candidate graph node refs
- candidate graph edge refs
- provenance refs distinct from evidence refs
- candidate-level diagnostics

## Minimal edit plan

Use Ponytail minimalism: extend `CandidatePacket` with optional tuple fields and validation rather than introducing a new class hierarchy or backend-specific graph abstraction.

Planned fields:

```python
schema_version: str = "universal-kb-candidate.v1"
graph_node_refs: tuple[str, ...] = field(default_factory=tuple)
graph_edge_refs: tuple[str, ...] = field(default_factory=tuple)
provenance_refs: tuple[str, ...] = field(default_factory=tuple)
diagnostics: tuple[str, ...] = field(default_factory=tuple)
```

Validation rules:

- `schema_version` must be non-empty.
- tuple-like fields are normalized through `_tuple`.
- `diagnostics` must remain refs/codes only, not payload values.
- `SafetyFlags` behavior remains unchanged.
- `to_dict()` remains JSON-safe.
- No NetworkX, LadybugDB, FalkorDB, or infrastructure import in domain code.

## Tests to add or update

Add focused tests in `tests/test_universal_kb_contracts.py` or a small new contract test file:

- candidate packet serializes graph projection metadata;
- empty `schema_version` is rejected;
- candidate diagnostics reject forbidden payload keys or raw data if represented as dicts later, or for tuple diagnostics ensure no raw payload field is introduced;
- safety flags remain false;
- no backend-specific imports in `src/research_graph/domain/universal_kb/contracts.py`.

## Disallowed in S02

- No graph import eligibility promotion.
- No graph backend adapter implementation.
- No production write flag changes.
- No new scheduler.
- No source movement.
- No retired `arxiv_archive` shim.
