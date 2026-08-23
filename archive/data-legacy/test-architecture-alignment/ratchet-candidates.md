# M129 Ratchet Candidates

Schema: `daily-archive-test-ratchet-candidates.v1`

## Before counts

| Bucket | Count |
|---|---:|
| `dynamic_script_import` | 56 |
| `legacy_mixed` | 70 |
| `strict_application` | 6 |
| `strict_domain` | 0 |
| `strict_infrastructure` | 5 |
| `strict_script_wrapper` | 2 |

## Selected batch

| Path | Current bucket | Target bucket | Strategy |
|---|---|---|---|
| `tests/test_m044_sidecar_architecture_guardrail.py` | `legacy-mixed` | `script-wrapper` | Replace `importlib.util.spec_from_file_location` script loading with `from scripts import verify_m044_sidecar_architecture_guardrail as guard`. |
| `tests/test_m026_validation_remediation.py` | `legacy-mixed` | `script-wrapper` | Replace dynamic script loading with `from scripts import verify_m026_validation_remediation`. |
| `tests/test_m027_validation_remediation.py` | `legacy-mixed` | `script-wrapper` | Replace dynamic script loading with `from scripts import verify_m027_validation_remediation`. |

## Rationale

These files are low-risk first ratchet candidates because they use dynamic imports only as a loading mechanism for ordinary `scripts/*.py` modules. They do not require subprocess behavior, broad acceptance wiring, or optional parser dependencies. The target classification is `script-wrapper`: these tests continue to validate script-module contracts, but without adding dynamic-loader debt.

## Explicit exclusions

- Subprocess wrapper tests where process invocation is the behavior under test.
- Large acceptance tests that cover many scripts at once.
- Tests involving optional parser dependencies or broad corpus setup.

## After counts

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 56 | 53 | -3 |
| `legacy_mixed` | 70 | 67 | -3 |
| `strict_script_wrapper` | 2 | 5 | +3 |
