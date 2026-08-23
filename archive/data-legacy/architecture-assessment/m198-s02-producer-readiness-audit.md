# M198 S02 Producer Readiness Audit

## Verdict

**PASS: S03-S08 producers have explicit contract obligations before implementation.**

## Contract obligations by producer

| Producer slice | Source kind | Must produce | Must not do |
|---|---|---|---|
| S03 Dry Run Probe Harness | `reactive_dry_run` | readiness evidence refs to M197 JSONL events, event count, checksums, no-write flags | create queue.sqlite, raw payloads, import eligibility |
| S04 Sync Rehearsal Parity Baseline | `sync_no_write_rehearsal` | queue artifact refs, projection safety flags, absence of standalone queue_events.json | change rehearsal semantics or copy raw payloads |
| S05 Smoke Boundary Baseline | `smoke_boundary` | smoke command and false-flag compatibility evidence | edit smoke runner or claim smoke readiness beyond compatibility |
| S06 Graph Readiness Validate Only Map | `graph_readiness_validate_only` | canonical validate-only command refs and retired-shim blocked evidence | restore retired shim, run import promotion, skip completed review checks |
| S07 Readiness Drift Classifier | derived from S03-S04 | drift class expected, warning, blocker, or not_applicable | classify write/import readiness as acceptable |
| S08 Evidence Index Writer | all source kinds | metadata-only evidence index with refs, checksums, statuses, non-goals | persist source text, embeddings, vectors, secrets, or production payloads |

## Shared required fields

All produced evidence must include:

- `schema_version=m198.readiness_evidence.v1`
- `evidence_id`
- `source_kind`
- `correlation_id`
- `status`
- `drift_class`
- `timestamp`
- `graph_writes_allowed=false`
- `schema_migration_allowed=false`
- `import_eligible=false`
- `evidence_refs`
- `diagnostics`
- `non_goals`

## Shared blocked transitions

Every producer must preserve blocked transitions:

- production graph import;
- schema migration;
- queue dependency semantic change;
- smoke semantic change;
- rehearsal semantic change;
- retired graph readiness shim restoration;
- `import_eligible=true` evidence.

## Downstream readiness

S03-S08 can now implement evidence producers against the contract without inventing new flags, source categories, or drift classes.
