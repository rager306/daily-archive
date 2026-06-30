# M198 S08 Evidence Index Boundary

## Verdict

**PASS: S08 may add a metadata-only evidence index writer, but must not edit producers, classifier, runtime workflow code, graph backend/import code, or schema migration code.**

## GitNexus evidence

GitNexus was refreshed with `gitnexus analyze` after S07.

| Target | Result | Scope decision |
|---|---|---|
| `Function:scripts/run_m198_drift_classifier.py:classify` | LOW, impacted_count=2 | S08 may consume classifier output as input; do not edit classifier. |
| `Function:scripts/run_m198_dry_run_probe.py:build_evidence` | LOW, impacted_count=2 | S08 may consume output as input; do not edit producer. |
| S04-S06 producer `build_evidence` symbols | Previously unresolved after refresh | Treat as new-symbol GitNexus limitation; consume outputs only and verify locally. |
| `UniversalKBQueue._dependencies_satisfied#1` | HIGH from M195/M198 boundary work | Out of scope; do not edit queue dependency semantics. |

## Required source evidence

S08 index requires one evidence/report JSON for each source kind:

- `reactive_dry_run`
- `sync_no_write_rehearsal`
- `smoke_boundary`
- `graph_readiness_validate_only`
- `governance_ratchet` from S07 drift classifier

## Index output contract

S08 writes `m198.readiness_evidence_index.v1` JSON with metadata only:

- source kind;
- evidence id;
- status;
- drift class;
- evidence refs;
- source checksums;
- file checksum;
- non-goal coverage;
- warning and blocker summaries;
- missing-source diagnostics.

## Payload boundary

The index must not copy:

- source document text;
- raw prompts;
- embeddings;
- vectors;
- queue database bytes;
- credentials or credential-shaped strings;
- production graph payloads.

It may store only paths, checksums, booleans, counts, statuses, drift classes, and diagnostics summaries.

## Blocker rules

The index writer must fail closed when:

- required source kind is missing;
- duplicate source kind appears;
- any source has forbidden payload-shaped terms;
- source checksum changed between expected and actual input;
- any source has `graph_writes_allowed`, `schema_migration_allowed`, or `import_eligible` not false.

## Allowed S08 edits

- `scripts/run_m198_evidence_index.py`
- `tests/test_m198_evidence_index.py`
- S08 architecture assessment artifacts

## Disallowed S08 edits

- S03-S07 producer/classifier scripts
- `src/research_graph/workflows/universal_kb/*`
- `src/research_graph/infrastructure/graph/*` backend/import code
- schema migration code
- retired graph readiness alias restoration

## Downstream dependency map

- S09 consumes indexed warnings/blockers for operator diagnostics.
- S16-S18 consume the index for final evidence packaging and closeout validation.
