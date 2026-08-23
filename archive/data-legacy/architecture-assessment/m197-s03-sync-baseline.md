# M197 S03 Sync No Write Baseline

## Verdict

**PASS: current synchronous no-write rehearsal produces deterministic metadata artifacts suitable for future async equivalence checks.**

## Runtime evidence

| Check | Result | Evidence |
|---|---|---|
| Initial baseline probe | FAIL: expected nonexistent `queue_events.json` | `gsd_exec[58419e19-0294-4d3f-980d-d2daafa43bd0]` |
| Corrected baseline probe | PASS | `gsd_exec[09ef8d4a-fd39-4f5e-901b-da21e57fcb19]` |

## Baseline facts

- artifact_count: 8
- artifact_names:
  - `candidate.json`
  - `projection_result.json`
  - `queue_inspect.json`
  - `readiness_handoff.json`
  - `review_packet.json`
  - `review_trace.json`
  - `schema_gate_result.json`
  - `summary.json`
- queue_status: `ready`
- schema_gate diagnostics: `schema_versions_current`
- projection_backend: `networkx`
- import_eligible: `false`

## Contract comparison target

Future async events should map this sync baseline into `m197.reactive_event.v1` fields:

| Contract field | Sync baseline source |
|---|---|
| `job_id` | `queue_inspect.json` job identity |
| `stage_id` | derived future async stage identity |
| `correlation_id` | future pilot-generated run correlation |
| `phase` | queue, schema gate, projection, review, summary |
| `status` | queue status and stage result |
| `attempt` | queue attempt metadata when present, otherwise pilot default |
| `artifact_refs` | eight baseline artifact names |
| `diagnostics` | schema gate and projection diagnostics |
| `graph_writes_allowed` | false |
| `schema_migration_allowed` | false |
| `import_eligible` | false |

## Important observation

The current sync rehearsal does not emit a standalone `queue_events.json`. Queue state is represented through `queue_inspect.json` plus the SQLite queue artifact. Future reactive event emission should be additive and must not assume existing event artifact names.

## Boundary statement

S03 baseline captures current behavior only. It does not implement async execution, change queue semantics, or enable graph/import/write readiness.
