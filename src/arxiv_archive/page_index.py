"""Compatibility delegates for PageIndex construction.

The implementation lives in :mod:`arxiv_archive.indexing.page_index` and the
parser lives in :mod:`arxiv_archive.parsing.parser`. Keep this module as the
legacy public import surface for downstream callers.
"""

from __future__ import annotations

from arxiv_archive.indexing.navigation import NavigationAnchor, PageIndexDocument, PageIndexNode
from arxiv_archive.indexing.page_index import build_page_index, build_page_index_from_parsed

__all__ = [
    "NavigationAnchor",
    "PageIndexDocument",
    "PageIndexNode",
    "build_page_index",
    "build_page_index_from_parsed",
]
