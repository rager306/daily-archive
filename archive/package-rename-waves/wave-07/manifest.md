# Package Rename Wave 07

Moved corpus ingestion and deterministic parsing internals from `arxiv_archive` into canonical `research_graph.corpus` packages.

## Moves

| Old runtime path | New canonical path |
|---|---|
| `src/arxiv_archive/ingestion/fetchers.py` | `src/research_graph/infrastructure/corpus/ingestion/fetchers.py` |
| `src/arxiv_archive/ingestion/loader.py` | `src/research_graph/infrastructure/corpus/ingestion/loader.py` |
| `src/arxiv_archive/ingestion/logging.py` | `src/research_graph/infrastructure/corpus/ingestion/logging.py` |
| `src/arxiv_archive/ingestion/__init__.py` | package-level exports from `src/research_graph/infrastructure/corpus/ingestion/__init__.py` |
| `src/arxiv_archive/full_text.py` | package-level exports from `src/research_graph/infrastructure/corpus/ingestion/__init__.py` |
| `src/arxiv_archive/article_loader.py` | package-level exports from `src/research_graph/infrastructure/corpus/ingestion/__init__.py` |
| `src/arxiv_archive/pdf_downloader.py` | package-level exports from `src/research_graph/infrastructure/corpus/ingestion/__init__.py` |
| `src/arxiv_archive/parsing/normalization.py` | `src/research_graph/infrastructure/corpus/parsing/normalization.py` |
| `src/arxiv_archive/parsing/parser.py` | `src/research_graph/infrastructure/corpus/parsing/parser.py` |
| `src/arxiv_archive/parsing/structure.py` | `src/research_graph/infrastructure/corpus/parsing/structure.py` |
| `src/arxiv_archive/parsing/__init__.py` | package-level exports from `src/research_graph/infrastructure/corpus/parsing/__init__.py` |

## Scope note

`src/arxiv_archive/md_converter.py` remains in place because it owns network conversion behavior and should move, if needed, in a dedicated wave. It now imports ingestion helpers from `research_graph.infrastructure.corpus.ingestion`.

## Archive policy

The old files are archived here for historical reference only. They are intentionally not importable from `src/`, and no compatibility shims were added for `arxiv_archive.ingestion`, `arxiv_archive.parsing`, `arxiv_archive.full_text`, `arxiv_archive.article_loader`, or `arxiv_archive.pdf_downloader`.

## Verification contract

- Canonical modules include `Formerly:` breadcrumbs.
- Old runtime `.py` files selected for this wave are absent from source.
- Source, tests, and scripts import canonical `research_graph.infrastructure.corpus.ingestion` and `research_graph.infrastructure.corpus.parsing` paths.
- Direct old import search is clean for moved paths.
- Targeted tests, compile checks, import smoke, and GitNexus change detection must pass before closeout.
