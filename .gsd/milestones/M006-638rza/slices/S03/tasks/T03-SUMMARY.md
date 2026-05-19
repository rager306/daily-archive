---
id: T03
parent: S03
milestone: M006-638rza
key_files:
  - .gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md
key_decisions:
  - Use M005/S03 as the primary apples-to-apples baseline for structure-aware route shares, while treating M005/S06 as broader mixed benchmark context.
  - Recommend S04 review before turning patterns into automation rules or CLI workflow requirements.
duration: 
verification_result: passed
completed_at: 2026-05-19T18:08:16.054Z
blocker_discovered: false
---

# T03: Reported 30-paper deviation patterns against the M005 baseline.

**Reported 30-paper deviation patterns against the M005 baseline.**

## What Happened

Wrote the 30-paper deviation report. The report compares M006/S03's 30-paper structure-aware scan against M005/S03's 10-paper structure-aware baseline, identifies route-share shifts, lists 11 outlier papers, documents continued zero import eligibility, and separates Markdown-based chunking conclusions from PDF/multimodal caveats. The report highlights stronger method, figure, citation, claim, and table route visibility in the broader sample, with retrieval-only still dominant but less so than in M005.

## Verification

Report guard passed: the report exists, references M005 and 30-paper deviation analysis, includes 4,289 chunk evidence, and states import eligibility remains zero.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `test -s .gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md && uv run python - <<'PY' ... report guard ... PY` | 0 | ✅ pass — deviation-report-ok | 16100ms |

## Deviations

None.

## Known Issues

The report is Markdown-based and not multimodal/PDF-complete; cached PDFs are available for 8/30 papers. Outlier flags are deterministic heuristics and need independent review before being treated as final quality judgments.

## Files Created/Modified

- `.gsd/milestones/M006-638rza/slices/S03/thirty-paper-deviation-report.md`
