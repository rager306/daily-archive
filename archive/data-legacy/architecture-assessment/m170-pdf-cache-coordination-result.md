# M170 PDF Cache Coordination Result

## Verdict

**PDF cache coordination closes as policy-only under the S05 atomic-only decision.**

No lock-file or compare-and-swap code was added for PDF cache writes in M170.

## Why no PDF code change is needed now

The current downloader already uses same-directory temporary files plus atomic replacement:

- `src/research_graph/infrastructure/corpus/ingestion/fetchers.py::PDFDownloader.download(...)`
- `src/research_graph/infrastructure/corpus/ingestion/fetchers.py::_atomic_write_bytes(...)`

This prevents partially written final PDFs. Same-key concurrent missing-cache downloads may duplicate network work, but normal arXiv PDF cache semantics converge on the same PDF for the same id.

## Verification

Focused PDF tests:

```text
uv run pytest \
  tests/test_pdf_downloader.py::test_download_writes_pdf_with_atomic_replacement \
  tests/test_pdf_downloader.py::test_download_rejects_non_pdf_response \
  -q
```

Result:

```text
2 passed
```

Evidence: `gsd_exec[b9fac488-8f43-41d8-905a-3c78dae8f43c]`.

## Residual risk

Atomic-only does not prevent duplicate same-key PDF downloads. It only guarantees that successful writes replace the final cache file atomically. If upstream serves different bytes for the same id, a lock would not solve authority; checksum or provenance policy would be needed.

## Future trigger

Add PDF lock/CAS work only if:

1. duplicate same-key PDF downloads become a measured operational problem;
2. cache consumers require stale-overwrite detection;
3. an expected PDF checksum or generation id becomes part of the downloader contract;
4. high-concurrency PDF cache population is activated.
