# M167 Test Allowlist Reduction Result

## Verdict

**Item 3 status: CLOSED for safe reduction scope.**

M167 reduced `legacy_mixed` from 18 to 3 by adding a `strict_workflows` guard bucket and moving 15 workflow-only tests out of the legacy allowlist. The 3 dynamic script import files remain allowlisted because they are true dynamic loader debt and need separate per-file refactors.

## Before

```text
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=18
strict_workflows=0
violations=0
```

## After

```text
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=3
strict_workflows=15
violations=0
```

## Guard change

`strict_workflows` entries must:

- import `research_graph.workflows`,
- not use normal `scripts.*` imports,
- not use dynamic script import,
- not invoke scripts through subprocess as the architecture seam.

This makes workflow tests explicit rather than hiding them in `legacy_mixed`.

## Files moved to `strict_workflows`

- `tests/test_import_boundary_rehearsal.py`
- `tests/test_m052_rlm_workflow.py`
- `tests/test_universal_kb_queue.py`
- `tests/test_universal_kb_review_assistance.py`
- `tests/test_universal_kb_sidecar_boundary.py`
- `tests/test_universal_kb_smoke_cli.py`
- `tests/test_validation_batch_cli_article_report.py`
- `tests/test_validation_batch_cli_freshness.py`
- `tests/test_validation_batch_provenance.py`
- `tests/test_validation_batch_quota_fill.py`
- `tests/test_validation_batch_scan_workflow.py`
- `tests/test_validation_batch_state.py`
- `tests/test_validation_batch_top_up.py`
- `tests/test_validation_batch_workflow.py`
- `tests/test_validation_logging.py`

## Remaining dynamic blockers

- `tests/test_m060d_s01.py`
- `tests/test_m061_s03.py`
- `tests/test_m062_s03.py`

These remain in both `dynamic_script_import` and `legacy_mixed` allowlists.

## Verification

```text
uv run python scripts/verify_test_architecture.py --json
status=passed
violations=0
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=3
strict_workflows=15

uv run pytest tests/test_test_architecture_guardrail.py -q
6 passed
```
