# M148 M028 Batch Script Wrapper Ratchet

Schema: `daily-archive-m148-m028-batch-ratchet.v1`

Promoted:
- `tests/test_m028_hermes_digest_projection.py`
- `tests/test_m028_pdf_acquisition_diagnostics.py`
- `tests/test_m028_requirement_scope_reconciliation.py`
- `tests/test_m028_source_metadata_adapters.py`
- `tests/test_m028_universal_loader_evidence_bundles.py`

Classification: `strict_script_wrapper`

## Count delta

| Bucket | Before | After | Delta |
|---|---:|---:|---:|
| `dynamic_script_import` | 35 | 30 | -5 |
| `legacy_mixed` | 49 | 44 | -5 |
| `strict_script_wrapper` | 23 | 28 | +5 |
| `strict_infrastructure` | 6 | 6 | +0 |

## Rationale

Five M028 cohort tests were baseline-green and now import their scripts under test through normal repo-root `scripts` imports instead of dynamic importlib loading.

## Compatibility notes

- tests/test_m028_hermes_digest_projection.py preserves sys.modules alias for build_m028_hermes_digest_projection because the verifier imports the build script by bare module name.
- tests/test_m028_universal_loader_evidence_bundles.py preserves sys.modules alias for build_m028_universal_loader_evidence_bundles because the verifier imports the build script by bare module name.

## Verification

- Focused pytest: `120 passed`.
- Ruff: passed.
- Pyrefly: `0 errors`.
