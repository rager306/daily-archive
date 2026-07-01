# M198 S13 Realistic Readiness Rehearsal Boundary

## Verdict

**PASS: S13 may add an additive temp-dir rehearsal harness that runs the S08 index, S09 diagnostics, and S10 report commands, but must not edit readiness producers, runtime workflow code, queue, smoke, rehearsal, graph backend/import code, or schema migration code.**

## GitNexus evidence

GitNexus was refreshed with `gitnexus analyze` after S12.

| Target | Result | Scope decision |
|---|---|---|
| `Function:scripts/run_m198_readiness_report.py:build_report` | LOW, impacted_count=2 | S13 may invoke the report command; do not edit the report generator. |
| `Method:src/research_graph/workflows/universal_kb/queue.py:UniversalKBQueue._dependencies_satisfied#1` | Current depth-2 impact returned LOW but still affects rehearsal/smoke flows; S12 contract preserves HIGH/out-of-scope gate from prior exact evidence | Do not edit queue dependency semantics. |
| Scoped detect_changes | LOW, affected_count=0 | No code changes pending before S13 start. |

## Dependency map

S13 consumes outputs and contracts from:

- S08: `m198.readiness_evidence_index.v1` command surface.
- S09: `m198.operator_diagnostics.v1` command surface.
- S10: `m198.readiness_report.v1` command surface.
- S11: no-write/import governance ratchets.
- S12: `m198.gitnexus_impact_gates.v1` contract.

## Rehearsal rules

The rehearsal harness must:

1. Use an isolated working/output directory supplied by the caller or created under temp space.
2. Create fixture readiness evidence metadata, not source payloads.
3. Run the S08 index, S09 diagnostics, and S10 report scripts as subprocess commands.
4. Capture command arguments, exit codes, artifact refs, and final verdict.
5. Confirm no graph writes, schema migrations, import eligibility, production imports, queue semantic changes, smoke semantic changes, rehearsal semantic changes, or retired shim restoration.
6. Exit 2 if the final report verdict is blocked.

## Output contract

S13 writes:

- JSON: `m198.readiness_rehearsal.v1`
- Markdown: command-level rehearsal summary

Required JSON content:

- command log with names, exit codes, and outputs;
- produced artifact paths;
- final readiness verdict;
- metadata-only confirmation;
- no-write/import boundary confirmation;
- downstream handoff to S14-S16.

## Allowed S13 edits

- `scripts/run_m198_readiness_rehearsal.py`
- `tests/test_m198_readiness_rehearsal.py`
- S13 architecture assessment artifacts

## Disallowed S13 edits

- S03-S10 readiness scripts
- `src/research_graph/workflows/universal_kb/*`
- graph backend/import code
- schema migration code
- retired graph readiness alias restoration

## Downstream dependency map

- S14 consumes rehearsal output for smoke parity audit.
- S15 consumes rehearsal output for disabled backend safety checks.
- S16 consumes rehearsal output for the end-to-end validation package.
