# S01: Thirty paper corpus selection and availability audit — UAT

**Milestone:** M006-638rza
**Written:** 2026-05-19T16:28:37.880Z

# S01: Thirty paper corpus selection and availability audit — UAT

## Smoke Test

Open `.gsd/milestones/M006-638rza/slices/S01/run-evidence/thirty-paper-availability-summary.json` and confirm:

- `paper_count=30`
- `m005_overlap_count=10`
- `expansion_count=20`
- `available_markdown=10`
- `missing_markdown=20`
- `production_import_attempted=false`
- `ladybugdb_written=false`

## Expected Result

S01 does not prove a full 30-paper chunking scan is ready. It proves the corpus is selected and that source availability is the first deviation to address.

## Not Proven

- Full 30-paper chunking/import-model measurement.
- Positive KG import readiness.
- Production writes.
- Semantic/vector retrieval quality.
