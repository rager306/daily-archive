# M198 S09 Operator Diagnostics Boundary

## Verdict

**PASS: S09 may add an operator diagnostics writer that consumes S08 metadata-only index output, but must not edit producers, classifier, index writer, runtime workflow code, graph backend/import code, or schema migration code.**

## GitNexus evidence

GitNexus was refreshed with `gitnexus analyze` after S08.

| Target | Result | Scope decision |
|---|---|---|
| `Function:scripts/run_m198_evidence_index.py:build_index` | LOW, impacted_count=2 | S09 may consume index output as input; do not edit index writer. |
| `Function:scripts/run_m198_drift_classifier.py:classify` | LOW partial, impacted_count=0 | S09 consumes S08 index, not classifier internals. |
| `UniversalKBQueue._dependencies_satisfied#1` | HIGH from M195/M198 boundary work | Out of scope; do not edit queue dependency semantics. |

## Input contract

S09 consumes only `m198.readiness_evidence_index.v1` JSON produced by S08.

Required input fields:

- `schema_version`
- `status`
- `required_source_kinds`
- `observed_source_kinds`
- `missing_source_kinds`
- `entry_count`
- `entries`
- `warnings`
- `blockers`
- `metadata_only`
- `payload_policy`

## Output contract

S09 writes:

- JSON: `m198.operator_diagnostics.v1`
- Markdown: human/operator summary

Required diagnostic content:

- verdict: `ready`, `needs_attention`, or `blocked`;
- readiness state: boolean;
- source coverage summary;
- blocker list;
- warning list;
- required next actions;
- blocked transitions/non-goals;
- payload policy confirmation.

## Payload boundary

S09 must not read source evidence payloads. It must operate only on the S08 index. It may copy only metadata already present in the index: statuses, drift classes, source kinds, counts, warnings, blockers, checksums, and policy booleans.

## Verdict rules

- `ready`: index status is `pass`, no blockers, no warnings, no missing sources.
- `needs_attention`: index status is `pass`, no blockers, warnings are present.
- `blocked`: index status is `fail`, blockers are present, missing sources exist, or payload policy is not metadata-only.

## Allowed S09 edits

- `scripts/run_m198_operator_diagnostics.py`
- `tests/test_m198_operator_diagnostics.py`
- S09 architecture assessment artifacts

## Disallowed S09 edits

- S03-S08 producer/classifier/index scripts
- `src/research_graph/workflows/universal_kb/*`
- `src/research_graph/infrastructure/graph/*` backend/import code
- schema migration code
- retired graph readiness alias restoration

## Downstream dependency map

- S10 consumes S09 diagnostics for readiness report synthesis.
- S16-S18 consume S09 diagnostics for final evidence package and closeout validation.
