# Package Rename Wave 05

Archived the top-level `arxiv_archive.page_index` compatibility bridge. PageIndex callers now import canonical symbols from `research_graph.papers.indexing`.

## Move

| Old runtime path | New canonical path |
|---|---|
| `src/arxiv_archive/page_index.py` | package-level exports from `src/research_graph/papers/indexing/__init__.py` |

## Canonical exports

`research_graph.papers.indexing` exports:

- `NavigationAnchor`
- `PageIndexDocument`
- `PageIndexNode`
- `build_navigation_anchors`
- `build_page_index`
- `build_page_index_from_parsed`

## Archive policy

The old bridge is archived here for historical reference only. It is intentionally not importable from `src/`, and no compatibility shim was added.

## Verification contract

- Old `src/arxiv_archive/page_index.py` runtime file is absent.
- Archive copy exists under wave-05 and contains a `Formerly:` breadcrumb.
- Source, tests, and scripts import canonical `research_graph.papers.indexing` exports.
- Direct old import search is clean.
- Targeted tests, compile checks, import smoke, and GitNexus change detection must pass before closeout.
