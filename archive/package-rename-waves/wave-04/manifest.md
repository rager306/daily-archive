# Package Rename Wave 04

Moved lower-level deterministic PageIndex construction internals from `arxiv_archive.indexing` into canonical `research_graph.papers.indexing` paths.

## Moves

| Old runtime path | New canonical path |
|---|---|
| `src/arxiv_archive/indexing/navigation.py` | `src/research_graph/infrastructure/papers/indexing/navigation.py` |
| `src/arxiv_archive/indexing/page_index.py` | `src/research_graph/infrastructure/papers/indexing/parsed_page_index.py` |
| `src/arxiv_archive/indexing/__init__.py` | no runtime shim; public package exports now live in `src/research_graph/infrastructure/papers/indexing/__init__.py` |

## Naming note

`page_index.py` was not reused because wave-03 already made `src/research_graph/infrastructure/papers/indexing/page_index.py` canonical for the metadata-only article PageIndex manifest contract formerly at `src/arxiv_archive/article_page_index.py`. The parser-output builder moved to `parsed_page_index.py` to avoid conflating those two contracts.

## Archive policy

The old files are archived here for historical reference only. They are intentionally not importable from `src/` and no compatibility shims were added for `arxiv_archive.indexing`.

`src/arxiv_archive/page_index.py` remains temporarily as a broader legacy public bridge for a later dedicated wave, but it imports canonical `research_graph.papers.indexing` internals after this wave.

## Verification contract

- New modules include `Formerly:` breadcrumbs.
- Old `src/arxiv_archive/indexing/` runtime package is absent.
- Source, tests, and scripts import `research_graph.papers.indexing.navigation` or `research_graph.papers.indexing.parsed_page_index`.
- Targeted tests, compile checks, import smoke, and GitNexus change detection must pass before closeout.
