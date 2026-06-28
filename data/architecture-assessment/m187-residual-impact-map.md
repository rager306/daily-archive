# M187 Residual Impact Map

## Verdict

**All four residual impact checks are exact. M059 is the only MEDIUM-risk target.**

## Impact table

| Residual | Symbol | Risk | Direct callers | Test surfaces | Batch |
|---|---|---:|---|---|---|
| `m055-five-pdf` | `Function:scripts/benchmark_m055_corpus_manifest.py:build_corpus_manifest` | LOW | `main`, `test_corpus_manifest_5_pdfs`, `test_corpus_manifest_idempotent`, `test_corpus_manifest_safety_defaults` | `tests/test_m055_benchmark_s01.py -k corpus_manifest` | S02 |
| `m055deep-20-pdf` | `Function:scripts/build_m055deep_corpus_manifest_20.py:write_manifest` | LOW | `main`, `test_corpus_manifest_idempotent` | `tests/test_m055deep_corpus_20.py` | S02 |
| `m058-graph-manifest` | `Function:scripts/m058_build_graph_manifest.py:write_json` | LOW | `build_graph_manifest` | `tests/test_m058_s05.py::test_graph_manifest_combined` | S03 |
| `m059-batch-manifest` | `Function:scripts/m059_build_manifest.py:finalize_manifest` | MEDIUM | `build_m054`, `build_m055`, `build_m055deep`, `build_m056`, `build_m057`, `build_m058` | `tests/test_m059_s01.py` | S03 |

## Batch assignment rationale

S02 handles the M055 family first because both targets are LOW risk and already have focused manifest idempotency/safety tests. This batch should prove the mechanics of using the S09 atomic writer under transition-ratchet.

S03 handles M058 and M059. M058 is LOW risk, but M059 is MEDIUM risk with six direct builder callers, so it should remain in a separate batch after S02 proves the transition mechanics.

## Required handling for M059 MEDIUM risk

M059 movement must include:

- exact impact re-run immediately before editing `finalize_manifest`,
- review of all six direct builder callers,
- focused `tests/test_m059_s01.py` execution,
- strict drift explanation,
- rollback if any builder behavior changes unexpectedly.

## No source-edit note

This impact map records planning evidence only. No source code is edited in S01.
