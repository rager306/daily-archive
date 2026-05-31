# M025 S08 Final Preprocessing Replay Report

## Decision

- Larger preprocessing validation ready: **false**
- Decision: **blocked**
- Blockers: baseline_missing
- Graph readiness claim: **false**

M025 makes no graph readiness claim. This report only evaluates whether the refactored preprocessing replay over the fixed five-article local smoke corpus is ready for larger preprocessing validation.

## Baseline Comparison

- Baseline path: `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/baseline`
- Final replay path: `data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay`
- Behavior counts: `{"blocked": 5}`
- Baseline comparison counts: `{"baseline_missing": 5}`

| Article | Baseline | Behavior | Ready | Diagnostics |
|---|---:|---|---|---:|
| arxiv/cs-ai/2512.24601 | baseline_missing | blocked | no | 2 |
| arxiv/cs-ai/2605.28617v1 | baseline_missing | blocked | no | 2 |
| arxiv/cs-cl/2507.19457 | baseline_missing | blocked | no | 2 |
| arxiv/cs-cv/2605.26525v1 | baseline_missing | blocked | no | 2 |
| company_blog/cs-ir/pageindex_zhang2025pageindex | baseline_missing | blocked | no | 2 |

## Diagnostics

`{"BASELINE_ARTIFACT_MISSING": 5, "S06_ROADMAP_HANDOFF_RECONSTRUCTED": 5}`

## Readiness Blockers

- baseline_missing

## No-Network Proof

`{"all_events_no_network": true, "network_fetch_attempted": false, "required": true}`

## No-Write Safety Evidence

`{"graph_import_allowed": false, "ladybugdb_written": false, "production_import_attempted": false, "require_no_import_flags": true, "safety_violations": []}`

## Failure Modes

- Filesystem inputs: missing or malformed catalog, index, selection, chunking, or evidence JSON raises `FinalReplayError` and exits non-zero rather than fetching or synthesizing data.
- Network dependency: intentionally disabled by `--require-no-network`/`--no-network`; any missing local artifact fails closed.
- Production graph writes/imports: guarded by required false safety flags; `--require-no-import-flags` verifies graph/import/write flags remain false in final artifacts.
- Subprocess dependency: the verifier is invoked through `uv run python`; interpreter or dependency failures bubble as command failures.

## Load Profile

Expected load is the fixed five-article smoke corpus. At 10x, filesystem JSON reads/writes and artifact enumeration saturate first; no network, database, or graph writer pool is involved. Protection is fail-closed local artifact validation and bounded per-article JSON artifacts, with larger-corpus validation blocked until a baseline exists.

## Negative Tests

- `tests/test_article_preprocessing_replay_contract.py::test_final_replay_requires_no_network_execution` covers missing no-network enforcement.
- `tests/test_article_preprocessing_replay_contract.py::test_final_replay_rejects_missing_local_evidence_instead_of_fetching` covers missing local evidence and verifies no fetch fallback.
- `tests/test_article_preprocessing_replay_contract.py::test_final_replay_writes_contract_compliant_per_article_artifact` covers baseline-missing blocked readiness and false safety flags.
