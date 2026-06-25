# M169 Fetcher Write Resolution

## Verdict

**Final unknown write path is resolved.**

S07 routes `PDFDownloader.download(...)` cache writes through atomic sibling temporary replacement. The write-path inventory now reports no `unknown` category.

## Impact analysis

GitNexus impact for the current indexed method:

```text
Method:src/research_graph/corpus/ingestion/fetchers.py:PDFDownloader.download#2
risk=LOW
impactedCount=0
affected_processes=0
```

The first lookup by `PDFDownloader.download` failed because GitNexus still indexes the moved package path; the UID-disambiguated method returned LOW risk.

## Changes

### `src/research_graph/infrastructure/corpus/ingestion/fetchers.py`

- Added fetcher-local `_atomic_write_bytes(path, content)` helper using stdlib `tempfile.NamedTemporaryFile(...)` in the destination directory and `Path.replace(...)`.
- Routed the validated PDF payload through `_atomic_write_bytes(pdf_path, response.content)`.
- Preserved existing cache hit, content-type/signature validation, and explicit failure behavior.

### `tests/test_pdf_downloader.py`

- Added `test_download_writes_pdf_with_atomic_replacement(...)`.
- The test mocks HTTP, records `Path.replace`, verifies the final target is replaced, and verifies bytes are preserved.

## Verification

| Check | Result | Evidence |
|---|---|---|
| Focused downloader tests | PASS: 5 passed | `gsd_exec[083a0e8b-2e6a-4a66-b176-3bfaa7c6b68e]` |
| Write-path inventory | PASS: no `unknown` category | `gsd_exec[327e3608-4582-4982-98ef-0c063d0f7270]` |
| Scoped ruff | PASS | `gsd_exec[e65323b1-cf22-4501-9a4d-f00ac29fa31e]` |

## Inventory before and after

Before S07:

```text
unknown=1
remaining unknown:
  src/research_graph/infrastructure/corpus/ingestion/fetchers.py L44 pdf_path
```

After S07:

```text
unknown=0
categories:
  append-log=7
  caller-owned=38
  database=1
  run-scoped=25
  script-only=263
  shared-state=4
  temporary=1
```

## Residual risk

The downloader still uses a stable cache path keyed by arXiv id. Atomic replacement prevents partial final PDF files but does not add lock-based coordination for two simultaneous downloads of the same id. That broader multi-writer cache policy remains out of scope unless future concurrency evidence requires it.
