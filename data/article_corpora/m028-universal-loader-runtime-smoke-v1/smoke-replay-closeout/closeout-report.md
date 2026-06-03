# M028 Smoke Replay Closeout

- Milestone: `M028-8hwqjk`
- Slice: `S06`
- Status: `pass`
- Corpus: `data/article_corpora/m028-universal-loader-runtime-smoke-v1`
- Replay artifacts: `data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout/replay-artifacts`
- Git commit: `d6bb61b36a66872d2c6ff7588d614681aa2baf6a`

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
| S02_build_source_metadata_adapters | pass | 0 | 84 |
| S02_verify_source_metadata_adapters | pass | 0 | 46 |
| S03_build_pdf_acquisition_diagnostics | pass | 0 | 71 |
| S03_verify_pdf_acquisition_diagnostics | pass | 0 | 81 |
| S04_build_universal_loader_evidence_bundles | pass | 0 | 59 |
| S04_verify_universal_loader_evidence_bundles | pass | 0 | 74 |
| S05_build_hermes_digest_projection | pass | 0 | 67 |
| S05_verify_hermes_digest_projection | pass | 0 | 65 |

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

- `tests/test_replay_m028_smoke_closeout.py::test_validate_source_acquisition_rejects_missing_artifact` covers missing repo-relative artifacts.
- `tests/test_replay_m028_smoke_closeout.py::test_validate_source_acquisition_rejects_unsafe_flag` covers unsafe graph/write flags.
- `tests/test_replay_m028_smoke_closeout.py::test_read_jsonl_rejects_malformed_row` covers malformed JSONL rows.

## Diagnostics

- None.
