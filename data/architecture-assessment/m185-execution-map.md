# M185 Execution Map

## Ordered waves

1. **S01 GitNexus Refresh and Candidate Map**: current slice; no source edits.
2. **S02 Wrapper Contract Baseline**: confirm wrapper tests before movement.
3. **S03 Test Architecture Audit Extraction Pilot**: first source pilot because GitNexus impact is LOW/exact and affected callers are local script mains.
4. **S04 Pipeline Script Audit Extraction Pilot**: second pilot because `build_inventory` impact is LOW/exact and existing application inventory types already exist.
5. **S05-S06 Higher-risk verifier probes**: M025 and M031 only after the first two pilots are stable.
6. **S07 Wrapper Extraction Ratchet Update**: summarize and ratchet only proven outcomes.
7. **S08-S11 Manifest/cache lifecycle probes**: review residuals; no movement without complete lifecycle proof.
8. **S12-S14 Verification, quality, and closeout**.

## Initial impact summaries

| Candidate | GitNexus target | Result | Blast radius | Decision |
|---|---|---|---|---|
| Test architecture `write_outputs` | `Function:scripts/audit_test_architecture.py:write_outputs` | LOW, exact | direct caller `main`, imported by `verify_test_architecture.py` | safe enough to inspect in S03 |
| Test architecture `build_inventory` | `Function:scripts/audit_test_architecture.py:build_inventory` | LOW, exact | direct callers `audit_test_architecture.main`, `verify_test_architecture.main` | safe enough to inspect in S03 |
| Pipeline audit `build_inventory` | `Function:scripts/audit_pipeline_scripts.py:build_inventory` | LOW, exact | direct caller `audit_pipeline_scripts.main` | safe enough to inspect in S04 |

## First-pilot choices

S03 should inspect `scripts/audit_test_architecture.py` first. If moving the helper would create a worse boundary than the script-local implementation, S03 must record no-move and still pass guards.

S04 should inspect `scripts/audit_pipeline_scripts.py` second. Because it already depends on `research_graph.application.pipeline_script_inventory`, consolidation may be possible with a minimal diff.

## No-move boundaries

- Four manifest/cache residuals remain no-move until lifecycle proof is complete.
- M025 and M031 verifier scripts are not first pilots because they are larger and likely have more safety-specific local helpers.
- GitNexus ambiguous results are not safety proof; only disambiguated LOW/exact results are considered planning evidence.
