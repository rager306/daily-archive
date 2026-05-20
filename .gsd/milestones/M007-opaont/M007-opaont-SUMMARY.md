---
id: M007-opaont
title: "Iterative Validation CLI Automation"
status: complete
completed_at: 2026-05-20T02:06:03.748Z
key_decisions:
  - Use deterministic CLI/state artifacts for validation-batch automation.
  - Make init/preflight/scan real while review/resume remain future work or stubs.
  - Separate workflow proof from new-batch validation.
  - Proceed next with one reviewed +10 batch, not unattended scaling to 100.
  - Keep MiniMax optional/future-only and outside the deterministic core workflow.
key_files:
  - src/arxiv_archive/validation_batch_state.py
  - src/arxiv_archive/validation_batch_workflow.py
  - src/arxiv_archive/cli.py
  - .gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json
  - .gsd/milestones/M007-opaont/slices/S03/run-evidence/validation-scan-summary.json
  - .gsd/milestones/M007-opaont/slices/S03/run-evidence/delta-report.json
  - .gsd/milestones/M007-opaont/slices/S04/validation-workflow-final-recommendation.md
lessons_learned:
  - Adding Typer subcommands can break legacy root options; keep regression tests for CLI entrypoints.
  - Source preflight must inspect deterministic fallback paths, not stale manifest source paths only.
  - Mixed benchmark artifacts may have nested aggregate shapes; parsers must account for that before interpreting deltas.
  - A workflow proof over an existing corpus is valuable but must not be claimed as a new expansion batch.
---

# M007-opaont: Iterative Validation CLI Automation

**M007 built and reviewed deterministic validation-batch CLI automation for init, preflight, and scan, readying the project for the first new +10 batch while keeping KG import blocked.**

## What Happened

M007 converted the M006 manual diagnostic workflow into deterministic validation-batch CLI automation. S01 created the contract, state schema, and safe CLI namespace. S02 made batch initialization and source preflight real, producing 30-paper readiness artifacts with 30 Markdown-ready papers, 8 PDFs present, 22 PDFs missing, 20 historical missing-Markdown warnings, and 0 blockers. S03 automated scan/delta/outlier generation through `validation-batch scan`, reproducing the 30-paper evidence with 4,289 chunks, 11 outliers, zero import-eligible chunks, and separated M005/S03 versus M005/S06 baseline contexts. S04 independently reviewed the workflow, returned a scoped FLAG that M007 proves workflow over the existing 30-paper corpus rather than a new +10 batch, and recommended the next milestone run the first genuinely new +10 batch. Throughout, positive KG import and production LadybugDB writes remained blocked.

## Success Criteria Results

- Deterministic CLI-first workflow exists: PASS.
- Batch state is resumable/artifact-driven: PASS.
- Source readiness, scan metrics, deltas, outliers, and gates automated: PASS.
- Safety boundaries enforced: PASS.
- Independent review approves proceeding to first new +10 batch: PASS with FLAG framing.
- Positive KG import blocked: PASS.

## Definition of Done Results

- PASS: All four slices complete.
- PASS: 59 tests passed and ruff passed.
- PASS: Artifact guards confirmed preflight and scan evidence.
- PASS: Independent review completed.
- PASS: Final recommendation produced.
- PASS: Positive KG import remains blocked.
- PASS: No raw/chunk text, embeddings, vectors, optimizer traces, secrets, production imports, or LadybugDB writes in machine artifacts.

## Requirement Outcomes

- R033 validated: deterministic resumable validation-batch CLI workflow exists for init/preflight/scan with persisted state and redacted artifacts.
- R032 advanced: the +10-to-100 loop now has a workflow foundation and next-step recommendation for the first new +10 batch.
- R029/R030 preserved: KG import remains blocked; source/PDF readiness remains separate from Markdown-scan readiness.

## Deviations

S03/S04 review clarified that M007 proves the workflow over the existing 30-paper corpus, not a newly selected +10 batch. This is captured as a FLAG and next-step boundary rather than a blocker. During S03, mixed benchmark parsing was corrected to handle nested S06 aggregate totals.

## Follow-ups

Plan M008 for the first genuinely new +10-paper validation batch using the M007 workflow. Add structured JSON error payloads for invalid CLI phases and historical risk-tag resolution fields in future work. Keep KG import out of M008 unless separately reviewed promotion criteria exist.
