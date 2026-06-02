# M027 S05 End-to-End Mixed Replay Report

## Decision

- End-to-end replay completed: **true**
- Validate-only decision: **not_import_ready_validate_only**
- Ready for import: **false**
- Graph readiness claim: **false**
- Trusted fact claim: **false**

This report records a local-only replay through loader, parser, PageIndex, chunking, separated evidence, import-contract, and S04 baseline-comparison boundaries. Metadata-only variants are skipped by design. Outputs are redacted and must not be interpreted as graph/import-ready artifacts.

## Aggregate Summary

- Articles: 6
- Variants: 11
- Parser-ready variants: 6
- Metadata-only variants: 5
- Chunks observed: 5
- Evidence paths observed: 5
- Zero-chunk parser-ready variants: 1
- Import-ready records: 0
- Import-eligible chunks: 0
- Baseline comparison counts: `{"exact_match": 11}`

## Article Results

| Article | Variant | Parser-ready | Chunks | Evidence paths | Baseline comparison | Replay artifact |
|---|---|---:|---:|---:|---|---|
| arxiv/mixed-source/2605.20897 | 2605.20897:source:arxiv-abs | no | 0 | 0 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/arxiv_mixed-source_2605.20897/replay.json` |
| arxiv/mixed-source/2605.20897 | 2605.20897:source:arxiv-pdf-seed | yes | 1 | 1 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/arxiv_mixed-source_2605.20897/replay.json` |
| arxiv/mixed-source/2605.21401 | 2605.21401:source:arxiv-abs | no | 0 | 0 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/arxiv_mixed-source_2605.21401/replay.json` |
| arxiv/mixed-source/2605.21401 | 2605.21401:source:arxiv-pdf | yes | 1 | 1 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/arxiv_mixed-source_2605.21401/replay.json` |
| nature/mixed-source/s44387-025-00019-5 | s44387-025-00019-5:source:nature-html | yes | 1 | 1 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/nature_mixed-source_s44387-025-00019-5/replay.json` |
| arxiv/mixed-source/2605.25522 | 2605.25522:source:arxiv-abs | no | 0 | 0 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/arxiv_mixed-source_2605.25522/replay.json` |
| arxiv/mixed-source/2605.25522 | 2605.25522:source:arxiv-pdf | yes | 0 | 0 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/arxiv_mixed-source_2605.25522/replay.json` |
| arxiv/mixed-source/2603.04448 | 2603.04448:source:arxiv-abs | no | 0 | 0 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/arxiv_mixed-source_2603.04448/replay.json` |
| arxiv/mixed-source/2603.04448 | 2603.04448:source:arxiv-pdf | yes | 1 | 1 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/arxiv_mixed-source_2603.04448/replay.json` |
| arxiv/mixed-source/2604.18478 | 2604.18478:source:arxiv-abs | no | 0 | 0 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/arxiv_mixed-source_2604.18478/replay.json` |
| arxiv/mixed-source/2604.18478 | 2604.18478:source:arxiv-pdf | yes | 1 | 1 | exact_match | `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay/arxiv_mixed-source_2604.18478/replay.json` |

## Diagnostics

`{"converted_payload_hash_verified": 6, "end_to_end_boundaries_completed": 5, "metadata_only_no_converted_payload_expected": 5, "metadata_only_not_parser_ready_skipped": 5, "parser_ready_zero_chunks_preserved": 1, "s03_linkage_verified": 1, "s04_baseline_exact_match": 11, "s04_baseline_summary_loaded": 1}`

## Provenance

- Command: `scripts/replay_m027_end_to_end_mixed_replay.py`
- CWD: `/root/daily-archive`
- Git commit: `d2fc9b2a200f3078125173a24476e3c16bf35a47`
- Conversion summary: `data/article_corpora/m027-mixed-source-corpus-v1/conversion-quality-summary.json`
- S04 baseline summary: `data/article_corpora/m027-mixed-source-corpus-v1/current-pipeline-baseline-summary.json`
- Replay diagnostics: `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay-diagnostics.jsonl`
- Per-article replay directory: `data/article_corpora/m027-mixed-source-corpus-v1/end-to-end-mixed-replay`

## Failure Modes

- Filesystem inputs: missing/malformed S03/S04 JSON, stale S03 source-summary linkage, missing converted payloads, unsafe relative paths, and converted payload hash/size mismatches raise `EndToEndReplayError` and exit non-zero before readiness claims are written.
- Network dependency: intentionally absent; `--no-network` is required and artifacts carry `network_fetch_attempted=false`.
- Graph/import/write dependencies: intentionally absent; all graph/import/LadybugDB/production safety flags are fail-closed false in summary, diagnostics, events, decision, and per-article artifacts.
- Subprocess dependency: intentionally absent; git commit provenance is read from `.git/HEAD` when available and omitted otherwise.

## Load Profile

Expected load is the real six-article, eleven-variant M027 corpus. At 10x, local filesystem reads/writes and in-memory loader/parser/PageIndex/chunk/evidence construction over converted text payloads saturate first. Protection is bounded S03 converted payload input, one-variant-at-a-time replay, redacted per-article JSON artifacts, metadata-only skips, and no network/database/graph writer pools.

## Negative Tests

- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_requires_no_network_and_s03_linkage` covers no-network enforcement and stale S03 source linkage.
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_rejects_converted_payload_hash_mismatch` covers stale/tampered converted payload hashes.
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_captures_boundaries_and_baseline_comparison` covers loader/parser/PageIndex/chunk/evidence metrics, S04 comparison rows, provenance, and fail-closed flags.
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_preserves_parser_ready_zero_chunk_diagnostic` covers parser-ready zero-chunk preservation and diagnostic emission.
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_skips_metadata_only_without_payload` covers metadata-only skip behavior without payload reads.
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_rejects_unsafe_output_dir` covers unsafe output path rejection.
- `tests/test_m027_end_to_end_mixed_replay.py::test_metadata_outputs_are_redacted` covers raw text/HTML/PDF/key leakage protections.
