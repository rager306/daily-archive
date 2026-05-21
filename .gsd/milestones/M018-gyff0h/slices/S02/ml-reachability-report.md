# M018 S02 ML reachability report

## Summary

`torch` and `transformers` are not directly imported by project source. They are reachable through the direct runtime dependency `docling`, specifically via the lazy Docling PDF-conversion fallback in `MDConverter._try_docling`.

## Key reachability facts

| Question | Finding |
|---|---|
| Direct `torch` imports in `src/` | 0 |
| Direct `transformers` imports in `src/` | 0 |
| Direct `docling` imports in `src/` | 1 |
| Lazy Docling import site | `src/arxiv_archive/md_converter.py:211` |
| Runtime helper entry | `src/arxiv_archive/thirty_paper_source_scan.py:63` |
| CLI active exposure found | false |
| Helper/script exposure found | true |
| Test exposure found | true |
| Production KG import/write enabled | false |

## Execution path

The relevant path is:

```text
acquire_sources_for_manifest
  -> MDConverter.convert
    -> _try_arxiv2md
    -> _try_marker
      -> _try_docling
        -> from docling.document_converter import DocumentConverter
        -> DocumentConverter().convert(pdf_path)
```

`_try_docling` is reached only when:

1. source acquisition attempts conversion for a missing markdown paper;
2. arxiv2md fails or returns low-quality markdown;
3. PDF fallback is needed;
4. Marker CLI is unavailable;
5. Docling is installed.

## Untrusted input boundary

`PDFDownloader.download` fetches arXiv PDF bytes and validates that the response is a PDF by content type or `%PDF-` magic before writing to cache. Docling then consumes a local PDF path. That still means the ML stack can process externally sourced PDFs if bounded source acquisition is run.

## Exposure classification

| Surface | Classification | Rationale |
|---|---|---|
| Main daily archive CLI | not currently exposed by found references | The CLI references validation-batch operations and normal archive flow; no direct source-acquisition command was found in `cli.py`. |
| Validation-batch CLI | not Docling-executing | `validation_batch_workflow.py` preflights existing sources and scans markdown; it explicitly does not acquire/convert PDFs. |
| Source acquisition helpers | reachable helper/script surface | `thirty_paper_source_scan.py` can instantiate `MDConverter` and therefore reach Docling fallback unless `fast_only=True` or a test converter is injected. |
| Unit tests | reachable mocked/test surface | Tests instantiate `MDConverter` and monkeypatch fallback paths. |
| Production KG import/write | not enabled | Existing safety flags and M017/M018 planning still block production import/write paths. |

## Preliminary risk

`medium if source acquisition is run on untrusted PDFs; otherwise low/dormant`.

The vulnerable packages are not on the main validation-batch scan path, but they are reachable from a helper that can process external arXiv PDFs. S03 should recommend isolating or gating Docling fallback before broad source acquisition, rather than treating the audit as harmless.

## Evidence

Machine-readable evidence:

```text
.gsd/milestones/M018-gyff0h/slices/S02/run-evidence/ml-reachability-map.json
```

## Safety

No code or dependency files were changed. No raw PDF/text/corpus content, secrets, embeddings, vectors, or model payloads were persisted.
