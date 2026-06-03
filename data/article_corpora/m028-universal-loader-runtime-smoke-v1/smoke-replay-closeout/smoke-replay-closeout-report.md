# M028 Smoke Replay Closeout

- Milestone: `M028-8hwqjk`
- Slice: `S06`
- Status: `pass`
- Corpus: `data/article_corpora/m028-universal-loader-runtime-smoke-v1`
- Replay artifacts: `data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts`
- Git commit: `5a8bdb6c446020c11d8d1e5e4b561233517ad08d`

## Metadata-only Boundary

This closeout replays only local metadata/provenance stages S02-S05. It does not perform network fetches, live acquisition, parser or chunker calls, graph imports, LadybugDB writes, model calls, crawler calls, or production writes.

## Source Acquisition Preflight

- URL refs: 21
- Normalized identities: 20
- Terminal captured events: 21
- Expansion refs: R15, R16, R17, R18, R19, R20, R21
- Duplicate identity: `arxiv:2605.20897` (2 refs)

## Stage Replay

| Stage | Status | Exit Code | Duration ms |
|---|---:|---:|---:|
| S02_build_source_metadata_adapters | pass | 0 | 88 |
| S02_verify_source_metadata_adapters | pass | 0 | 47 |
| S03_build_pdf_acquisition_diagnostics | pass | 0 | 70 |
| S03_verify_pdf_acquisition_diagnostics | pass | 0 | 67 |
| S04_build_universal_loader_evidence_bundles | pass | 0 | 64 |
| S04_verify_universal_loader_evidence_bundles | pass | 0 | 64 |
| S05_build_hermes_digest_projection | pass | 0 | 64 |
| S05_verify_hermes_digest_projection | pass | 0 | 63 |

## Safety Flags

All closeout safety flags remain fail-closed: graph/import/write/model/crawler/parser/chunker behavior is false and unsafe counters are zero.

## Failure Modes

- Filesystem inputs: missing or malformed JSON/JSONL fails before replay with stable diagnostics (`INPUT_MISSING`, `JSON_MALFORMED`, `JSONL_MALFORMED`).
- Local artifact provenance: unsafe, missing, byte-count-mismatched, or SHA-256-mismatched artifact paths fail preflight.
- Subprocess stages: timeout or nonzero exit records the failed stage, command, cwd, exit code, bounded output excerpts, and stops subsequent replay.
- Network/API dependencies: live fetch, crawler, model, graph, LadybugDB, and production-write paths are intentionally not invoked; any unsafe flag/counter in inputs or verifier output fails closed.

## Load Profile

Expected load is exactly 21 URL refs and 20 normalized identities. At 10x, local filesystem hashing/subprocess replay would saturate first; this runner protects the boundary with exact-count checks rather than expanding into batch ingestion.

## Negative Tests

- `tests/test_m028_smoke_replay_closeout.py::test_verifier_rejects_absolute_or_escaping_artifact_path` covers unsafe repo-relative artifact paths.
- `tests/test_m028_smoke_replay_closeout.py::test_verifier_rejects_nonzero_unsafe_counter_and_flag` covers unsafe graph/write flags and counters.
- `tests/test_m028_smoke_replay_closeout.py::test_verifier_rejects_payload_bearing_key` and `tests/test_m028_smoke_replay_closeout.py::test_verifier_rejects_raw_payload_marker` cover payload-bearing keys and raw payload markers.

## Diagnostics

- None.
