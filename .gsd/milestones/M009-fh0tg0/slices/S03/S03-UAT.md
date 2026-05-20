# S03: Active scan lineage metadata — UAT

**Milestone:** M009-fh0tg0
**Written:** 2026-05-20T05:15:47.371Z

# S03: Active scan lineage metadata — UAT

## Expected

- Validation-batch scan artifacts can carry active milestone and batch context.
- Stale M006-style metadata can be detected by the freshness verifier.
- Existing scan behavior remains compatible.

## Result

- Added `--milestone-id` to `validation-batch scan`.
- Scan summary/delta/outlier artifacts include `milestone_id` and `batch_id` when active lineage is supplied.
- Summary `milestone` is set to active milestone when provided.
- Fresh lineage sample verdict: `fresh`.
- Mismatch lineage sample verdict: `stale`.
- Mismatch diagnostic: `artifact_metadata_mismatch`.
- 19 focused tests passed.
- Ruff passed.

## Caveat

Real CLI provenance emission is still not automatic; this slice fixes lineage metadata and verifier detection only.
