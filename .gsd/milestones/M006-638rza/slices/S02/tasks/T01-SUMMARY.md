---
id: T01
parent: S02
milestone: M006-638rza
key_files:
  - .gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md
key_decisions:
  - Use existing MDConverter/PDFDownloader/full_text quality mechanisms instead of a new acquisition stack.
  - Run one bounded conversion attempt per missing-Markdown paper; no unbounded retries or Marker dependency.
  - Persist only redacted diagnostics and path/hash/quality metadata, not raw Markdown text in JSON/JSONL.
duration: 
verification_result: passed
completed_at: 2026-05-19T16:43:29.108Z
blocker_discovered: false
---

# T01: Defined the bounded source acquisition plan for the 20 missing-Markdown expansion papers.

**Defined the bounded source acquisition plan for the 20 missing-Markdown expansion papers.**

## What Happened

Inspected the existing source acquisition and conversion paths. The project already provides a bounded converter path through `MDConverter`: cached Markdown, arxiv2md, PDF download, optional Marker, and Docling fallback with full-text quality checks. The S02 plan defines sequential/bounded conversion attempts for the 20 missing-Markdown papers, strict no-import/no-write boundaries, no raw text in machine logs, and clear go/partial-go criteria for S03.

## Verification

Plan guard passed: source acquisition plan exists and contains the required bounded acquisition language.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `test -s .gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md && grep -q 'bounded' .gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md` | 0 | ✅ pass | 4500ms |

## Deviations

None.

## Known Issues

Network/API/PDF conversion reliability is still unknown for the 20 missing-Markdown expansion papers; T03 will reveal actual success/failure rates.

## Files Created/Modified

- `.gsd/milestones/M006-638rza/slices/S02/source-acquisition-plan.md`
