# M168 Catalog Write Result

## Verdict

**Backlog item 1 status: CLOSED for canonical catalog JSON atomicity scope.**

M168 hardens canonical catalog `article.json` and `article_catalog/index.json` writes with atomic sibling-temp replacement. This prevents partial or truncated JSON files from replacing the last good file when a write fails before final replacement.

## Implementation

Changed `src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py`:

- added `_atomic_write_text(path, text, encoding="utf-8")`;
- writes to a hidden temp file in the target directory;
- flushes and fsyncs the temp file;
- uses `Path.replace()` for final atomic replacement;
- removes the temp file on failure before replacement;
- routes `write_article_record(...)` through `_atomic_write_text(...)`;
- routes `update_index_if_exists(...)` through `_atomic_write_text(...)`.

## Tests added

`tests/test_catalog_ingest.py`:

- `_atomic_write_text` preserves existing file when replace fails;
- `write_article_record()` delegates through the atomic helper;
- `update_index_if_exists()` delegates through the atomic helper.

`tests/test_catalog_ingest_filesystem_adapter.py`:

- `FilesystemCatalogRepository.write_article_record()` delegates to the catalog helper.

## Contract boundary

This closes partial/truncated JSON replacement risk for shared canonical catalog JSON files.

It does **not** claim full multi-writer index merge serialization. The index update remains an explicit process-boundary single-writer contract for merge semantics.

## Residual paths

- PDF copy remains direct `shutil.copy2(...)` plus SHA256 verification.
- M056 summary JSON remains run-scoped and direct-write.
- JSONL event output remains append-log.

## Verification

```text
uv run pytest tests/test_catalog_ingest.py tests/test_catalog_ingest_filesystem_adapter.py -q
35 passed

uv run ruff check src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py tests/test_catalog_ingest.py tests/test_catalog_ingest_filesystem_adapter.py
All checks passed

uv run python scripts/verify_onion_layering.py --json
violation_count=0
allowed_violation_count=0

uv run pyrefly check
0 errors
```
