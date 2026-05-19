---
id: T02
parent: S01
milestone: M006-638rza
key_files:
  - .gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json
  - .gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-diagnostics.jsonl
key_decisions:
  - Treat missing Markdown/PDF as the first M006 deviation pattern, not as a hidden failure.
  - Proceed with reporting availability before deciding whether S02 should run only the 10 available papers or add a source acquisition step.
duration: 
verification_result: passed
completed_at: 2026-05-19T16:26:42.976Z
blocker_discovered: false
---

# T02: Audited source availability and found the first major deviation: 20 expansion papers lack Markdown source artifacts.

**Audited source availability and found the first major deviation: 20 expansion papers lack Markdown source artifacts.**

## What Happened

Audited source availability for the 30 selected papers. All 30 have research workspaces and paper metadata, but only the original M005 overlap appears ready for Markdown-based chunking: 10/30 have available Markdown and 2/30 have PDFs. The 20 expansion papers are currently blocked for Markdown scan due to missing full text/Markdown artifacts. The audit writes redacted per-paper diagnostics with availability flags, readiness status, missing-source reasons, risk tags, and no-import/no-write safety flags.

## Verification

Availability guard passed: 30 papers audited, diagnostics JSONL non-empty, no raw text or production import flags enabled.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python - <<'PY' ... availability summary guard ... PY` | 0 | ✅ pass — paper_count=30, ready_for_markdown_scan_count=10, missing_markdown=20, missing_pdf=28 | 8000ms |

## Deviations

The 30-paper corpus is selected, but only 10 papers currently have available Markdown and only 2 have PDFs in local cache. This is a key availability deviation and will constrain S02 unless source acquisition/conversion is added.

## Known Issues

20 expansion papers have research workspaces and paper metadata but lack available Markdown; 28/30 lack cached PDFs. A true 30-paper chunking scan requires acquiring/converting missing source artifacts or selecting a different expansion set with Markdown already present.

## Files Created/Modified

- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json`
- `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-diagnostics.jsonl`
