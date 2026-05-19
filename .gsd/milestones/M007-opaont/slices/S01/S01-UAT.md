# S01: CLI contract and batch state model — UAT

**Milestone:** M007-opaont
**Written:** 2026-05-19T18:58:15.667Z

# S01: CLI contract and batch state model — UAT

## Smoke Test

Run:

```bash
uv run python -m arxiv_archive validation-batch contract --json
```

Expected:

- exit code 0;
- `status=contract_only`;
- `real_source_acquisition_performed=false`;
- `real_scan_performed=false`;
- `production_import_attempted=false`;
- `ladybugdb_written=false`;
- boundary mentions `No production KG import`.

Run:

```bash
uv run python -m arxiv_archive validation-batch init --batch-id fixture-b001 --json
```

Expected:

- exit code 1;
- `status=not_implemented`;
- no source acquisition or scan claims.

## Regression

Existing root command remains valid:

```bash
uv run python -m arxiv_archive --date YYYY-MM-DD --json
```

## Not implemented yet

Real batch init, source preflight, acquisition, scan, review, and resume are intentionally deferred to later slices.
