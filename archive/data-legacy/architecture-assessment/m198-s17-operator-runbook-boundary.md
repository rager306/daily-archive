# M198 S17 Operator Runbook Boundary

## Verdict

**PASS: S17 may add operator documentation and runbook tests, but must not edit runtime workflow code, queue, smoke, rehearsal, graph backend/import code, schema migration code, or readiness scripts.**

## GitNexus evidence

Post-S16 GitNexus evidence:

| Target | Result | Scope decision |
|---|---|---|
| `Function:scripts/run_m198_validation_package.py:build_package` | UNKNOWN target not found after refresh | Treat as new-symbol limitation; S17 references package command only and does not edit package script. |
| Scoped detect_changes | LOW, affected_count=0 | No code changes pending before S17 start. |

## Runbook inputs

The runbook must reference these contracts:

- `m198.readiness_rehearsal.v1`
- `m198.smoke_parity_audit.v1`
- `m198.disabled_backend_safety.v1`
- `m198.validation_package.v1`
- `m198.gitnexus_impact_gates.v1`

## Required operator guidance

- Command sequence for S13-S16 scripts.
- Expected exit code 0 for ready/pass and 2 for blocked/fail.
- How to interpret blockers and warnings.
- GitNexus refresh command: `gitnexus analyze` from repo root.
- GitNexus detect_changes must be repo-scoped to `daily-archive`.
- No-write/import non-goals remain enforced.

## Forbidden runbook instructions

The runbook must not instruct operators to:

- enable production graph import;
- run schema migrations;
- edit queue dependency semantics;
- edit smoke or rehearsal runtime semantics;
- restore retired graph readiness shims;
- copy or expose raw payloads, embeddings, vectors, secrets, or credentials.

## Allowed S17 edits

- `data/architecture-assessment/m198-operator-readiness-runbook.md`
- `tests/test_m198_operator_runbook.py`
- S17 architecture assessment artifacts

## Disallowed S17 edits

- S03-S16 readiness scripts
- `src/research_graph/workflows/universal_kb/*`
- graph backend/import code
- schema migration code
- retired graph readiness alias restoration

## Downstream dependency map

- S18 consumes the runbook for final validation and milestone closeout.
