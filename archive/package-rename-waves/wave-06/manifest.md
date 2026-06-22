# Package Rename Wave 06

Moved source asset registry/provenance and structure-aware chunking internals from `arxiv_archive` into canonical `research_graph.papers` contexts.

## Moves

| Old runtime path | New canonical path |
|---|---|
| `src/arxiv_archive/assets/registry.py` | `src/research_graph/infrastructure/papers/source_assets/registry.py` |
| `src/arxiv_archive/assets/provenance.py` | `src/research_graph/infrastructure/papers/source_assets/provenance.py` |
| `src/arxiv_archive/assets/__init__.py` | package-level exports from `src/research_graph/infrastructure/papers/source_assets/__init__.py` |
| `src/arxiv_archive/source_asset_manifest.py` | package-level exports from `src/research_graph/infrastructure/papers/source_assets/__init__.py` |
| `src/arxiv_archive/chunking/chunker.py` | `src/research_graph/infrastructure/papers/chunking/chunker.py` |
| `src/arxiv_archive/chunking/figure_units.py` | `src/research_graph/infrastructure/papers/chunking/figure_units.py` |
| `src/arxiv_archive/chunking/table_units.py` | `src/research_graph/infrastructure/papers/chunking/table_units.py` |
| `src/arxiv_archive/chunking/__init__.py` | package-level exports from `src/research_graph/infrastructure/papers/chunking/__init__.py` |
| `src/arxiv_archive/structure_aware_chunking.py` | package-level exports from `src/research_graph/infrastructure/papers/chunking/__init__.py` |

## Naming note

`research_graph.papers.assets.py` already exists as the article asset manifest module from an earlier wave. The source preservation registry moved to `research_graph.infrastructure.papers.source_assets` to avoid conflating article asset records with preserved source-file assets.

## Archive policy

The old files are archived here for historical reference only. They are intentionally not importable from `src/`, and no compatibility shims were added for `arxiv_archive.assets`, `arxiv_archive.chunking`, `arxiv_archive.source_asset_manifest`, or `arxiv_archive.structure_aware_chunking`.

## Verification contract

- Canonical modules include `Formerly:` breadcrumbs.
- Old runtime `.py` files are absent from `src/arxiv_archive/assets`, `src/arxiv_archive/chunking`, and the two top-level bridge paths.
- Source, tests, and scripts import canonical `research_graph.infrastructure.papers.source_assets` and `research_graph.infrastructure.papers.chunking` paths.
- Direct old import search is clean.
- Targeted tests, compile checks, import smoke, and GitNexus change detection must pass before closeout.
