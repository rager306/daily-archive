# M027 S04 Current Pipeline Baseline Report

## Decision

- Baseline capture completed: **true**
- Hardening applied: **false**
- Graph readiness claim: **false**
- Trusted fact claim: **false**

This report captures accepted current mixed-source pipeline behavior before S05 hardening. Metadata-only variants are skipped by design; parser-ready converted payloads are replayed through the existing conservative local preprocessing/chunk path and remain retrieval-only/not import-ready.

## Aggregate Summary

- Articles: 6
- Variants: 11
- Parser-ready variants: 6
- Metadata-only variants: 5
- Current pipeline chunks observed: 5
- Zero-chunk parser-ready variants: 1
- Import-ready records: 0
- Import-eligible chunks: 0

## Article Results

| Article | Variant | Parser-ready | Chunks | Import ready | Diagnostic artifact |
|---|---|---:|---:|---:|---|
| arxiv/mixed-source/2605.20897 | 2605.20897:source:arxiv-abs | no | 0 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2605.20897/baseline.json` |
| arxiv/mixed-source/2605.20897 | 2605.20897:source:arxiv-pdf-seed | yes | 1 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2605.20897/baseline.json` |
| arxiv/mixed-source/2605.21401 | 2605.21401:source:arxiv-abs | no | 0 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2605.21401/baseline.json` |
| arxiv/mixed-source/2605.21401 | 2605.21401:source:arxiv-pdf | yes | 1 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2605.21401/baseline.json` |
| nature/mixed-source/s44387-025-00019-5 | s44387-025-00019-5:source:nature-html | yes | 1 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/nature_mixed-source_s44387-025-00019-5/baseline.json` |
| arxiv/mixed-source/2605.25522 | 2605.25522:source:arxiv-abs | no | 0 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2605.25522/baseline.json` |
| arxiv/mixed-source/2605.25522 | 2605.25522:source:arxiv-pdf | yes | 0 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2605.25522/baseline.json` |
| arxiv/mixed-source/2603.04448 | 2603.04448:source:arxiv-abs | no | 0 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2603.04448/baseline.json` |
| arxiv/mixed-source/2603.04448 | 2603.04448:source:arxiv-pdf | yes | 1 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2603.04448/baseline.json` |
| arxiv/mixed-source/2604.18478 | 2604.18478:source:arxiv-abs | no | 0 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2604.18478/baseline.json` |
| arxiv/mixed-source/2604.18478 | 2604.18478:source:arxiv-pdf | yes | 1 | no | `data/architecture-assessment/m190-m027-current-pipeline-replay/arxiv_mixed-source_2604.18478/baseline.json` |

## Diagnostics

`{"converted_payload_hash_verified": 6, "current_pipeline_retrieval_only_chunks": 5, "current_pipeline_zero_chunks": 1, "metadata_only_not_replayed": 5, "no_converted_payload_expected": 5, "s03_converter_refreshed_after_source_verifier": 1, "s03_linkage_verified": 1}`

## Provenance

- Conversion summary: `data/article_corpora/m027-mixed-source-corpus-v1/conversion-quality-summary.json`
- Conversion summary SHA-256: `b51dae1fbd084c3e4ba9bb80a7a10614ba79410d0114247783ce70fddb8c3404`
- Baseline diagnostics: `data/article_corpora/m027-mixed-source-corpus-v1/current-pipeline-baseline-diagnostics.jsonl`
- Per-article baseline directory: `data/architecture-assessment/m190-m027-current-pipeline-replay`

## Failure Modes

- Filesystem inputs: missing/malformed `conversion-quality-summary.json`, stale S03 source-summary linkage, missing converted payloads, unsafe relative paths, and converted payload hash/size mismatches raise `BaselineReplayError` and exit non-zero before readiness claims are written.
- Network dependency: intentionally absent; `--no-network` is required and artifacts carry `network_fetch_attempted=false`.
- Graph/import/write dependencies: intentionally absent; all graph/import/LadybugDB/production safety flags are fail-closed false in summary, diagnostics, and per-article artifacts.
- Subprocess dependency: the command spawns no subprocesses; interpreter/import failures bubble through the CLI exit code.

## Load Profile

Expected load is the real six-article, eleven-variant M027 corpus. At 10x, local filesystem reads/writes and in-memory PageIndex/chunk construction over converted text payloads saturate first. Protection is bounded S03 converted payload input, one-variant-at-a-time replay, redacted per-article JSON artifacts, and no network/database/graph writer pools.

## Negative Tests

- `tests/test_m027_current_pipeline_baseline.py::test_replay_requires_s03_linkage_and_no_network` covers no-network enforcement.
- `tests/test_m027_current_pipeline_baseline.py::test_replay_rejects_converted_payload_hash_mismatch` covers stale/tampered converted payload hashes.
- `tests/test_m027_current_pipeline_baseline.py::test_replay_captures_parser_ready_and_metadata_only_variants` covers parser-ready replay, metadata-only skip, provenance, and no import/write flags.
- `tests/test_m027_current_pipeline_baseline.py::test_replay_records_zero_chunk_current_failure` covers current zero-chunk/failure recording without repair.
- `tests/test_m027_current_pipeline_baseline.py::test_metadata_artifacts_are_redacted` covers metadata redaction of raw text-like keys/snippets.
