# M082 Artifact Reducer Package Move Summary

## Canonical path

Article artifact reducer now lives at:

```text
arxiv_archive.artifacts.reducer
src/arxiv_archive/artifacts/reducer.py
```

## Compatibility shim

The old import path remains available:

```text
arxiv_archive.article_artifact_reducer
src/arxiv_archive/article_artifact_reducer.py
```

The old module explicitly re-exports:

- `REDUCER_SCHEMA_VERSION`
- `DEFAULT_VALIDATION_BUCKETS`
- `_safety_defaults`
- `merge_article_artifact_results`
- `aggregate_article_artifact_log`

`_safety_defaults` is preserved even though it is private because existing workflow/test code uses it.

## Repo import updates

Updated repo-internal imports to prefer the canonical path in:

- `src/arxiv_archive/rlm_workflow.py`
- `tests/test_m050_article_artifact_reducer.py`
- `tests/test_m050_e2e_pipeline.py`

Added a compatibility test proving the legacy module re-exports canonical reducer objects.

## GitNexus blast radius

Before moving reducer code, GitNexus impact checks were run:

- `merge_article_artifact_results`: LOW risk; direct callers include `aggregate_article_artifact_log` and affected process `run_document_workflow`.
- `aggregate_article_artifact_log`: LOW risk; no upstream callers reported.
- `_safety_defaults`: ambiguous symbol name globally, so direct import inventory was used to preserve the reducer helper in the shim.

## Verification

Fresh targeted tests:

```bash
uv run pytest tests/test_m050_article_artifact_reducer.py tests/test_m050_e2e_pipeline.py -q
```

Result: **PASS** — 20 passed.

Fresh compile check:

```bash
python3 -m py_compile src/arxiv_archive/artifacts/reducer.py src/arxiv_archive/article_artifact_reducer.py src/arxiv_archive/rlm_workflow.py
```

Result: **PASS**.

## Boundaries

- no live API calls
- no secrets collected or printed
- no graph writes
- no fact promotion
- no worker/model/minimax module moves
- no broad package restructure
- no shim removal

## Next candidate

A future milestone can move another low-risk artifact module, but should repeat the same pattern: GitNexus impact, copy implementation to canonical package, explicit shim, canonical repo imports, compatibility test, targeted tests, and GitNexus detect_changes.
