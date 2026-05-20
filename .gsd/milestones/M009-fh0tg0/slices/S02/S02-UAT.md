# S02: Artifact freshness verifier — UAT

**Milestone:** M009-fh0tg0
**Written:** 2026-05-20T04:47:14.321Z

# S02: Artifact freshness verifier — UAT

## Expected

- CLI can verify a provenance JSONL run.
- Fresh artifacts exit 0.
- Stale/missing/invalid artifacts exit nonzero.
- Reports remain redacted.

## Result

- Added `validation-batch verify-artifacts`.
- Fresh sample verdict: `fresh`.
- Stale sample verdict: `stale`.
- Stale diagnostic codes: `output_hash_changed`, `output_size_changed`.
- Safety flags false.
- 20 focused tests passed.
- Ruff passed.

## Caveat

Real validation-batch commands are not yet writing provenance logs automatically; that belongs to the next integration slice.
