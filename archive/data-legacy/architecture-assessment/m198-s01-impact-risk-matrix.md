# M198 S01 Impact Risk Matrix

## Verdict

**PASS: readiness seams are classified for safe M198 sequencing.** M198 starts with contracts, probes, and artifacts before any source-level transition work.

## Risk classes

| Class | Meaning | Allowed in early M198 |
|---|---|---|
| Safe read-only input | Existing command/function can be invoked in temp dirs or inspected without semantic edits | yes |
| Compatibility input | Existing behavior can be tested and compared, but source semantics should not be changed | yes, test/artifact only |
| Blocked edit target | Edits require separate replan, fresh GitNexus impact, and expanded verification | no |
| Future transition target | Belongs to later write/import transition milestone | no |

## Seam classification

| Seam | Class | Risk | Reason |
|---|---|---:|---|
| `scripts/run_m197_reactive_dry_run.py` | Safe read-only input | LOW | Exact impact LOW and no affected processes. |
| `run_universal_kb_no_write_rehearsal` | Safe read-only input | LOW | Read-only baseline with no upstream impact when used as compatibility evidence. |
| `UniversalKBQueue._dependencies_satisfied` | Blocked edit target | HIGH | Feeds no-write rehearsal and smoke article paths; prior M197 impact recorded HIGH. |
| `UniversalKBQueue.unblock_ready_jobs` | Blocked edit target | HIGH | Direct caller of dependency satisfaction and part of queue readiness semantics. |
| `smoke_runner.run_article` | Compatibility input | MEDIUM to HIGH | Smoke article path participates in queue dependency process links. |
| `smoke_runner.run_smoke` | Compatibility input | MEDIUM | Aggregates false-flag, output containment, payload safety, and article runs. |
| Graph readiness validate-only command | Compatibility input | MEDIUM | Safe only in validate-only mode; retired shim remains blocked. |
| Disabled backend adapters | Compatibility input | MEDIUM | Must remain fail-closed; any enablement belongs to a later milestone. |

## Sequencing consequences

- S02-S04 may define contracts and compare dry-run/sync rehearsal evidence.
- S05-S06 may map smoke and graph-readiness surfaces as compatibility inputs only.
- S07-S10 may classify drift and produce reports without source semantic edits.
- S11-S12 must ratchet blocked transitions and exact impact requirements.
- S13-S18 may rehearse and validate readiness packages only after contracts and ratchets exist.

## Mandatory warning

Any proposal to edit `UniversalKBQueue._dependencies_satisfied`, `UniversalKBQueue.unblock_ready_jobs`, smoke semantics, graph backend writes, schema migration paths, or import eligibility is a plan-invalidating escalation unless a later slice explicitly replans with fresh GitNexus impact and compatibility tests.
