# M198 S07 Drift Classifier Boundary

## Verdict

**PASS: S07 may add a metadata-only drift classifier, but must not edit readiness producers or runtime workflow code.**

## GitNexus evidence

GitNexus was refreshed with `gitnexus analyze` after S04-S06 commits.

| Target | Result | Scope decision |
|---|---|---|
| `Function:scripts/run_m198_dry_run_probe.py:build_evidence` | LOW, impacted_count=2 | Available producer impact evidence. |
| `Function:scripts/run_m198_sync_rehearsal_probe.py:build_evidence` | UNKNOWN: target not found after refresh | Treat as new-symbol GitNexus limitation; rely on scoped detect_changes and local verification. |
| `Function:scripts/run_m198_smoke_boundary_probe.py:build_evidence` | UNKNOWN: target not found after refresh | Treat as new-symbol GitNexus limitation; rely on scoped detect_changes and local verification. |
| `Function:scripts/run_m198_graph_readiness_probe.py:build_evidence` | UNKNOWN: target not found after refresh | Treat as new-symbol GitNexus limitation; rely on scoped detect_changes and local verification. |
| `UniversalKBQueue._dependencies_satisfied#1` | HIGH from M195/M198 boundary work | Out of scope; do not edit queue dependency semantics. |

## Classifier output contract

The S02 evidence contract has no dedicated `source_kind=drift_classifier`. S07 therefore writes a metadata-only M198 evidence/report with:

- `schema_version=m198.readiness_evidence.v1`
- `source_kind=governance_ratchet`
- `drift_class=expected`, `warning`, or `blocker`
- `graph_writes_allowed=false`
- `schema_migration_allowed=false`
- `import_eligible=false`

## Required source evidence

S07 requires one evidence JSON for each producer source kind:

- `reactive_dry_run`
- `sync_no_write_rehearsal`
- `smoke_boundary`
- `graph_readiness_validate_only`

## Expected drift rules

These differences are expected and must not become blockers:

- reactive dry-run evidence has no queue artifact;
- sync rehearsal evidence has `queue.sqlite` and `queue_inspect.json`;
- sync rehearsal lacks standalone `queue_events.json`;
- smoke boundary records `queue_status=ready` from existing smoke behavior;
- graph readiness records retired alias absence and validate-only review status.

## Blocker rules

Classifier must fail closed when any source evidence has:

- missing required source kind;
- `graph_writes_allowed=true`;
- `schema_migration_allowed=true`;
- `import_eligible=true`;
- `status` not equal to `pass`;
- missing `evidence_refs`;
- forbidden payload-shaped terms;
- missing required source-specific diagnostics.

## Allowed S07 edits

- `scripts/run_m198_drift_classifier.py`
- `tests/test_m198_drift_classifier.py`
- S07 architecture assessment artifacts

## Disallowed S07 edits

- S03-S06 producer scripts
- `src/research_graph/workflows/universal_kb/*`
- `src/research_graph/infrastructure/graph/*` backend/import code
- schema migration code
- retired graph readiness alias restoration

## Downstream dependency map

- S08 consumes S07 drift classifier output for metadata-only evidence indexing.
- S09 consumes S07/S08 diagnostic surfaces for operator failure visibility.
