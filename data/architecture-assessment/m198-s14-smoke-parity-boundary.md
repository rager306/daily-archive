# M198 S14 Smoke Parity Boundary

## Verdict

**PASS: S14 may add an additive smoke parity audit over S13 rehearsal output, but must not edit smoke runner, smoke workflow, queue, rehearsal runtime, graph backend/import code, schema migration code, or prior readiness scripts.**

## GitNexus evidence

GitNexus was refreshed with `gitnexus analyze` after S13.

| Target | Result | Scope decision |
|---|---|---|
| `Function:scripts/run_m198_readiness_rehearsal.py:run_rehearsal` | LOW, impacted_count=2 | S14 may consume rehearsal output; do not edit rehearsal harness. |
| `Function:src/research_graph/workflows/universal_kb/smoke_runner.py:run_article` | LOW partial, impacted_count=0 | Treat as smoke runtime no-edit seam; S14 audits parity only. |
| Scoped detect_changes | LOW, affected_count=0 | No code changes pending before S14 start. |

## Input contracts

S14 consumes:

- S13 `m198.readiness_rehearsal.v1` JSON.
- The rehearsal-produced S08 index artifact referenced by the summary.
- Metadata only: source kinds, command exits, blocked transitions, boundary confirmations, warnings, blockers, and verdicts.

S14 must not read source payloads or execute smoke runtime commands.

## Required parity checks

- Smoke boundary source kind is present in the rehearsal evidence files or index entries.
- Rehearsal command chain includes `evidence_index`, `operator_diagnostics`, and `readiness_report`.
- Smoke semantic change remains blocked/non-goal.
- No-write/import boundary confirmations remain false.
- Final verdict propagation is consistent with blockers.
- Payload policy remains metadata-only.

## Allowed S14 edits

- `scripts/run_m198_smoke_parity_audit.py`
- `tests/test_m198_smoke_parity_audit.py`
- S14 architecture assessment artifacts

## Disallowed S14 edits

- S03-S13 readiness scripts
- `src/research_graph/workflows/universal_kb/*`
- graph backend/import code
- schema migration code
- retired graph readiness alias restoration

## Downstream dependency map

- S15 consumes smoke parity findings alongside disabled backend safety checks.
- S16 consumes smoke parity evidence in the end-to-end validation package.
