"""Back-compat shim: navigation models now live in
:mod:`research_graph.domain.navigation` (D086 onion Core).

This module re-exports the navigation model types so historical imports
``from research_graph.papers.indexing.navigation import ...`` keep working.
Canonical home is the domain; new code imports from there.

Formerly: src/arxiv_archive/indexing/navigation.py
"""

from __future__ import annotations

from research_graph.domain.navigation import (  # noqa: F401
    NavigationAnchor,
    PageIndexDocument,
    PageIndexNode,
    build_navigation_anchors,
)

__all__ = [
    "NavigationAnchor",
    "PageIndexDocument",
    "PageIndexNode",
    "build_navigation_anchors",
]
