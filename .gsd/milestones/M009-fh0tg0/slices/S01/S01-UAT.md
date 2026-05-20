# S01: CLI run provenance log — UAT

**Milestone:** M009-fh0tg0
**Written:** 2026-05-20T04:41:55.621Z

# S01: CLI run provenance log — UAT

## Expected

- Provenance module hashes inputs/outputs without raw contents.
- Secret-like argv values are redacted.
- Freshness report passes unchanged files and can fail stale/missing/unsafe cases in tests.
- Sample artifacts are written under S01 run-evidence.

## Result

- Provenance schema: `m009-validation-cli-provenance.v1`.
- Freshness schema: `m009-artifact-freshness-report.v1`.
- Sample freshness verdict: `fresh`.
- Input hashes recorded: 1.
- Output hashes recorded: 1.
- Safety flags false.
- 18 focused tests passed.
- Ruff passed.

## Caveat

S01 does not wire existing CLI commands yet; S02/S03 will add verifier and real command integration.
