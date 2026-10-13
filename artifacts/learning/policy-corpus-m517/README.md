# M517 paired-policy corpus experiment

Status: all 320 fresh PDFs were captured under the repaired runner. Training cycles 60, 120 and 240 used the same fixed 80 validation PDFs. No validation execution changed between cycles. An overall parse-quality gain is not established: mean token F1 rose from 0.799 to 0.871, but mean local ordered-5gram F1 fell from 0.713 to 0.688 with 31 regressions. No metric or table was adopted.

The invalid 132-document preflight remains under `preflight-captures/` and is excluded. Native PyMuPDF diagnostics had corrupted its metadata channel. The repaired runner reads a separate metadata file. Historical journals, prior manifests and protocol failures remain retained.

## Frozen inputs

- 320 unique PDFs not present in the earlier 150-document paired corpus.
- PDF identity is SHA-256. Duplicate paths are not duplicate documents.
- 240 training documents and 80 fixed validation documents. Training prefixes are 60, 120 and 240 documents.
- Sources are selected by sorted PDF SHA-256 after a 6,000,000-byte size cap and one-PDF-per-source-family filter. This excludes large books and is not a representative random sample of the full archive. The original manifest contained two textbook supplements with the same family ID; it is retained as `preflight-manifest.json`. The corrected manifest has 320 distinct PDF hashes and 320 distinct source-family IDs and was frozen before any cycle.
- Validation is an adaptive diagnostic set, not an untouched final test.
- `manifest.json`, `experiment.yaml`, `registry.yaml`, `parse_pair_metric.yaml` and provenance files bind the recipe. Earlier journals are preserved under `historical/`.

## What is measured

TOC export, Rust TOC assembly and direct Java GROBID structural parsing write separate raw outputs in new directories. Source, exporter DATA, references and outputs are hashed. Only typed empty TOC is NotApplicable; other errors stay failures. Missing measured features or independent references are refused, not replaced with zeros.

Every metric trains its own candidate table. Minimum support is four arm observations, usually two complete paired documents. The report also records distinct-document support. No metric is selected for production. The legacy sections/ms integer rule is unchanged; raw sections, fractional sections/ms, body characters, characters/ms, token-overlap F1, ordered 5-token-gram F1 and quality-then-latency are separate predeclared alternatives. The quality tie margin is 0.02.

Candidate and native-only decisions are consulted on identical held-out documents. Uncovered cells use native DATA. The baseline is NativeFallbackPolicy under the explicit captured-arm execution map, not literal production `parse-pdf` routing. Forced, tied and failed cases are separate from decisive full-pair winner agreement.

## Interpretation limits

The candidate text is concatenated `sections.text`; the `pdftotext` reference is the full PDF. Separate citations, title, abstract, tables and furniture affect coverage. Token/5-gram scores are independent PDF-text diagnostics, not body-only fidelity, publisher ground truth, bibliography correctness or KG readiness. A multiset of locally ordered 5-token grams does not establish global reading order.

Latency includes per-arm setup, release probes, export/parse and output writes. It excludes shared reference extraction and classification. Capture uses at most two processes on a shared host, with Rust checks and GitNexus indexing also running. One wall-clock sample cannot establish controlled production speedup or batch capacity.

All artifacts remain candidate-only: `deployed=false`, `import_eligible=false`, `auto_promote=forbidden`, `graph_write=forbidden`. Production routing and parser defaults are unchanged. The unsafe product `policy pair`/`gold-pair` actions are retired and refuse before parsing.

## Verification

Application library: 565 passed. CLI library: 421 passed. Isolated capture: 5 passed, including the separate metadata-file regression. Paired evaluator: 10 passed, including genuine TOC schema loading. Opt-in integration: 5 passed. Retired actions: 1 passed. Current `da-parse --tests`: 1,922 passed. Full architecture passed all 23 steps with 0 timed out and exit 0. The actual log and receipt are `verification/architecture-final.log` and `verification/architecture-final-receipt.json`. Measured capture/evaluator runner identities remain unchanged.

Fresh artifact UAT `0aba1b91-7596-4fea-a54a-c3753d2df8f6` independently verified all 320 source/capture/reference/arm hashes, three-cycle arithmetic, current-source seals, historical hashes and production sentinels. It exited 0 without timeout, abort or signal. Offline replay of cycle 240 reproduces every report field after normalizing only table output paths. No new parse or timing sample was created.

Final fmt/check exited 0 without changing Rust sources. Artifact UAT `6d0465d8-4faa-4153-a12f-d3b71c1dd866` passed independent arithmetic/hashes and actual full-architecture/current-source/review/replay/process/operational assertions in 20,765 ms, without timeout, abort or signal. Final source-bound GSD validation and execution disposition remain separate from these measurements. No canonical Task Attempt was executed. The three Q6 tenfold-capacity findings remain flagged. Full results and regressions: `doc/review/policy-corpus-cycles.md` and `.json`. Raw payloads use binary Git diffs to avoid the diff buffer limit; their exact bytes remain retained and hash-verified.

The earlier 61.1% gain claim is withdrawn. It was arm-label agreement on duplicated/mixed-feature evidence, not selected-policy quality. ADR-080 records the correction.
