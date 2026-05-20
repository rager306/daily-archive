# S03: Active lineage scan and freshness proof — UAT

**Milestone:** M010-06v9ke
**Written:** 2026-05-20T07:31:44.439Z

# S03: Active lineage scan and freshness proof — UAT

## Expected

- Scan only the materialized S02 batch state.
- Use active `--milestone-id M010-06v9ke`.
- Produce real provenance and verify freshness.
- Keep import/write gates closed.

## Result

- Batch state: `.gsd/milestones/M010-06v9ke/slices/S02/run-evidence/source-ready-batch-state.json`
- Milestone id: `M010-06v9ke`
- Batch id: `m010-next-plus-ten-materialized`
- Paper count: `10`
- Chunk count: `1477`
- Outlier count: `7`
- Import-eligible chunk count: `0`
- Freshness run id: `m010-s03-scan-002`
- Freshness verdict: `fresh`
- Positive import allowed: `false`
- Production import attempted: `false`
- LadybugDB written: `false`

## Review note

A first provenance attempt (`m010-s03-scan-001`) returned stale because metadata expectations included JSONL diagnostics and the response wrapper. The accepted proof is `m010-s03-scan-002`.
