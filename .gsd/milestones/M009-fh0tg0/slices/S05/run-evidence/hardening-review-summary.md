# Independent review — M009 validation hardening

Verdict: FLAG

## Evidence checked

- `src/arxiv_archive/validation_batch_provenance.py`
- `src/arxiv_archive/validation_batch_workflow.py`
- `src/arxiv_archive/cli.py`
- `tests/test_validation_batch_provenance.py`
- `tests/test_validation_batch_cli_freshness.py`
- `tests/test_validation_batch_top_up.py`
- `tests/test_validation_batch_scan_workflow.py`
- `tests/test_validation_batch_cli_scan.py`
- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-pass-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S02/run-evidence/freshness-stale-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-pass-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S03/run-evidence/lineage-mismatch-report.json`
- `.gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-pass-summary.json`
- `.gsd/milestones/M009-fh0tg0/slices/S04/run-evidence/top-up-blocked-summary.json`

## Findings

### 1. Provenance/freshness verification is solid as a verifier, but not yet integrated with real runs

`validation-batch verify-artifacts` reads provenance JSONL, selects a run, checks input/output hashes, rejects stale/missing/invalid provenance, and exits nonzero unless `verdict=fresh`.

Tests cover:

- fresh pass;
- report writing;
- output mutation;
- output deletion;
- input mutation;
- invalid selection redaction;
- no raw sentinel leakage.

However, real `validation-batch init`, `preflight`, and `scan` commands still do not automatically emit provenance logs. Freshness is currently enforceable only when a provenance entry is produced separately or by a wrapper.

### 2. Active scan lineage is improved, but optional

`validation-batch scan --milestone-id ...` stamps active lineage into scan manifest, source-readiness, summary, delta, and outlier artifacts. The summary `milestone` value is corrected when active milestone lineage is supplied.

The freshness verifier can detect a hash-valid but lineage-wrong artifact through `expected_artifact_metadata` and `artifact_metadata_mismatch`.

However, `--milestone-id` is optional, so a future run can still produce lineage-less artifacts unless the runbook/final guard requires it.

### 3. Bounded top-up planning is auditable, but not full acquisition integration

The top-up planner:

- skips already selected papers;
- respects `max_candidates_to_consider`;
- records accepted and rejected candidates;
- blocks scan when target quota remains short.

Evidence demonstrates both pass and blocked outcomes.

But readiness is inferred from candidate inventory metadata. The planner does not fetch sources, convert papers, mutate batch state, or rerun preflight over replacements.

## Risks and caveats

- Real validation-batch commands do not yet automatically emit provenance logs.
- Active milestone lineage works only when `--milestone-id` is passed.
- Top-up success is planning permission, not proof that replacement papers are materialized and preflight-ready.
- S02/S03/S04 evidence is strong for library/CLI primitives, but sample-based rather than from an automatically provenance-emitting real scan.
- Positive KG import and production LadybugDB writes remain blocked.

## Recommendation

Do not treat M009 S01-S04 as full unattended automation readiness.

A carefully reviewed next +10 may proceed only if its runbook enforces these gates:

1. Real scan must be invoked with active `--milestone-id`.
2. A provenance JSONL entry must be produced for the real run.
3. `validation-batch verify-artifacts` must return `fresh` for the real provenance entry.
4. Expected artifact metadata must include at least `milestone_id` and `batch_id`.
5. Any top-up replacements must be materialized into batch state and preflighted before scan.
6. `scan_allowed=true` from top-up is planning permission only, not final scan proof.

Without those gates, the next +10 should remain blocked.
