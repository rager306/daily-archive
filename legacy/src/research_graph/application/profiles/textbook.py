"""Textbook domain profile constants (ADR-032 / M222).

First non-paper domain: GNN textbook (HTML chapters).
Profile config only — no fetch, no graph import.
"""

from __future__ import annotations

from typing import Final

DOMAIN_PROFILE: Final[str] = "textbook"
SOURCE_KINDS: Final[frozenset[str]] = frozenset({"html", "markdown", "pdf"})
ENTITY_FOCUS: Final[tuple[str, ...]] = (
    "Concept",
    "Definition",
    "Example",
    "Exercise",
    "Theorem",
    "CodeBlock",
)
RELATION_FOCUS: Final[tuple[str, ...]] = (
    "CONSISTS_OF",
    "SUBSET_OF",
    "DERIVED_FROM",
    "MOTIVATED_BY",
    "REQUIRES",
)

# ADR-032 §2.4 reference implementation
GNN_TEXTBOOK_TITLE: Final[str] = "Graph Neural Networks Textbook"
GNN_TEXTBOOK_AUTHOR: Final[str] = "Anvith Pothula"
GNN_TEXTBOOK_BASE_URL: Final[str] = (
    "https://anvithpothula.github.io/graph-neural-networks-textbook/"
)
GNN_TEXTBOOK_SITEMAP_URL: Final[str] = (
    "https://anvithpothula.github.io/graph-neural-networks-textbook/sitemap.xml"
)
GNN_TEXTBOOK_SOURCE_CODE: Final[str] = "gnn_textbook"
GNN_TEXTBOOK_CATALOG_ROOT: Final[str] = "gnn_textbook/html"

# Bounded chapter seeds for optional live fetch / offline fixtures
GNN_TEXTBOOK_SEED_PATHS: Final[tuple[str, ...]] = (
    "about/",
    "chapters/",
    "chapters/00-math-prerequisites/",
    "chapters/01-intro-to-graphs/",
    "chapters/02-graph-properties-and-features/",
)

PROFILE_NOTE: Final[str] = (
    "textbook profile constants for GNN HTML path; import_eligible remains false"
)


def textbook_profile_dict() -> dict[str, object]:
    return {
        "domain_profile": DOMAIN_PROFILE,
        "source_kinds": sorted(SOURCE_KINDS),
        "entity_focus": list(ENTITY_FOCUS),
        "relation_focus": list(RELATION_FOCUS),
        "reference": {
            "title": GNN_TEXTBOOK_TITLE,
            "author": GNN_TEXTBOOK_AUTHOR,
            "base_url": GNN_TEXTBOOK_BASE_URL,
            "source_code": GNN_TEXTBOOK_SOURCE_CODE,
            "seed_paths": list(GNN_TEXTBOOK_SEED_PATHS),
        },
        "import_eligible": False,
        "graph_writes_allowed": False,
        "note": PROFILE_NOTE,
    }


__all__ = [
    "DOMAIN_PROFILE",
    "ENTITY_FOCUS",
    "GNN_TEXTBOOK_AUTHOR",
    "GNN_TEXTBOOK_BASE_URL",
    "GNN_TEXTBOOK_CATALOG_ROOT",
    "GNN_TEXTBOOK_SEED_PATHS",
    "GNN_TEXTBOOK_SITEMAP_URL",
    "GNN_TEXTBOOK_SOURCE_CODE",
    "GNN_TEXTBOOK_TITLE",
    "PROFILE_NOTE",
    "RELATION_FOCUS",
    "SOURCE_KINDS",
    "textbook_profile_dict",
]
