# M025 S09 Baseline Recovery Report

## Decision

- Baseline recovery completed: **true**
- Decision: **ready**
- Blockers: None
- Graph readiness claim: **false**

This report discloses that the baseline was regenerated from local current-pipeline artifacts only. It is not a recovered historical production baseline and carries no graph-readiness claim.

## Baseline Provenance

- Baseline path: `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline`
- Provenance counts: `{"regenerated_local_baseline": 5}`

| Article | Provenance | Chunks | Diagnostics | Compatible |
|---|---|---:|---:|---|
| arxiv/cs-ai/2512.24601 | regenerated_local_baseline | 5 | 2 | yes |
| arxiv/cs-ai/2605.28617v1 | regenerated_local_baseline | 5 | 2 | yes |
| arxiv/cs-cl/2507.19457 | regenerated_local_baseline | 5 | 2 | yes |
| arxiv/cs-cv/2605.26525v1 | regenerated_local_baseline | 5 | 2 | yes |
| company_blog/cs-ir/pageindex_zhang2025pageindex | regenerated_local_baseline | 5 | 2 | yes |

## Diagnostics

`{"BASELINE_REGENERATED_LOCAL_ONLY": 5, "S06_ROADMAP_HANDOFF_RECONSTRUCTED": 5}`

## No-Network Proof

`{"all_events_no_network": true, "network_fetch_attempted": false, "required": true}`

## No-Write Safety Evidence

`{"graph_import_allowed": false, "ladybugdb_written": false, "production_import_attempted": false, "require_no_import_flags": true, "safety_violations": []}`

## Failure Modes

- Filesystem inputs: missing or malformed catalog, index, selection, chunking, or evidence JSON raises `BaselineRecoveryError` and exits non-zero rather than fetching or synthesizing data.
- Network dependency: intentionally disabled by `--require-no-network`/`--no-network`; URL-like paths are rejected and missing local artifacts fail closed.
- Production graph writes/imports: guarded by required false safety flags; `--require-no-import-flags` verifies graph/import/write flags remain false in generated baseline artifacts.
- Subprocess dependency: the verifier is invoked through `uv run python`; interpreter or dependency failures bubble as command failures.

## Load Profile

Expected load is the fixed five-article smoke corpus. At 10x, local filesystem JSON reads/writes and artifact enumeration saturate first; no network, database, graph writer, or connection pool is involved. Protection is bounded per-article JSON processing and fail-closed validation of every local artifact.

## Negative Tests

- `tests/test_article_baseline_recovery_replay.py::test_baseline_recovery_requires_no_network_execution` covers missing no-network enforcement.
- `tests/test_article_baseline_recovery_replay.py::test_baseline_recovery_rejects_missing_local_chunking_without_fetching` covers missing local chunking and verifies no fetch fallback.
- `tests/test_article_baseline_recovery_replay.py::test_baseline_recovery_requires_local_paths` covers URL-like path rejection.
- `tests/test_article_baseline_recovery_replay.py::test_validation_helper_rejects_baseline_missing_final_summary` covers the downstream validation blocker.
- `tests/test_article_baseline_recovery_replay.py::test_validation_helper_rejects_unsafe_baseline_flags` covers malformed graph/import/write safety flags.
