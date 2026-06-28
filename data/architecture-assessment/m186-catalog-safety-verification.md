# M186 Catalog Safety Verification

## Verdict

**PASS for catalog safety contract scope.**

## Evidence

| Check | Result | Evidence |
|---|---|---|
| M025 catalog verifier tests | PASS: 9 passed | `gsd_exec[25e770cf-8eaf-4b81-b629-b0ceb117a7ac]` |
| M027 mixed-source catalog full file | FAIL: 2 unrelated baseline failures, 11 passed | `gsd_exec[076e1b1a-600d-4c5c-bcd1-888459b1da2f]` |
| M027 catalog safety subset | PASS: 11 passed, 2 deselected | `gsd_exec[234f9b89-154a-4680-a49c-c9b5440528f8]` |
| Ruff on catalog tests | PASS | `gsd_exec[d07bb8f8-4f22-452a-946f-9f4ef85b8559]` |

## Scoped-out M027 baseline failures

The two full-file failures are catalog data baseline drift, not S04 helper contract behavior:

1. `test_m027_wrapper_emits_local_only_handoff_artifacts`: real corpus has missing `article.json`, duplicate lookup keys, and rebuilt index is not idempotent.
2. `test_m030_requested_ref_intake_closeout_baseline_is_current`: `stanford:cs224n:gradient-notes` marked cataloged but absent from catalog index.

S04 therefore uses the passing M027 subset as cross-flow proof and carries the baseline failures forward for S16 broad-suite debt isolation.

## Movement gate for S05

S05 may attempt M025 helper movement only if it preserves the S04 contract and re-runs:

- `uv run pytest tests/test_m025_article_catalog_verifier.py -q`
- `uv run pytest tests/test_m027_mixed_source_catalog.py -q -k 'not wrapper_emits_local_only_handoff_artifacts and not requested_ref_intake_closeout_baseline_is_current'`
- strict write-path drift
