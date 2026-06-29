# M195 S12 End to End Rehearsal Baseline

## Verdict

**PASS: S12 may proceed with a narrow rehearsal evidence edit.** Exact GitNexus impact for current-layout rehearsal, queue dependency semantics, NetworkX projection, and rehearsal tests is LOW. The new S11 schema gate file is not indexed yet, so S12 treats it as a local focused-test target and includes it in compatibility verification.

## GitNexus impact evidence

| Target | Result | Notes |
|---|---|---|
| `Function:src/research_graph/workflows/universal_kb/rehearsal.py:run_universal_kb_no_write_rehearsal` | LOW, impactedCount=0 | exact current-layout UID |
| `Method:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue._dependencies_satisfied#1` | LOW, impactedCount=4 | affects no-write rehearsal and smoke runner; do not edit queue semantics |
| `File:src/research_graph/infrastructure/graph/networkx_probe.py` | LOW, impactedCount=0 | exact file target |
| `File:tests/test_universal_kb_rehearsal.py` | LOW, impactedCount=0 | exact file target |
| `File:src/research_graph/domain/graph_projection_schema.py` | UNKNOWN/not indexed | new S11 file; cover with focused tests |

## Compatibility plan

Run after S12 edit:

```bash
uv run pytest \
  tests/test_universal_kb_rehearsal.py \
  tests/test_graph_projection_schema_gate.py \
  tests/test_graph_projection_port.py -q
```

Final closeout should also include:

```bash
uv run pytest \
  tests/test_graph_projection_schema_gate.py \
  tests/test_graph_projection_port.py \
  tests/test_networkx_graph_probe_adapter.py \
  tests/test_projection_backend_seams.py \
  tests/test_universal_kb_contracts.py \
  tests/test_universal_kb_queue.py \
  tests/test_universal_kb_rehearsal.py \
  tests/test_universal_kb_substrate_rehearsal.py -q
```

## S12 source boundary

Allowed source/test edits:

- `src/research_graph/workflows/universal_kb/rehearsal.py`
- `tests/test_universal_kb_rehearsal.py`

Do not edit:

- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/infrastructure/graph/networkx_probe.py`
- `src/research_graph/infrastructure/graph/projection_backends.py`
- graph DB adapters
- import/readiness review command paths

## Required artifact evidence

- `schema_gate_result.json` exists.
- `projection_result.json` still exists.
- `summary.json` includes schema gate metadata and projection metadata.
- Schema gate accepted current schemas.
- All graph/import/write flags remain false.

## Boundary statement

S12 proves only queue-to-schema-to-projection no-write rehearsal. It is not production graph readiness, not migration execution, not backend activation, and not import eligibility.
