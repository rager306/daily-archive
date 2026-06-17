"""Compatibility delegates for PageIndex construction.

The implementation lives in :mod:`research_graph.papers.indexing.parsed_page_index` and the
parser lives in :mod:`arxiv_archive.parsing.parser`. Keep this module as the
legacy public import surface for downstream callers.


Formerly: src/arxiv_archive/page_index.py"""

from __future__ import annotations

from research_graph.papers.indexing.navigation import NavigationAnchor, PageIndexDocument, PageIndexNode
from research_graph.papers.indexing.parsed_page_index import build_page_index, build_page_index_from_parsed

__all__ = [
    "NavigationAnchor",
    "PageIndexDocument",
    "PageIndexNode",
    "build_page_index",
    "build_page_index_from_parsed",
]
