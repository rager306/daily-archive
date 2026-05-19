# S02: Batch initialization and source preflight — UAT

**Milestone:** M007-opaont
**Written:** 2026-05-19T19:15:23.164Z

# S02: Batch initialization and source preflight — UAT

## Smoke Test

Run the validation-batch init/preflight commands over the M006 manifest. Expected final source-preflight summary:

- `paper_count=30`
- `ready_for_markdown_scan_count=30`
- `pdf_present_count=8`
- `pdf_missing_count=22`
- `warning_count=20`
- `blocker_count=0`
- `production_import_attempted=false`
- `ladybugdb_written=false`

## Expected diagnostics

20 warnings with code:

```text
ready_with_missing_markdown_risk_tag
```

These represent historical missing-Markdown tags after successful source acquisition, not current blockers.

## Not implemented yet

- Acquisition/conversion repair.
- Deviation scan execution through validation-batch CLI.
- Delta/outlier gate reports.
- Review mutation.
- KG import/promotion.
