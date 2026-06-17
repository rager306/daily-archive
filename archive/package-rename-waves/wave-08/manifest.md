# Package Rename Wave 08

Moved the arXiv markdown conversion source adapter from `arxiv_archive` into the canonical corpus sources context.

## Move

| Old runtime path | New canonical path |
|---|---|
| `src/arxiv_archive/md_converter.py` | `src/research_graph/corpus/sources/markdown_converter.py` |

## Scope note

This module owns network-capable arxiv2md and Marker fallback behavior. The move is path-only; verification must remain local/isolated and must not perform live arxiv2md, arXiv PDF, or Marker calls.

## Archive policy

The old file is archived here for historical reference only. It is intentionally not importable from `src/`, and no compatibility shim was added for `arxiv_archive.md_converter`.

## Verification contract

- Canonical module includes `Formerly:` breadcrumb.
- Old `src/arxiv_archive/md_converter.py` runtime file is absent.
- Source, tests, and scripts import `research_graph.corpus.sources.markdown_converter`.
- Direct old import/string search is clean.
- Isolated converter tests, compile checks, import smoke, and GitNexus change detection must pass before closeout.
