# M198 S05 Smoke Boundary

## Verdict

**PASS: S05 may add a smoke boundary probe, but must not edit smoke, queue, rehearsal, graph backend, or schema migration semantics.**

## GitNexus evidence

| Target | Result | Scope decision |
|---|---|---|
| `Function:src/research_graph/workflows/universal_kb/smoke_runner.py:run_article` | LOW, impacted_count=6, affected_processes include smoke main | Read-only runtime input; do not edit. |
| `Function:src/research_graph/workflows/universal_kb/smoke.py:main` | LOW, impacted_count=1, affected_processes=[] | Read-only CLI input; do not edit. |
| `Method:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue._dependencies_satisfied#1` | HIGH, affects no-write rehearsal, smoke runner, and smoke main | Out of scope; do not edit dependency semantics. |

## Observed smoke fixture shape

Runtime probe evidence: `gsd_exec[71138c7f-9840-415b-b7f4-5c9462a85455]`.

Accepted minimal metadata-only fixture includes:

- `candidate_id`
- `article_key`
- `title`
- `abstract`
- `safety_flags.graph_write_allowed=false`
- `safety_flags.schema_migration_allowed=false`
- `safety_flags.import_eligible=false`
- `safety_flags.production_import_attempted=false`
- `safety_flags.promotion_allowed=false`

## Observed smoke output shape

`run_article` returns metadata with:

- `article_key`
- `candidate_id`
- `artifact_dir`
- `continuity_ref`
- `queue_status`
- `graph_write_allowed=false`
- `import_eligible=false`
- `production_import_attempted=false`
- `promotion_allowed=false`
- `safety_flags`
- source/loader ref counts and diagnostics

Expected files under `articles/<candidate_id>/`:

- `candidate.json`
- `continuity.json`
- `queue_inspect.json`
- `readiness_handoff.json`
- `review_packet.json`
- `review_trace.json`

## Allowed S05 edits

- `scripts/run_m198_smoke_boundary_probe.py`
- `tests/test_m198_smoke_boundary_probe.py`
- S05 architecture assessment artifacts

## Disallowed S05 edits

- `src/research_graph/workflows/universal_kb/smoke_runner.py`
- `src/research_graph/workflows/universal_kb/smoke.py`
- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/workflows/universal_kb/rehearsal.py`
- production graph backend code
- schema migration code

## Required probe behavior

- Runs existing `run_article(article, output_dir=artifact_dir)` on a metadata-only fixture.
- Writes one `m198.readiness_evidence.v1` JSON file.
- Uses `source_kind=smoke_boundary`.
- Preserves `graph_writes_allowed=false`, `schema_migration_allowed=false`, and `import_eligible=false`.
- Records continuity/readiness/queue refs, queue status, false safety flags, checksums, diagnostics, and non-goals.
- Rejects missing continuity/readiness artifact, missing candidate id, bad write/import flags, and forbidden payload-shaped terms.

## Downstream dependency map

- S07 consumes S05 evidence for smoke boundary drift classification.
- S08 consumes S05 evidence for metadata-only evidence indexing.
