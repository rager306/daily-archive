# M197 S01 Reactive Seam Inventory

## Verdict

**PASS: reactive adoption has clear additive seams, but queue dependency semantics are high-risk and must not be changed first.**

## GitNexus evidence used

- Query: `UniversalKBQueue no-write rehearsal smoke runner graph projection schema gate staged validation governance ratchets async reactive orchestration`
- Exact context: `Class:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue`
- Exact context: `Function:src/research_graph/workflows/universal_kb/rehearsal.py:run_universal_kb_no_write_rehearsal`
- Exact context: `Function:src/research_graph/workflows/universal_kb/smoke_runner.py:run_smoke`

## Source seams

| Seam | Path | Role in reactive adoption |
|---|---|---|
| UniversalKBQueue | `src/research_graph/workflows/universal_kb/queue.py` | Durable job state, leases, retries, events, artifacts; high-value but high-risk for semantic edits. |
| No-write rehearsal | `src/research_graph/workflows/universal_kb/rehearsal.py` | Current end-to-end no-write flow from candidate packet through queue, schema gate, projection, and artifacts. |
| Smoke runner | `src/research_graph/workflows/universal_kb/smoke_runner.py` | Script-level safety harness for false flags, artifact path safety, and metadata-only checks. |
| Smoke CLI wrapper | `src/research_graph/workflows/universal_kb/smoke.py` | Operator-facing runner path that calls smoke runner. |
| Schema gate | `src/research_graph/domain/graph_projection_schema.py` | Deterministic graph projection compatibility and no-write guard. |
| Projection port | `src/research_graph/domain/ports.py` | Projection request/result boundary and `assert_no_write`. |
| Universal KB contracts | `src/research_graph/domain/universal_kb/contracts.py` | Candidate packet, tool invocation sanitization, and no-write assertions. |

## Script seams

| Script | Role |
|---|---|
| `scripts/soak_universal_kb_queue.py` | Queue soak and stress style verification candidate. |
| Future M197 dry-run script | Should wrap additive async pilot only after S02-S08 contracts and tests exist. |

## Test seams

| Test | Role |
|---|---|
| `tests/test_universal_kb_queue.py` | Queue lifecycle and compatibility floor. |
| `tests/test_universal_kb_rehearsal.py` | No-write rehearsal end-to-end artifact floor. |
| `tests/test_universal_kb_substrate_rehearsal.py` | Readiness handoff and write-promotion rejection floor. |
| `tests/test_universal_kb_contracts.py` | Contract and sanitization floor. |
| `tests/test_graph_projection_schema_gate.py` | Schema gate no-write compatibility floor. |
| `tests/test_graph_projection_port.py` | Projection port no-write floor. |
| `tests/test_m195_governance_ratchets.py` | Existing no-write governance ratchets. |
| `tests/test_m196_governance_ratchets.py` | M196 staged validation and artifact governance ratchets. |

## Reactive observability fields to introduce

- `job_id`
- `stage_id`
- `correlation_id`
- `phase`
- `status`
- `attempt`
- `started_at`
- `completed_at`
- `timeout_ms`
- `cancelled`
- `last_error_code`
- `artifact_refs`
- `graph_writes_allowed=false`
- `schema_migration_allowed=false`
- `import_eligible=false`

## Boundaries

- Keep deterministic domain contracts synchronous unless measured evidence requires otherwise.
- Prefer additive async wrappers around existing no-write stages.
- Do not edit `UniversalKBQueue._dependencies_satisfied` in early waves.
- Do not enable production graph import, LadybugDB writes, FalkorDB writes, schema migration execution, or `import_eligible=true`.
