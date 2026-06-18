# M081 Artifact Metrics Package Move Summary

## Canonical path

Article artifact metrics now live at:

```text
arxiv_archive.artifacts.metrics
src/arxiv_archive/artifacts/metrics.py
```

## Compatibility shim

The old import path remains available:

```text
arxiv_archive.article_artifact_metrics
src/arxiv_archive/article_artifact_metrics.py
```

The old module explicitly re-exports public constants/classes/functions from `arxiv_archive.artifacts.metrics`.

## Repo import updates

Updated repo-internal imports to prefer the canonical path in:

- `tests/test_article_artifact_metrics.py`
- `scripts/verify_m023_artifact_scaffold_gate.py`

Added a compatibility test proving the legacy module re-exports canonical objects.

## Behavior note

During targeted verification, existing fixture drift surfaced: older redacted M023 fixture safety flags omitted newer false-valued keys such as `graph_import_allowed`, `graphdb_written`, and `import_eligible`. The metrics implementation now treats missing false-valued safety flags as the safe default while still counting explicit unsafe values and missing true-valued defaults as unsafe.

This is a compatibility fix for historical redacted fixtures; it does not enable live API calls, graph writes, fact promotion, or secret handling.

## GitNexus blast radius

Before editing public metrics logic, GitNexus impact checks were run:

- `calculate_article_artifact_metrics`: LOW risk; direct caller is internal `calculate_benchmark_metrics`.
- `build_article_artifact_benchmark_report`: LOW risk; direct caller is internal `write_article_artifact_benchmark_report`.
- `write_article_artifact_benchmark_report`: LOW risk; no upstream callers.
- `_unsafe_flags` and `count_unsafe_authorizations`: LOW risk; internal metric call chain only.

## Verification

Fresh verification:

```bash
uv run pytest tests/test_article_artifact_metrics.py tests/test_m023_artifact_scaffold_gate.py -q
```

Result: **PASS** — 15 passed.

```bash
python3 -m py_compile src/arxiv_archive/artifacts/metrics.py src/arxiv_archive/article_artifact_metrics.py scripts/verify_m023_artifact_scaffold_gate.py
```

Result: **PASS**.

## Boundaries

- no live API calls
- no secrets collected or printed
- no graph writes
- no fact promotion
- no broad package restructure
- no shim removal
