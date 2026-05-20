---
id: M009-fh0tg0
title: "Validation CLI Provenance and Top Up Hardening"
status: complete
completed_at: 2026-05-20T05:35:40.869Z
key_decisions:
  - Use provenance/freshness as an audit layer outside ValidationBatchState.
  - Expose `validation-batch verify-artifacts` for freshness checks.
  - Use optional `--milestone-id` for active scan lineage metadata.
  - Treat top-up planning as permission only; replacements still require materialization and preflight.
  - Allow one next +10 only under explicit gates, not unattended automation.
key_files:
  - src/arxiv_archive/validation_batch_provenance.py
  - src/arxiv_archive/validation_batch_workflow.py
  - src/arxiv_archive/cli.py
  - tests/test_validation_batch_provenance.py
  - tests/test_validation_batch_cli_freshness.py
  - tests/test_validation_batch_top_up.py
  - .gsd/milestones/M009-fh0tg0/slices/S05/run-evidence/final-hardening-guard.json
  - .gsd/milestones/M009-fh0tg0/slices/S05/hardening-final-recommendation.md
lessons_learned:
  - Artifact existence is not enough; run provenance and hash freshness must be checked.
  - Hash-valid artifacts can still be stale if embedded milestone/batch lineage is wrong.
  - Top-up planning must not be confused with materialized source readiness.
  - Review FLAG can be a valid close state when it gates the next step clearly.
---

# M009-fh0tg0: Validation CLI Provenance and Top Up Hardening

**M009 added provenance, freshness, active lineage, and bounded top-up hardening, allowing one next +10 only under explicit gates.**

## What Happened

M009 hardened the validation-batch workflow after M008 revealed weak CLI provenance and underfilled-batch handling. S01 added provenance/freshness primitives that hash inputs and outputs without serializing raw contents. S02 exposed `validation-batch verify-artifacts` to prove freshness or fail stale/missing/invalid artifacts. S03 added active milestone/batch lineage metadata to scan outputs and verifier checks for metadata mismatch. S04 added deterministic bounded top-up planning with successful and blocked shortage evidence. S05 independently reviewed the work and returned FLAG: the hardening is meaningful, but another +10 may run only with explicit runbook gates because real commands still do not auto-emit provenance and top-up is planning-only. M009 closes as operational hardening, not unattended automation readiness.

## Success Criteria Results

- PASS — Provenance entries and freshness reports implemented and tested.
- PASS — Verifier detects fresh, stale, missing, input mutation, and invalid selection cases.
- PASS — Active scan lineage metadata implemented and stale metadata mismatch detected.
- PASS — Bounded top-up planner produces pass and blocked shortage artifacts.
- PASS — Independent review completed.
- ATTENTION — Real commands do not auto-emit provenance; top-up does not materialize replacements.
- PASS — Import/write safety boundaries preserved.

## Definition of Done Results

- PASS — All five slices complete.
- PASS — Fresh milestone verification passed: 42 focused tests and ruff.
- PASS — Provenance/freshness primitives and CLI verifier exist.
- PASS — Active scan lineage metadata exists.
- PASS — Bounded top-up planning exists with pass/block samples.
- PASS WITH ATTENTION — Independent review verdict FLAG.
- PASS — No production import or LadybugDB writes enabled.

## Requirement Outcomes

- R036 advanced: provenance primitives, CLI verifier, and metadata mismatch checks exist; automatic real-run provenance emission remains active follow-up.
- R035 advanced: bounded top-up planning and blockers exist; replacement materialization/preflight remains active follow-up.
- R034 prepared for next gated +10, not unattended scaling.

## Deviations

M009 intentionally stops short of automatic provenance emission and real replacement acquisition/preflight integration. The final recommendation allows one next +10 only with explicit runbook gates rather than unattended automation.

## Follow-ups

Plan one next reviewed +10 validation batch using M009 gates: active --milestone-id, real provenance entry, verify-artifacts fresh verdict, expected milestone/batch metadata, materialized/preflighted replacements, and no-write/no-import boundaries. Future hardening can automate provenance emission and top-up acquisition integration.
