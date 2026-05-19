---
id: M006-638rza
title: "Thirty Paper Deviation Scan"
status: complete
completed_at: 2026-05-19T18:20:37.235Z
key_decisions:
  - Separate Markdown-scan readiness from full source/PDF completeness.
  - Use fast acquisition plus targeted Docling repair instead of bulk slow fallback.
  - Use M005/S03 as the apples-to-apples structure-aware baseline and M005/S06 only as import-boundary context.
  - Treat 30-paper patterns as routing/review evidence, not semantic correctness proof.
  - Recommend M007 deterministic CLI-first +10-to-100 validation automation; MiniMax remains optional bounded helper only.
key_files:
  - src/arxiv_archive/thirty_paper_source_scan.py
  - src/arxiv_archive/thirty_paper_deviation_scan.py
  - .gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json
  - .gsd/milestones/M006-638rza/slices/S02/run-evidence/source-acquisition-summary.json
  - .gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json
  - .gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md
  - .gsd/milestones/M006-638rza/slices/S04/thirty-paper-final-recommendation.md
lessons_learned:
  - Source availability can be the first scaling blocker even before chunking/import-readiness analysis.
  - A 30-paper scan exposes route-share shifts and outliers not obvious from the 10-paper baseline.
  - Independent review is useful even when it returns FLAG; flags can become planning requirements rather than blockers.
  - Operational scan automation and trusted KG promotion must remain separate milestones.
---

# M006-638rza: Thirty Paper Deviation Scan

**M006 expanded validation to 30 papers, found route/outlier patterns, and recommended deterministic +10-to-100 validation automation while keeping KG import blocked.**

## What Happened

M006 expanded validation from the M005 10-paper baseline to a deterministic 30-paper diagnostic scan. S01 selected and audited the corpus, discovering that 20/30 initially lacked Markdown and 28/30 lacked cached PDFs. S02 implemented bounded source acquisition, using fast arxiv2md-first acquisition and targeted Docling repair to make all 30 papers Markdown-scan-ready while preserving the no-import/no-write boundary. S03 implemented and ran a deterministic deviation scanner, producing 4,289 structure-aware chunks over 1,761,102 Markdown bytes, 30 per-paper diagnostics, 17,573 annotations, and 11 outlier papers. Import eligibility remained zero across all chunks. S04 independently reviewed the evidence, flagged overclaiming risks, and produced a final recommendation for a future deterministic +10-to-100 validation CLI milestone. The milestone closes with positive KG import blocked and a concrete next automation direction.

## Success Criteria Results

- 30-paper corpus selected: PASS.
- Markdown readiness achieved for scanning: PASS, 30/30.
- Deviation scan run: PASS, 4,289 chunks and 30 diagnostics.
- Baseline comparison produced: PASS, M005/S03 primary and M005/S06 contextual.
- Outlier and pattern analysis produced: PASS, 11 outliers and route shifts.
- Independent review completed: PASS with FLAG addressed.
- No production import/write: PASS.
- Future automation recommendation produced: PASS.

## Definition of Done Results

- PASS: All four slices are complete.
- PASS: Fresh focused verification passed: 47 tests, ruff, and artifact guards.
- PASS: Validation verdict is pass.
- PASS: Positive KG import remains blocked.
- PASS: Final recommendation identifies next M007 automation milestone scope.
- PASS: No raw/chunk text, embeddings, vectors, secrets, optimizer traces, or production writes are present in machine evidence.

## Requirement Outcomes

- R031 advanced/validated for this milestone: 30-paper deviation scan completed with source acquisition, scan evidence, review, and recommendation.
- R032 advanced: future deterministic +10-to-100 CLI automation requirements are now concrete.
- R030 advanced: source/PDF caveats are preserved and separated from Markdown-scan readiness.
- R029 advanced: chunk/import review boundary remains explicit and positive import remains blocked.

## Deviations

S01 revealed the first blocker was source availability, so S02 was added/re-scoped for bounded source acquisition. S03 initially produced a zero Markdown byte-size metric; this was corrected before completion. S04 independent review returned FLAG rather than PASS, and the final recommendation addressed the flags by narrowing claims and adding future automation requirements.

## Follow-ups

Plan M007 for deterministic +10-to-100 validation CLI automation. Keep positive KG import and trusted fact promotion out of M007 unless a reviewed promotion path is separately designed. Consider a bounded MiniMax adapter spike only after deterministic CLI artifacts exist.
