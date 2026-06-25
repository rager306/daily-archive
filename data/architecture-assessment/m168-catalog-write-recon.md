# M168 Catalog Write Recon

## Scope

This recon covers canonical catalog writes used by the migrated catalog ingest path:

- `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py`
- `src/research_graph/infrastructure/corpus/ingestion/catalog_adapters.py`
- focused tests in `tests/test_catalog_ingest.py` and `tests/test_catalog_ingest_filesystem_adapter.py`

## Current write paths

| Path | Function or method | Write type | Current behavior | Safety classification |
|---|---|---|---|---|
| `catalog_ingest.py` | `write_article_record(article_path, article)` | `article.json` shared catalog JSON | `mkdir(parents=True)` then direct `Path.write_text(...)` | shared-state, not atomic |
| `catalog_ingest.py` | `update_index_if_exists(catalog_root)` | `article_catalog/index.json` shared catalog index JSON | reads existing index, rebuilds in memory, then direct `index_path.write_text(...)` | shared-state, not atomic |
| `catalog_ingest.py` | `ingest_catalog(...)` | source PDF copy + optional missing article record creation | copies local PDF into canonical arxiv tree; creates article.json if missing | source-copy with checksum; article JSON uses non-atomic helper |
| `catalog_adapters.py` | `FilesystemCatalogRepository.store_pdf_asset(...)` | source PDF copy | `shutil.copy2(...)`, then SHA256 verification | source-copy with checksum; not this slice's JSON atomicity target |
| `catalog_adapters.py` | `FilesystemCatalogRepository.write_article_record(...)` | `article.json` shared catalog JSON | delegates to `write_article_record(...)` | shared-state, inherits helper behavior |
| `catalog_adapters.py` | `FilesystemCatalogRepository.update_index()` | index JSON shared catalog index | delegates to `update_index_if_exists(...)` | shared-state, inherits helper behavior |
| `catalog_adapters.py` | `write_m056_ingest_events(...)` | JSONL append diagnostics | append event log | append-log; outside catalog atomic JSON target |
| `catalog_adapters.py` | `write_m056_ingest_summary(...)` | summary JSON | direct write to caller-provided summary path | run-scoped artifact; outside shared catalog target |

## Failure modes

### Article record JSON

Current direct `Path.write_text(...)` can leave a truncated or partially-written `article.json` if the process is interrupted mid-write or the filesystem errors after truncation. Parent directory creation is safe enough, but final content replacement is not atomic.

### Index JSON

Current direct `index_path.write_text(...)` has the same partial-write risk for `article_catalog/index.json`. Because index rebuild starts by reading the current index, concurrent writers are still not fully serialized by atomic replacement. Atomic replacement prevents partial/truncated JSON but does not by itself provide multi-writer merge semantics.

### PDF copy

`shutil.copy2(...)` writes directly to the destination PDF then verifies SHA256. This catches checksum mismatch after copy, but a crash during copy can leave partial PDF data. M168 will not broaden PDF copy migration unless tests show a minimal safe helper can be reused without scope creep; JSON writes are the highest leverage P1 target.

## Selected minimal contract for S03/S04

M168 will implement a small stdlib-only atomic text write helper for canonical JSON writes:

1. Write UTF-8 text to a temporary file in the destination directory.
2. `flush()` and `os.fsync()` the temporary file.
3. Atomically replace the destination with `Path.replace()` / `os.replace` semantics.
4. Best-effort clean up the temporary file on failure.

This contract will be applied to:

- `write_article_record(...)`
- `update_index_if_exists(...)`

The contract deliberately does **not** claim full multi-writer serialization. The documented concurrency contract is:

- single-writer at the catalog ingest process boundary remains expected for index merge semantics;
- atomic replacement prevents partial/truncated JSON files if the process crashes or fails during write.

## S03 test target

Add focused tests around the helper and delegated catalog functions:

- article record writes leave the previous file intact when temp write fails before replace;
- index update writes through the same atomic helper path or a monkeypatched helper proves the path is used;
- filesystem adapter article writes inherit the atomic helper behavior through delegation.

## Residual paths not changed in S04 unless very cheap

- PDF copy atomicity: currently checksum-verified but direct-copy; leave as future work unless helper reuse is trivial and tests remain small.
- M056 summary JSON: run-scoped artifact; lower priority than canonical shared article/index JSON.
- JSONL event append: append-log category, not an atomic replacement target.

## Verification plan

- `uv run pytest tests/test_catalog_ingest.py tests/test_catalog_ingest_filesystem_adapter.py -q`
- scoped ruff for touched catalog files/tests
- onion guard after implementation
- final inventory rerun in S11
