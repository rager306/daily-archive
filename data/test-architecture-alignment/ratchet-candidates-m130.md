# M130 Ratchet Candidates

Schema: `daily-archive-test-ratchet-candidates.v1`

## Before counts

| Bucket | Count |
|---|---:|
| `dynamic_script_import` | 53 |
| `legacy_mixed` | 67 |
| `strict_application` | 6 |
| `strict_domain` | 0 |
| `strict_infrastructure` | 5 |
| `strict_script_wrapper` | 5 |

## Selected batch

| Path | Current bucket | Target bucket | Strategy |
|---|---|---|---|
| `tests/test_r024_218_document_coverage_report.py` | `legacy-mixed` | `script-wrapper` | Replace dynamic loading of `scripts/build_r024_coverage_report.py` with a normal `from scripts import build_r024_coverage_report` import. |

## Rationale

This file is the next low-risk explicit script-path candidate after M129. It validates a script-wrapper artifact contract and uses dynamic import as test module loading boilerplate rather than as the behavior under test.

`tests/test_r024_218_document_coverage_report.py` calls wrapper functions and classes from `scripts/build_r024_coverage_report.py`; normal import preserves that contract directly.

## Rejected after baseline

`tests/test_m061_s02.py` was initially considered, but focused baseline pytest failed before migration: `test_m050_m064_s01_regression` expects a stale git blob SHA for `artifacts/m061-2hop/s01-decision.md`. It is not safe for an import-only ratchet until that fixture expectation is repaired separately.

## Explicit exclusions

- Subprocess wrapper tests where process invocation is the behavior under test.
- Large acceptance tests that span many scripts at once.
- Tests with infrastructure signals and multiple script modules that deserve a separate focused review.
- Tests that fail focused baseline pytest before migration.

## After counts

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 53 | 52 | -1 |
| `legacy_mixed` | 67 | 66 | -1 |
| `strict_script_wrapper` | 5 | 6 | +1 |
