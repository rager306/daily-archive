# M198 S04 Sync Rehearsal Boundary

## Verdict

**PASS: S04 may add a sync rehearsal probe, but must not edit queue or rehearsal semantics.**

## GitNexus evidence

| Target | Result | Scope decision |
|---|---|---|
| `Function:src/research_graph/workflows/universal_kb/rehearsal.py:run_universal_kb_no_write_rehearsal` | LOW, impacted_count=0, affected_processes=[] | Read-only runtime input; do not edit. |
| `Method:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue._dependencies_satisfied#1` | HIGH, affected processes include no-write rehearsal, smoke runner, and smoke main | Out of scope; do not edit dependency semantics. |

## Observed rehearsal artifact shape

Runtime probe evidence: `gsd_exec[9a3aa25d-c497-413d-b58a-fa5911043bb5]`.

Expected files in a rehearsal artifact directory:

- `candidate.json`
- `projection_result.json`
- `queue.sqlite`
- `queue_inspect.json`
- `readiness_handoff.json`
- `review_packet.json`
- `review_trace.json`
- `schema_gate_result.json`
- `summary.json`

Important parity note: sync rehearsal creates `queue.sqlite` and `queue_inspect.json`; it does **not** create standalone `queue_events.json`.

## Allowed S04 edits

- `scripts/run_m198_sync_rehearsal_probe.py`
- `tests/test_m198_sync_rehearsal_probe.py`
- S04 architecture assessment artifacts

## Disallowed S04 edits

- `src/research_graph/workflows/universal_kb/queue.py`
- `src/research_graph/workflows/universal_kb/rehearsal.py`
- `src/research_graph/workflows/universal_kb/smoke_runner.py`
- `src/research_graph/workflows/universal_kb/smoke.py`
- production graph backend code
- schema migration code

## Required probe behavior

- Runs existing `run_universal_kb_no_write_rehearsal(artifact_dir)`.
- Writes one `m198.readiness_evidence.v1` JSON file.
- Uses `source_kind=sync_no_write_rehearsal`.
- Preserves `graph_writes_allowed=false`, `schema_migration_allowed=false`, and `import_eligible=false`.
- Records queue artifact refs, absence of standalone `queue_events.json`, schema gate status, promotion/import blocked status, checksums, diagnostics, and non-goals.
- Rejects missing summary, bad write flags, promotion/import leakage, and forbidden payload-shaped terms.

## Downstream dependency map

- S07 consumes S04 evidence for drift classification against S03 dry-run probe output.
- S08 consumes S04 evidence for the metadata-only evidence index.
