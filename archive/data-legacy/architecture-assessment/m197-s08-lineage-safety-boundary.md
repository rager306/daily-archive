# M197 S08 Lineage Payload Safety Boundary

## Verdict

**PASS: S08 may add lineage metadata to the additive runner.** GitNexus impact is LOW for `run_reactive_stage` and `_base_event`; no existing queue/rehearsal/smoke process is affected.

## GitNexus impact evidence

| Target | Risk | Impact | Affected processes |
|---|---:|---:|---|
| `Function:src/research_graph/workflows/universal_kb/reactive_runner.py:run_reactive_stage` | LOW | impacted_count=2 | none |
| `Function:src/research_graph/workflows/universal_kb/reactive_runner.py:_base_event` | LOW | impacted_count=3 | none |

## Allowed S08 edits

- `src/research_graph/workflows/universal_kb/reactive_runner.py`
- `tests/test_m197_reactive_runner.py`
- S08 architecture assessment artifacts

## Disallowed S08 edits

- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/workflows/universal_kb/rehearsal.py`
- `src/research_graph/workflows/universal_kb/smoke_runner.py`
- `src/research_graph/workflows/universal_kb/smoke.py`

## Required lineage semantics

- Events may include `parent_artifact_refs`.
- Events may include `child_artifact_refs`.
- Events may include `checksum_sha256`.
- Lineage fields reference artifact IDs/names/checksums only, never raw payload text.
- Payload-shaped forbidden terms from `m197.reactive_event.v1` must not appear in emitted events.
- All events keep `graph_writes_allowed=false`, `schema_migration_allowed=false`, and `import_eligible=false`.

## Boundary statement

S08 adds lineage and payload-safety metadata only. It does not persist raw prompts, source text, chunk text, embeddings, vectors, secrets, graph writes, schema migrations, or import eligibility.
