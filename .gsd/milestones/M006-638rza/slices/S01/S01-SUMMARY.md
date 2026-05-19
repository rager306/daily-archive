---
id: S01
parent: M006-638rza
milestone: M006-638rza
provides:
  - 30-paper corpus manifest
  - Source availability summary and per-paper diagnostics
  - Recommendation to add source acquisition/conversion before full 30-paper dry run
requires:
  []
affects:
  - S02
key_files:
  - .gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json
  - .gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json
  - .gsd/milestones/M006-638rza/slices/S01/thirty-paper-availability-report.md
key_decisions:
  - Keep all 10 M005 papers as overlap for direct baseline comparison.
  - Use deterministic local expansion for 20 additional papers, but treat source availability as a first-class deviation.
  - Do not claim a full 30-paper chunking scan until missing Markdown is acquired or converted.
patterns_established:
  - Broader validation must audit source readiness before chunking/import metrics.
  - Research workspace presence is not enough evidence of full-text readiness.
  - Missing-source deviations should be separated from chunking/import-model deviations.
observability_surfaces:
  - .gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json — selected paper ids, roles, risk tags, availability flags
  - .gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json — aggregate availability and missing-source counts
  - .gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-diagnostics.jsonl — per-paper availability diagnostics
drill_down_paths:
  - .gsd/milestones/M006-638rza/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M006-638rza/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M006-638rza/slices/S01/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-19T16:28:37.880Z
blocker_discovered: false
---

# S01: Thirty paper corpus selection and availability audit

**The 30-paper corpus is selected, but the audit shows only 10 papers are currently Markdown-ready.**

## What Happened

S01 selected and audited a 30-paper corpus for the M006 deviation scan. The manifest includes the full M005 10-paper overlap plus 20 deterministic local expansion papers. The availability audit found that all 30 papers have research workspaces and metadata, but only 10 have Markdown source artifacts and only 2 have cached PDFs. This means the first new pattern is source-readiness drift: local paper metadata does not imply full-text availability. S01 recommends reworking S02 to acquire/convert the missing Markdown before trying to measure chunking/import-model deviations across all 30.

## Verification

S01 guards passed: manifest has 30 unique paper ids with 10 M005 overlap; availability summary audited all 30 papers; report exists and documents missing-Markdown blockers; all no-import/no-write/no-payload flags remain false.

## Requirements Advanced

- R031 — S01 selects the 30-paper corpus and shows that source availability itself is a major deviation to handle before chunking analysis.
- R030 — S01 reinforces that source artifact preservation/readiness must be measured before broader import-model validation.

## Requirements Validated

None.

## New Requirements Surfaced

- A future operational requirement may be needed: expansion batches must include a source-acquisition/conversion preflight before chunking/import-model scans.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

The planned 30-paper scan cannot yet run meaningful chunking/import-model evidence for all 30 because 20 expansion papers lack Markdown. S01 therefore recommends changing S02 from direct dry-run evidence to source acquisition/conversion plus dry-run where available.

## Known Limitations

The expansion corpus is availability-biased and source-incomplete. It is suitable for deviation discovery only after source acquisition/conversion or as a source-readiness finding.

## Follow-ups

Replan S02 to include bounded source acquisition/conversion for the 20 missing-Markdown expansion papers, or explicitly run only a partial scan with source blockers. For the user's goal of identifying deviations over 30 papers, source acquisition/conversion is the better next step.

## Files Created/Modified

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json` — 30-paper corpus manifest with M005 overlap and expansion papers.
- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-rationale.md` — Selection rationale.
- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json` — Availability summary showing only 10 Markdown-ready papers and 2 cached PDFs.
- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-diagnostics.jsonl` — Per-paper source availability diagnostics.
- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-availability-report.md` — Readiness report and S02 recommendation.
