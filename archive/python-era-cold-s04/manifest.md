# Archive Manifest — Python ArXiv Downloader (S04, M386-57ae9l)

## Archived file

`download_arxiv_to_catalog.py` — former product-runtime Python downloader for arXiv articles.

## Archive reason

D237 (Rust-only production pipeline): product code must not invoke Python fallback subprocesses.
The Rust command `da acquire-arxiv` (crates/da-cli/src/acquire_arxiv.rs →
da-adapters/src/arxiv_acquisition.rs) is the sole product acquisition path. It
provides full parity with the Python script:

| Python feature | Rust parity |
|----------------|-------------|
| Single/batch ID acquisition | `--ids` (comma-list) |
| Network override (no-network fail-closed) | `--allow-network` flag (off by default) |
| Catalog root | `--catalog-root` |
| Durable report | `--report` JSON |
| Checksum-bound SHA-256 | `source_variants[].sha256` |
| Atomic write (.partial → rename) | `write_article_json` atomic path |
| Bounded retry/backoff | `DA_HTTP_RETRY_BACKOFF_SECONDS` (DATA-driven, M386 S02) |
| Atom meta + category detection | `fetch_atom_meta` + `extract_category` |
| PDF + HTML fetch (first-valid) | `http_get_first_valid` with validation |
| TeX source sidecar (optional) | `--include-tex-source` (capped, validated) |

## Tests

`test_download_arxiv_to_catalog.py` is preserved here as **frozen evidence** of
the Python-era contract. It is not run on the CI path; the active contract
suite is `crates/da-adapters/tests/arxiv_acquisition_contract.rs` (wiremock
matrix: happy, 503 retry, partial fail, network denied).

## Do NOT restore

Do not move this file back to `scripts/`. The Rust command is the single
source of truth for product acquisition. To re-introduce Python acquisition
would violate D237 and the Governor `rust_only_product_pipeline` rule.

## D127

`import_eligible=false` — archiving the downloader does not authorize import;
graph materialization always runs through Rust extract + Samyama.
