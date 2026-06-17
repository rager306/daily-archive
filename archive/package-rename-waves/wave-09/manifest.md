# Package Rename Wave 09

Moved the bounded thirty-paper source acquisition/source-scan helper into the canonical corpus sources context.

## Move

| Old runtime path | New canonical path |
|---|---|
| `src/arxiv_archive/thirty_paper_source_scan.py` | `src/research_graph/corpus/sources/thirty_paper_source_scan.py` |

## Scope note

This helper can acquire Markdown through an injected converter, but tests use local fakes and fixtures. Verification must not perform live arxiv2md, arXiv PDF, Marker, or external provider calls.

## Archive policy

The old file is archived here for historical reference only. It is intentionally not importable from `src/`, and no compatibility shim was added for `arxiv_archive.thirty_paper_source_scan`.

## Verification contract

- Canonical module includes `Formerly:` breadcrumb.
- Old `src/arxiv_archive/thirty_paper_source_scan.py` runtime file is absent.
- Source, tests, and scripts import `research_graph.corpus.sources.thirty_paper_source_scan`.
- Direct old import/string search is clean.
- Targeted local tests, compile checks, import smoke, and GitNexus change detection must pass before closeout.
