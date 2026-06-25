# M167 Test Allowlist Recon

## Verdict

**Item 3 is reducible.**

The current legacy-mixed count is inflated by workflow-only tests. There are 18 legacy-mixed entries, but 15 of them only import `research_graph.workflows` and have no dynamic script import. They can be moved into a new strict workflow bucket if the guard learns that workflow tests may import workflows but must not import scripts dynamically or normally.

The remaining 3 files are true dynamic script import debt and should stay allowlisted unless refactored in a dedicated milestone.

## Evidence

- Guard baseline: `.gsd/exec/535ea7fe-3f9a-44df-ac01-4768357a69c1.stdout`
- Entry disposition scan: `.gsd/exec/b78eac0f-ce35-4dbb-aaa0-427a3018169d.stdout`

Current guard counts:

```text
allowlisted_dynamic_script_import=3
allowlisted_legacy_mixed=18
```

## Dynamic entries

Keep these for now:

| Path | Bucket | Signals | Disposition |
|---|---|---|---|
| `tests/test_m060d_s01.py` | legacy-mixed | dynamic_script_import | keep; requires separate dynamic loader refactor |
| `tests/test_m061_s03.py` | legacy-mixed | dynamic_script_import | keep; historical M061 path and known runtime caveats |
| `tests/test_m062_s03.py` | legacy-mixed | dynamic_script_import | keep; requires separate dynamic loader refactor |

## Legacy workflow-only entries

Move these out of `legacy_mixed` into a new `strict_workflows` bucket:

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

These all have the signal `imports_workflows` and no dynamic script import in the current inventory.

## Proposed strict workflow rule

A strict workflow test should:

- import `research_graph.workflows`,
- not use dynamic script import,
- not use normal `scripts.*` imports,
- not invoke scripts through subprocess as its main architecture seam.

It may import domain/application/infrastructure indirectly as part of workflow orchestration tests, because workflows are outer orchestration surfaces.

## Expected reduction

If S07 implements `strict_workflows` and moves the 15 workflow-only entries:

```text
legacy_mixed: 18 -> 3
strict_workflows: 0 -> 15
dynamic_script_import: 3 -> 3
```

This is a safe reduction because it does not rewrite historical dynamic tests and does not hide dynamic script import debt.

## Blockers

The 3 dynamic files should not be forced into strict buckets in M167. They likely require per-file refactors similar to earlier M151-M162 ratchets and may touch historical artifacts or slow runtime assumptions.
