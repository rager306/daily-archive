# Package Rename Wave 03

Moved paper indexing/retrieval contract modules from top-level `arxiv_archive` into canonical `research_graph.papers.indexing` paths.

## Moves

| Old runtime path | New canonical path |
|---|---|
| `src/arxiv_archive/article_links_dedup.py` | `src/research_graph/papers/indexing/links_dedup.py` |
| `src/arxiv_archive/article_page_index.py` | `src/research_graph/papers/indexing/page_index.py` |
| `src/arxiv_archive/article_retrieval_tables.py` | `src/research_graph/papers/indexing/retrieval_tables.py` |

## Archive policy

The old files are archived here for historical reference only. They are intentionally not importable from `src/` and no compatibility shims were added.

## Verification contract

- New modules include `Formerly:` breadcrumbs.
- Old top-level runtime files are absent from `src/arxiv_archive/`.
- Source, tests, and scripts import `research_graph.papers.indexing.*`.
- Targeted tests, compile checks, import smoke, and GitNexus change detection must pass before closeout.
